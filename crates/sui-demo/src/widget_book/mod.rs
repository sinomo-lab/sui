#![forbid(unsafe_code)]

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{OnceLock, RwLock},
};

use sui::prelude::*;
use sui::{
    FramePhase, HdrLuminanceTokens, HdrThemeMode, HdrThemeTokens, InvalidationKind,
    InvalidationRequest, InvalidationTarget, PlacementBadge, PointerEventKind, Rect,
    SceneStatisticsDetailMode, SemanticColorToken, SemanticRegion, SemanticTone, SemanticsNode,
    SemanticsRole, SemanticsValue, SignalMeter, StatusBadge, TextDirection, TextStyle, TextSurface,
    TextSurfaceOverlayKind, TextSurfaceStyleOverlay, TextSurfaceStyleSpan, TextWrap,
    ThemeTextToken, Vector, WidgetColorRole, WidgetLuminanceRole, WidgetMaterialRole,
    WidgetPodMutVisitor, WidgetPodVisitor, WindowEvent, WindowPerformanceSnapshot,
    paint_single_line_aligned_text, resolve_semantic_color, resolve_widget_hdr_style,
    set_window_scene_statistics_detail_mode, window_performance_snapshot,
    window_scene_statistics_detail_mode,
};
use sui_runtime::{LayerOptions, PaintBoundaryMode};
use sui_scene::{LayerCompositionMode, LayerProperties};

use crate::app::{DemoTextRole, demo_mono_text_style_when, demo_text_style, demo_text_style_when};

#[cfg(all(feature = "artifacts", not(target_arch = "wasm32")))]
mod visual_artifacts;

#[cfg(all(feature = "artifacts", not(target_arch = "wasm32")))]
pub use visual_artifacts::write_visual_artifacts;

pub const WINDOW_TITLE: &str = "SUI Widget Book";
pub const WINDOW_DESCRIPTION: &str =
    "Development gallery for common built-in widgets in sui-widgets";

fn widget_book_theme_text_style(
    theme: DefaultTheme,
    token: ThemeTextToken,
    color: Color,
) -> TextStyle {
    TextStyle {
        font_size: token.size,
        line_height: token.line_height,
        color,
        ..theme.body_text_style()
    }
}

fn widget_book_mono_text_style(token: ThemeTextToken, color: Color) -> TextStyle {
    let theme = DefaultTheme::default();
    widget_book_theme_mono_text_style(theme, token, color)
}

fn widget_book_theme_mono_text_style(
    theme: DefaultTheme,
    token: ThemeTextToken,
    color: Color,
) -> TextStyle {
    TextStyle {
        font_size: token.size,
        line_height: token.line_height,
        ..theme.mono_text_style(color)
    }
}
pub const RETAINED_TEXT_BENCHMARK_TITLE: &str = "SUI Retained Text Scroll Benchmark";
pub const ANIMATION_BENCHMARK_TITLE: &str = "SUI Animation Benchmark";
pub const TEXT_RENDERING_COMPARISON_TITLE: &str = "SUI Text Rendering Comparison";
pub const COLOR_VALIDATION_VIEW_TITLE: &str = "SUI HDR and Color Validation";
pub const TEXT_VALIDATION_VIEW_TITLE: &str = "SUI Text Validation";
pub const TEXT_EDITING_BENCHMARK_TITLE: &str = "SUI Text Editing Benchmark";
pub const NAME_INPUT_LABEL: &str = "Name";
pub const PASSWORD_INPUT_LABEL: &str = "Password";
pub const DATETIME_INPUT_LABEL: &str = "Scheduled for";
pub const TEXT_AREA_LABEL: &str = "Notes";
pub const SUBSCRIBE_LABEL: &str = "Subscribe to product updates";
pub const PRIMARY_BUTTON_LABEL: &str = "Trigger action";
pub const TOOLBAR_SEPARATOR_NAME: &str = "Toolbar divider";
pub const ICON_LABEL: &str = "Search icon";
pub const ICON_BUTTON_LABEL: &str = "More actions";
pub const SWITCH_LABEL: &str = "Enable snapping";
pub const RADIO_BUTTON_LABEL: &str = "Standalone radio sample";
pub const RADIO_GROUP_NAME: &str = "Render quality";
pub const SLIDER_NAME: &str = "Opacity";
pub const NUMBER_INPUT_NAME: &str = "Brush size";
pub const SELECT_NAME: &str = "Blend mode";
pub const TAB_BAR_NAME: &str = "Workspace tabs";
pub const TABS_NAME: &str = "Inspector tabs";
pub const MENU_NAME: &str = "Command menu";
pub const CONTEXT_MENU_NAME: &str = "Layer context menu";
pub const TOOLTIP_TRIGGER_LABEL: &str = "Hover for shortcuts";
pub const TOOLTIP_TEXT: &str = "Quick access to common commands";
pub const POPOVER_NAME: &str = "Inline inspector";
pub const POPOVER_TRIGGER_LABEL: &str = "Open inspector";
pub const WIDGET_STATES_GALLERY_NAME: &str = "Widget States";
pub const WIDGET_STATES_BUTTON_LABEL: &str = "States button";
pub const WIDGET_STATES_ICON_BUTTON_LABEL: &str = "States icon action";
pub const WIDGET_STATES_TEXT_INPUT_LABEL: &str = "States text input";
pub const WIDGET_STATES_TEXT_AREA_LABEL: &str = "States text area";
pub const WIDGET_STATES_SELECT_NAME: &str = "States select";
pub const WIDGET_STATES_CHECKBOX_LABEL: &str = "States checkbox";
pub const WIDGET_STATES_SWITCH_LABEL: &str = "States switch";
pub const WIDGET_STATES_SLIDER_NAME: &str = "States slider";
pub const WIDGET_STATES_TABS_NAME: &str = "States tabs";
pub const WIDGET_STATES_MENU_NAME: &str = "States menu";
pub const WIDGET_STATES_POPOVER_NAME: &str = "States popover";
pub const SIZE_PRESETS_GALLERY_NAME: &str = "Size presets";
pub const SIZE_PRESET_SMALL_ACTION_LABEL: &str = "Small preset action";
pub const SIZE_PRESET_MEDIUM_ACTION_LABEL: &str = "Medium preset action";
pub const SIZE_PRESET_LARGE_ACTION_LABEL: &str = "Large preset action";
pub const SIZE_PRESET_SMALL_INPUT_LABEL: &str = "Small preset input";
pub const SIZE_PRESET_MEDIUM_INPUT_LABEL: &str = "Medium preset input";
pub const SIZE_PRESET_LARGE_INPUT_LABEL: &str = "Large preset input";
const SIZE_PRESET_CARD_MIN_WIDTH: f32 = 260.0;
const SIZE_PRESET_CARD_MAX_WIDTH: f32 = 360.0;
const CONTROL_STORY_CARD_MIN_WIDTH: f32 = 280.0;
const CONTROL_STORY_CARD_MAX_WIDTH: f32 = 560.0;
const WIDE_CONTROL_STORY_CARD_MAX_WIDTH: f32 = 620.0;
const CONTROL_STORY_CONTENT_MAX_WIDTH: f32 = 300.0;
pub const DIALOG_TITLE: &str = "Project settings";
pub const DIALOG_TRIGGER_LABEL: &str = "Toggle project settings";
pub const PROGRESS_NAME: &str = "Export progress";
pub const SPINNER_NAME: &str = "Background work";
pub const SUMMARY_NAME: &str = "Widget book summary";
pub const GALLERY_SCROLL_NAME: &str = "Widget book gallery";
pub const GALLERY_SCROLL_BAR_NAME: &str = "Widget book gallery vertical scroll bar";
pub const WIDGET_BOOK_SHELL_NAME: &str = "Widget book shell";
pub const WIDGET_BOOK_SEARCH_NAME: &str = "Filter widget stories";
pub const WIDGET_BOOK_THEME_SELECT_NAME: &str = "Widget book theme";
pub const WIDGET_BOOK_CATEGORY_NAV_NAME: &str = "Widget categories";
const GALLERY_TEXT_MAX_WIDTH: f32 = 980.0;
const GALLERY_CONTENT_MAX_WIDTH: f32 = 1180.0;
const WIDGET_BOOK_RAIL_WIDTH: f32 = 216.0;
const WIDGET_BOOK_RAIL_BREAKPOINT: f32 = 900.0;
const WIDGET_BOOK_SECTION_GAP: f32 = 18.0;
const ROOT_GALLERY_PADDING: Insets = Insets {
    left: 24.0,
    top: 0.0,
    right: 24.0,
    bottom: 0.0,
};
const WIDGET_BOOK_GALLERY_PADDING: Insets = Insets {
    left: 24.0,
    top: 18.0,
    right: 24.0,
    bottom: 28.0,
};
pub const THEME_DEMO_TITLE: &str = "Themes";
pub const THEME_DEMO_DESCRIPTION: &str =
    "Compare the SUI and neutral presets across standard, dark, OLED, and HDR UI styling.";
pub const THEME_DEMO_SCROLL_NAME: &str = "Theme demo gallery";
pub const RETAINED_TEXT_BENCHMARK_SCROLL_NAME: &str = "Retained text benchmark scroll";
pub const RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME: &str =
    "Retained text benchmark vertical scroll bar";
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
pub const TEXT_EDITING_BENCHMARK_SPLIT_NAME: &str = "Text editing benchmark split";
pub const TEXT_EDITING_BENCHMARK_EDITOR_NAME: &str = "Text editing benchmark editor";
pub const TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME: &str = "Text editing benchmark syntax preview";
pub const THEME_PREVIEW_NAME: &str = "Theme preview showcase";
pub const LIGHT_THEME_PREVIEW_CARD_NAME: &str = "Light theme preview card";
pub const NEUTRAL_THEME_PREVIEW_CARD_NAME: &str = "Neutral theme preview card";
pub const DARK_THEME_PREVIEW_CARD_NAME: &str = "Dark theme preview card";
pub const NEUTRAL_DARK_THEME_PREVIEW_CARD_NAME: &str = "Neutral dark theme preview card";
pub const TRUE_BLACK_THEME_PREVIEW_CARD_NAME: &str = "True black theme preview card";
pub const HDR_THEME_LAB_NAME: &str = "HDR theme mode lab";
pub const HDR_THEME_LAB_ACTIVE_PREVIEW_NAME: &str = "Current HDR theme mode preview";
pub const LIGHT_PREVIEW_ACTION_LABEL: &str = "Light preview action";
pub const NEUTRAL_PREVIEW_ACTION_LABEL: &str = "Neutral preview action";
pub const DARK_PREVIEW_ACTION_LABEL: &str = "Dark preview action";
pub const NEUTRAL_DARK_PREVIEW_ACTION_LABEL: &str = "Neutral dark preview action";
pub const TRUE_BLACK_PREVIEW_ACTION_LABEL: &str = "True black preview action";
pub const LIGHT_PREVIEW_INPUT_LABEL: &str = "Light preview query";
pub const NEUTRAL_PREVIEW_INPUT_LABEL: &str = "Neutral preview query";
pub const DARK_PREVIEW_INPUT_LABEL: &str = "Dark preview query";
pub const NEUTRAL_DARK_PREVIEW_INPUT_LABEL: &str = "Neutral dark preview query";
pub const TRUE_BLACK_PREVIEW_INPUT_LABEL: &str = "True black preview query";
pub const LIST_VIEW_NAME: &str = "Assets list";
pub const TREE_VIEW_NAME: &str = "Scene tree";
pub const TABLE_NAME: &str = "Material table";
pub const SPLIT_VIEW_NAME: &str = "Editor split";
pub const BREADCRUMB_NAME: &str = "Project path";
pub const COLOR_SWATCH_NAME: &str = "Primary swatch";
pub const COLOR_PICKER_NAME: &str = "Accent picker";
pub const DEMO_IMAGE_LABEL: &str = "Preview image";
pub const COMPOSITE_WIDGETS_GALLERY_NAME: &str = "Composite widgets";
pub const LAYOUT_WIDGETS_GALLERY_NAME: &str = "Layout widgets";
pub const TEXT_WIDGETS_GALLERY_NAME: &str = "Text widgets";
pub const DATA_WIDGETS_GALLERY_NAME: &str = "Data and interaction widgets";
pub const CANVAS_WIDGETS_GALLERY_NAME: &str = "Canvas and media widgets";
pub const SURFACE_SAMPLE_NAME: &str = "Panel surface sample";
pub const ACTION_CARD_NAME: &str = "Open canvas workspace";
pub const SECTION_LABEL_NAME: &str = "Inspector section label";
pub const STATUS_BADGE_NAME: &str = "Synced";
pub const STATUS_BAR_NAME: &str = "Document status bar";
pub const COMMAND_GROUP_NAME: &str = "View commands";
pub const TOOL_PALETTE_NAME: &str = "Paint tools";
pub const TOOLBAR_NAME: &str = "Document toolbar";
pub const FORM_SECTION_NAME: &str = "Publish settings";
pub const PROPERTY_ROW_NAME: &str = "Opacity property";
pub const DETAIL_ROW_NAME: &str = "Last publish";
pub const PANEL_SECTION_NAME: &str = "Layer properties";
pub const DOCK_PANEL_NAME: &str = "Inspector dock panel";
pub const EMPTY_STATE_NAME: &str = "No search results";
pub const PRESET_STRIP_NAME: &str = "Export presets";
pub const SEGMENTED_CONTROL_NAME: &str = "Preview mode";
pub const COVERAGE_DOTS_NAME: &str = "Replica coverage";
pub const PLACEMENT_BADGE_NAME: &str = "Cluster";
pub const BUSY_INDICATOR_NAME: &str = "Indexing busy indicator";
pub const LAYOUT_REGION_NAME: &str = "Layout primitive region";
pub const SCROLL_VIEW_NAME: &str = "Inner scroll view";
pub const VIRTUAL_SCROLL_SAMPLE_NAME: &str = "Virtual scroll sample";
pub const FIXED_PANE_SPLIT_NAME: &str = "Fixed pane split sample";
pub const DOCK_LAYOUT_NAME: &str = "Dock layout sample";
pub const MEASURED_BOTTOM_DOCK_NAME: &str = "Measured bottom dock sample";
pub const SWITCH_VIEW_NAME: &str = "Switch view sample";
pub const TRAILING_SLOT_ROW_NAME: &str = "Trailing slot row sample";
pub const RICH_TEXT_NAME: &str = "Rich text sample";
pub const COMBO_BOX_ALIAS_NAME: &str = "ComboBox alias";
pub const SPIN_BOX_ALIAS_NAME: &str = "SpinBox alias";
pub const MULTILINE_ALIAS_NAME: &str = "MultilineTextInput alias";
pub const DIVIDER_ALIAS_NAME: &str = "Divider alias";
pub const LINK_NAME: &str = "Documentation link";
pub const PATH_BAR_NAME: &str = "Asset path bar";
pub const DATA_GRID_NAME: &str = "Data grid alias";
pub const VIRTUAL_TABLE_NAME: &str = "Virtual asset table";
pub const LAYER_LIST_NAME: &str = "Layer stack";
pub const REORDERABLE_LIST_NAME: &str = "Reorderable task list";
pub const DRAG_SOURCE_NAME: &str = "Drag source item";
pub const DROP_TARGET_NAME: &str = "Drop target slot";
pub const CANVAS_NAME: &str = "Vector canvas";
pub const CANVAS_RULER_NAME: &str = "Canvas horizontal ruler";
pub const PIXEL_CANVAS_NAME: &str = "Pixel canvas";
pub const COLOR_PALETTE_NAME: &str = "Document palette";
pub const BRUSH_PREVIEW_NAME: &str = "Brush preview";
pub const SIGNAL_METER_NAME: &str = "Live signal meter";
pub const ANIMATION_BENCHMARK_RETAINED_NAME: &str = "Animation benchmark retained lane";
pub const ANIMATION_BENCHMARK_REPAINT_NAME: &str = "Animation benchmark repaint lane";
pub const ANIMATION_BENCHMARK_SCALE_NAME: &str = "Animation benchmark scale grid";

const WIDGET_BOOK_IMAGE_HANDLE: ImageHandle = ImageHandle::new(1);
const ANIMATION_BENCHMARK_RETAINED_TARGET: &str = "animation-benchmark-retained";
const ANIMATION_BENCHMARK_REPAINT_TARGET: &str = "animation-benchmark-repaint";
const ANIMATION_BENCHMARK_SCALE_TARGET_PREFIX: &str = "animation-benchmark-cell-";
const ANIMATION_BENCHMARK_RADIUS_PATH: &str = "paint.radius";
const ANIMATION_BENCHMARK_ALPHA_PATH: &str = "paint.alpha";
const ANIMATION_BENCHMARK_SCALE_CELLS: usize = 96;
const ANIMATION_BENCHMARK_SCALE_COLUMNS: usize = 12;

const RADIO_OPTIONS: [&str; 3] = ["Balanced", "High", "Fast"];
const BLEND_MODE_OPTIONS: [&str; 4] = ["Normal", "Multiply", "Screen", "Overlay"];
const TAB_BAR_OPTIONS: [&str; 3] = ["Canvas", "Inspector", "Export"];
const TAB_PANEL_OPTIONS: [&str; 3] = ["Layout", "Data", "History"];
const WIDGET_BOOK_THEME_OPTIONS: [&str; 6] = [
    "Application theme",
    "SUI light",
    "Neutral light",
    "SUI dark",
    "Neutral dark",
    "SUI true black",
];
const TEXT_RENDERING_COMPARISON_MIN_WIDTH: f32 = 1094.0;
const TEXT_RENDERING_COMPARISON_CARD_WIDTH: f32 = 520.0;
const TEXT_RENDERING_SAMPLE_TILE_WIDTH: f32 = 232.0;
const TEXT_RENDERING_SAMPLE_TILE_HEIGHT: f32 = 108.0;
const TEXT_VALIDATION_CONTENT_WIDTH: f32 = 1040.0;
const TEXT_VALIDATION_PROBE_CARD_WIDTH: f32 = 320.0;

pub type WidgetBookThemeReader = Rc<dyn Fn() -> DefaultTheme>;

fn default_widget_book_theme_reader() -> WidgetBookThemeReader {
    let theme = DefaultTheme::default();
    Rc::new(move || theme)
}

fn clone_widget_book_theme_reader(
    theme_reader: &WidgetBookThemeReader,
) -> impl Fn() -> DefaultTheme + 'static {
    let theme_reader = Rc::clone(theme_reader);
    move || theme_reader()
}

fn widget_book_size_theme_reader(
    theme_reader: &WidgetBookThemeReader,
    size: ControlSize,
) -> impl Fn() -> DefaultTheme + 'static {
    let theme_reader = Rc::clone(theme_reader);
    move || theme_reader().with_size(size)
}

#[derive(Debug, Clone, Copy)]
enum DemoTextColor {
    Text,
    Muted,
}

fn demo_label(
    theme_reader: &WidgetBookThemeReader,
    text: impl Into<String>,
    role: DemoTextRole,
    color: DemoTextColor,
) -> Label {
    Label::new(text).style_when(demo_text_style_when(
        theme_reader,
        role,
        move |theme| match color {
            DemoTextColor::Text => theme.palette.text,
            DemoTextColor::Muted => theme.palette.text_muted,
        },
    ))
}

fn demo_mono_label<F>(
    theme_reader: &WidgetBookThemeReader,
    text: impl Into<String>,
    role: DemoTextRole,
    color: F,
) -> Label
where
    F: Fn(DefaultTheme) -> Color + 'static,
{
    Label::new(text).style_when(demo_mono_text_style_when(theme_reader, role, color))
}

fn widget_book_theme_color<F>(
    theme_reader: &WidgetBookThemeReader,
    color: F,
) -> impl Fn() -> Color + 'static
where
    F: Fn(DefaultTheme) -> Color + 'static,
{
    let theme_reader = Rc::clone(theme_reader);
    move || color(theme_reader())
}

#[derive(Debug, Clone, Copy)]
struct TextRenderingModeSpec {
    title: &'static str,
    subtitle: &'static str,
    notes: &'static str,
    setting: &'static str,
    policy: TextRenderPolicy,
}

const TEXT_RENDERING_MODE_DATA: [TextRenderingModeSpec; 9] = [
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

fn hdr_theme_lab_mode_store() -> &'static RwLock<HdrThemeMode> {
    static STORE: OnceLock<RwLock<HdrThemeMode>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HdrThemeMode::Disabled))
}

pub fn widget_book_hdr_theme_mode() -> HdrThemeMode {
    *hdr_theme_lab_mode_store()
        .read()
        .expect("widget-book HDR theme mode lock should not be poisoned")
}

pub fn set_widget_book_hdr_theme_mode(mode: HdrThemeMode) {
    *hdr_theme_lab_mode_store()
        .write()
        .expect("widget-book HDR theme mode lock should not be poisoned") = mode;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum WidgetBookCategory {
    #[default]
    All,
    Foundations,
    Controls,
    Navigation,
    Data,
    Layout,
    Text,
    Canvas,
}

impl WidgetBookCategory {
    const ALL: [Self; 8] = [
        Self::All,
        Self::Foundations,
        Self::Controls,
        Self::Text,
        Self::Navigation,
        Self::Data,
        Self::Layout,
        Self::Canvas,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::All => "All components",
            Self::Foundations => "Foundations",
            Self::Controls => "Controls",
            Self::Navigation => "Navigation",
            Self::Data => "Data views",
            Self::Layout => "Layout",
            Self::Text => "Text",
            Self::Canvas => "Canvas & media",
        }
    }

    fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::All)
    }

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|category| *category == self)
            .unwrap_or(0)
    }

    const fn gallery_item_index(self) -> usize {
        match self {
            Self::All => 0,
            Self::Foundations => 1,
            Self::Controls => 3,
            Self::Text => 7,
            Self::Navigation => 8,
            Self::Data => 14,
            Self::Layout => 15,
            Self::Canvas => 20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WidgetBookThemeSelection {
    Application,
    SuiLight,
    NeutralLight,
    SuiDark,
    NeutralDark,
    SuiTrueBlack,
}

impl WidgetBookThemeSelection {
    fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Application,
            2 => Self::NeutralLight,
            3 => Self::SuiDark,
            4 => Self::NeutralDark,
            5 => Self::SuiTrueBlack,
            _ => Self::SuiLight,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Application => 0,
            Self::SuiLight => 1,
            Self::NeutralLight => 2,
            Self::SuiDark => 3,
            Self::NeutralDark => 4,
            Self::SuiTrueBlack => 5,
        }
    }

    fn resolve(self, application_theme: DefaultTheme) -> DefaultTheme {
        match self {
            Self::Application => application_theme,
            Self::SuiLight => DefaultTheme::sui(),
            Self::NeutralLight => DefaultTheme::neutral(),
            Self::SuiDark => DefaultTheme::dark(),
            Self::NeutralDark => DefaultTheme::neutral_dark(),
            Self::SuiTrueBlack => DefaultTheme::high_contrast(),
        }
    }
}

#[derive(Debug, Clone)]
struct WidgetBookShellState {
    query: String,
    category: WidgetBookCategory,
    theme_selection: WidgetBookThemeSelection,
}

impl Default for WidgetBookShellState {
    fn default() -> Self {
        Self {
            query: String::new(),
            category: WidgetBookCategory::default(),
            theme_selection: WidgetBookThemeSelection::SuiLight,
        }
    }
}

impl WidgetBookShellState {
    fn section_matches(&self, search_text: &str) -> bool {
        let query = self.query.trim();
        query.is_empty()
            || search_text
                .to_ascii_lowercase()
                .contains(&query.to_ascii_lowercase())
    }
}

fn request_widget_book_shell_refresh(ctx: &mut EventCtx) {
    for kind in [
        InvalidationKind::Measure,
        InvalidationKind::Paint,
        InvalidationKind::HitTest,
        InvalidationKind::Semantics,
    ] {
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            kind,
        ));
    }
}

fn reset_widget_book_gallery_scroll(scroll_state: &ScrollState, ctx: &mut EventCtx) {
    let _ = scroll_state.set_offset(Vector::ZERO);
    request_widget_book_shell_refresh(ctx);
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WidgetBookState {
    pub name: String,
    pub password: String,
    pub scheduled_for: String,
    pub subscribed: bool,
    pub button_presses: usize,
    pub icon_button_presses: usize,
    pub switch_on: bool,
    pub standalone_radio_selected: bool,
    pub radio_choice: String,
    pub slider_value: f64,
    pub number_value: f64,
    pub notes: String,
    pub mode: String,
    pub tab_bar_choice: String,
    pub tabs_choice: String,
    pub last_menu_action: String,
    pub last_context_action: String,
    pub dialog_apply_count: usize,
}

pub struct LivePerformanceRoot {
    content: SingleChild,
    performance_overlay: SingleChild,
    performance_display: Rc<RefCell<LivePerformanceDisplay>>,
    watched_state: Option<Rc<RefCell<WidgetBookState>>>,
    last_seen_state: Option<WidgetBookState>,
    window_title: String,
    window_description: String,
    overlay_enabled: bool,
    overlay_enabled_reader: Option<Rc<dyn Fn() -> bool>>,
    last_overlay_enabled: bool,
    owns_detail_mode: bool,
}

impl LivePerformanceRoot {
    const OVERLAY_MARGIN: Insets = Insets {
        left: 0.0,
        top: 18.0,
        right: 18.0,
        bottom: 0.0,
    };

    pub fn new<Content>(
        window_title: impl Into<String>,
        window_description: impl Into<String>,
        content: Content,
    ) -> Self
    where
        Content: Widget + 'static,
    {
        let performance_display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
        Self {
            content: SingleChild::new(content),
            performance_overlay: SingleChild::new(LivePerformancePanel::with_display(Rc::clone(
                &performance_display,
            ))),
            performance_display,
            watched_state: None,
            last_seen_state: None,
            window_title: window_title.into(),
            window_description: window_description.into(),
            overlay_enabled: false,
            overlay_enabled_reader: None,
            last_overlay_enabled: false,
            owns_detail_mode: false,
        }
    }

    pub fn show_performance_overlay(mut self) -> Self {
        self.overlay_enabled = true;
        self
    }

    pub fn performance_overlay_enabled_when<F>(mut self, enabled: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.overlay_enabled_reader = Some(Rc::new(enabled));
        self
    }

    pub fn watch_widget_book_state(mut self, state: Rc<RefCell<WidgetBookState>>) -> Self {
        self.last_seen_state = Some(state.borrow().clone());
        self.watched_state = Some(state);
        self
    }

    fn overlay_enabled(&self) -> bool {
        self.overlay_enabled
            || self
                .overlay_enabled_reader
                .as_ref()
                .is_some_and(|enabled| enabled())
    }

    fn set_performance_display(
        &mut self,
        snapshot: Option<WindowPerformanceSnapshot>,
        idle: bool,
    ) -> bool {
        let mut samples = self.performance_display.borrow().samples.clone();
        if let Some(snapshot) = &snapshot {
            if samples
                .last()
                .is_none_or(|sample| sample.frame_index != snapshot.frame_index)
            {
                samples.push(LivePerformanceFrameSample::from_snapshot(snapshot));
                if samples.len() > LIVE_PERFORMANCE_HISTORY_LIMIT {
                    let overflow = samples.len() - LIVE_PERFORMANCE_HISTORY_LIMIT;
                    samples.drain(0..overflow);
                }
            }
        } else {
            samples.clear();
        }

        let next = LivePerformanceDisplay {
            snapshot,
            idle,
            samples,
        };
        let mut display = self.performance_display.borrow_mut();
        if *display == next {
            return false;
        }

        *display = next;
        true
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct LivePerformanceDisplay {
    snapshot: Option<WindowPerformanceSnapshot>,
    idle: bool,
    samples: Vec<LivePerformanceFrameSample>,
}

const LIVE_PERFORMANCE_HISTORY_LIMIT: usize = 72;
const LIVE_PERFORMANCE_STAGE_COUNT: usize = 9;

#[derive(Debug, Clone, PartialEq)]
struct LivePerformanceFrameSample {
    frame_index: u64,
    total_time_ms: f32,
    stage_costs: [f32; LIVE_PERFORMANCE_STAGE_COUNT],
}

impl LivePerformanceFrameSample {
    fn from_snapshot(snapshot: &WindowPerformanceSnapshot) -> Self {
        let mut stage_costs = [0.0; LIVE_PERFORMANCE_STAGE_COUNT];
        for timing in &snapshot.phase_timings {
            stage_costs[frame_phase_index(timing.phase)] += timing.duration_ms.max(0.0) as f32;
        }

        if snapshot.phase_timings.is_empty() {
            stage_costs[frame_phase_index(FramePhase::Renderer)] = snapshot.total_time_ms as f32;
        }

        Self {
            frame_index: snapshot.frame_index,
            total_time_ms: snapshot.total_time_ms.max(0.0) as f32,
            stage_costs,
        }
    }
}

const fn frame_phase_index(phase: FramePhase) -> usize {
    match phase {
        FramePhase::Event => 0,
        FramePhase::Redraw => 1,
        FramePhase::MeasureArrange => 2,
        FramePhase::HitTest => 3,
        FramePhase::Paint => 4,
        FramePhase::Semantics => 5,
        FramePhase::Renderer => 6,
        FramePhase::SurfaceWait => 7,
        FramePhase::Diagnostics => 8,
    }
}

pub fn default_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
    Rc::new(RefCell::new(WidgetBookState {
        name: "Ada".to_string(),
        password: "sui-demo".to_string(),
        scheduled_for: "2026-07-15 14:30".to_string(),
        subscribed: true,
        button_presses: 0,
        icon_button_presses: 0,
        switch_on: true,
        standalone_radio_selected: false,
        radio_choice: RADIO_OPTIONS[0].to_string(),
        slider_value: 72.0,
        number_value: 12.0,
        notes: "Pinned notes for inspector workflows.\nSupports multiline editing.".to_string(),
        mode: BLEND_MODE_OPTIONS[0].to_string(),
        tab_bar_choice: TAB_BAR_OPTIONS[0].to_string(),
        tabs_choice: TAB_PANEL_OPTIONS[0].to_string(),
        last_menu_action: "New tab".to_string(),
        last_context_action: "Rename".to_string(),
        dialog_apply_count: 0,
    }))
}

/// Register the images used by [`build_widget_book_gallery`] onto the given
/// application. Call this while configuring app resources when you are
/// assembling the application yourself rather than using
/// [`build_widget_book_application`].
pub fn register_widget_book_images(resources: &mut ResourceRegistry<'_>) {
    resources
        .image(
            WIDGET_BOOK_IMAGE_HANDLE,
            RegisteredImage::from_rgba8(72, 72, widget_book_demo_image_pixels())
                .expect("widget-book demo image is valid RGBA data"),
        )
        .expect("widget-book demo image handle should register exactly once");
}

pub fn build_widget_book_application(state: Rc<RefCell<WidgetBookState>>) -> Application {
    set_widget_book_hdr_theme_mode(HdrThemeMode::Disabled);

    App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })
        .expect("widget-book image resources should be valid")
        .window(
            Window::new(WINDOW_TITLE).root(
                LivePerformanceRoot::new(
                    WINDOW_TITLE,
                    WINDOW_DESCRIPTION,
                    build_widget_book_gallery(Rc::clone(&state)),
                )
                .watch_widget_book_state(state),
            ),
        )
        .into_application()
}

pub fn build_theme_demo_application(state: Rc<RefCell<WidgetBookState>>) -> Application {
    set_widget_book_hdr_theme_mode(HdrThemeMode::Disabled);

    App::new()
        .window(
            Window::new(THEME_DEMO_TITLE).root(
                LivePerformanceRoot::new(
                    THEME_DEMO_TITLE,
                    THEME_DEMO_DESCRIPTION,
                    build_theme_demo_surface(Rc::clone(&state)),
                )
                .watch_widget_book_state(state),
            ),
        )
        .into_application()
}

#[cfg(feature = "native")]
pub fn run_desktop_widget_book() -> Result<()> {
    build_widget_book_application(default_widget_book_state()).run()
}

impl Widget for LivePerformanceRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Window(WindowEvent::RedrawRequested)) {
            let overlay_enabled = self.overlay_enabled();
            if overlay_enabled != self.last_overlay_enabled {
                self.last_overlay_enabled = overlay_enabled;
                ctx.request_measure();
                ctx.request_semantics();
                ctx.request_paint();

                if !overlay_enabled {
                    self.set_performance_display(None, true);
                    if self.owns_detail_mode {
                        set_window_scene_statistics_detail_mode(
                            ctx.window_id(),
                            SceneStatisticsDetailMode::Lightweight,
                        );
                        self.owns_detail_mode = false;
                    }
                }
            }

            if let Some(state) = &self.watched_state {
                let next_state = state.borrow().clone();
                if self.last_seen_state.as_ref() != Some(&next_state) {
                    self.last_seen_state = Some(next_state);
                    let content_id = self.content.child().id();
                    ctx.request(
                        InvalidationRequest::new(
                            InvalidationTarget::Widget(content_id),
                            InvalidationKind::Paint,
                        )
                        .with_region(self.content.child().bounds()),
                    );
                    ctx.request(
                        InvalidationRequest::new(
                            InvalidationTarget::Widget(content_id),
                            InvalidationKind::Semantics,
                        )
                        .with_region(self.content.child().bounds()),
                    );
                }
            }

            if overlay_enabled {
                if !window_scene_statistics_detail_mode(ctx.window_id()).is_detailed() {
                    set_window_scene_statistics_detail_mode(
                        ctx.window_id(),
                        SceneStatisticsDetailMode::Detailed,
                    );
                    self.owns_detail_mode = true;
                }

                if let Some(snapshot) = window_performance_snapshot(ctx.window_id())
                    && self.set_performance_display(Some(snapshot), false)
                {
                    let overlay_id = self.performance_overlay.child().id();
                    ctx.request(
                        InvalidationRequest::new(
                            InvalidationTarget::Widget(overlay_id),
                            InvalidationKind::Paint,
                        )
                        .with_region(self.performance_overlay.child().bounds()),
                    );
                }
            }
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let viewport = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                1280.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                720.0
            },
        ));
        self.content.measure(ctx, Constraints::tight(viewport));
        if self.overlay_enabled() {
            self.performance_overlay
                .measure(ctx, Constraints::new(Size::ZERO, viewport));
        }
        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content
            .arrange(ctx, Rect::from_origin_size(bounds.origin, bounds.size));

        if self.overlay_enabled() {
            let overlay_size = self.performance_overlay.child().measured_size();
            let overlay_x = (bounds.max_x() - overlay_size.width - Self::OVERLAY_MARGIN.right)
                .max(bounds.x() + Self::OVERLAY_MARGIN.left);
            let overlay_y = bounds.y() + Self::OVERLAY_MARGIN.top;
            self.performance_overlay.arrange(
                ctx,
                Rect::from_origin_size(Point::new(overlay_x, overlay_y), overlay_size),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.95, 0.968, 0.985, 1.0));
        self.content.paint(ctx);
        if self.overlay_enabled() {
            self.performance_overlay.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
        root.name = Some(self.window_title.clone());
        root.description = Some(self.window_description.clone());
        ctx.push(root);
        self.content.semantics(ctx);
        if self.overlay_enabled() {
            self.performance_overlay.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        if self.overlay_enabled() {
            self.performance_overlay.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        if self.overlay_enabled() {
            self.performance_overlay.visit_children_mut(visitor);
        }
    }
}

struct ProjectSettingsPreview {
    trigger: SingleChild,
    dialog: SingleChild,
    dialog_open: bool,
    trigger_pressed: bool,
}

struct MinimumWidth {
    min_width: f32,
    child: SingleChild,
}

impl MinimumWidth {
    fn new<W>(min_width: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            min_width: min_width.max(0.0),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for MinimumWidth {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_width = constraints.max.width.max(self.min_width);
        let child_constraints = Constraints::new(
            Size::new(
                constraints.min.width.max(self.min_width).min(max_width),
                constraints.min.height,
            ),
            Size::new(max_width, constraints.max.height),
        );
        let child_size = self.child.measure(ctx, child_constraints);
        Size::new(child_size.width.max(self.min_width), child_size.height)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin, self.child.child().measured_size()),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct MaximumWidth {
    max_width: f32,
    child: SingleChild,
}

impl MaximumWidth {
    fn new<W>(max_width: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            max_width: max_width.max(1.0),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for MaximumWidth {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_width = if constraints.max.width.is_finite() {
            constraints.max.width.min(self.max_width)
        } else {
            self.max_width
        };
        let child_constraints = Constraints::new(
            Size::new(constraints.min.width.min(max_width), constraints.min.height),
            Size::new(max_width, constraints.max.height),
        );
        let child_size = self.child.measure(ctx, child_constraints);

        Size::new(
            child_size
                .width
                .min(max_width)
                .max(constraints.min.width.min(max_width)),
            child_size
                .height
                .clamp(constraints.min.height, constraints.max.height),
        )
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin, self.child.child().measured_size()),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct VerticalScrollPane {
    spacing: f32,
    content: SingleChild,
    scroll_bar: SingleChild,
}

impl VerticalScrollPane {
    fn new<W, S>(content: W, scroll_bar: S) -> Self
    where
        W: Widget + 'static,
        S: Widget + 'static,
    {
        Self {
            spacing: 0.0,
            content: SingleChild::new(content),
            scroll_bar: SingleChild::new(scroll_bar),
        }
    }
}

impl Widget for VerticalScrollPane {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let scroll_bar_size = self.scroll_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(f32::INFINITY, constraints.max.height)),
        );
        let content_constraints = Constraints::new(
            Size::new(
                (constraints.min.width - scroll_bar_size.width - self.spacing).max(0.0),
                constraints.min.height,
            ),
            Size::new(
                (constraints.max.width - scroll_bar_size.width - self.spacing).max(0.0),
                constraints.max.height,
            ),
        );
        let content_size = self.content.measure(ctx, content_constraints);

        constraints.clamp(Size::new(
            content_size.width + scroll_bar_size.width + self.spacing,
            content_size.height.max(scroll_bar_size.height),
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let scroll_bar_size = self.scroll_bar.child().measured_size();
        let content_width = (bounds.width() - scroll_bar_size.width - self.spacing).max(0.0);
        self.content.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), content_width, bounds.height()),
        );
        self.scroll_bar.arrange(
            ctx,
            Rect::new(
                bounds.max_x() - scroll_bar_size.width,
                bounds.y(),
                scroll_bar_size.width,
                bounds.height(),
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
        self.scroll_bar.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
        self.scroll_bar.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.scroll_bar.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.scroll_bar.visit_children_mut(visitor);
    }
}

struct TwoAxisScrollPane {
    spacing: f32,
    state: ScrollState,
    show_vertical_scroll_bar: bool,
    show_horizontal_scroll_bar: bool,
    content: SingleChild,
    vertical_scroll_bar: SingleChild,
    horizontal_scroll_bar: SingleChild,
}

impl TwoAxisScrollPane {
    fn new<W, V, H>(
        state: ScrollState,
        content: W,
        vertical_scroll_bar: V,
        horizontal_scroll_bar: H,
    ) -> Self
    where
        W: Widget + 'static,
        V: Widget + 'static,
        H: Widget + 'static,
    {
        Self {
            spacing: 0.0,
            state,
            show_vertical_scroll_bar: true,
            show_horizontal_scroll_bar: true,
            content: SingleChild::new(content),
            vertical_scroll_bar: SingleChild::new(vertical_scroll_bar),
            horizontal_scroll_bar: SingleChild::new(horizontal_scroll_bar),
        }
    }

    fn viewport_size(&self, bounds: Size) -> Size {
        let vertical_size = self.vertical_scroll_bar.child().measured_size();
        let horizontal_size = self.horizontal_scroll_bar.child().measured_size();
        let vertical_extent = if self.show_vertical_scroll_bar {
            vertical_size.width + self.spacing
        } else {
            0.0
        };
        let horizontal_extent = if self.show_horizontal_scroll_bar {
            horizontal_size.height + self.spacing
        } else {
            0.0
        };
        Size::new(
            (bounds.width - vertical_extent).max(0.0),
            (bounds.height - horizontal_extent).max(0.0),
        )
    }

    fn content_constraints(
        constraints: Constraints,
        vertical_size: Size,
        horizontal_size: Size,
        show_vertical_scroll_bar: bool,
        show_horizontal_scroll_bar: bool,
        spacing: f32,
    ) -> Constraints {
        let vertical_extent = if show_vertical_scroll_bar {
            vertical_size.width + spacing
        } else {
            0.0
        };
        let horizontal_extent = if show_horizontal_scroll_bar {
            horizontal_size.height + spacing
        } else {
            0.0
        };
        Constraints::new(
            Size::new(
                (constraints.min.width - vertical_extent).max(0.0),
                (constraints.min.height - horizontal_extent).max(0.0),
            ),
            Size::new(
                (constraints.max.width - vertical_extent).max(0.0),
                (constraints.max.height - horizontal_extent).max(0.0),
            ),
        )
    }

    fn scroll_bar_visibility(&self) -> (bool, bool) {
        let viewport = self.state.viewport_size();
        let content = self.state.content_size();
        (
            content.width > viewport.width + 0.001,
            content.height > viewport.height + 0.001,
        )
    }
}

impl Widget for TwoAxisScrollPane {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let vertical_size = self.vertical_scroll_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(f32::INFINITY, constraints.max.height)),
        );
        let horizontal_size = self.horizontal_scroll_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(constraints.max.width, f32::INFINITY)),
        );
        let mut show_vertical = false;
        let mut show_horizontal = false;
        let mut content_size = self.content.measure(
            ctx,
            Self::content_constraints(
                constraints,
                vertical_size,
                horizontal_size,
                show_vertical,
                show_horizontal,
                self.spacing,
            ),
        );
        for _ in 0..3 {
            let (next_horizontal, next_vertical) = self.scroll_bar_visibility();
            if next_vertical == show_vertical && next_horizontal == show_horizontal {
                break;
            }
            show_vertical = next_vertical;
            show_horizontal = next_horizontal;
            content_size = self.content.measure(
                ctx,
                Self::content_constraints(
                    constraints,
                    vertical_size,
                    horizontal_size,
                    show_vertical,
                    show_horizontal,
                    self.spacing,
                ),
            );
        }

        self.show_vertical_scroll_bar = show_vertical;
        self.show_horizontal_scroll_bar = show_horizontal;
        let vertical_extent = if show_vertical {
            vertical_size.width + self.spacing
        } else {
            0.0
        };
        let horizontal_extent = if show_horizontal {
            horizontal_size.height + self.spacing
        } else {
            0.0
        };
        constraints.clamp(Size::new(
            content_size.width + vertical_extent,
            content_size.height + horizontal_extent,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let viewport = self.viewport_size(bounds.size);
        self.content.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), viewport.width, viewport.height),
        );
        self.vertical_scroll_bar.arrange(
            ctx,
            if self.show_vertical_scroll_bar {
                Rect::new(
                    bounds.x() + viewport.width + self.spacing,
                    bounds.y(),
                    self.vertical_scroll_bar.child().measured_size().width,
                    viewport.height,
                )
            } else {
                Rect::new(bounds.max_x(), bounds.y(), 0.0, 0.0)
            },
        );
        self.horizontal_scroll_bar.arrange(
            ctx,
            if self.show_horizontal_scroll_bar {
                Rect::new(
                    bounds.x(),
                    bounds.y() + viewport.height + self.spacing,
                    viewport.width,
                    self.horizontal_scroll_bar.child().measured_size().height,
                )
            } else {
                Rect::new(bounds.x(), bounds.max_y(), 0.0, 0.0)
            },
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
        if self.show_vertical_scroll_bar {
            self.vertical_scroll_bar.paint(ctx);
        }
        if self.show_horizontal_scroll_bar {
            self.horizontal_scroll_bar.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
        if self.show_vertical_scroll_bar {
            self.vertical_scroll_bar.semantics(ctx);
        }
        if self.show_horizontal_scroll_bar {
            self.horizontal_scroll_bar.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.vertical_scroll_bar.visit_children(visitor);
        self.horizontal_scroll_bar.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.vertical_scroll_bar.visit_children_mut(visitor);
        self.horizontal_scroll_bar.visit_children_mut(visitor);
    }
}

impl ProjectSettingsPreview {
    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        Self {
            trigger: SingleChild::new(Button::new(DIALOG_TRIGGER_LABEL).min_width(220.0)),
            dialog: SingleChild::new(
                Dialog::new(
                    DIALOG_TITLE,
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Stretch)
                        .with_child(
                            Label::new("Autosave every 90 seconds").theme(DefaultTheme::default()),
                        )
                        .with_child(
                            Label::new("Export color profile: Display P3")
                                .theme(DefaultTheme::default()),
                        )
                        .with_child(
                            Label::new("Scratch disk: fast-local-ssd")
                                .theme(DefaultTheme::default()),
                        ),
                )
                .description(
                    "Compact dialog framing for confirmations, settings, and import/export flows.",
                )
                .modal(false)
                .secondary_action("Cancel", || {})
                .primary_action("Apply", move || {
                    state.borrow_mut().dialog_apply_count += 1;
                }),
            ),
            dialog_open: false,
            trigger_pressed: false,
        }
    }

    fn trigger_bounds(&self) -> Rect {
        self.trigger.child().bounds()
    }
}

impl Widget for ProjectSettingsPreview {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() != sui::EventPhase::Capture {
            return;
        }

        match event {
            Event::Window(WindowEvent::RedrawRequested) => {}
            Event::Pointer(pointer)
                if pointer.kind == sui::PointerEventKind::Down
                    && pointer.button == Some(sui::PointerButton::Primary) =>
            {
                self.trigger_pressed = self.trigger_bounds().contains(pointer.position);
            }
            Event::Pointer(pointer)
                if pointer.kind == sui::PointerEventKind::Up
                    && pointer.button == Some(sui::PointerButton::Primary) =>
            {
                let activate =
                    self.trigger_pressed && self.trigger_bounds().contains(pointer.position);
                self.trigger_pressed = false;
                if activate {
                    self.dialog_open = !self.dialog_open;
                    ctx.request_measure();
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == sui::PointerEventKind::Cancel => {
                self.trigger_pressed = false;
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let trigger_size = self.trigger.measure(ctx, constraints.loosen());

        if !self.dialog_open {
            return constraints.clamp(trigger_size);
        }

        let dialog_size = self
            .dialog
            .measure(ctx, Constraints::tight(Size::new(560.0, 320.0)));

        constraints.clamp(Size::new(
            trigger_size.width.max(dialog_size.width),
            trigger_size.height + 12.0 + dialog_size.height,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let trigger_size = self.trigger.child().measured_size();
        self.trigger
            .arrange(ctx, Rect::from_origin_size(bounds.origin, trigger_size));
        if self.dialog_open {
            let dialog_size = self.dialog.child().measured_size();
            self.dialog.arrange(
                ctx,
                Rect::new(
                    bounds.x(),
                    bounds.y() + trigger_size.height + 12.0,
                    dialog_size.width,
                    dialog_size.height,
                ),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.trigger.paint(ctx);
        if self.dialog_open {
            self.dialog.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.trigger.semantics(ctx);
        if self.dialog_open {
            self.dialog.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.trigger.visit_children(visitor);
        if self.dialog_open {
            self.dialog.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.trigger.visit_children_mut(visitor);
        if self.dialog_open {
            self.dialog.visit_children_mut(visitor);
        }
    }
}

struct ThemePreviewGrid {
    cards: WidgetChildren,
}

impl ThemePreviewGrid {
    const GAP: f32 = 16.0;
    const CARD_HEIGHT: f32 = 248.0;

    fn new() -> Self {
        let mut cards = WidgetChildren::with_capacity(5);
        for (name, theme, title, action_label, input_label) in [
            (
                LIGHT_THEME_PREVIEW_CARD_NAME,
                DefaultTheme::sui(),
                "SUI light",
                LIGHT_PREVIEW_ACTION_LABEL,
                LIGHT_PREVIEW_INPUT_LABEL,
            ),
            (
                NEUTRAL_THEME_PREVIEW_CARD_NAME,
                DefaultTheme::neutral(),
                "Neutral light",
                NEUTRAL_PREVIEW_ACTION_LABEL,
                NEUTRAL_PREVIEW_INPUT_LABEL,
            ),
            (
                DARK_THEME_PREVIEW_CARD_NAME,
                DefaultTheme::dark(),
                "SUI dark",
                DARK_PREVIEW_ACTION_LABEL,
                DARK_PREVIEW_INPUT_LABEL,
            ),
            (
                NEUTRAL_DARK_THEME_PREVIEW_CARD_NAME,
                DefaultTheme::neutral_dark(),
                "Neutral dark",
                NEUTRAL_DARK_PREVIEW_ACTION_LABEL,
                NEUTRAL_DARK_PREVIEW_INPUT_LABEL,
            ),
            (
                TRUE_BLACK_THEME_PREVIEW_CARD_NAME,
                DefaultTheme::high_contrast(),
                "SUI true black",
                TRUE_BLACK_PREVIEW_ACTION_LABEL,
                TRUE_BLACK_PREVIEW_INPUT_LABEL,
            ),
        ] {
            cards.push(NamedSection::new(
                name,
                theme_preview_card(theme, title, action_label, input_label),
            ));
        }
        Self { cards }
    }

    fn columns_for_width(width: f32) -> usize {
        if width >= 1040.0 {
            3
        } else if width >= 680.0 {
            2
        } else {
            1
        }
    }

    fn column_width(width: f32, columns: usize) -> f32 {
        ((width - Self::GAP * columns.saturating_sub(1) as f32).max(0.0) / columns as f32).max(0.0)
    }

    fn content_height(&self, columns: usize) -> f32 {
        let rows = self.cards.len().div_ceil(columns) as f32;
        rows * Self::CARD_HEIGHT + (rows - 1.0).max(0.0) * Self::GAP
    }
}

impl Widget for ThemePreviewGrid {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(1120.0)
        };
        let columns = Self::columns_for_width(width);
        let column_width = Self::column_width(width, columns);
        let card_constraints = Constraints::tight(Size::new(column_width, Self::CARD_HEIGHT));
        for index in 0..self.cards.len() {
            self.cards.measure_child(index, ctx, card_constraints);
        }
        constraints.clamp(Size::new(width, self.content_height(columns)))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let columns = Self::columns_for_width(bounds.width());
        let column_width = Self::column_width(bounds.width(), columns);
        for index in 0..self.cards.len() {
            let column = index % columns;
            let row = index / columns;
            self.cards.arrange_child(
                index,
                ctx,
                Rect::new(
                    bounds.x() + column as f32 * (column_width + Self::GAP),
                    bounds.y() + row as f32 * (Self::CARD_HEIGHT + Self::GAP),
                    column_width,
                    Self::CARD_HEIGHT,
                ),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.cards.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(THEME_PREVIEW_NAME.to_string());
        node.description =
            Some("Five built-in theme preview cards are visible in a responsive grid.".to_string());
        ctx.push(node);
        self.cards.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.cards.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.cards.visit_children_mut(visitor);
    }
}

fn hdr_theme_mode_title(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => "SDR baseline",
        HdrThemeMode::WideGamutOnly => "Wide-gamut-only",
        HdrThemeMode::ConstrainedHdr => "Constrained HDR",
        HdrThemeMode::FullHdr => "Full HDR",
    }
}

fn hdr_theme_mode_explanation(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => {
            "Uses the SDR fallback path only. Wide-gamut and HDR token branches stay available in the theme, but built-in widgets resolve to the existing SDR palette and luminance ceilings."
        }
        HdrThemeMode::WideGamutOnly => {
            "Prefers richer gamut variants while keeping luminance pinned to reference white. This validates color-volume differences without introducing above-white UI chrome."
        }
        HdrThemeMode::ConstrainedHdr => {
            "Allows a modest lift for accents, focused states, and emissive indicators while still treating reference white as the visual anchor."
        }
        HdrThemeMode::FullHdr => {
            "Allows the same semantic tokens to push farther into HDR headroom so popup arrivals and indicator energy can separate more clearly from the constrained path."
        }
    }
}

fn hdr_theme_lab_section_name(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => "SDR baseline comparison",
        HdrThemeMode::WideGamutOnly => "Wide-gamut-only comparison",
        HdrThemeMode::ConstrainedHdr => "Constrained HDR comparison",
        HdrThemeMode::FullHdr => "Full HDR comparison",
    }
}

fn hdr_theme_lab_theme(mode: HdrThemeMode) -> DefaultTheme {
    let mut theme = DefaultTheme::dark();
    theme.hdr = HdrThemeTokens::from_default_theme(theme);
    theme.hdr.mode = mode;
    theme.hdr.color_roles.surface = SemanticColorToken::from_sdr(theme.colors.base_100)
        .with_wide_gamut(Color::display_p3(0.13, 0.16, 0.23, 1.0))
        .with_hdr(Color::linear_display_p3(0.18, 0.21, 0.30, 1.0));
    theme.hdr.color_roles.surface_elevated = SemanticColorToken::from_sdr(theme.colors.base_200)
        .with_wide_gamut(Color::display_p3(0.16, 0.19, 0.28, 1.0))
        .with_hdr(Color::linear_display_p3(0.24, 0.27, 0.38, 1.0));
    theme.hdr.color_roles.surface_outline = SemanticColorToken::from_sdr(theme.colors.base_300)
        .with_wide_gamut(Color::display_p3(0.33, 0.39, 0.50, 1.0))
        .with_hdr(Color::linear_display_p3(0.42, 0.48, 0.62, 1.0));
    theme.hdr.color_roles.text = SemanticColorToken::from_sdr(theme.colors.base_content)
        .with_wide_gamut(Color::display_p3(0.92, 0.95, 0.99, 1.0))
        .with_hdr(Color::linear_display_p3(1.02, 1.04, 1.10, 1.0));
    theme.hdr.color_roles.text_muted =
        SemanticColorToken::from_sdr(theme.colors.base_content.with_alpha(0.74))
            .with_wide_gamut(Color::display_p3(0.75, 0.80, 0.89, 1.0))
            .with_hdr(Color::linear_display_p3(0.86, 0.90, 0.98, 1.0));
    theme.hdr.color_roles.accent = SemanticColorToken::from_sdr(theme.colors.primary)
        .with_wide_gamut(Color::display_p3(0.18, 0.74, 0.96, 1.0))
        .with_hdr(Color::linear_display_p3(0.78, 2.40, 3.20, 1.0));
    theme.hdr.color_roles.accent_text = SemanticColorToken::from_sdr(theme.colors.primary_content)
        .with_wide_gamut(Color::display_p3(0.03, 0.08, 0.12, 1.0))
        .with_hdr(Color::linear_display_p3(0.10, 0.14, 0.20, 1.0));
    theme.hdr.color_roles.secondary = SemanticColorToken::from_sdr(theme.colors.secondary)
        .with_wide_gamut(Color::display_p3(0.43, 0.66, 0.98, 1.0))
        .with_hdr(Color::linear_display_p3(0.96, 1.72, 2.42, 1.0));
    theme.hdr.color_roles.warning = SemanticColorToken::from_sdr(theme.colors.warning)
        .with_wide_gamut(Color::display_p3(0.98, 0.68, 0.18, 1.0))
        .with_hdr(Color::linear_display_p3(3.00, 1.50, 0.30, 1.0));
    theme.hdr.color_roles.info = SemanticColorToken::from_sdr(theme.colors.info)
        .with_wide_gamut(Color::display_p3(0.40, 0.78, 0.98, 1.0))
        .with_hdr(Color::linear_display_p3(0.88, 2.00, 2.90, 1.0));
    theme.hdr.luminance = HdrLuminanceTokens::constrained_defaults();
    theme.hdr.policy.max_large_area_lift = 1.18;
    theme.hdr.policy.max_constrained_lift = 1.32;
    theme.hdr.policy.max_emissive_lift = 1.75;
    theme.hdr.effects.pulse.speed = 1.1;
    theme.hdr.effects.pulse.color = Some(resolve_semantic_color(
        theme.hdr.color_roles.warning,
        HdrThemeMode::FullHdr,
    ));

    match mode {
        HdrThemeMode::Disabled | HdrThemeMode::WideGamutOnly => {}
        HdrThemeMode::ConstrainedHdr => {
            theme.hdr.luminance.focused = 1.08;
            theme.hdr.luminance.semantic_accent = 1.16;
            theme.hdr.luminance.emissive_indicator = 1.55;
            theme.hdr.luminance.alert_pulse = 1.42;
        }
        HdrThemeMode::FullHdr => {
            theme.hdr.luminance.focused = 1.18;
            theme.hdr.luminance.semantic_accent = 1.34;
            theme.hdr.luminance.emissive_indicator = 2.40;
            theme.hdr.luminance.alert_pulse = 2.05;
            theme.hdr.policy.max_large_area_lift = 1.36;
            theme.hdr.policy.max_emissive_lift = 2.60;
            theme.hdr.materials.raised.specular_strength = 0.18;
            theme.hdr.materials.raised.rim_light_strength = 0.14;
            theme.hdr.effects.glow.intensity = 0.32;
            theme.hdr.effects.pulse.intensity = 0.54;
        }
    }

    theme
}

fn hdr_theme_lab_card(
    section_name: impl Into<String>,
    mode: HdrThemeMode,
    prefix: impl Into<String>,
    lead_text: impl Into<String>,
) -> impl Widget {
    let section_name = section_name.into();
    let prefix = prefix.into();
    let lead_text = lead_text.into();
    let theme = hdr_theme_lab_theme(mode);
    let indicator_style = resolve_widget_hdr_style(
        &theme.hdr,
        WidgetColorRole::Accent,
        WidgetLuminanceRole::EmissiveIndicator,
        WidgetMaterialRole::Flat,
        None,
    );
    let indicator_color = Color::new(
        indicator_style.color.space,
        indicator_style
            .color
            .red
            .clamp(0.0, indicator_style.peak_lift),
        indicator_style
            .color
            .green
            .clamp(0.0, indicator_style.peak_lift),
        indicator_style
            .color
            .blue
            .clamp(0.0, indicator_style.peak_lift),
        indicator_style.color.alpha,
    );
    let button_label = format!("{prefix} sample action");
    let switch_label = format!("{prefix} sample live indicator");
    let popover_name = format!("{prefix} attention popover");
    let popover_trigger_label = format!("{prefix} attention trigger");
    let swatch_name = format!("{prefix} emissive indicator");

    NamedSection::new(
        section_name,
        ThemePreviewCardFrame::new(
            theme,
            Stack::vertical()
                .spacing(12.0)
                .alignment(Alignment::Start)
                .with_child(
                    Label::new(hdr_theme_mode_title(mode)).style(demo_text_style(
                        theme,
                        DemoTextRole::Emphasis,
                        theme.palette.text,
                    )),
                )
                .with_child(MaximumWidth::new(
                    980.0,
                    Label::new(lead_text).style(demo_text_style(
                        theme,
                        DemoTextRole::Supporting,
                        theme.palette.placeholder,
                    )),
                ))
                .with_child(MaximumWidth::new(
                    980.0,
                    Label::new(format!(
                        "Token mode: {} Â· accent peak {:.2}Ã— Â· indicator peak {:.2}Ã— Â· alert peak {:.2}Ã—",
                        hdr_theme_mode_title(mode),
                        theme.hdr.luminance.semantic_accent,
                        theme.hdr.luminance.emissive_indicator,
                        theme.hdr.luminance.alert_pulse,
                    ))
                    .style(demo_text_style(
                        theme,
                        DemoTextRole::Metadata,
                        theme.palette.placeholder,
                    )),
                ))
                .with_child(
                    Stack::horizontal()
                        .spacing(12.0)
                        .alignment(Alignment::Center)
                        .with_child(
                            SizedBox::new().width(300.0).with_child(
                                Button::primary(button_label)
                                    .min_width(280.0)
                                    .theme(theme),
                            ),
                        )
                        .with_child(
                            ColorSwatch::new(swatch_name, indicator_color)
                                .size(Size::new(64.0, 28.0)),
                        )
                        .with_child(MaximumWidth::new(
                            520.0,
                            Label::new(
                                "The swatch mirrors the accent token resolved for the current gamut/HDR mode.",
                            )
                            .style(demo_text_style(
                                theme,
                                DemoTextRole::Metadata,
                                theme.palette.placeholder,
                            )),
                        )),
                )
                .with_child(
                    SizedBox::new().width(520.0).with_child(
                        Switch::new(switch_label)
                            .on(!matches!(mode, HdrThemeMode::Disabled))
                            .theme(theme),
                    ),
                )
                .with_child(
                    SizedBox::new().width(430.0).with_child(
                        Popover::new(
                            popover_name,
                            Button::new(popover_trigger_label)
                                .min_width(400.0)
                                .theme(theme),
                            MaximumWidth::new(
                                380.0,
                                Stack::vertical()
                                    .spacing(8.0)
                                    .alignment(Alignment::Stretch)
                                    .with_child(
                                        Label::new(
                                            "Small popup surfaces are where constrained vs full HDR arrival cues become easiest to validate.",
                                        )
                                        .style(demo_text_style(
                                            theme,
                                            DemoTextRole::Supporting,
                                            theme.palette.text,
                                        )),
                                    )
                                    .with_child(
                                        Label::new(
                                            "Use this trigger to compare popup chrome, border lift, and arrival emphasis against the matching button and switch.",
                                        )
                                        .style(demo_text_style(
                                            theme,
                                            DemoTextRole::Metadata,
                                            theme.palette.placeholder,
                                        )),
                                    )
                            ),
                        )
                        .theme(theme),
                    ),
                ),
        ),
    )
}

struct HdrThemeLabShowcase {
    active_mode: HdrThemeMode,
    active_preview: SingleChild,
    sdr_card: SingleChild,
    wide_gamut_card: SingleChild,
    constrained_card: SingleChild,
    full_hdr_card: SingleChild,
}

impl HdrThemeLabShowcase {
    const SECTION_GAP: f32 = 14.0;

    fn new() -> Self {
        let active_mode = widget_book_hdr_theme_mode();
        Self {
            active_mode,
            active_preview: SingleChild::new(Self::build_active_preview(active_mode)),
            sdr_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::Disabled),
                HdrThemeMode::Disabled,
                hdr_theme_mode_title(HdrThemeMode::Disabled),
                hdr_theme_mode_explanation(HdrThemeMode::Disabled),
            )),
            wide_gamut_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::WideGamutOnly),
                HdrThemeMode::WideGamutOnly,
                hdr_theme_mode_title(HdrThemeMode::WideGamutOnly),
                hdr_theme_mode_explanation(HdrThemeMode::WideGamutOnly),
            )),
            constrained_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::ConstrainedHdr),
                HdrThemeMode::ConstrainedHdr,
                hdr_theme_mode_title(HdrThemeMode::ConstrainedHdr),
                hdr_theme_mode_explanation(HdrThemeMode::ConstrainedHdr),
            )),
            full_hdr_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::FullHdr),
                HdrThemeMode::FullHdr,
                hdr_theme_mode_title(HdrThemeMode::FullHdr),
                hdr_theme_mode_explanation(HdrThemeMode::FullHdr),
            )),
        }
    }

    fn build_active_preview(mode: HdrThemeMode) -> impl Widget {
        hdr_theme_lab_card(
            HDR_THEME_LAB_ACTIVE_PREVIEW_NAME,
            mode,
            format!("Current {} preview", hdr_theme_mode_title(mode)),
            format!(
                "This preview follows the shared HDR theme mode currently selected by the dev host: {}. Use it to compare the active styling path against the four fixed comparison cards below.",
                hdr_theme_mode_title(mode),
            ),
        )
    }

    fn sync_active_preview(&mut self) -> bool {
        let next_mode = widget_book_hdr_theme_mode();
        if next_mode == self.active_mode {
            return false;
        }

        self.active_mode = next_mode;
        self.active_preview = SingleChild::new(Self::build_active_preview(next_mode));
        true
    }
}

impl Widget for HdrThemeLabShowcase {
    fn event(&mut self, ctx: &mut EventCtx, _event: &Event) {
        if self.sync_active_preview() {
            ctx.request_measure();
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_width = if constraints.max.width.is_finite() {
            constraints.max.width.max(320.0)
        } else {
            760.0
        };
        let child_constraints = Constraints::new(Size::ZERO, Size::new(max_width, f32::INFINITY));
        let mut height = 0.0;
        let mut width: f32 = 0.0;

        for child in [
            &mut self.active_preview,
            &mut self.sdr_card,
            &mut self.wide_gamut_card,
            &mut self.constrained_card,
            &mut self.full_hdr_card,
        ] {
            let size = child.measure(ctx, child_constraints);
            width = width.max(size.width);
            height += size.height;
        }

        height += Self::SECTION_GAP * 4.0;
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let mut y = bounds.y();
        for child in [
            &mut self.active_preview,
            &mut self.sdr_card,
            &mut self.wide_gamut_card,
            &mut self.constrained_card,
            &mut self.full_hdr_card,
        ] {
            let size = child.child().measured_size();
            child.arrange(
                ctx,
                Rect::new(bounds.x(), y, bounds.width().min(size.width), size.height),
            );
            y += size.height + Self::SECTION_GAP;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.active_preview.paint(ctx);
        self.sdr_card.paint(ctx);
        self.wide_gamut_card.paint(ctx);
        self.constrained_card.paint(ctx);
        self.full_hdr_card.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(HDR_THEME_LAB_NAME.to_string());
        node.description = Some(format!(
            "Compares the same button, switch, emissive indicator, and popup trigger across SDR baseline, wide-gamut-only, constrained HDR, and full HDR. The shared preview currently uses {}.",
            hdr_theme_mode_title(self.active_mode),
        ));
        ctx.push(node);
        self.active_preview.semantics(ctx);
        self.sdr_card.semantics(ctx);
        self.wide_gamut_card.semantics(ctx);
        self.constrained_card.semantics(ctx);
        self.full_hdr_card.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.active_preview.visit_children(visitor);
        self.sdr_card.visit_children(visitor);
        self.wide_gamut_card.visit_children(visitor);
        self.constrained_card.visit_children(visitor);
        self.full_hdr_card.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.active_preview.visit_children_mut(visitor);
        self.sdr_card.visit_children_mut(visitor);
        self.wide_gamut_card.visit_children_mut(visitor);
        self.constrained_card.visit_children_mut(visitor);
        self.full_hdr_card.visit_children_mut(visitor);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AnimationBenchmarkRetainedPresentation {
    opacity: f32,
    translation: Vector,
}

impl Default for AnimationBenchmarkRetainedPresentation {
    fn default() -> Self {
        Self {
            opacity: 0.72,
            translation: Vector::new(-24.0, 0.0),
        }
    }
}

impl TimelineBindingSink for AnimationBenchmarkRetainedPresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != ANIMATION_BENCHMARK_RETAINED_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::LayerOpacity, AnimationValue::Scalar(value)) => {
                let value = value.clamp(0.25, 1.0);
                let changed = (self.opacity - value).abs() > 0.001;
                self.opacity = value;
                changed
            }
            (AnimationProperty::LayerTranslation, AnimationValue::Vector(value)) => {
                let changed = self.translation != value;
                self.translation = value;
                changed
            }
            _ => false,
        }
    }
}

struct AnimationBenchmarkRetainedLane {
    player: TimelinePlayer,
    presentation: AnimationBenchmarkRetainedPresentation,
}

impl AnimationBenchmarkRetainedLane {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(animation_benchmark_retained_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        let mut presentation = AnimationBenchmarkRetainedPresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
            ctx.request_animation_frame();
        }
    }
}

impl Widget for AnimationBenchmarkRetainedLane {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(920.0, 112.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.10, 0.12, 0.14, 1.0),
        );

        let rail = Rect::new(
            bounds.x() + 38.0,
            bounds.y() + bounds.height() * 0.5 - 3.0,
            bounds.width() - 76.0,
            6.0,
        );
        ctx.fill(
            Path::rounded_rect(rail, 3.0),
            Color::rgba(0.42, 0.47, 0.56, 0.40),
        );

        let marker = Rect::new(
            bounds.x() + bounds.width() * 0.5 - 36.0,
            bounds.y() + 28.0,
            72.0,
            44.0,
        );
        ctx.fill(
            Path::rounded_rect(marker, 7.0),
            Color::rgba(0.34, 0.72, 0.88, 0.88),
        );
        ctx.stroke_rect(
            marker,
            Color::rgba(0.86, 0.96, 1.0, 0.78),
            StrokeStyle::new(1.0),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties::default()
            .with_opacity(self.presentation.opacity)
            .with_translation(self.presentation.translation)
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(ANIMATION_BENCHMARK_RETAINED_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "opacity {:.2}, x {:.1}",
            self.presentation.opacity, self.presentation.translation.x
        )));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AnimationBenchmarkPaintPresentation {
    fill: Color,
    radius: f32,
    alpha: f32,
}

impl Default for AnimationBenchmarkPaintPresentation {
    fn default() -> Self {
        Self {
            fill: Color::rgba(0.82, 0.33, 0.24, 1.0),
            radius: 18.0,
            alpha: 0.76,
        }
    }
}

impl TimelineBindingSink for AnimationBenchmarkPaintPresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != ANIMATION_BENCHMARK_REPAINT_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::FillColor, AnimationValue::Color(value)) => {
                let changed = self.fill != value;
                self.fill = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_RADIUS_PATH =>
            {
                let value = value.max(3.0);
                let changed = (self.radius - value).abs() > 0.001;
                self.radius = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_ALPHA_PATH =>
            {
                let value = value.clamp(0.25, 1.0);
                let changed = (self.alpha - value).abs() > 0.001;
                self.alpha = value;
                changed
            }
            _ => false,
        }
    }
}

struct AnimationBenchmarkRepaintLane {
    player: TimelinePlayer,
    presentation: AnimationBenchmarkPaintPresentation,
}

impl AnimationBenchmarkRepaintLane {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(animation_benchmark_repaint_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        let mut presentation = AnimationBenchmarkPaintPresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
            ctx.request_animation_frame();
        }
    }
}

impl Widget for AnimationBenchmarkRepaintLane {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(920.0, 136.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.13, 0.12, 0.11, 1.0),
        );

        let lanes = 11;
        for lane in 0..lanes {
            let t = lane as f32 / (lanes - 1) as f32;
            let x = bounds.x() + 38.0 + (bounds.width() - 76.0) * t;
            let y = bounds.y() + bounds.height() * 0.5;
            let radius = self.presentation.radius * (0.56 + 0.045 * lane as f32);
            let alpha = (self.presentation.alpha * (1.0 - t * 0.35)).clamp(0.15, 1.0);
            let color = Color::rgba(
                (self.presentation.fill.red + t * 0.10).min(1.0),
                self.presentation.fill.green,
                (self.presentation.fill.blue + (1.0 - t) * 0.10).min(1.0),
                alpha,
            );
            ctx.fill(Path::circle(Point::new(x, y), radius), color);
            ctx.stroke(
                Path::circle(Point::new(x, y), radius + 3.5),
                Color::rgba(1.0, 1.0, 1.0, 0.20 * alpha),
                StrokeStyle::new(1.0),
            );
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(ANIMATION_BENCHMARK_REPAINT_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "radius {:.1}, alpha {:.2}",
            self.presentation.radius, self.presentation.alpha
        )));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AnimationBenchmarkCellPresentation {
    fill: Color,
    radius: f32,
    alpha: f32,
}

impl Default for AnimationBenchmarkCellPresentation {
    fn default() -> Self {
        Self {
            fill: Color::rgba(0.20, 0.48, 0.86, 1.0),
            radius: 7.0,
            alpha: 0.7,
        }
    }
}

struct AnimationBenchmarkScalePresentation {
    cells: Vec<AnimationBenchmarkCellPresentation>,
}

impl Default for AnimationBenchmarkScalePresentation {
    fn default() -> Self {
        Self {
            cells: vec![
                AnimationBenchmarkCellPresentation::default();
                ANIMATION_BENCHMARK_SCALE_CELLS
            ],
        }
    }
}

impl AnimationBenchmarkScalePresentation {
    fn cell_index(&self, binding: &AnimationBinding) -> Option<usize> {
        let index = binding
            .target
            .as_str()
            .strip_prefix(ANIMATION_BENCHMARK_SCALE_TARGET_PREFIX)?
            .parse::<usize>()
            .ok()?;
        (index < self.cells.len()).then_some(index)
    }
}

impl TimelineBindingSink for AnimationBenchmarkScalePresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        let Some(index) = self.cell_index(binding) else {
            return false;
        };
        let cell = &mut self.cells[index];

        match (&binding.property, value) {
            (AnimationProperty::FillColor, AnimationValue::Color(value)) => {
                let changed = cell.fill != value;
                cell.fill = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_RADIUS_PATH =>
            {
                let value = value.max(2.0);
                let changed = (cell.radius - value).abs() > 0.001;
                cell.radius = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_ALPHA_PATH =>
            {
                let value = value.clamp(0.18, 1.0);
                let changed = (cell.alpha - value).abs() > 0.001;
                cell.alpha = value;
                changed
            }
            _ => false,
        }
    }
}

struct AnimationBenchmarkScaleGrid {
    player: TimelinePlayer,
    presentation: AnimationBenchmarkScalePresentation,
}

impl AnimationBenchmarkScaleGrid {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(animation_benchmark_scale_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        let mut presentation = AnimationBenchmarkScalePresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
            ctx.request_animation_frame();
        }
    }
}

impl Widget for AnimationBenchmarkScaleGrid {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(920.0, 296.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.085, 0.095, 0.11, 1.0),
        );

        let rows = ANIMATION_BENCHMARK_SCALE_CELLS / ANIMATION_BENCHMARK_SCALE_COLUMNS;
        let grid = Rect::new(
            bounds.x() + 20.0,
            bounds.y() + 18.0,
            bounds.width() - 40.0,
            bounds.height() - 36.0,
        );
        let cell_width = grid.width() / ANIMATION_BENCHMARK_SCALE_COLUMNS as f32;
        let cell_height = grid.height() / rows as f32;

        for (index, cell) in self.presentation.cells.iter().enumerate() {
            let column = index % ANIMATION_BENCHMARK_SCALE_COLUMNS;
            let row = index / ANIMATION_BENCHMARK_SCALE_COLUMNS;
            let center = Point::new(
                grid.x() + cell_width * (column as f32 + 0.5),
                grid.y() + cell_height * (row as f32 + 0.5),
            );
            let bounds = Rect::new(
                center.x - cell_width * 0.34,
                center.y - cell_height * 0.30,
                cell_width * 0.68,
                cell_height * 0.60,
            );
            ctx.fill(
                Path::rounded_rect(bounds, 5.0),
                Color::rgba(0.14, 0.16, 0.19, 0.92),
            );
            ctx.fill(
                Path::circle(center, cell.radius),
                cell.fill.with_alpha(cell.alpha),
            );
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(ANIMATION_BENCHMARK_SCALE_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "{} animated cells",
            self.presentation.cells.len()
        )));
        ctx.push(node);
    }
}

fn animation_benchmark_retained_timeline() -> Timeline {
    let target = AnimationTargetId::new(ANIMATION_BENCHMARK_RETAINED_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.4).with_clip(
        Clip::new("retained-lane", 0.0, 1.4)
            .with_track(
                Track::new(binding(AnimationProperty::LayerOpacity)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.44)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.7, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.4, AnimationValue::Scalar(0.44)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::LayerTranslation)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Vector(Vector::new(-32.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(0.7, AnimationValue::Vector(Vector::new(32.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.4, AnimationValue::Vector(Vector::new(-32.0, 0.0))),
                ]),
            ),
    )
}

fn animation_benchmark_repaint_timeline() -> Timeline {
    let target = AnimationTargetId::new(ANIMATION_BENCHMARK_REPAINT_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.2).with_clip(
        Clip::new("repaint-lane", 0.0, 1.2)
            .with_track(
                Track::new(binding(AnimationProperty::FillColor)).with_keyframes([
                    Keyframe::new(
                        0.0,
                        AnimationValue::Color(Color::rgba(0.86, 0.30, 0.22, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        0.6,
                        AnimationValue::Color(Color::rgba(0.22, 0.66, 0.82, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        1.2,
                        AnimationValue::Color(Color::rgba(0.86, 0.30, 0.22, 1.0)),
                    ),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(ANIMATION_BENCHMARK_RADIUS_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(14.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.6, AnimationValue::Scalar(26.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.2, AnimationValue::Scalar(14.0)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(ANIMATION_BENCHMARK_ALPHA_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.52)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.6, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.2, AnimationValue::Scalar(0.52)),
                ]),
            ),
    )
}

fn animation_benchmark_scale_timeline() -> Timeline {
    let mut clip = Clip::new("scale-grid", 0.0, 1.8);
    for index in 0..ANIMATION_BENCHMARK_SCALE_CELLS {
        let target =
            AnimationTargetId::new(format!("{ANIMATION_BENCHMARK_SCALE_TARGET_PREFIX}{index}"));
        let column = index % ANIMATION_BENCHMARK_SCALE_COLUMNS;
        let row = index / ANIMATION_BENCHMARK_SCALE_COLUMNS;
        let phase = ((column + row) % 6) as f32 / 6.0;
        let low_radius = 4.0 + (index % 5) as f32 * 0.35;
        let high_radius = 9.0 + (index % 7) as f32 * 0.45;
        let cool = Color::rgba(0.16 + phase * 0.16, 0.42 + phase * 0.16, 0.84, 1.0);
        let warm = Color::rgba(0.84, 0.36 + phase * 0.18, 0.20 + phase * 0.18, 1.0);

        clip.push_track(
            Track::new(AnimationBinding::new(
                target.clone(),
                AnimationProperty::Custom(AnimationPropertyPath::new(
                    ANIMATION_BENCHMARK_RADIUS_PATH,
                )),
            ))
            .with_keyframes([
                Keyframe::new(0.0, AnimationValue::Scalar(low_radius))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(0.9, AnimationValue::Scalar(high_radius))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(1.8, AnimationValue::Scalar(low_radius)),
            ]),
        );
        clip.push_track(
            Track::new(AnimationBinding::new(
                target.clone(),
                AnimationProperty::Custom(AnimationPropertyPath::new(
                    ANIMATION_BENCHMARK_ALPHA_PATH,
                )),
            ))
            .with_keyframes([
                Keyframe::new(0.0, AnimationValue::Scalar(0.38 + phase * 0.24))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(0.9, AnimationValue::Scalar(0.82 + phase * 0.14))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(1.8, AnimationValue::Scalar(0.38 + phase * 0.24)),
            ]),
        );
        clip.push_track(
            Track::new(AnimationBinding::new(target, AnimationProperty::FillColor)).with_keyframes(
                [
                    Keyframe::new(0.0, AnimationValue::Color(cool)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.9, AnimationValue::Color(warm)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.8, AnimationValue::Color(cool)),
                ],
            ),
        );
    }

    Timeline::new(1.8).with_clip(clip)
}

pub fn build_theme_demo_surface(state: Rc<RefCell<WidgetBookState>>) -> impl Widget {
    build_theme_demo_surface_with_theme(state, default_widget_book_theme_reader())
}

pub fn build_theme_demo_surface_with_theme(
    _state: Rc<RefCell<WidgetBookState>>,
    theme_reader: WidgetBookThemeReader,
) -> impl Widget {
    let scroll_state = ScrollState::new();

    VirtualScrollView::new()
        .name(THEME_DEMO_SCROLL_NAME)
        .state(scroll_state)
        .padding(ROOT_GALLERY_PADDING)
        .spacing(18.0)
        .with_child(
            Stack::vertical()
                .spacing(6.0)
                .alignment(Alignment::Stretch)
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    demo_label(
                        &theme_reader,
                        THEME_DEMO_TITLE,
                        DemoTextRole::PageTitle,
                        DemoTextColor::Text,
                    ),
                ))
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    demo_label(
                        &theme_reader,
                        THEME_DEMO_DESCRIPTION,
                        DemoTextRole::Body,
                        DemoTextColor::Muted,
                    ),
                )),
        )
        .with_child(panel_with_theme(
            Rc::clone(&theme_reader),
            "Built-in themes",
            "Compare every built-in theme with the same compact set of controls and source color swatches.",
            ThemePreviewGrid::new(),
        ))
        .with_child(panel_with_theme(
            Rc::clone(&theme_reader),
            "HDR theme lab",
            "Compare the same tokenized theme across SDR baseline, wide-gamut-only, constrained HDR, and full HDR. The first card follows the shared mode currently selected by the dev host.",
            HdrThemeLabShowcase::new(),
        ))
}

pub fn build_widget_book_gallery(state: Rc<RefCell<WidgetBookState>>) -> impl Widget {
    build_widget_book_gallery_with_theme_selection(
        state,
        default_widget_book_theme_reader(),
        WidgetBookThemeSelection::SuiLight,
    )
}

fn build_widget_book_intro(theme_reader: WidgetBookThemeReader) -> impl Widget {
    CenteredContentWidth::new(
        GALLERY_CONTENT_MAX_WIDTH,
        Padding::new(
            Insets {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: WIDGET_BOOK_SECTION_GAP,
            },
            Stack::vertical()
                .spacing(4.0)
                .alignment(Alignment::Start)
                .with_child(demo_label(
                    &theme_reader,
                    WINDOW_TITLE,
                    DemoTextRole::PageTitle,
                    DemoTextColor::Text,
                ))
                .with_child(demo_label(
                    &theme_reader,
                    "Browse, compare, and exercise SUI components in one place.",
                    DemoTextRole::Supporting,
                    DemoTextColor::Muted,
                )),
        ),
    )
}

fn build_widget_book_category_rail(
    theme_reader: WidgetBookThemeReader,
    shell_state: Rc<RefCell<WidgetBookShellState>>,
    gallery_scroll_state: ScrollState,
) -> impl Widget {
    let theme_selection_state = Rc::clone(&shell_state);
    let theme_change_state = Rc::clone(&shell_state);
    let query_state = Rc::clone(&shell_state);
    let query_scroll_state = gallery_scroll_state.clone();
    let selected_state = Rc::clone(&shell_state);
    let change_state = Rc::clone(&shell_state);

    Padding::all(
        14.0,
        Stack::vertical()
            .spacing(12.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Stack::vertical()
                    .spacing(6.0)
                    .alignment(Alignment::Stretch)
                    .with_child(demo_label(
                        &theme_reader,
                        "Theme",
                        DemoTextRole::Metadata,
                        DemoTextColor::Muted,
                    ))
                    .with_child(
                        Select::new(WIDGET_BOOK_THEME_SELECT_NAME)
                            .options(WIDGET_BOOK_THEME_OPTIONS)
                            .selected_when(move || {
                                Some(theme_selection_state.borrow().theme_selection.index())
                            })
                            .theme_when(clone_widget_book_theme_reader(&theme_reader))
                            .on_change_with_ctx(move |ctx, index, _| {
                                theme_change_state.borrow_mut().theme_selection =
                                    WidgetBookThemeSelection::from_index(index);
                                request_widget_book_shell_refresh(ctx);
                            }),
                    ),
            )
            .with_child(
                TextInput::new(WIDGET_BOOK_SEARCH_NAME)
                    .placeholder("Search widgets")
                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                    .on_change_with_ctx(move |ctx, value| {
                        query_state.borrow_mut().query = value;
                        reset_widget_book_gallery_scroll(&query_scroll_state, ctx);
                    }),
            )
            .with_child(
                ListView::new(WIDGET_BOOK_CATEGORY_NAV_NAME)
                    .items(WidgetBookCategory::ALL.map(|category| ListItem::new(category.label())))
                    .selected_when(move || Some(selected_state.borrow().category.index()))
                    .row_height(34.0)
                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                    .on_change_with_ctx(move |index, _, ctx| {
                        let category = WidgetBookCategory::from_index(index);
                        change_state.borrow_mut().category = category;
                        let _ = gallery_scroll_state
                            .scroll_to_item_with_ctx(category.gallery_item_index(), ctx);
                        request_widget_book_shell_refresh(ctx);
                    }),
            ),
    )
}

pub fn build_widget_book_gallery_with_theme(
    state: Rc<RefCell<WidgetBookState>>,
    theme_reader: WidgetBookThemeReader,
) -> impl Widget {
    build_widget_book_gallery_with_theme_selection(
        state,
        theme_reader,
        WidgetBookThemeSelection::Application,
    )
}

fn build_widget_book_gallery_with_theme_selection(
    state: Rc<RefCell<WidgetBookState>>,
    application_theme_reader: WidgetBookThemeReader,
    initial_theme_selection: WidgetBookThemeSelection,
) -> impl Widget {
    let shell_state = Rc::new(RefCell::new(WidgetBookShellState {
        theme_selection: initial_theme_selection,
        ..WidgetBookShellState::default()
    }));
    let selected_theme_state = Rc::clone(&shell_state);
    let theme_reader: WidgetBookThemeReader = Rc::new(move || {
        selected_theme_state
            .borrow()
            .theme_selection
            .resolve(application_theme_reader())
    });
    let snapshot = state.borrow().clone();
    let initial_name = snapshot.name.clone();
    let initial_password = snapshot.password.clone();
    let initial_scheduled_for = snapshot.scheduled_for.clone();
    let initial_notes = snapshot.notes.clone();
    let initial_subscribed = snapshot.subscribed;
    let initial_switch_on = snapshot.switch_on;
    let initial_standalone_radio = snapshot.standalone_radio_selected;
    let initial_slider_value = snapshot.slider_value;
    let initial_number_value = snapshot.number_value;
    let initial_radio_choice = snapshot.radio_choice.clone();
    let initial_mode = snapshot.mode.clone();
    let initial_tab_bar_choice = snapshot.tab_bar_choice.clone();
    let initial_tabs_choice = snapshot.tabs_choice.clone();

    let name_state = Rc::clone(&state);
    let password_state = Rc::clone(&state);
    let scheduled_for_state = Rc::clone(&state);
    let subscribed_state = Rc::clone(&state);
    let action_state = Rc::clone(&state);
    let icon_action_state = Rc::clone(&state);
    let switch_state = Rc::clone(&state);
    let radio_button_state = Rc::clone(&state);
    let radio_group_state = Rc::clone(&state);
    let slider_state = Rc::clone(&state);
    let number_state = Rc::clone(&state);
    let notes_state = Rc::clone(&state);
    let select_state = Rc::clone(&state);
    let tab_bar_state = Rc::clone(&state);
    let tabs_state = Rc::clone(&state);
    let menu_state = Rc::clone(&state);
    let context_menu_state = Rc::clone(&state);
    let dialog_state = Rc::clone(&state);
    let scroll_state = ScrollState::new();

    let gallery = VirtualScrollView::new()
        .name(GALLERY_SCROLL_NAME)
        .state(scroll_state.clone())
        .theme_when(clone_widget_book_theme_reader(&theme_reader))
        .padding(WIDGET_BOOK_GALLERY_PADDING)
        // Filterable sections own their trailing gap so hidden stories collapse
        // completely instead of leaving one virtual-scroll gap per match miss.
        .spacing(0.0)
        .with_child(build_widget_book_intro(Rc::clone(&theme_reader)))
        .with_child(filterable_widget_book_section(
            Rc::clone(&shell_state),
            WidgetBookCategory::Foundations,
            "widget states interaction matrix enabled disabled hover focus",
            build_widget_states_gallery_with_theme(Rc::clone(&theme_reader)),
        ))
        .with_child(filterable_widget_book_section(
            Rc::clone(&shell_state),
            WidgetBookCategory::Foundations,
            "size presets small medium large density",
            build_size_presets_gallery_with_theme(Rc::clone(&theme_reader)),
        ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Controls,
                Rc::clone(&theme_reader),
                "Common controls",
                "Core inputs and actions for forms, inspectors, and side panels.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Start)
                    .with_child(
                        responsive_control_story_pair(
                            control_story_with_theme(
                                Rc::clone(&theme_reader),
                                "Input state",
                                "Editable text, password, date/time, and boolean controls keep their natural widths instead of stretching across the page.",
                                Stack::vertical()
                                    .spacing(12.0)
                                    .alignment(Alignment::Start)
                                    .with_child(
                                        SizedBox::new().width(300.0).with_child(
                                            TextInput::new(NAME_INPUT_LABEL)
                                                .value(initial_name)
                                                .placeholder("Type your name")
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &theme_reader,
                                                ))
                                                .on_change(move |value| {
                                                    name_state.borrow_mut().name = value;
                                                }),
                                        ),
                                    )
                                    .with_child(
                                        SizedBox::new().width(300.0).with_child(
                                            PasswordInput::new(PASSWORD_INPUT_LABEL)
                                                .value(initial_password)
                                                .placeholder("Enter a password")
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &theme_reader,
                                                ))
                                                .on_change(move |value| {
                                                    password_state.borrow_mut().password = value;
                                                }),
                                        ),
                                    )
                                    .with_child(
                                        SizedBox::new().width(300.0).with_child(
                                            DateTimeInput::new(DATETIME_INPUT_LABEL)
                                                .value(initial_scheduled_for)
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &theme_reader,
                                                ))
                                                .on_change(move |value| {
                                                    scheduled_for_state.borrow_mut().scheduled_for =
                                                        value;
                                                }),
                                        ),
                                    )
                                    .with_child(
                                        Checkbox::new(SUBSCRIBE_LABEL)
                                            .checked(initial_subscribed)
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            ))
                                            .on_toggle(move |checked| {
                                                subscribed_state.borrow_mut().subscribed = checked;
                                            }),
                                    ),
                            ),
                            control_story_with_theme(
                                Rc::clone(&theme_reader),
                                "Primary action",
                                "A dense action plus supporting copy demonstrates the button without turning the row into a full-width form.",
                                Stack::vertical()
                                    .spacing(12.0)
                                    .alignment(Alignment::Start)
                                    .with_child(
                                        SizedBox::new().width(180.0).with_child(
                                            Button::primary(PRIMARY_BUTTON_LABEL).on_press(move || {
                                                action_state.borrow_mut().button_presses += 1;
                                            })
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            )),
                                        ),
                                    )
                                    .with_child(
                                        demo_label(
                                            &theme_reader,
                                            "Related controls should feel like one composed workflow, not separate experiments.",
                                            DemoTextRole::Supporting,
                                            DemoTextColor::Muted,
                                        ),
                                    ),
                            ),
                        ),
                    )
                    .with_child(MaximumWidth::new(
                        GALLERY_TEXT_MAX_WIDTH,
                        demo_label(
                            &theme_reader,
                            "The widget book tests capture these controls directly so visual regressions can be reviewed manually or compared automatically.",
                            DemoTextRole::Supporting,
                            DemoTextColor::Muted,
                        ),
                    )),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Controls,
                Rc::clone(&theme_reader),
                "Toolbar pieces",
                "Compact actions, icons, and separators for dense toolbars.",
                responsive_control_story(control_story_with_theme(
                    Rc::clone(&theme_reader),
                    "Toolbar cluster",
                    "Small controls stay aligned, scannable, and visually grouped before any app-specific toolbar exists.",
                    Stack::vertical()
                        .spacing(14.0)
                        .alignment(Alignment::Start)
                        .with_child(
                            Flex::horizontal()
                                .gap(14.0)
                                .wrap(FlexWrap::Wrap)
                                .align_items(Alignment::Center)
                                .align_content(FlexAlignContent::Start)
                                .with_item(
                                    Icon::new(IconGlyph::Search).label(ICON_LABEL).size(24.0),
                                    FlexItem::fixed(24.0),
                                )
                                .with_item(
                                    IconButton::new(IconGlyph::MoreHorizontal, ICON_BUTTON_LABEL)
                                        .theme_when(clone_widget_book_theme_reader(
                                            &theme_reader,
                                        ))
                                        .on_press(move || {
                                            icon_action_state.borrow_mut().icon_button_presses += 1;
                                        }),
                                    FlexItem::new(),
                                )
                                .with_item(
                                    demo_label(
                                        &theme_reader,
                                        "Icons and icon buttons round out dense toolbar layouts.",
                                        DemoTextRole::Supporting,
                                        DemoTextColor::Muted,
                                    ),
                                    FlexItem::new().grow(1.0).basis(220.0).min_width(200.0),
                                ),
                        )
                        .with_child(SizedBox::new().width(260.0).with_child(
                            Separator::horizontal()
                                .name(TOOLBAR_SEPARATOR_NAME)
                                .inset(12.0),
                        )),
                )),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Controls,
                Rc::clone(&theme_reader),
                "Choices and ranges",
                "Boolean, exclusive, and numeric choices for inspector workflows.",
                responsive_control_story_pair(
                    control_story_with_theme(
                        Rc::clone(&theme_reader),
                        "Boolean choices",
                        "Switches and radios are compact choices, so they should sit in a compact inspector-like block.",
                        Stack::vertical()
                            .spacing(12.0)
                            .alignment(Alignment::Start)
                            .with_child(
                                Switch::new(SWITCH_LABEL)
                                    .on(initial_switch_on)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                    .on_toggle(move |checked| {
                                        switch_state.borrow_mut().switch_on = checked;
                                    }),
                            )
                            .with_child(
                                RadioButton::new(RADIO_BUTTON_LABEL)
                                    .selected(initial_standalone_radio)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                    .on_select(move || {
                                        radio_button_state.borrow_mut().standalone_radio_selected =
                                            true;
                                    }),
                            )
                            .with_child(
                                SizedBox::new().width(280.0).with_child(
                                    RadioGroup::new(RADIO_GROUP_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .options(RADIO_OPTIONS)
                                        .selected(
                                            option_index(&RADIO_OPTIONS, &initial_radio_choice)
                                                .unwrap_or(0),
                                        )
                                        .on_change(move |_, value| {
                                            radio_group_state.borrow_mut().radio_choice = value;
                                        }),
                                ),
                            ),
                    ),
                    control_story_with_theme(
                        Rc::clone(&theme_reader),
                        "Numeric range",
                        "Slider, spinbox, and select examples now read like inspector controls instead of long page rows.",
                        Stack::vertical()
                            .spacing(12.0)
                            .alignment(Alignment::Start)
                            .with_child(
                                SizedBox::new()
                                    .width(CONTROL_STORY_CONTENT_MAX_WIDTH)
                                    .with_child(
                                    Slider::new(SLIDER_NAME)
                                        .range(0.0, 100.0)
                                        .step(1.0)
                                        .value(initial_slider_value)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .on_change(move |value| {
                                            slider_state.borrow_mut().slider_value = value;
                                        }),
                                    ),
                            )
                            .with_child(
                                SizedBox::new().width(220.0).with_child(
                                    NumberInput::new(NUMBER_INPUT_NAME)
                                        .range(1.0, 256.0)
                                        .step(1.0)
                                        .precision(0)
                                        .value(initial_number_value)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .on_change(move |value| {
                                            number_state.borrow_mut().number_value = value;
                                        }),
                                ),
                            )
                            .with_child(
                                SizedBox::new().width(260.0).with_child(
                                    Select::new(SELECT_NAME)
                                        .placeholder("Choose blend mode")
                                        .options(BLEND_MODE_OPTIONS)
                                        .selected(
                                            option_index(&BLEND_MODE_OPTIONS, &initial_mode)
                                                .unwrap_or(0),
                                        )
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .on_change(move |_, value| {
                                            select_state.borrow_mut().mode = value;
                                        }),
                                ),
                            ),
                    ),
                ),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Controls,
                Rc::clone(&theme_reader),
                "Multiline and scroll",
                "Long-form input for notes, JSON, and small scripting panes.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(420.0).with_child(
                            TextArea::new(TEXT_AREA_LABEL)
                                .min_height(150.0)
                                .value(initial_notes)
                                .placeholder("Write notes")
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .on_change(move |value| {
                                    notes_state.borrow_mut().notes = value;
                                }),
                        ),
                    )
                    .with_child(
                        demo_label(
                            &theme_reader,
                            "Use PageDown on the outer scroll view story to capture the lower panels and prove the gallery exceeds the viewport.",
                            DemoTextRole::Supporting,
                            DemoTextColor::Muted,
                        ),
                    ),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Text,
                Rc::clone(&theme_reader),
                "Typography",
                "Semantic text roles for headings, body copy, and metadata.",
                Stack::vertical()
                    .spacing(8.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        demo_label(
                            &theme_reader,
                            "Section heading",
                            DemoTextRole::SectionTitle,
                            DemoTextColor::Text,
                        ),
                    )
                    .with_child(
                        demo_label(
                            &theme_reader,
                            "Body copy can use the same widget with different size and color settings.",
                            DemoTextRole::Body,
                            DemoTextColor::Text,
                        ),
                    )
                    .with_child(
                        demo_label(
                            &theme_reader,
                            "Secondary note",
                            DemoTextRole::Supporting,
                            DemoTextColor::Muted,
                        ),
                    ),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Navigation,
                Rc::clone(&theme_reader),
                "Navigation surfaces",
                "Tabs for editor chrome, workspaces, and docked inspectors.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(520.0).with_child(
                            TabBar::new(TAB_BAR_NAME)
                                .tabs(TAB_BAR_OPTIONS)
                                .selected(option_index(&TAB_BAR_OPTIONS, &initial_tab_bar_choice).unwrap_or(0))
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .on_change(move |_, value| {
                                    tab_bar_state.borrow_mut().tab_bar_choice = value;
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(540.0).height(220.0).with_child(
                            Tabs::new(TABS_NAME)
                                .selected(option_index(&TAB_PANEL_OPTIONS, &initial_tabs_choice).unwrap_or(0))
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .tab(
                                    TAB_PANEL_OPTIONS[0],
                                    Padding::all(
                                        4.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                demo_label(
                                                    &theme_reader,
                                                    "Alignment, spacing, and surface geometry controls belong in a compact inspector tab.",
                                                    DemoTextRole::Supporting,
                                                    DemoTextColor::Text,
                                                ),
                                            )
                                            .with_child(
                                                ProgressBar::new("Layout completion")
                                                    .range(0.0, 100.0)
                                                    .value(initial_slider_value)
                                                    .show_value(true)
                                                    .theme_when(clone_widget_book_theme_reader(
                                                        &theme_reader,
                                                    )),
                                            ),
                                    ),
                                )
                                .tab(
                                    TAB_PANEL_OPTIONS[1],
                                    Padding::all(
                                        4.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                demo_label(
                                                    &theme_reader,
                                                    "Inline data summaries and editable metadata fit naturally in a reusable tabs widget.",
                                                    DemoTextRole::Supporting,
                                                    DemoTextColor::Text,
                                                ),
                                            )
                                            .with_child(
                                                demo_label(
                                                    &theme_reader,
                                                    "Selection: 4 layers, 2 masks, 1 smart object",
                                                    DemoTextRole::Supporting,
                                                    DemoTextColor::Muted,
                                                ),
                                            ),
                                    ),
                                )
                                .tab(
                                    TAB_PANEL_OPTIONS[2],
                                    Padding::all(
                                        4.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                demo_label(
                                                    &theme_reader,
                                                    "Undo groups, import checkpoints, and review markers are another common fit for tabbed panels.",
                                                    DemoTextRole::Supporting,
                                                    DemoTextColor::Text,
                                                ),
                                            )
                                            .with_child(
                                                demo_label(
                                                    &theme_reader,
                                                    "Replaying history cache",
                                                    DemoTextRole::Supporting,
                                                    DemoTextColor::Muted,
                                                ),
                                            ),
                                    ),
                                )
                                .on_change(move |_, value| {
                                    tabs_state.borrow_mut().tabs_choice = value;
                                }),
                        ),
                    ),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Navigation,
                Rc::clone(&theme_reader),
                "Menus and overlays",
                "Menus, popovers, tooltips, and dialogs for desktop workflows.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(300.0).with_child(
                            Menu::new(MENU_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .item(MenuItem::new("New tab").shortcut("Ctrl+T"))
                                .item(MenuItem::new("Duplicate panel").shortcut("Ctrl+D"))
                                .item(
                                    MenuItem::new("Delete layer")
                                        .shortcut("Del")
                                        .separator_before()
                                        .destructive(),
                                )
                                .on_activate(move |_, item| {
                                    menu_state.borrow_mut().last_menu_action = item.label().to_string();
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            ContextMenu::new(
                                CONTEXT_MENU_NAME,
                                Background::new(
                                    theme_reader().palette.control,
                                    Padding::all(
                                        14.0,
                                        demo_label(
                                            &theme_reader,
                                            "Right-click this explicit surface",
                                            DemoTextRole::Body,
                                            DemoTextColor::Text,
                                        ),
                                    ),
                                )
                                .brush_when(widget_book_theme_color(
                                    &theme_reader,
                                    |theme| theme.palette.control,
                                )),
                            )
                            .theme_when(clone_widget_book_theme_reader(&theme_reader))
                            .item(MenuItem::new("Rename"))
                            .item(MenuItem::new("Duplicate"))
                            .item(MenuItem::new("Move to").submenu([
                                MenuItem::new("Archive"),
                                MenuItem::new("Shared").submenu([
                                    MenuItem::new("Team workspace"),
                                    MenuItem::new("Project workspace"),
                                ]),
                            ]))
                            .item(MenuItem::new("Delete").separator_before().destructive())
                            .on_activate(move |_, item| {
                                context_menu_state.borrow_mut().last_context_action = item.label().to_string();
                            }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(220.0).with_child(
                            Tooltip::new(
                                TOOLTIP_TEXT,
                                Button::new(TOOLTIP_TRIGGER_LABEL)
                                    .min_width(180.0)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(360.0).with_child(
                            Popover::new(
                                POPOVER_NAME,
                                Button::new(POPOVER_TRIGGER_LABEL)
                                    .min_width(190.0)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                Stack::vertical()
                                    .spacing(8.0)
                                    .alignment(Alignment::Stretch)
                                    .with_child(
                                        demo_label(
                                            &theme_reader,
                                            "Inline inspector content can stay lightweight instead of forcing a full modal.",
                                            DemoTextRole::Supporting,
                                            DemoTextColor::Text,
                                        ),
                                    )
                                    .with_child(
                                        demo_label(
                                            &theme_reader,
                                            "Blend preview: Screen @ 72%",
                                            DemoTextRole::Supporting,
                                            DemoTextColor::Muted,
                                        ),
                                    ),
                            ),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(560.0).with_child(
                            ProjectSettingsPreview::new(dialog_state),
                        ),
                    ),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Controls,
                Rc::clone(&theme_reader),
                "Progress and busy",
                "Progress and busy feedback for background work.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            ProgressBar::new(PROGRESS_NAME)
                                .range(0.0, 100.0)
                                .value(initial_slider_value)
                                .show_value(true)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            Spinner::new(SPINNER_NAME)
                                .label(SPINNER_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ),
                    )
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Foundations,
                Rc::clone(&theme_reader),
                "Live state",
                "A composed summary driven by the controls above.",
                WidgetBookSummary::new(state, Rc::clone(&theme_reader)),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Foundations,
                Rc::clone(&theme_reader),
                "Live performance overlay",
                "Frame timing remains available while the gallery scrolls.",
                demo_label(
                    &theme_reader,
                    "Use the compact panel pinned in the top-right corner while you scroll and interact with the rest of the gallery.",
                    DemoTextRole::Supporting,
                    DemoTextColor::Muted,
                ),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Foundations,
                Rc::clone(&theme_reader),
                "Debugging and inspection",
                "Focus, semantics, widget graph, and scene diagnostics.",
                demo_label(
                    &theme_reader,
                    "The sui-debug application inspector now combines semantics and accessibility warnings, widget IDs and bounds, event routes, invalidation and rebuild reasons, scheduler state, virtual collection counters, and paint damage. Keep it in a separate window to avoid inspector self-noise."
                    , DemoTextRole::Supporting,
                    DemoTextColor::Muted,
                ),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Data,
                Rc::clone(&theme_reader),
                "Collections and hierarchy",
                "Padded lists and trees mix stock rows with arbitrary retained widgets.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(360.0).height(220.0).with_child(
                            ListView::new(LIST_VIEW_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .padding(Insets::all(10.0))
                                .items([
                                    ListItem::new("Hero texture").detail("2048 x 2048 RGBA").accent(Color::rgba(0.16, 0.54, 0.88, 1.0)),
                                    ListItem::new("Normals atlas").detail("Streaming mip chain"),
                                    ListItem::new("Glass material").with_content(
                                        Stack::horizontal()
                                            .spacing(10.0)
                                            .alignment(Alignment::Center)
                                            .with_child(
                                                SizedBox::new()
                                                    .width(190.0)
                                                    .with_child(demo_label(
                                                        &theme_reader,
                                                        "Glass material",
                                                        DemoTextRole::Body,
                                                        DemoTextColor::Text,
                                                    )),
                                            )
                                            .with_child(
                                                StatusBadge::new("3 prefabs")
                                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                                    .tone(SemanticTone::Accent),
                                            ),
                                    ),
                                    ListItem::new("UI icon sheet").detail("Tagged for export").accent(Color::rgba(0.78, 0.50, 0.17, 1.0)),
                                    ListItem::new("Archive cache").detail("Read only").disabled(),
                                ])
                                .selected(1),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(420.0).height(240.0).with_child(
                            TreeView::new(TREE_VIEW_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .padding(Insets::all(10.0))
                                .items([
                                    TreeItem::new("Scene")
                                        .expanded(true)
                                        .with_child(
                                            TreeItem::new("Environment")
                                                .expanded(true)
                                                .with_child(TreeItem::new("Sky dome").detail("Visible"))
                                                .with_child(TreeItem::new("Fog volume").detail("Animated")),
                                        )
                                        .with_child(
                                            TreeItem::new("Characters")
                                                .expanded(true)
                                                .with_child(
                                                    TreeItem::new("Pilot").with_content(
                                                        Stack::horizontal()
                                                            .spacing(10.0)
                                                            .alignment(Alignment::Center)
                                                            .with_child(demo_label(
                                                                &theme_reader,
                                                                "Pilot",
                                                                DemoTextRole::Body,
                                                                DemoTextColor::Text,
                                                            ))
                                                            .with_child(
                                                                StatusBadge::new("Selected")
                                                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                                                    .tone(SemanticTone::Accent),
                                                            ),
                                                    ),
                                                )
                                                .with_child(TreeItem::new("Companion drone")),
                                        )
                                        .with_child(TreeItem::new("FX").detail("Collapsed group")),
                                ]),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(720.0).height(250.0).with_child(
                            Table::new(TABLE_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .columns([
                                    TableColumn::new("Material"),
                                    TableColumn::new("Domain").width(120.0),
                                    TableColumn::new("Shader").width(180.0),
                                    TableColumn::new("Passes").width(90.0).alignment(TableColumnAlignment::End),
                                    TableColumn::new("Last edit").width(130.0),
                                ])
                                .rows([
                                    TableRow::new(["ClearCoat_Glass", "Surface", "pbr.clearcoat", "3", "2 min ago"]),
                                    TableRow::new(["Terrain_Master", "Surface", "terrain.layered", "5", "11 min ago"]),
                                    TableRow::new(["UI_Highlight", "Overlay", "ui.gradient", "1", "24 min ago"]),
                                    TableRow::new(["CloudShadow", "Decal", "fx.projected", "2", "1 hour ago"]),
                                    TableRow::new(["Water_Foam", "Surface", "water.foam", "4", "yesterday"]),
                                ])
                                .selected(2),
                        ),
                    ),
            ))
            .with_child(filtered_panel_with_theme(
                Rc::clone(&shell_state),
                WidgetBookCategory::Layout,
                Rc::clone(&theme_reader),
                "Layout and pathing",
                "Split panes and breadcrumbs for editor shells.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(620.0).with_child(
                            Breadcrumb::new(BREADCRUMB_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .items([
                                    BreadcrumbItem::new("Workspace"),
                                    BreadcrumbItem::new("Projects"),
                                    BreadcrumbItem::new("Starfall"),
                                    BreadcrumbItem::new("Materials"),
                                    BreadcrumbItem::new("Glass"),
                                ])
                                .current(4),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(720.0).height(240.0).with_child(
                            SplitView::horizontal(
                                Background::new(
                                    theme_reader().palette.control,
                                    Padding::all(
                                        16.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                demo_label(
                                                    &theme_reader,
                                                    "Viewport",
                                                    DemoTextRole::Emphasis,
                                                    DemoTextColor::Text,
                                                ),
                                            )
                                            .with_child(
                                                demo_label(
                                                    &theme_reader,
                                                    "Resizable panes let editor shells settle into familiar two-up and inspector layouts.",
                                                    DemoTextRole::Supporting,
                                                    DemoTextColor::Muted,
                                                ),
                                            ),
                                    ),
                                )
                                .brush_when(widget_book_theme_color(
                                    &theme_reader,
                                    |theme| theme.palette.control,
                                )),
                                Background::new(
                                    theme_reader().palette.surface_raised,
                                    Padding::all(
                                        16.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                demo_label(
                                                    &theme_reader,
                                                    "Inspector",
                                                    DemoTextRole::Emphasis,
                                                    DemoTextColor::Text,
                                                ),
                                            )
                                            .with_child(
                                                demo_label(
                                                    &theme_reader,
                                                    "Drag the divider to rebalance the viewport and detail pane without custom shell code.",
                                                    DemoTextRole::Supporting,
                                                    DemoTextColor::Muted,
                                                ),
                                            ),
                                    ),
                                )
                                .brush_when(widget_book_theme_color(
                                    &theme_reader,
                                    |theme| theme.palette.surface_raised,
                                )),
                            )
                            .name(SPLIT_VIEW_NAME)
                            .ratio(0.62),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(720.0).height(132.0).with_child(
                            AdaptiveView::new(
                                Background::new(
                                    theme_reader().palette.control,
                                    Padding::all(
                                        16.0,
                                        demo_label(
                                            &theme_reader,
                                            "Compact: one-pane navigation with drawers and bottom sheets",
                                            DemoTextRole::Supporting,
                                            DemoTextColor::Text,
                                        ),
                                    ),
                                )
                                .brush_when(widget_book_theme_color(
                                    &theme_reader,
                                    |theme| theme.palette.control,
                                )),
                                Background::new(
                                    theme_reader().palette.surface,
                                    Padding::all(
                                        16.0,
                                        demo_label(
                                            &theme_reader,
                                            "Medium: retained rail plus content",
                                            DemoTextRole::Supporting,
                                            DemoTextColor::Text,
                                        ),
                                    ),
                                )
                                .brush_when(widget_book_theme_color(
                                    &theme_reader,
                                    |theme| theme.palette.surface,
                                )),
                                Background::new(
                                    theme_reader().palette.surface_raised,
                                    Padding::all(
                                        16.0,
                                        demo_label(
                                            &theme_reader,
                                            "Expanded: stable sidebar, content, and inspector regions",
                                            DemoTextRole::Supporting,
                                            DemoTextColor::Text,
                                        ),
                                    ),
                                )
                                .brush_when(widget_book_theme_color(
                                    &theme_reader,
                                    |theme| theme.palette.surface_raised,
                                )),
                            )
                            .breakpoints(AdaptiveBreakpoints::new(480.0, 680.0)),
                        ),
                    ),
            ))
            .with_child(filterable_widget_book_section(
                Rc::clone(&shell_state),
                WidgetBookCategory::Controls,
                "composite widgets surfaces cards forms properties docks commands tools",
                build_composite_widgets_gallery_with_theme(Rc::clone(&theme_reader)),
            ))
            .with_child(filterable_widget_book_section(
                Rc::clone(&shell_state),
                WidgetBookCategory::Layout,
                "layout widgets stack flex grid scroll dock switch view slots",
                build_layout_widgets_gallery_with_theme(Rc::clone(&theme_reader)),
            ))
            .with_child(filterable_widget_book_section(
                Rc::clone(&shell_state),
                WidgetBookCategory::Text,
                "text widgets label rich text link input aliases",
                build_text_widgets_gallery_with_theme(Rc::clone(&theme_reader)),
            ))
            .with_child(filterable_widget_book_section(
                Rc::clone(&shell_state),
                WidgetBookCategory::Data,
                "data interaction grid table layer list reorder drag drop",
                build_data_and_interaction_gallery_with_theme(Rc::clone(&theme_reader)),
            ))
            .with_child(filterable_widget_book_section(
                Rc::clone(&shell_state),
                WidgetBookCategory::Canvas,
                "canvas media vector pixel ruler brush signal meter",
                build_canvas_and_media_gallery_with_theme(Rc::clone(&theme_reader)),
            ))
            .with_child(filterable_widget_book_section(
                Rc::clone(&shell_state),
                WidgetBookCategory::Canvas,
                "color imagery swatch picker palette image preview",
                build_color_and_imagery_story_with_theme(Rc::clone(&theme_reader)),
            ));

    let content = gallery;
    let rail = build_widget_book_category_rail(
        Rc::clone(&theme_reader),
        Rc::clone(&shell_state),
        scroll_state,
    );

    WidgetBookShell::new(theme_reader, rail, content)
}

fn build_size_presets_gallery_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    NamedSection::new(
        SIZE_PRESETS_GALLERY_NAME,
        panel_with_theme(
            Rc::clone(&theme_reader),
            SIZE_PRESETS_GALLERY_NAME,
            "Contextual size presets resize the same theme-aware widgets for dense tools, standard application interfaces, and focused actions.",
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Flex::horizontal()
                        .gap(12.0)
                        .wrap(FlexWrap::Wrap)
                        .align_items(Alignment::Stretch)
                        .align_content(FlexAlignContent::Start)
                        .with_item(
                            size_preset_column_with_theme(
                                Rc::clone(&theme_reader),
                                ControlSize::Small,
                            ),
                            size_preset_flex_item(),
                        )
                        .with_item(
                            size_preset_column_with_theme(
                                Rc::clone(&theme_reader),
                                ControlSize::Medium,
                            ),
                            size_preset_flex_item(),
                        )
                        .with_item(
                            size_preset_column_with_theme(
                                Rc::clone(&theme_reader),
                                ControlSize::Large,
                            ),
                            size_preset_flex_item(),
                        ),
                ),
        ),
    )
}

fn size_preset_flex_item() -> FlexItem {
    FlexItem::new()
        .grow(1.0)
        .basis_gap_aware_fraction(1.0 / 3.0)
        .min_width(SIZE_PRESET_CARD_MIN_WIDTH)
        .max_width(SIZE_PRESET_CARD_MAX_WIDTH)
}

fn size_preset_column_with_theme(
    theme_reader: WidgetBookThemeReader,
    size: ControlSize,
) -> impl Widget {
    let title = size_preset_title(size);
    let action_label = size_preset_action_label(size);
    let input_label = size_preset_input_label(size);
    let switch_label = format!("{title} preset switch");
    let checkbox_label = format!("{title} preset checkbox");
    let select_name = format!("{title} preset select");
    let slider_name = format!("{title} preset slider");
    let tab_name = format!("{title} preset tabs");
    let preset_name = format!("{title} preset strip");
    let toolbar_name = format!("{title} preset toolbar");

    StoryCard::new(
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Start)
            .with_child(demo_label(
                &theme_reader,
                title,
                DemoTextRole::CardTitle,
                DemoTextColor::Text,
            ))
            .with_child(MaximumWidth::new(
                250.0,
                demo_label(
                    &theme_reader,
                    size_preset_caption(size),
                    DemoTextRole::Metadata,
                    DemoTextColor::Muted,
                ),
            ))
            .with_child(
                Button::primary(action_label)
                    .icon(IconGlyph::Check)
                    .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
            )
            .with_child(
                SizedBox::new().width(230.0).with_child(
                    TextInput::new(input_label)
                        .value("Layer name")
                        .leading_icon(IconGlyph::Search)
                        .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
                ),
            )
            .with_child(
                Checkbox::new(checkbox_label)
                    .checked(true)
                    .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
            )
            .with_child(
                Switch::new(switch_label)
                    .on(true)
                    .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
            )
            .with_child(
                SizedBox::new().width(230.0).with_child(
                    Slider::new(slider_name)
                        .range(0.0, 100.0)
                        .value(64.0)
                        .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
                ),
            )
            .with_child(
                SizedBox::new().width(230.0).with_child(
                    Select::new(select_name)
                        .options(BLEND_MODE_OPTIONS)
                        .selected(1)
                        .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
                ),
            )
            .with_child(
                SizedBox::new().width(250.0).with_child(
                    TabBar::new(tab_name)
                        .tabs(["Canvas", "Inspect"])
                        .selected(1)
                        .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
                ),
            )
            .with_child(
                PresetStrip::new(preset_name)
                    .presets(["8 px", "18 px", "36 px"])
                    .selected(1)
                    .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
            )
            .with_child(
                Toolbar::horizontal()
                    .name(toolbar_name)
                    .theme_when(widget_book_size_theme_reader(&theme_reader, size))
                    .with_child(
                        IconButton::new(IconGlyph::Undo, format!("{title} preset undo"))
                            .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
                    )
                    .with_child(
                        IconButton::new(IconGlyph::Redo, format!("{title} preset redo"))
                            .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
                    )
                    .with_child(
                        Button::new("Apply")
                            .semantic_name(format!("{title} preset apply"))
                            .theme_when(widget_book_size_theme_reader(&theme_reader, size)),
                    ),
            ),
    )
    .theme_when(widget_book_size_theme_reader(&theme_reader, size))
}

fn size_preset_title(size: ControlSize) -> &'static str {
    match size {
        ControlSize::Small => "Small",
        ControlSize::Medium => "Medium",
        ControlSize::Large => "Large",
    }
}

fn size_preset_caption(size: ControlSize) -> &'static str {
    match size {
        ControlSize::Small => "Dense inspectors, configuration panels, and toolbars.",
        ControlSize::Medium => "Standard desktop application interfaces.",
        ControlSize::Large => "Hero actions and focused overlays.",
    }
}

fn size_preset_action_label(size: ControlSize) -> &'static str {
    match size {
        ControlSize::Small => SIZE_PRESET_SMALL_ACTION_LABEL,
        ControlSize::Medium => SIZE_PRESET_MEDIUM_ACTION_LABEL,
        ControlSize::Large => SIZE_PRESET_LARGE_ACTION_LABEL,
    }
}

fn size_preset_input_label(size: ControlSize) -> &'static str {
    match size {
        ControlSize::Small => SIZE_PRESET_SMALL_INPUT_LABEL,
        ControlSize::Medium => SIZE_PRESET_MEDIUM_INPUT_LABEL,
        ControlSize::Large => SIZE_PRESET_LARGE_INPUT_LABEL,
    }
}

fn build_widget_states_gallery_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    NamedSection::new(
        WIDGET_STATES_GALLERY_NAME,
        panel_with_theme(
            Rc::clone(&theme_reader),
            WIDGET_STATES_GALLERY_NAME,
            "Compact state matrix for the core controls. Each sample uses the same theme tokens so density, alignment, focus chrome, and overlay spacing can be reviewed together.",
            Stack::vertical()
                .spacing(16.0)
                .alignment(Alignment::Stretch)
                .with_child(widget_state_row_with_theme(
                    Rc::clone(&theme_reader),
                    "Actions",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Start)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Default",
                            Button::new(WIDGET_STATES_BUTTON_LABEL)
                                .icon(IconGlyph::Check)
                                .min_width(170.0)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Selected",
                            IconButton::new(IconGlyph::Hand, WIDGET_STATES_ICON_BUTTON_LABEL)
                                .selected(true)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Disabled",
                            Button::new("Disabled action")
                                .icon(IconGlyph::Lock)
                                .min_width(170.0)
                                .enabled(false)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        )),
                    "Text entry",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Start)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Placeholder",
                            SizedBox::new().width(240.0).with_child(
                                TextInput::new(WIDGET_STATES_TEXT_INPUT_LABEL)
                                    .placeholder("Search layers")
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Value",
                            SizedBox::new().width(240.0).with_child(
                                TextInput::new("States text input value")
                                    .value("Layer 08 / mask")
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Multiline",
                            SizedBox::new().width(240.0).with_child(
                                TextArea::new(WIDGET_STATES_TEXT_AREA_LABEL)
                                    .value("Frame notes\nOpacity ramp is locked")
                                    .min_height(72.0)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        )),
                ))
                .with_child(widget_state_row_with_theme(
                    Rc::clone(&theme_reader),
                    "Choices",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Start)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Unchecked",
                            Checkbox::new(WIDGET_STATES_CHECKBOX_LABEL)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Checked",
                            Checkbox::new("States checkbox checked")
                                .checked(true)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Switch on",
                            Switch::new(WIDGET_STATES_SWITCH_LABEL)
                                .on(true)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        )),
                    "Ranges and selects",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Start)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Low value",
                            SizedBox::new().width(240.0).with_child(
                                Slider::new(WIDGET_STATES_SLIDER_NAME)
                                    .range(0.0, 100.0)
                                    .value(28.0)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Selected",
                            SizedBox::new().width(240.0).with_child(
                                Select::new(WIDGET_STATES_SELECT_NAME)
                                    .placeholder("Blend mode")
                                    .options(BLEND_MODE_OPTIONS)
                                    .selected(2)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Expandable",
                            SizedBox::new().width(240.0).with_child(
                                Select::new("States select expandable")
                                    .options(BLEND_MODE_OPTIONS)
                                    .selected(1)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        )),
                ))
                .with_child(widget_state_row_with_theme(
                    Rc::clone(&theme_reader),
                    "Navigation",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Stretch)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Tabs",
                            SizedBox::new().width(300.0).with_child(
                                Tabs::new(WIDGET_STATES_TABS_NAME)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                    .selected(1)
                                    .tab(
                                        "Canvas",
                                        demo_label(
                                            &theme_reader,
                                            "Viewport controls",
                                            DemoTextRole::Supporting,
                                            DemoTextColor::Muted,
                                        ),
                                    )
                                    .tab(
                                        "Inspector",
                                        demo_label(
                                            &theme_reader,
                                            "Selected layer properties",
                                            DemoTextRole::Supporting,
                                            DemoTextColor::Muted,
                                        ),
                                    )
                                    .tab(
                                        "Export",
                                        demo_label(
                                            &theme_reader,
                                            "Preset summary",
                                            DemoTextRole::Supporting,
                                            DemoTextColor::Muted,
                                        ),
                                    ),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Menu",
                            SizedBox::new().width(260.0).with_child(
                                Menu::new(WIDGET_STATES_MENU_NAME)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                    .highlighted(1)
                                    .items([
                                        MenuItem::new("Rename").shortcut("Enter"),
                                        MenuItem::new("Duplicate").shortcut("Ctrl+D"),
                                        MenuItem::new("Bake preview").disabled(),
                                        MenuItem::new("Delete").separator_before().destructive(),
                                    ]),
                            ),
                        )),
                    "Overlays",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Start)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Closed popover",
                            SizedBox::new().width(260.0).with_child(
                                Popover::new(
                                    WIDGET_STATES_POPOVER_NAME,
                                    Button::new("Open details")
                                        .min_width(180.0)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                    Label::new("Hidden until opened"),
                                )
                                .theme(theme_reader()),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Managed popover",
                            SizedBox::new().width(260.0).with_child(OverlayHost::new(
                                Popover::new(
                                    "States popover details",
                                    Button::new("Open managed details")
                                        .min_width(180.0)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                    Stack::vertical()
                                        .spacing(6.0)
                                        .alignment(Alignment::Start)
                                        .with_child(demo_label(
                                            &theme_reader,
                                            "Collision-aware placement",
                                            DemoTextRole::Supporting,
                                            DemoTextColor::Text,
                                        ))
                                        .with_child(demo_label(
                                            &theme_reader,
                                            "Escape or outside click dismisses",
                                            DemoTextRole::Metadata,
                                            DemoTextColor::Muted,
                                        )),
                                )
                                .theme(theme_reader()),
                            )),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Status notification",
                            SizedBox::new().width(360.0).height(92.0).with_child({
                                let center = NotificationCenter::new();
                                center.push(
                                    TransientNotification::new(
                                        "Workspace indexed",
                                        "42 files are ready for search",
                                    )
                                    .persistent(),
                                );
                                NotificationHost::new(center)
                                    .theme(theme_reader())
                                    .width(320.0)
                            }),
                        )),
                )),
        ),
    )
}

fn widget_state_row_with_theme<L, R>(
    theme_reader: WidgetBookThemeReader,
    left_title: &'static str,
    left_body: L,
    right_title: &'static str,
    right_body: R,
) -> impl Widget
where
    L: Widget + 'static,
    R: Widget + 'static,
{
    StoryCard::new(
        Flex::horizontal()
            .gap(14.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Stretch)
            .with_item(
                widget_state_column_with_theme(Rc::clone(&theme_reader), left_title, left_body),
                FlexItem::new()
                    .basis_gap_aware_fraction(0.5)
                    .min_width(280.0),
            )
            .with_item(
                widget_state_column_with_theme(Rc::clone(&theme_reader), right_title, right_body),
                FlexItem::new()
                    .basis_gap_aware_fraction(0.5)
                    .min_width(280.0),
            ),
    )
    .theme_when(clone_widget_book_theme_reader(&theme_reader))
}

fn widget_state_column_with_theme<W>(
    theme_reader: WidgetBookThemeReader,
    title: &'static str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Stretch)
        .with_child(demo_label(
            &theme_reader,
            title,
            DemoTextRole::Supporting,
            DemoTextColor::Text,
        ))
        .with_child(body)
}

fn state_sample_with_theme<W>(
    theme_reader: WidgetBookThemeReader,
    state: &'static str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(5.0)
        .alignment(Alignment::Start)
        .with_child(demo_label(
            &theme_reader,
            state,
            DemoTextRole::Metadata,
            DemoTextColor::Muted,
        ))
        .with_child(body)
}

fn widget_book_body_text(
    theme_reader: WidgetBookThemeReader,
    text: impl Into<String>,
) -> impl Widget {
    demo_label(&theme_reader, text, DemoTextRole::Body, DemoTextColor::Text)
}

fn widget_book_muted_text(
    theme_reader: WidgetBookThemeReader,
    text: impl Into<String>,
) -> impl Widget {
    demo_label(
        &theme_reader,
        text,
        DemoTextRole::Supporting,
        DemoTextColor::Muted,
    )
}

fn build_composite_widgets_gallery_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    NamedSection::new(
        COMPOSITE_WIDGETS_GALLERY_NAME,
        panel_with_theme(
            Rc::clone(&theme_reader),
            COMPOSITE_WIDGETS_GALLERY_NAME,
            "Composite widgets cover reusable app chrome, status, forms, empty states, and compact command surfaces.",
            Stack::vertical()
                .spacing(16.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Stack::horizontal()
                        .spacing(14.0)
                        .alignment(Alignment::Start)
                        .with_child(control_story_with_theme(
                            Rc::clone(&theme_reader),
                            "Surfaces and actions",
                            "Surface, action card, section label, status badge, coverage dots, and placement badge.",
                            Stack::vertical()
                                .spacing(12.0)
                                .alignment(Alignment::Start)
                                .with_child(SizedBox::new().width(340.0).with_child(
                                    Surface::panel(
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                SectionLabel::new("Inspector")
                                                    .semantic_name(SECTION_LABEL_NAME)
                                                    .theme_when(clone_widget_book_theme_reader(
                                                        &theme_reader,
                                                    )),
                                            )
                                            .with_child(widget_book_body_text(
                                                Rc::clone(&theme_reader),
                                                "Panel surface with compact content.",
                                            ))
                                            .with_child(
                                                Stack::horizontal()
                                                    .spacing(8.0)
                                                    .alignment(Alignment::Center)
                                                    .with_child(
                                                        StatusBadge::new(STATUS_BADGE_NAME)
                                                            .icon(IconGlyph::Check)
                                                            .tone(SemanticTone::Success)
                                                            .theme_when(
                                                                clone_widget_book_theme_reader(
                                                                    &theme_reader,
                                                                ),
                                                            ),
                                                    )
                                                    .with_child(
                                                        CoverageDots::new(
                                                            COVERAGE_DOTS_NAME,
                                                            3,
                                                            4,
                                                        )
                                                        .tone(SemanticTone::Accent)
                                                        .theme_when(
                                                            clone_widget_book_theme_reader(
                                                                &theme_reader,
                                                            ),
                                                        ),
                                                    )
                                                    .with_child(
                                                        PlacementBadge::new(PLACEMENT_BADGE_NAME)
                                                            .icon(IconGlyph::Storage)
                                                            .tone(SemanticTone::Info)
                                                            .coverage(2, 3)
                                                            .theme_when(
                                                                clone_widget_book_theme_reader(
                                                                    &theme_reader,
                                                                ),
                                                            ),
                                                    ),
                                            ),
                                    )
                                    .name(SURFACE_SAMPLE_NAME)
                                    .padding(Insets::all(12.0))
                                    .elevation(SurfaceElevation::Small)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                ))
                                .with_child(
                                    ActionCard::new(
                                        ACTION_CARD_NAME,
                                        "Jump into a focused drawing workspace.",
                                    )
                                    .icon(IconGlyph::Sparkles)
                                    .min_width(300.0)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                ),
                        ))
                        .with_child(control_story_with_theme(
                            Rc::clone(&theme_reader),
                            "Commands and tools",
                            "Toolbar, command group, tool palette, preset strip, segmented control, and busy indicator aliases.",
                            Stack::vertical()
                                .spacing(12.0)
                                .alignment(Alignment::Start)
                                .with_child(SizedBox::new().width(380.0).height(48.0).with_child(
                                    Toolbar::horizontal()
                                        .name(TOOLBAR_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .with_child(IconButton::new(
                                            IconGlyph::Undo,
                                            "Undo command",
                                        )
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader)))
                                        .with_child(IconButton::new(
                                            IconGlyph::Redo,
                                            "Redo command",
                                        )
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader)))
                                        .with_child(
                                            Divider::vertical()
                                                .name(DIVIDER_ALIAS_NAME)
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &theme_reader,
                                                )),
                                        )
                                        .with_child(
                                            Button::primary("Save").theme_when(
                                                clone_widget_book_theme_reader(&theme_reader),
                                            ),
                                        ),
                                ))
                                .with_child(
                                    CommandGroup::horizontal(COMMAND_GROUP_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .with_child(IconButton::new(
                                            IconGlyph::Add,
                                            "Zoom in",
                                        )
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader)))
                                        .with_child(IconButton::new(
                                            IconGlyph::Remove,
                                            "Zoom out",
                                        )
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader)))
                                        .with_child(IconButton::new(
                                            IconGlyph::FitView,
                                            "Fit canvas",
                                        )
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))),
                                )
                                .with_child(
                                    ToolPalette::horizontal(TOOL_PALETTE_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .items([
                                            ToolPaletteItem::new(IconGlyph::Check, "Select"),
                                            ToolPaletteItem::new(IconGlyph::Brush, "Brush"),
                                            ToolPaletteItem::new(IconGlyph::Eraser, "Erase"),
                                            ToolPaletteItem::new(IconGlyph::Hand, "Pan"),
                                        ])
                                        .selected(1),
                                )
                                .with_child(
                                    PresetStrip::new(PRESET_STRIP_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .presets(["Draft", "Review", "Final"])
                                        .selected(1),
                                )
                                .with_child(
                                    SegmentedControl::new(SEGMENTED_CONTROL_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .segments(["Preview", "Inspect", "Compare"])
                                        .selected(0),
                                )
                                .with_child(
                                    BusyIndicator::new(BUSY_INDICATOR_NAME)
                                        .label("Indexing assets")
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                ),
                        )),
                )
                .with_child(
                    Stack::horizontal()
                        .spacing(14.0)
                        .alignment(Alignment::Start)
                        .with_child(control_story_with_theme(
                            Rc::clone(&theme_reader),
                            "Forms and details",
                            "Form section, field group, form rows, property rows, and detail rows.",
                            SizedBox::new().width(380.0).with_child(
                                FormSection::new(
                                    FORM_SECTION_NAME,
                                    FieldGroup::new()
                                        .fill_width()
                                        .spacing(10.0)
                                        .with_child(
                                            FormRow::new(
                                                "Target",
                                                TextInput::new("Publish target")
                                                    .value("staging")
                                                    .theme_when(
                                                        clone_widget_book_theme_reader(
                                                            &theme_reader,
                                                        ),
                                                    ),
                                            )
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            )),
                                        )
                                        .with_child(
                                            PropertyRow::new(
                                                PROPERTY_ROW_NAME,
                                                Slider::new("Opacity property value")
                                                    .range(0.0, 100.0)
                                                    .value(72.0)
                                                    .theme_when(
                                                        clone_widget_book_theme_reader(
                                                            &theme_reader,
                                                        ),
                                                    ),
                                            )
                                            .inline()
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            )),
                                        )
                                        .with_child(
                                            DetailRow::new(DETAIL_ROW_NAME, "2 min ago")
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &theme_reader,
                                                )),
                                        ),
                                )
                                .description("Cluster-wide settings with reusable rows.")
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ))
                        .with_child(control_story_with_theme(
                            Rc::clone(&theme_reader),
                            "Panels and empty states",
                            "Panel section, dock panel, status bar, and empty state cover common app shells.",
                            Stack::vertical()
                                .spacing(12.0)
                                .alignment(Alignment::Stretch)
                                .with_child(SizedBox::new().width(380.0).with_child(
                                    DockPanel::new(
                                        "Inspector",
                                        PanelSection::new(
                                            PANEL_SECTION_NAME,
                                            Stack::vertical()
                                                .spacing(8.0)
                                                .alignment(Alignment::Stretch)
                                                .with_child(widget_book_muted_text(
                                                    Rc::clone(&theme_reader),
                                                    "Layer blend: Screen",
                                                ))
                                                .with_child(widget_book_muted_text(
                                                    Rc::clone(&theme_reader),
                                                    "Mask feather: 8 px",
                                                )),
                                        )
                                        .collapsible(true)
                                        .theme_when(clone_widget_book_theme_reader(
                                            &theme_reader,
                                        )),
                                    )
                                    .name(DOCK_PANEL_NAME)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                ))
                                .with_child(SizedBox::new().width(380.0).height(186.0).with_child(
                                    EmptyState::new(
                                        "No search results",
                                        "Try a broader query or clear active filters.",
                                    )
                                    .name(EMPTY_STATE_NAME)
                                    .icon(IconGlyph::Search)
                                    .action(Button::primary("Clear filters").theme_when(
                                        clone_widget_book_theme_reader(&theme_reader),
                                    ))
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                ))
                                .with_child(SizedBox::new().width(380.0).with_child(
                                    StatusBar::new()
                                        .name(STATUS_BAR_NAME)
                                        .description("Ready, two warnings, zoom 72 percent")
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .segment(
                                            StatusBarSegment::new("Ready").min_width(76.0),
                                        )
                                        .segment(
                                            StatusBarSegment::new("2 warnings")
                                                .tone(SemanticTone::Warning)
                                                .min_width(104.0),
                                        )
                                        .segment(
                                            StatusBarSegment::new("Zoom 72%")
                                                .min_width(96.0)
                                                .expand(true),
                                        ),
                                )),
                        )),
                ),
        ),
    )
}

fn build_layout_widgets_gallery_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    let scroll_state = ScrollState::new();
    let virtual_scroll_state = ScrollState::new();

    NamedSection::new(
        LAYOUT_WIDGETS_GALLERY_NAME,
        panel_with_theme(
            Rc::clone(&theme_reader),
            LAYOUT_WIDGETS_GALLERY_NAME,
            "Layout widgets cover explicit size, alignment, docking, fixed panes, measured bottom docks, switch views, and scroll containers.",
            SemanticRegion::new(
                LAYOUT_REGION_NAME,
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Start)
                    .with_child(
                        responsive_control_story_pair(
                            control_story_with_theme(
                                Rc::clone(&theme_reader),
                                "Box, stack, flex",
                                "SizedBox, Padding, Background, Align, Stack, and Flex.",
                                SizedBox::new().width(380.0).height(170.0).with_child(
                                    Background::new(
                                        theme_reader().palette.control,
                                        Padding::all(
                                            12.0,
                                            Align::center(
                                                Flex::horizontal()
                                                    .gap(10.0)
                                                    .wrap(FlexWrap::Wrap)
                                                    .with_child(
                                                        Surface::field(
                                                            demo_label(
                                                                &theme_reader,
                                                                "Sized",
                                                                DemoTextRole::Supporting,
                                                                DemoTextColor::Text,
                                                            ),
                                                        )
                                                        .padding(Insets::all(8.0))
                                                        .theme_when(
                                                            clone_widget_book_theme_reader(
                                                                &theme_reader,
                                                            ),
                                                        ),
                                                    )
                                                    .with_child(
                                                        Surface::field(
                                                            demo_label(
                                                                &theme_reader,
                                                                "Aligned",
                                                                DemoTextRole::Supporting,
                                                                DemoTextColor::Text,
                                                            ),
                                                        )
                                                        .padding(Insets::all(8.0))
                                                        .theme_when(
                                                            clone_widget_book_theme_reader(
                                                                &theme_reader,
                                                            ),
                                                        ),
                                                    )
                                                    .with_child(
                                                        Surface::field(
                                                            demo_label(
                                                                &theme_reader,
                                                                "Wrapped",
                                                                DemoTextRole::Supporting,
                                                                DemoTextColor::Text,
                                                            ),
                                                        )
                                                        .padding(Insets::all(8.0))
                                                        .theme_when(
                                                            clone_widget_book_theme_reader(
                                                                &theme_reader,
                                                            ),
                                                        ),
                                                    ),
                                            ),
                                        ),
                                    )
                                    .brush_when(widget_book_theme_color(&theme_reader, |theme| {
                                        theme.palette.control
                                    })),
                                ),
                            ),
                            control_story_with_theme(
                                Rc::clone(&theme_reader),
                                "Dock and switch",
                                "Dock, MeasuredBottomDock, SwitchView, and TrailingSlotRow.",
                                Stack::vertical()
                                    .spacing(12.0)
                                    .alignment(Alignment::Stretch)
                                    .with_child(SemanticRegion::new(
                                        DOCK_LAYOUT_NAME,
                                        SizedBox::new().width(380.0).height(150.0).with_child(
                                            Dock::new(
                                                Surface::panel(widget_book_muted_text(
                                                    Rc::clone(&theme_reader),
                                                    "Dock body fills remaining space.",
                                                ))
                                                .padding(Insets::all(12.0))
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &theme_reader,
                                                )),
                                            )
                                            .top(
                                                34.0,
                                                Surface::titlebar(Label::new("Top slot"))
                                                    .padding(Insets::all(8.0))
                                                    .theme_when(clone_widget_book_theme_reader(
                                                        &theme_reader,
                                                    )),
                                            )
                                            .bottom(
                                                34.0,
                                                Surface::titlebar(Label::new("Bottom slot"))
                                                    .padding(Insets::all(8.0))
                                                    .theme_when(clone_widget_book_theme_reader(
                                                        &theme_reader,
                                                    )),
                                            ),
                                        ),
                                    ))
                                    .with_child(SemanticRegion::new(
                                        MEASURED_BOTTOM_DOCK_NAME,
                                        SizedBox::new().width(380.0).height(130.0).with_child(
                                            MeasuredBottomDock::new(
                                                Surface::panel(widget_book_muted_text(
                                                    Rc::clone(&theme_reader),
                                                    "Measured body",
                                                ))
                                                .padding(Insets::all(12.0))
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &theme_reader,
                                                )),
                                                StatusBar::new()
                                                    .theme_when(clone_widget_book_theme_reader(
                                                        &theme_reader,
                                                    ))
                                                    .text_segment("Measured bottom"),
                                            ),
                                        ),
                                    ))
                                    .with_child(SemanticRegion::new(
                                        SWITCH_VIEW_NAME,
                                        SwitchView::new()
                                            .selected(1)
                                            .with_child(Label::new("First view"))
                                            .with_child(widget_book_body_text(
                                                Rc::clone(&theme_reader),
                                                "Second view is selected.",
                                            )),
                                    ))
                                    .with_child(SemanticRegion::new(
                                        TRAILING_SLOT_ROW_NAME,
                                        SizedBox::new().width(380.0).height(34.0).with_child(
                                            TrailingSlotRow::new(
                                                widget_book_body_text(
                                                    Rc::clone(&theme_reader),
                                                    "Trailing slot row",
                                                ),
                                                StatusBadge::new("Active")
                                                    .tone(SemanticTone::Success)
                                                    .theme_when(clone_widget_book_theme_reader(
                                                        &theme_reader,
                                                    )),
                                            )
                                            .trailing_width(96.0)
                                            .trailing_height(28.0)
                                            .gap(10.0),
                                        ),
                                    )),
                            ),
                        ),
                    )
                    .with_child(
                        responsive_control_story_pair(
                            control_story_with_theme(
                                Rc::clone(&theme_reader),
                                "Fixed panes",
                                "FixedPaneSplit and the ResizablePane alias both use the split-pane path.",
                                Stack::vertical()
                                    .spacing(12.0)
                                    .alignment(Alignment::Stretch)
                                    .with_child(SemanticRegion::new(
                                        FIXED_PANE_SPLIT_NAME,
                                        SizedBox::new().width(380.0).height(142.0).with_child(
                                            FixedPaneSplit::horizontal(
                                                Surface::sidebar(Label::new("Fixed"))
                                                    .padding(Insets::all(12.0))
                                                    .theme_when(clone_widget_book_theme_reader(
                                                        &theme_reader,
                                                    )),
                                                Separator::vertical()
                                                    .theme_when(clone_widget_book_theme_reader(
                                                        &theme_reader,
                                                    )),
                                                Surface::panel(Label::new("Flexible"))
                                                    .padding(Insets::all(12.0))
                                                    .theme_when(clone_widget_book_theme_reader(
                                                        &theme_reader,
                                                    )),
                                            )
                                            .fixed_first(126.0)
                                            .divider_extent(1.0),
                                        ),
                                    ))
                                    .with_child(SizedBox::new().width(380.0).height(120.0).with_child(
                                        ResizablePane::horizontal(
                                            Surface::panel(Label::new("Resizable alias A"))
                                                .padding(Insets::all(12.0))
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &theme_reader,
                                                )),
                                            Surface::panel(Label::new("Resizable alias B"))
                                                .padding(Insets::all(12.0))
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &theme_reader,
                                                )),
                                        )
                                        .name("ResizablePane alias")
                                        .ratio(0.45),
                                    )),
                            ),
                            control_story_with_theme(
                                Rc::clone(&theme_reader),
                                "Scroll containers",
                                "ScrollView and VirtualScrollView expose bounded content and virtualized child layout.",
                                Stack::vertical()
                                    .spacing(12.0)
                                    .alignment(Alignment::Stretch)
                                    .with_child(SizedBox::new().width(380.0).height(110.0).with_child(
                                        ScrollView::vertical(
                                            Stack::vertical()
                                                .spacing(8.0)
                                                .alignment(Alignment::Stretch)
                                                .with_child(widget_book_body_text(
                                                    Rc::clone(&theme_reader),
                                                    "Scroll item 1",
                                                ))
                                                .with_child(widget_book_body_text(
                                                    Rc::clone(&theme_reader),
                                                    "Scroll item 2",
                                                ))
                                                .with_child(widget_book_body_text(
                                                    Rc::clone(&theme_reader),
                                                    "Scroll item 3",
                                                ))
                                                .with_child(widget_book_body_text(
                                                    Rc::clone(&theme_reader),
                                                    "Scroll item 4",
                                                )),
                                        )
                                        .name(SCROLL_VIEW_NAME)
                                        .state(scroll_state.clone())
                                        .theme(theme_reader()),
                                    ))
                                    .with_child(SizedBox::new().width(380.0).height(130.0).with_child(
                                        VirtualScrollView::new()
                                            .name(VIRTUAL_SCROLL_SAMPLE_NAME)
                                            .state(virtual_scroll_state)
                                            .padding(Insets::all(8.0))
                                            .spacing(8.0)
                                            .with_child(Label::new("Virtual row 1"))
                                            .with_child(Label::new("Virtual row 2"))
                                            .with_child(Label::new("Virtual row 3"))
                                            .with_child(Label::new("Virtual row 4"))
                                            .theme(theme_reader()),
                                    )),
                            ),
                        ),
                    ),
            )
            .description("Named wrapper for layout-only widgets."),
        ),
    )
}

fn build_text_widgets_gallery_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    NamedSection::new(
        TEXT_WIDGETS_GALLERY_NAME,
        panel_with_theme(
            Rc::clone(&theme_reader),
            TEXT_WIDGETS_GALLERY_NAME,
            "Text widgets cover labels, links, retained rich documents, text input aliases, and separator aliases.",
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(control_story_with_theme(
                    Rc::clone(&theme_reader),
                    "Rich text and links",
                    "RichText handles styled text documents while Link exposes URL semantics.",
                    Stack::vertical()
                        .spacing(12.0)
                        .alignment(Alignment::Start)
                        .with_child(
                            RichText::from_plain_text(
                                "Rich text sample with wrapping and retained layout.",
                                theme_reader().body_text_style(),
                            )
                            .semantic_name(RICH_TEXT_NAME)
                            .padding(Insets::all(8.0))
                            .min_width(340.0),
                        )
                        .with_child(
                            Link::new("Open SUI docs", "https://example.invalid/sui")
                                .semantic_name(LINK_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ),
                ))
                .with_child({
                    let document = RichDocumentModel::from_markdown(
                        r#"## Retained rich document

Select across blocks, activate the [application link](https://example.invalid/sui), or copy the highlighted code.

```rust
document.append_markdown("incremental tail");
```
"#,
                    );
                    let mut operation =
                        RichExtensionBlock::new("operation-log", "Indexed workspace");
                    operation.status = RichDocumentStatus::Success;
                    operation.summary = Some("2,000 keyed rows ready".into());
                    operation.body = "documents  184\nassets      37\nerrors       0".into();
                    document.append_extension(operation);

                    control_story_with_theme(
                        Rc::clone(&theme_reader),
                        "Rich document",
                        "RichDocumentView retains Markdown blocks, selection, code actions, and extensible structured results.",
                        SizedBox::new().width(680.0).height(330.0).with_child(
                            ScrollView::vertical(
                                RichDocumentView::new(document).theme(theme_reader()),
                            )
                            .retain_content_layer()
                            .theme(theme_reader()),
                        ),
                    )
                })
                .with_child(control_story_with_theme(
                    Rc::clone(&theme_reader),
                    "Public aliases",
                    "ComboBox, SpinBox, MultilineTextInput, and Divider remain visible in the public API.",
                    Stack::vertical()
                        .spacing(12.0)
                        .alignment(Alignment::Start)
                        .with_child(SizedBox::new().width(260.0).with_child(
                            ComboBox::new(COMBO_BOX_ALIAS_NAME)
                                .options(["Small", "Medium", "Large"])
                                .selected(1)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(SizedBox::new().width(220.0).with_child(
                            SpinBox::new(SPIN_BOX_ALIAS_NAME)
                                .range(0.0, 64.0)
                                .step(1.0)
                                .precision(0)
                                .value(24.0)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(SizedBox::new().width(320.0).height(96.0).with_child(
                            MultilineTextInput::new(MULTILINE_ALIAS_NAME)
                                .value("Multiline alias\nuses the TextArea widget.")
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(SizedBox::new().width(260.0).with_child(
                            Divider::horizontal()
                                .name(DIVIDER_ALIAS_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        )),
                )),
        ),
    )
}

fn build_data_and_interaction_gallery_with_theme(
    theme_reader: WidgetBookThemeReader,
) -> impl Widget {
    let drag_scope = DragDropScope::new();

    NamedSection::new(
        DATA_WIDGETS_GALLERY_NAME,
        panel_with_theme(
            Rc::clone(&theme_reader),
            DATA_WIDGETS_GALLERY_NAME,
            "Data widgets and interaction primitives cover long lists, virtual tables, layered rows, reorderable lists, and drag/drop hosts.",
            Stack::vertical()
                .spacing(16.0)
                .alignment(Alignment::Start)
                .with_child(
                    responsive_wide_control_story_pair(
                        control_story_with_theme(
                            Rc::clone(&theme_reader),
                            "Path and layers",
                            "PathBar aliases Breadcrumb, while LayerList adds visibility, lock, and reorder affordances.",
                            Stack::vertical()
                                .spacing(12.0)
                                .alignment(Alignment::Stretch)
                                .with_child(SizedBox::new().width(380.0).with_child(
                                    PathBar::new(PATH_BAR_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .items([
                                            BreadcrumbItem::new("Workspace"),
                                            BreadcrumbItem::new("Assets"),
                                            BreadcrumbItem::new("Materials"),
                                            BreadcrumbItem::new("Glass"),
                                        ])
                                        .current(3),
                                ))
                                .with_child(SizedBox::new().width(380.0).height(160.0).with_child(
                                    LayerList::new(LAYER_LIST_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .layers([
                                            LayerListItem::new("Highlights")
                                                .detail("Screen 72%")
                                                .thumbnail(Color::rgba(0.9, 0.72, 0.18, 1.0)),
                                            LayerListItem::new("Glass")
                                                .detail("Normal")
                                                .thumbnail(Color::rgba(0.25, 0.56, 0.9, 1.0)),
                                            LayerListItem::new("Shadow")
                                                .detail("Multiply")
                                                .thumbnail(Color::rgba(0.08, 0.1, 0.16, 1.0))
                                                .locked(true),
                                        ])
                                    .selected(1),
                            )),
                        ),
                        control_story_with_theme(
                            Rc::clone(&theme_reader),
                            "Tables",
                            "VirtualList realizes keyed widget rows; DataGrid and VirtualTable cover eager and painter-delegate tables.",
                            Stack::vertical()
                                .spacing(12.0)
                                .alignment(Alignment::Stretch)
                                .with_child({
                                    let rows = VirtualCollectionModel::from_items(
                                        "Widget book virtual rows",
                                        (0_u64..2_000)
                                            .map(|index| (index, format!("Retained row {index}"))),
                                    )
                                    .expect("widget-book row keys are unique");
                                    let row_theme = Rc::clone(&theme_reader);
                                    SizedBox::new().width(420.0).height(132.0).with_child(
                                        VirtualList::new(
                                            "Virtual retained rows",
                                            rows,
                                            move |_key, text| {
                                                Surface::field(
                                                    Label::new("").text_from(text),
                                                )
                                                .padding(Insets {
                                                    left: 8.0,
                                                    top: 5.0,
                                                    right: 8.0,
                                                    bottom: 5.0,
                                                })
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &row_theme,
                                                ))
                                            },
                                        )
                                        .estimated_row_height(30.0)
                                        .row_name(|key, _| format!("Virtual retained row {key}")),
                                    )
                                })
                                .with_child(SizedBox::new().width(420.0).height(150.0).with_child(
                                    DataGrid::new(DATA_GRID_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .columns([
                                            TableColumn::new("Asset"),
                                            TableColumn::new("Type").width(92.0),
                                            TableColumn::new("State").width(100.0),
                                        ])
                                        .rows([
                                            TableRow::new(["hero.png", "Image", "Ready"]),
                                            TableRow::new(["glass.mat", "Material", "Dirty"]),
                                            TableRow::new(["rig.skel", "Rig", "Cached"]),
                                        ])
                                        .selected(0),
                                ))
                                .with_child(SizedBox::new().width(420.0).height(170.0).with_child(
                                    VirtualTable::new(VIRTUAL_TABLE_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .columns([
                                            VirtualTableColumn::new("Row").width(80.0),
                                            VirtualTableColumn::new("Asset").width(170.0),
                                            VirtualTableColumn::new("Status").width(120.0),
                                        ])
                                        .row_count(10_000)
                                        .selected(42)
                                        .row_name(|row| format!("Virtual row {row}"))
                                        .row_description(|row| format!("Asset record {row}")),
                                )),
                        ),
                    ),
                )
                .with_child(
                    responsive_control_story_pair(
                        control_story_with_theme(
                            Rc::clone(&theme_reader),
                            "Reorderable list",
                            "ReorderableList wraps child rows with drag-aware order state.",
                            SizedBox::new().width(380.0).with_child(
                                ReorderableList::new(REORDERABLE_LIST_NAME)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                    .item(
                                        Surface::field(Label::new("Capture screenshots"))
                                            .padding(Insets::all(10.0))
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            )),
                                    )
                                    .item(
                                        Surface::field(Label::new("Review contrast"))
                                            .padding(Insets::all(10.0))
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            )),
                                    )
                                    .item(
                                        Surface::field(Label::new("Update docs"))
                                            .padding(Insets::all(10.0))
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            )),
                                    ),
                            ),
                        ),
                        control_story_with_theme(
                            Rc::clone(&theme_reader),
                            "Drag and drop",
                            "DragDropHost coordinates Draggable and DropTarget under one scope.",
                            SizedBox::new().width(380.0).with_child(DragDropHost::new(
                                drag_scope.clone(),
                                Stack::horizontal()
                                    .spacing(12.0)
                                    .alignment(Alignment::Center)
                                    .with_child(
                                        Draggable::new(
                                            Surface::field(widget_book_body_text(
                                                Rc::clone(&theme_reader),
                                                DRAG_SOURCE_NAME,
                                            ))
                                            .padding(Insets::all(12.0))
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            )),
                                        )
                                        .scope(drag_scope.clone())
                                        .payload(|| DragPayload::text("asset://hero"))
                                        .effect(DropEffect::Copy)
                                        .preview_label(DRAG_SOURCE_NAME),
                                    )
                                    .with_child(
                                        DropTarget::new(
                                            Surface::panel(widget_book_body_text(
                                                Rc::clone(&theme_reader),
                                                DROP_TARGET_NAME,
                                            ))
                                            .padding(Insets::all(12.0))
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            )),
                                        )
                                        .scope(drag_scope)
                                        .accept(|_| DropEffect::Copy),
                                    ),
                            )
                            .theme_when(clone_widget_book_theme_reader(&theme_reader))),
                        ),
                    ),
                ),
        ),
    )
}

fn build_canvas_and_media_gallery_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    let canvas_viewport = CanvasViewport::new().zoom(0.78).pan(Vector::new(8.0, -6.0));
    let canvas_document = Size::new(640.0, 420.0);

    NamedSection::new(
        CANVAS_WIDGETS_GALLERY_NAME,
        panel_with_theme(
            Rc::clone(&theme_reader),
            CANVAS_WIDGETS_GALLERY_NAME,
            "Canvas and media widgets cover vector drawing, rulers, pixel editing, palettes, brush previews, and live signal meters.",
            responsive_control_story_pair(
                control_story_with_theme(
                    Rc::clone(&theme_reader),
                    "Vector and pixel canvas",
                    "Canvas, CanvasRuler, CanvasViewport, CanvasShape, CanvasStroke, PixelCanvas, and PixelCanvasState.",
                    Stack::vertical()
                        .spacing(12.0)
                        .alignment(Alignment::Stretch)
                        .with_child(
                            SizedBox::new().width(380.0).with_child(
                                CanvasRuler::horizontal(CANVAS_RULER_NAME, canvas_document)
                                    .viewport(canvas_viewport, Size::new(380.0, 26.0))
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        )
                        .with_child(
                            SizedBox::new().width(380.0).height(190.0).with_child(
                                Canvas::new(CANVAS_NAME)
                                    .desired_size(Size::new(380.0, 190.0))
                                    .viewport(canvas_viewport)
                                    .shape(CanvasShape::rect(
                                        Rect::new(80.0, 70.0, 160.0, 90.0),
                                        Some(Color::rgba(0.14, 0.46, 0.86, 0.28)),
                                        Some(CanvasStroke::new(
                                            Color::rgba(0.14, 0.46, 0.86, 1.0),
                                            2.0,
                                        )),
                                    ))
                                    .shape(CanvasShape::circle(
                                        Point::new(330.0, 160.0),
                                        48.0,
                                        Some(Color::rgba(0.88, 0.55, 0.18, 0.3)),
                                        Some(CanvasStroke::new(
                                            Color::rgba(0.88, 0.55, 0.18, 1.0),
                                            2.0,
                                        )),
                                    ))
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        )
                        .with_child(
                            SizedBox::new().width(380.0).height(170.0).with_child(
                                PixelCanvas::from_fn(PIXEL_CANVAS_NAME, 16, 12, |x, y| {
                                    let checker = (x + y) % 2 == 0;
                                    if checker {
                                        Color::rgba(0.12, 0.54, 0.88, 1.0)
                                    } else {
                                        Color::rgba(0.92, 0.74, 0.25, 1.0)
                                    }
                                })
                                .state(PixelCanvasState::new())
                                .desired_size(Size::new(380.0, 170.0))
                                .fit_on_first_layout()
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ),
                ),
                control_story_with_theme(
                    Rc::clone(&theme_reader),
                    "Media controls",
                    "ColorPalette, BrushPreview, ColorSwatch, Image, and SignalMeter sit beside the larger ColorPicker story below.",
                    Stack::vertical()
                        .spacing(12.0)
                        .alignment(Alignment::Start)
                        .with_child(
                            ColorPalette::new(COLOR_PALETTE_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .columns(4)
                                .swatch_size(28.0)
                                .swatches([
                                    ColorPaletteSwatch::new(
                                        "Primary",
                                        Color::rgba(0.12, 0.55, 0.88, 1.0),
                                    ),
                                    ColorPaletteSwatch::new(
                                        "Secondary",
                                        Color::rgba(0.88, 0.54, 0.22, 1.0),
                                    ),
                                    ColorPaletteSwatch::new(
                                        "Success",
                                        Color::rgba(0.18, 0.64, 0.38, 1.0),
                                    ),
                                    ColorPaletteSwatch::new(
                                        "Danger",
                                        Color::rgba(0.84, 0.22, 0.22, 1.0),
                                    ),
                                ])
                                .selected(0),
                        )
                        .with_child(
                            BrushPreview::new(BRUSH_PREVIEW_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .spec(BrushPreviewSpec::new(
                                    Color::rgba(0.12, 0.55, 0.88, 1.0),
                                    28.0,
                                    0.78,
                                    BrushPreviewShape::Round,
                                ))
                                .size(Size::new(280.0, 56.0)),
                        )
                        .with_child(
                            Stack::horizontal()
                                .spacing(10.0)
                                .alignment(Alignment::Center)
                                .with_child(
                                    ColorSwatch::new(
                                        "Canvas accent swatch",
                                        Color::rgba(0.12, 0.55, 0.88, 1.0),
                                    )
                                    .size(Size::new(54.0, 30.0)),
                                )
                                .with_child(
                                    Image::new(WIDGET_BOOK_IMAGE_HANDLE)
                                        .label("Media thumbnail")
                                        .fit(ImageFit::Contain)
                                        .corner_radius(8.0),
                                )
                                .with_child(
                                    SignalMeter::new(SIGNAL_METER_NAME)
                                        .active(true)
                                        .tone(SemanticTone::Success)
                                        .bars(10)
                                        .size(Size::new(92.0, 34.0))
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                ),
                        ),
                ),
            ),
        ),
    )
}

#[cfg(test)]
fn build_color_and_imagery_story() -> impl Widget {
    build_color_and_imagery_story_with_theme(default_widget_book_theme_reader())
}

fn build_color_and_imagery_story_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    panel_with_theme(
        Rc::clone(&theme_reader),
        "Color and imagery",
        "SUI targets visual tooling, so swatches, a usable picker, and image previews need to exist as first-class widgets.",
        Stack::vertical()
            .spacing(16.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Stack::horizontal()
                    .spacing(12.0)
                    .alignment(Alignment::Center)
                    .with_child(
                        ColorSwatch::new(COLOR_SWATCH_NAME, Color::rgba(0.12, 0.55, 0.88, 1.0))
                            .size(Size::new(64.0, 36.0)),
                    )
                    .with_child(
                        ColorSwatch::new("Shadow swatch", Color::rgba(0.08, 0.10, 0.14, 0.84))
                            .size(Size::new(64.0, 36.0)),
                    )
                    .with_child(demo_label(
                        &theme_reader,
                        "Use swatches for palettes, material chips, and compact property rows.",
                        DemoTextRole::Supporting,
                        DemoTextColor::Muted,
                    )),
            )
            .with_child(
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Start)
                    .with_child(
                        SizedBox::new().width(434.0).height(448.0).with_child(
                            ColorPicker::from_color(
                                COLOR_PICKER_NAME,
                                Color::new(sui::ColorSpace::LinearSrgb, 2.0, 0.65, 0.4, 1.0),
                            )
                            .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ),
                    )
                    .with_child(
                        Stack::horizontal()
                            .spacing(12.0)
                            .alignment(Alignment::Start)
                            .with_child(
                                SizedBox::new().width(211.0).with_child(
                                    SimpleColorPicker::from_color(
                                        "Simple HSL color picker",
                                        Color::rgba(0.20, 0.58, 0.86, 1.0),
                                    )
                                    .mode(SimpleColorPickerMode::Hsl)
                                    .show_alpha(false)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                ),
                            )
                            .with_child(
                                SizedBox::new().width(211.0).with_child(
                                    SimpleColorPicker::from_color(
                                        "Simple RGB color picker",
                                        Color::display_p3(0.86, 0.42, 0.20, 0.70),
                                    )
                                    .mode(SimpleColorPickerMode::Rgb)
                                    .color_space(sui::ColorSpace::DisplayP3)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                ),
                            ),
                    )
                    .with_child(
                        SizedBox::new().width(220.0).height(220.0).with_child(
                            Image::new(WIDGET_BOOK_IMAGE_HANDLE)
                                .label(DEMO_IMAGE_LABEL)
                                .fit(ImageFit::Contain)
                                .background_when(widget_book_theme_color(&theme_reader, |theme| {
                                    theme.palette.control
                                }))
                                .corner_radius(12.0),
                        ),
                    ),
            ),
    )
}

pub fn build_animation_benchmark() -> impl Widget {
    Padding::all(
        24.0,
        Stack::vertical()
            .spacing(18.0)
            .alignment(Alignment::Stretch)
            .with_child(AnimationBenchmarkRetainedLane::new())
            .with_child(AnimationBenchmarkRepaintLane::new())
            .with_child(AnimationBenchmarkScaleGrid::new()),
    )
}

pub fn build_animation_benchmark_application() -> Application {
    App::new()
        .window(Window::new(ANIMATION_BENCHMARK_TITLE).root(build_animation_benchmark()))
        .into_application()
}

pub fn build_retained_text_benchmark() -> impl Widget {
    build_retained_text_benchmark_with_theme(default_widget_book_theme_reader())
}

pub fn build_retained_text_benchmark_with_theme(
    theme_reader: WidgetBookThemeReader,
) -> impl Widget {
    const SECTION_COUNT: usize = 72;
    const PARAGRAPHS_PER_SECTION: usize = 4;

    let scroll_state = ScrollState::new();
    let mut content = Stack::vertical()
        .spacing(18.0)
        .alignment(Alignment::Stretch)
        .with_child(panel(
        "Retained text wall",
        "Focused benchmark surface for measuring text-heavy cached scroll regeneration without the live overlay or mixed control chrome.",
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                SizedBox::new().width(900.0).with_child(
                    Label::new(
                        "The outer scroll view stays retained, the visible content stays dominated by wrapped labels, and the benchmark scrolls through enough sections to keep retained packet rebuilds focused on atlas text payloads.",
                    )
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Body,
                        |_| Color::rgba(0.38, 0.46, 0.56, 1.0),
                    )),
                ),
            )
            .with_child(
                SizedBox::new().width(900.0).with_child(
                    Label::new(
                        "Each section deliberately uses several long paragraphs so the per-frame upload delta is shaped by text submission rather than button chrome, icons, or image content.",
                    )
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Body,
                        |_| Color::rgba(0.42, 0.49, 0.58, 1.0),
                    )),
                ),
            ),
    ));

    for section_index in 0..SECTION_COUNT {
        let (title, subtitle) = retained_text_benchmark_section(section_index);
        let mut body = Stack::vertical().spacing(8.0).alignment(Alignment::Stretch);

        for paragraph_index in 0..PARAGRAPHS_PER_SECTION {
            body = body.with_child(
                SizedBox::new().width(900.0).with_child(
                    Label::new(retained_text_benchmark_paragraph(
                        section_index,
                        paragraph_index,
                    ))
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Body,
                        |_| Color::rgba(0.36, 0.44, 0.53, 1.0),
                    )),
                ),
            );
        }

        content = content.with_child(Background::new(
            Color::rgba(0.985, 0.99, 1.0, 1.0),
            Padding::all(
                18.0,
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Stretch)
                    .with_child(Label::new(title).style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::SectionTitle,
                        |_| Color::rgba(0.11, 0.15, 0.21, 1.0),
                    )))
                    .with_child(Label::new(subtitle).style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Body,
                        |_| Color::rgba(0.44, 0.51, 0.60, 1.0),
                    )))
                    .with_child(body),
            ),
        ));
    }

    VerticalScrollPane::new(
        ScrollView::vertical(Padding::all(
            24.0,
            SizedBox::new().width(948.0).with_child(content),
        ))
        .state(scroll_state.clone())
        .overlay_scroll_bars(false)
        .name(RETAINED_TEXT_BENCHMARK_SCROLL_NAME),
        ScrollBar::vertical(scroll_state)
            .name(RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME)
            .theme_when(clone_widget_book_theme_reader(&theme_reader)),
    )
}

pub fn build_retained_text_benchmark_application() -> Application {
    App::new()
        .window(Window::new(RETAINED_TEXT_BENCHMARK_TITLE).root(build_retained_text_benchmark()))
        .into_application()
}

pub fn build_text_rendering_comparison_surface() -> impl Widget {
    build_text_rendering_comparison_surface_with_theme(default_widget_book_theme_reader())
}

pub fn build_text_rendering_comparison_surface_with_theme(
    theme_reader: WidgetBookThemeReader,
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
            .theme_when(clone_widget_book_theme_reader(&theme_reader)),
        ScrollBar::horizontal(scroll_state)
            .name(TEXT_RENDERING_COMPARISON_HORIZONTAL_SCROLL_BAR_NAME)
            .theme_when(clone_widget_book_theme_reader(&theme_reader)),
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
    build_color_validation_surface_with_theme(default_widget_book_theme_reader())
}

pub fn build_color_validation_surface_with_theme(
    theme_reader: WidgetBookThemeReader,
) -> impl Widget {
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
            .theme_when(clone_widget_book_theme_reader(&theme_reader)),
        ScrollBar::horizontal(scroll_state)
            .name(COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME)
            .theme_when(clone_widget_book_theme_reader(&theme_reader)),
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

fn build_color_validation_row_with_theme(
    theme_reader: WidgetBookThemeReader,
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
        .brush_when(widget_book_theme_color(&theme_reader, |theme| {
            theme.palette.surface_raised
        })),
    )
}

fn build_color_validation_quad_row_with_theme(
    theme_reader: WidgetBookThemeReader,
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
        .brush_when(widget_book_theme_color(&theme_reader, |theme| {
            theme.palette.surface_raised
        })),
    )
}

fn build_color_validation_swatch_with_theme(
    theme_reader: WidgetBookThemeReader,
    name: &'static str,
    color: Color,
    min_width: f32,
) -> impl Widget {
    MinimumWidth::new(
        min_width,
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Center)
            .with_child(ColorSwatch::new(name, color).size(Size::new(132.0, 56.0)))
            .with_child(demo_label(
                &theme_reader,
                name,
                DemoTextRole::Supporting,
                DemoTextColor::Text,
            )),
    )
}

fn build_text_rendering_mode_card_with_theme(
    theme_reader: WidgetBookThemeReader,
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
                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
            ),
    )
}

fn build_text_rendering_summary_metric_with_theme(
    theme_reader: WidgetBookThemeReader,
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
        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
    )
}

fn build_text_rendering_policy_snippet_with_theme(
    theme_reader: WidgetBookThemeReader,
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
    .brush_when(widget_book_theme_color(&theme_reader, |theme| {
        theme.palette.control
    }))
}

fn text_rendering_sample_name(title: &'static str, dark: bool) -> String {
    let surface = if dark { "dark" } else { "light" };
    format!("{title} {surface} sample")
}

struct TextRenderingSampleTile {
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

fn build_text_rendering_sample_tile(
    name: String,
    label: &'static str,
    dark: bool,
    spec: TextRenderingModeSpec,
) -> impl Widget {
    TextRenderingSampleTile::new(name, label, dark, spec)
}

fn paint_text_rendering_sample(ctx: &mut PaintCtx, bounds: Rect, label: &'static str, dark: bool) {
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

fn draw_text_rendering_sample_line(
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
    build_text_validation_surface_with_theme(default_widget_book_theme_reader())
}

pub fn build_text_validation_surface_with_theme(
    theme_reader: WidgetBookThemeReader,
) -> impl Widget {
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
                    "Aa ill minimum | Cyrillic ÐŸÑ€Ð¸Ð²ÐµÑ‚",
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
                    "abc 123 | RTL run | caret crosses",
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
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
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
}

fn build_text_validation_probe_card_with_theme(
    theme_reader: WidgetBookThemeReader,
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
                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
            ),
    )
}

pub fn build_text_editing_benchmark() -> impl Widget {
    build_text_editing_benchmark_with_theme(default_widget_book_theme_reader())
}

pub fn build_text_editing_benchmark_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    let editor_document = text_editing_benchmark_document();
    let editor_style_spans = text_editing_benchmark_style_spans(&editor_document);
    let editor_style_overlays = text_editing_benchmark_style_overlays(&editor_document);
    let editor_panel = panel_with_theme(
        Rc::clone(&theme_reader),
        "Editable styled code surface",
        "Benchmark typing, selection, IME preedit, wheel scrolling, and syntax-overlay churn against one long text surface.",
        SizedBox::new().width(560.0).height(700.0).with_child(
            TextSurface::new(TEXT_EDITING_BENCHMARK_EDITOR_NAME)
                .value(editor_document)
                .direction(TextDirection::LeftToRight)
                .min_width(560.0)
                .min_height(700.0)
                .style_spans(editor_style_spans)
                .style_overlays(editor_style_overlays)
                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                .text_style_when(|theme| {
                    widget_book_theme_mono_text_style(theme, theme.text.sm, theme.palette.text)
                }),
        ),
    );
    let syntax_panel = panel_with_theme(
        Rc::clone(&theme_reader),
        "Syntax-highlight preview",
        "A scrollable code-preview column keeps the benchmark honest about syntax-color churn instead of measuring only plain-text editing.",
        SizedBox::new().width(520.0).height(700.0).with_child(
            build_text_editing_syntax_preview_with_theme(Rc::clone(&theme_reader)),
        ),
    );

    Padding::all(
        24.0,
        SplitView::horizontal(editor_panel, syntax_panel)
            .name(TEXT_EDITING_BENCHMARK_SPLIT_NAME)
            .ratio(0.54)
            .min_first(420.0)
            .min_second(360.0),
    )
}

pub fn build_text_editing_benchmark_application() -> Application {
    App::new()
        .window(Window::new(TEXT_EDITING_BENCHMARK_TITLE).root(
            LivePerformanceRoot::new(
                TEXT_EDITING_BENCHMARK_TITLE,
                "Focused benchmark surface for editor-style typing, selection, scrolling, and syntax-highlight preview cost.",
                build_text_editing_benchmark(),
            ),
        ))
        .into_application()
}

fn retained_text_benchmark_section(section_index: usize) -> (String, String) {
    const THEMES: [(&str, &str); 6] = [
        (
            "Atlas residency",
            "Repeated prose keeps the retained packet mix biased toward atlas glyph work.",
        ),
        (
            "Viewport churn",
            "Small scroll deltas expose new wrapped lines while leaving most state unchanged.",
        ),
        (
            "Packet rebuilds",
            "Retained packets should stay text-heavy instead of expanding glyph quads into generic geometry.",
        ),
        (
            "Glyph density",
            "Wide paragraphs keep each visible retained surface loaded with enough glyph instances to show byte deltas clearly.",
        ),
        (
            "Cache locality",
            "Stable content and repeated vocabulary encourage glyph atlas reuse after the initial warmup.",
        ),
        (
            "Scroll pacing",
            "No-vsync harness runs isolate renderer prep and upload cost from present back-pressure.",
        ),
    ];

    let (topic, subtitle) = THEMES[section_index % THEMES.len()];
    (
        format!("Section {:02} Â· {topic}", section_index + 1),
        subtitle.to_string(),
    )
}

fn retained_text_benchmark_paragraph(section_index: usize, paragraph_index: usize) -> String {
    const OPENERS: [&str; 6] = [
        "Atlas uploads should now track per-glyph instance payloads instead of six transient vertices per shaped glyph.",
        "This retained scroll surface keeps the scene composition simple so upload accounting is easier to read.",
        "Visible paragraphs change a little on each wheel tick, which keeps retained packet rebuilds centered on text.",
        "Repeated headings and body copy help stabilize glyph atlas misses after the initial scroll warmup.",
        "The benchmark is intentionally prose-heavy because text submission is the renderer path under inspection.",
        "Scroll delta size is fixed so frame samples stay comparable across runs and git revisions.",
    ];
    const DETAILS: [&str; 6] = [
        "Long wrapped lines are useful here because they raise glyph count without introducing extra widget complexity.",
        "Retained caches should still avoid rebuilding unrelated packets while the scroll layer reveals fresh text bands.",
        "Frame summaries can then compare upload bytes, glyph counts, and timing without guessing how much non-text work leaked into the sample.",
        "The same prose appears in varied combinations so the atlas can reuse cached glyph shapes while the instance buffer still changes per frame.",
        "This also mirrors the next line-window phase, where large text surfaces should only submit visible lines to the renderer.",
        "Running the harness with vsync disabled keeps the benchmark focused on renderer cost rather than swapchain pacing.",
    ];

    let opener = OPENERS[(section_index + paragraph_index) % OPENERS.len()];
    let detail = DETAILS[(section_index * 3 + paragraph_index) % DETAILS.len()];
    let cadence = 12 + ((section_index + paragraph_index) % 9);
    let packet_hint = 4 + ((section_index * 5 + paragraph_index) % 7);

    format!(
        "Section {:02}, paragraph {}. {} {} The visible cadence in this sample targets about {} wrapped lines per viewport slice, while adjacent retained packets typically contribute around {} neighboring text blocks before the next wheel event moves the window again.",
        section_index + 1,
        paragraph_index + 1,
        opener,
        detail,
        cadence,
        packet_hint,
    )
}

fn text_validation_editor_seed() -> String {
    [
        "Validation checklist",
        "- Shape: Latin stems and common fallback families should stay readable.",
        "- Wrapping: long diagnostics must reflow without selection gaps when the viewport narrows.",
        "- IME: composition commits should land near the caret instead of invalidating the whole surface.",
        "- Caret: moving across bidi boundaries should preserve stable layout handles and visible overlays.",
        "",
        "Fallback probes to paste, edit, or compare:",
        "Arabic: Ù…Ø±Ø­Ø¨Ø§ | Hebrew: ×©×œ×•× | Hindi: à¤¨à¤®à¤¸à¥à¤¤à¥‡ | Han: ä¸­æ–‡ | Emoji: ðŸ™‚",
        "",
        "Type here to confirm the runtime still exposes semantics-first text input for automated tests.",
    ]
    .join("\n")
}

fn text_editing_benchmark_document() -> String {
    let mut lines = Vec::new();
    lines.push("// Text editing benchmark: long code-like document with mixed comments and repeated glyph traffic".to_string());
    lines.push("mod editor_benchmark {".to_string());
    for index in 0..240 {
        let indent = if index % 6 == 0 { "        " } else { "    " };
        let keyword = ["let", "if", "match", "while", "for", "return"][index % 6];
        let symbol = [
            "shape_visible_window",
            "apply_incremental_edit",
            "measure_selection_overlay",
            "resolve_fallback_face",
            "update_syntax_cache",
            "record_scroll_sample",
        ][(index * 3) % 6];
        let comment = [
            "// atlas reuse should stay warm ðŸ™‚",
            "// bidi note: abc ××‘×’ 123 Ù…Ø±Ø­Ø¨Ø§",
            "// syntax colors keep changing across the preview pane",
            "// fallback sample includes Ð–, ä¸­, and à¤¨à¤®à¤¸à¥à¤¤à¥‡ in comments",
            "// selection overlays should repaint locally",
            "// retained packets should not rebuild unrelated code blocks",
        ][(index * 5) % 6];
        lines.push(format!(
            "{indent}{keyword} row_{index:03} = {symbol}(cursor + {delta}, viewport_height - {trim}); {comment}",
            delta = 3 + (index % 17),
            trim = 1 + (index % 7),
        ));
        if index % 8 == 7 {
            lines.push(format!(
                "        // folded section {:02}: syntax_color = accent::{:?}; ime = \"å€™è£œ{}\";",
                (index / 8) + 1,
                ["Keyword", "Type", "Comment", "Number"][index % 4],
                index
            ));
        }
    }
    lines.push("}".to_string());
    lines.join("\n")
}

fn text_editing_benchmark_style_spans(document: &str) -> Vec<TextSurfaceStyleSpan> {
    let keyword_style = text_editing_benchmark_span_style(Color::rgba(0.78, 0.34, 0.16, 1.0));
    let symbol_style = text_editing_benchmark_span_style(Color::rgba(0.09, 0.43, 0.58, 1.0));
    let string_style = text_editing_benchmark_span_style(Color::rgba(0.42, 0.32, 0.74, 1.0));
    let comment_style = text_editing_benchmark_span_style(Color::rgba(0.36, 0.45, 0.25, 1.0));
    let number_style = text_editing_benchmark_span_style(Color::rgba(0.14, 0.49, 0.24, 1.0));
    let keywords = ["mod", "let", "if", "match", "while", "for", "return"];
    let symbols = [
        "editor_benchmark",
        "shape_visible_window",
        "apply_incremental_edit",
        "measure_selection_overlay",
        "resolve_fallback_face",
        "update_syntax_cache",
        "record_scroll_sample",
    ];
    let mut spans = Vec::new();
    let mut line_offset = 0usize;

    for line_with_break in document.split_inclusive('\n') {
        let line = line_with_break
            .strip_suffix('\n')
            .unwrap_or(line_with_break);
        let comment_start = line.find("//");
        let code_end = comment_start.unwrap_or(line.len());

        for keyword in keywords {
            collect_text_editing_word_spans(
                &mut spans,
                line_offset,
                &line[..code_end],
                keyword,
                keyword_style.clone(),
            );
        }
        for symbol in symbols {
            collect_text_editing_word_spans(
                &mut spans,
                line_offset,
                &line[..code_end],
                symbol,
                symbol_style.clone(),
            );
        }
        collect_text_editing_number_spans(
            &mut spans,
            line_offset,
            &line[..code_end],
            number_style.clone(),
        );
        collect_text_editing_string_spans(
            &mut spans,
            line_offset,
            &line[..code_end],
            string_style.clone(),
        );
        if let Some(comment_start) = comment_start {
            spans.push(TextSurfaceStyleSpan::new(
                line_offset + comment_start..line_offset + line.len(),
                comment_style.clone(),
            ));
        }

        line_offset += line_with_break.len();
    }

    spans
}

fn text_editing_benchmark_style_overlays(document: &str) -> Vec<TextSurfaceStyleOverlay> {
    let search_style = text_editing_benchmark_span_style(Color::rgba(0.08, 0.38, 0.72, 1.0));
    let diagnostic_style = text_editing_benchmark_span_style(Color::rgba(0.70, 0.14, 0.20, 1.0));
    let rich_preview_style = text_editing_benchmark_span_style(Color::rgba(0.46, 0.23, 0.66, 1.0));
    let mut overlays = Vec::new();

    collect_text_editing_overlays(
        &mut overlays,
        document,
        "shape_visible_window",
        TextSurfaceOverlayKind::SearchMatch,
        search_style,
    );
    collect_text_editing_overlays(
        &mut overlays,
        document,
        "fallback",
        TextSurfaceOverlayKind::Diagnostic,
        diagnostic_style,
    );
    collect_text_editing_overlays(
        &mut overlays,
        document,
        "ðŸ™‚",
        TextSurfaceOverlayKind::RichTextPreview,
        rich_preview_style,
    );

    overlays
}

fn text_editing_benchmark_span_style(color: Color) -> TextStyle {
    let text = DefaultTheme::default().text;
    widget_book_mono_text_style(text.sm, color)
}

fn collect_text_editing_word_spans(
    spans: &mut Vec<TextSurfaceStyleSpan>,
    line_offset: usize,
    line: &str,
    word: &str,
    style: TextStyle,
) {
    let mut search_offset = 0usize;
    while let Some(relative_start) = line[search_offset..].find(word) {
        let start = search_offset + relative_start;
        let end = start + word.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        if text_editing_word_boundary(before) && text_editing_word_boundary(after) {
            spans.push(TextSurfaceStyleSpan::new(
                line_offset + start..line_offset + end,
                style.clone(),
            ));
        }
        search_offset = end;
    }
}

fn collect_text_editing_number_spans(
    spans: &mut Vec<TextSurfaceStyleSpan>,
    line_offset: usize,
    line: &str,
    style: TextStyle,
) {
    let mut span_start = None;
    for (index, ch) in line.char_indices() {
        let number_char = ch.is_ascii_digit() || ch == '.';
        match (span_start, number_char) {
            (None, true) => span_start = Some(index),
            (Some(start), false) => {
                spans.push(TextSurfaceStyleSpan::new(
                    line_offset + start..line_offset + index,
                    style.clone(),
                ));
                span_start = None;
            }
            _ => {}
        }
    }

    if let Some(start) = span_start {
        spans.push(TextSurfaceStyleSpan::new(
            line_offset + start..line_offset + line.len(),
            style,
        ));
    }
}

fn collect_text_editing_string_spans(
    spans: &mut Vec<TextSurfaceStyleSpan>,
    line_offset: usize,
    line: &str,
    style: TextStyle,
) {
    let mut string_start = None;
    for (index, ch) in line.char_indices() {
        if ch != '"' {
            continue;
        }
        if let Some(start) = string_start.take() {
            spans.push(TextSurfaceStyleSpan::new(
                line_offset + start..line_offset + index + ch.len_utf8(),
                style.clone(),
            ));
        } else {
            string_start = Some(index);
        }
    }
}

fn collect_text_editing_overlays(
    overlays: &mut Vec<TextSurfaceStyleOverlay>,
    document: &str,
    needle: &str,
    kind: TextSurfaceOverlayKind,
    style: TextStyle,
) {
    let mut search_offset = 0usize;
    while let Some(relative_start) = document[search_offset..].find(needle) {
        let start = search_offset + relative_start;
        let end = start + needle.len();
        overlays.push(TextSurfaceStyleOverlay::new(
            start..end,
            style.clone(),
            kind.clone(),
        ));
        search_offset = end;
    }
}

fn text_editing_word_boundary(ch: Option<char>) -> bool {
    match ch {
        Some(ch) => !ch.is_alphanumeric() && ch != '_',
        None => true,
    }
}

fn build_text_editing_syntax_preview_with_theme(
    theme_reader: WidgetBookThemeReader,
) -> impl Widget {
    let theme = theme_reader();
    let (document, style_spans) = text_editing_syntax_preview_content(theme);

    TextSurface::new(TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME)
        .value(document)
        .read_only()
        .min_width(520.0)
        .min_height(700.0)
        .style_spans(style_spans)
        .theme_when(clone_widget_book_theme_reader(&theme_reader))
        .text_style_when(|theme| {
            widget_book_theme_mono_text_style(theme, theme.text.sm, theme.palette.text)
        })
}

fn text_editing_syntax_preview_content(theme: DefaultTheme) -> (String, Vec<TextSurfaceStyleSpan>) {
    let text = widget_book_theme_mono_text_style(theme, theme.text.sm, theme.palette.text);
    let muted = widget_book_theme_mono_text_style(theme, theme.text.sm, theme.palette.text_muted);
    let accent = widget_book_theme_mono_text_style(theme, theme.text.sm, theme.palette.accent);
    let success = widget_book_theme_mono_text_style(theme, theme.text.sm, theme.palette.success);
    let mut document = String::new();
    let mut spans = Vec::with_capacity(220 * 8);

    for line_index in 0..220 {
        let keyword = ["fn", "let", "match", "if", "while", "return"][line_index % 6];
        let type_name =
            ["Editor", "Glyphs", "Select", "Syntax", "Window", "Frame"][(line_index * 7) % 6];
        let method =
            ["shape", "cache", "cursor", "paint", "fallback", "commit"][(line_index * 11) % 6];
        let color_name = ["keyword", "type", "comment", "number"][line_index % 4];

        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!("{:>3} ", line_index + 1),
            &muted,
        );
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!("{keyword} "),
            &accent,
        );
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!("sample_{line_index:03}"),
            &text,
        );
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!(": {type_name}"),
            &muted,
        );
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!(" = {method}("),
            &text,
        );
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!("{:.2}", 0.5 + ((line_index % 17) as f32 * 0.125)),
            &success,
        );
        push_text_editing_syntax_segment(&mut document, &mut spans, "); ", &text);
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!("// {color_name} glyph {}", line_index % 9),
            &muted,
        );
        if line_index + 1 < 220 {
            document.push('\n');
        }
    }

    (document, spans)
}

fn push_text_editing_syntax_segment(
    document: &mut String,
    spans: &mut Vec<TextSurfaceStyleSpan>,
    segment: &str,
    style: &TextStyle,
) {
    let start = document.len();
    document.push_str(segment);
    spans.push(TextSurfaceStyleSpan::new(
        start..document.len(),
        style.clone(),
    ));
}

fn widget_book_demo_image_pixels() -> Vec<u8> {
    let width = 72usize;
    let height = 72usize;
    let mut pixels = vec![0u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) * 4;
            let checker = ((x / 8) + (y / 8)) % 2 == 0;
            let mut red = if checker { 228 } else { 208 };
            let mut green = if checker { 236 } else { 216 };
            let mut blue = if checker { 248 } else { 228 };
            let alpha = 255u8;

            if x > 10 && x < 62 && y > 10 && y < 62 {
                red = 38 + ((x as f32 / width as f32) * 50.0) as u8;
                green = 108 + ((y as f32 / height as f32) * 60.0) as u8;
                blue = 190;
            }

            if (x > 18 && x < 54) && (y > 18 && y < 54) {
                red = 245;
                green = 248;
                blue = 252;
            }

            if (x > 28 && x < 44) && (y > 24 && y < 48) {
                red = 255;
                green = 168;
                blue = 60;
            }

            pixels[index] = red;
            pixels[index + 1] = green;
            pixels[index + 2] = blue;
            pixels[index + 3] = alpha;
        }
    }

    pixels
}

fn panel<W>(title: &str, subtitle: &str, body: W) -> impl Widget
where
    W: Widget + 'static,
{
    panel_with_theme(default_widget_book_theme_reader(), title, subtitle, body)
}

fn filtered_panel_with_theme<W>(
    shell_state: Rc<RefCell<WidgetBookShellState>>,
    category: WidgetBookCategory,
    theme_reader: WidgetBookThemeReader,
    title: &'static str,
    subtitle: &'static str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    FilterableSection::new(
        shell_state,
        category,
        format!("{title} {subtitle}"),
        panel_with_theme(theme_reader, title, subtitle, body),
    )
}

fn filterable_widget_book_section<W>(
    shell_state: Rc<RefCell<WidgetBookShellState>>,
    category: WidgetBookCategory,
    search_text: &str,
    child: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    FilterableSection::new(shell_state, category, search_text, child)
}

fn panel_with_theme<W>(
    theme_reader: WidgetBookThemeReader,
    title: &str,
    subtitle: &str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    CenteredContentWidth::new(
        GALLERY_CONTENT_MAX_WIDTH,
        Background::new(
            theme_reader().palette.surface,
            Padding::all(
                18.0,
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Stretch)
                    .with_child(MaximumWidth::new(
                        GALLERY_TEXT_MAX_WIDTH,
                        demo_label(
                            &theme_reader,
                            title,
                            DemoTextRole::SectionTitle,
                            DemoTextColor::Text,
                        ),
                    ))
                    .with_child(MaximumWidth::new(
                        GALLERY_TEXT_MAX_WIDTH,
                        demo_label(
                            &theme_reader,
                            subtitle,
                            DemoTextRole::Supporting,
                            DemoTextColor::Muted,
                        ),
                    ))
                    .with_child(body),
            ),
        )
        .brush_when(widget_book_theme_color(&theme_reader, |theme| {
            theme.palette.surface
        })),
    )
}

fn control_story_with_theme<W>(
    theme_reader: WidgetBookThemeReader,
    title: &str,
    caption: &str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    StoryCard::new(
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Start)
            .with_child(demo_label(
                &theme_reader,
                title,
                DemoTextRole::CardTitle,
                DemoTextColor::Text,
            ))
            .with_child(MaximumWidth::new(
                CONTROL_STORY_CONTENT_MAX_WIDTH,
                demo_label(
                    &theme_reader,
                    caption,
                    DemoTextRole::Metadata,
                    DemoTextColor::Muted,
                ),
            ))
            .with_child(body),
    )
    .theme_when(clone_widget_book_theme_reader(&theme_reader))
}

fn responsive_control_story<Story>(story: Story) -> impl Widget
where
    Story: Widget + 'static,
{
    Flex::horizontal()
        .wrap(FlexWrap::Wrap)
        .justify(FlexJustify::Center)
        .align_items(Alignment::Stretch)
        .align_content(FlexAlignContent::Start)
        .with_item(
            story,
            FlexItem::new()
                .grow(1.0)
                .basis_gap_aware_fraction(1.0)
                .min_width(CONTROL_STORY_CARD_MIN_WIDTH)
                .max_width(CONTROL_STORY_CARD_MAX_WIDTH),
        )
}

fn responsive_control_story_pair<Left, Right>(left: Left, right: Right) -> impl Widget
where
    Left: Widget + 'static,
    Right: Widget + 'static,
{
    responsive_control_story_pair_with_max_width(left, right, CONTROL_STORY_CARD_MAX_WIDTH)
}

fn responsive_wide_control_story_pair<Left, Right>(left: Left, right: Right) -> impl Widget
where
    Left: Widget + 'static,
    Right: Widget + 'static,
{
    responsive_control_story_pair_with_max_width(left, right, WIDE_CONTROL_STORY_CARD_MAX_WIDTH)
}

fn responsive_control_story_pair_with_max_width<Left, Right>(
    left: Left,
    right: Right,
    max_width: f32,
) -> impl Widget
where
    Left: Widget + 'static,
    Right: Widget + 'static,
{
    Flex::horizontal()
        .gap(14.0)
        .wrap(FlexWrap::Wrap)
        .justify(FlexJustify::Center)
        .align_items(Alignment::Stretch)
        .align_content(FlexAlignContent::Start)
        .with_item(left, control_story_flex_item_with_max_width(max_width))
        .with_item(right, control_story_flex_item_with_max_width(max_width))
}

fn control_story_flex_item_with_max_width(max_width: f32) -> FlexItem {
    FlexItem::new()
        .grow(1.0)
        .basis_gap_aware_fraction(0.5)
        .min_width(CONTROL_STORY_CARD_MIN_WIDTH)
        .max_width(max_width)
}

struct StoryCard {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    padding: Insets,
    child: SingleChild,
}

impl StoryCard {
    fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            padding: Insets::all(14.0),
            child: SingleChild::new(child),
        }
    }

    fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for StoryCard {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child_constraints = Constraints::new(
            Size::new(
                (constraints.min.width - self.padding.left - self.padding.right).max(0.0),
                (constraints.min.height - self.padding.top - self.padding.bottom).max(0.0),
            ),
            Size::new(
                (constraints.max.width - self.padding.left - self.padding.right).max(0.0),
                (constraints.max.height - self.padding.top - self.padding.bottom).max(0.0),
            ),
        );
        let child_size = self.child.measure(ctx, child_constraints);
        constraints.clamp(Size::new(
            child_size.width + self.padding.left + self.padding.right,
            child_size.height + self.padding.top + self.padding.bottom,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let measured = self.child.child().measured_size();
        let child_bounds = Rect::new(
            bounds.x() + self.padding.left,
            bounds.y() + self.padding.top,
            (bounds.width() - self.padding.left - self.padding.right)
                .max(0.0)
                .min(measured.width),
            (bounds.height() - self.padding.top - self.padding.bottom)
                .max(0.0)
                .min(measured.height),
        );
        self.child.arrange(ctx, child_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        ctx.fill(Path::rounded_rect(bounds, 8.0), palette.surface_raised);
        ctx.stroke(
            Path::rounded_rect(bounds, 8.0),
            palette.border,
            StrokeStyle::new(1.0),
        );
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct CenteredContentWidth {
    max_width: f32,
    content_width: f32,
    child: SingleChild,
}

impl CenteredContentWidth {
    fn new<W>(max_width: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            max_width: max_width.max(1.0),
            content_width: 0.0,
            child: SingleChild::new(child),
        }
    }
}

impl Widget for CenteredContentWidth {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let outer_width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            self.max_width.max(constraints.min.width)
        };
        self.content_width = outer_width.min(self.max_width).max(0.0);
        let child_size = self.child.measure(
            ctx,
            Constraints::new(
                Size::new(self.content_width, 0.0),
                Size::new(self.content_width, constraints.max.height),
            ),
        );
        constraints.clamp(Size::new(outer_width, child_size.height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let measured = self.child.child().measured_size();
        let content_width = self.content_width.min(bounds.width());
        self.child.arrange(
            ctx,
            Rect::new(
                bounds.x() + ((bounds.width() - content_width) * 0.5).max(0.0),
                bounds.y(),
                content_width,
                measured.height.min(bounds.height()),
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct FilterableSection {
    shell_state: Rc<RefCell<WidgetBookShellState>>,
    search_text: String,
    trailing_gap: f32,
    visible: bool,
    child: SingleChild,
}

impl FilterableSection {
    fn new<W>(
        shell_state: Rc<RefCell<WidgetBookShellState>>,
        category: WidgetBookCategory,
        search_text: impl Into<String>,
        child: W,
    ) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            shell_state,
            search_text: format!("{} {}", category.label(), search_text.into()),
            trailing_gap: WIDGET_BOOK_SECTION_GAP,
            visible: true,
            child: SingleChild::new(child),
        }
    }

    fn current_visibility(&self) -> bool {
        self.shell_state.borrow().section_matches(&self.search_text)
    }
}

impl Widget for FilterableSection {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.visible = self.current_visibility();
        if !self.visible {
            let width = if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                constraints.min.width
            };
            return constraints.clamp(Size::new(width, 0.0));
        }
        let child_size = self.child.measure(ctx, constraints);
        constraints.clamp(Size::new(
            child_size.width,
            child_size.height + self.trailing_gap,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if self.visible {
            let child_height = self
                .child
                .child()
                .measured_size()
                .height
                .min(bounds.height());
            self.child.arrange(
                ctx,
                Rect::new(bounds.x(), bounds.y(), bounds.width(), child_height),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if self.visible {
            self.child.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if self.visible {
            self.child.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if self.visible {
            self.child.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.visible {
            self.child.visit_children_mut(visitor);
        }
    }
}

struct WidgetBookShell {
    theme_reader: WidgetBookThemeReader,
    rail: SingleChild,
    content: SingleChild,
    rail_width: f32,
    rail_visible: bool,
}

impl WidgetBookShell {
    fn new<Rail, Content>(theme_reader: WidgetBookThemeReader, rail: Rail, content: Content) -> Self
    where
        Rail: Widget + 'static,
        Content: Widget + 'static,
    {
        Self {
            theme_reader,
            rail: SingleChild::new(rail),
            content: SingleChild::new(content),
            rail_width: 0.0,
            rail_visible: true,
        }
    }
}

impl Widget for WidgetBookShell {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(1280.0)
        };
        let height = if constraints.max.height.is_finite() {
            constraints.max.height
        } else {
            constraints.min.height.max(760.0)
        };
        self.rail_visible = width >= WIDGET_BOOK_RAIL_BREAKPOINT;
        self.rail_width = if self.rail_visible {
            WIDGET_BOOK_RAIL_WIDTH.min(width * 0.28)
        } else {
            0.0
        };

        if self.rail_visible {
            self.rail
                .measure(ctx, Constraints::tight(Size::new(self.rail_width, height)));
        }
        let content_width = (width - self.rail_width - self.rail_visible as u8 as f32).max(0.0);
        self.content
            .measure(ctx, Constraints::tight(Size::new(content_width, height)));

        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if self.rail_visible {
            self.rail.arrange(
                ctx,
                Rect::new(bounds.x(), bounds.y(), self.rail_width, bounds.height()),
            );
        }
        let divider = self.rail_visible as u8 as f32;
        self.content.arrange(
            ctx,
            Rect::new(
                bounds.x() + self.rail_width + divider,
                bounds.y(),
                (bounds.width() - self.rail_width - divider).max(0.0),
                bounds.height(),
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        ctx.fill_rect(bounds, theme.palette.surface);
        if self.rail_visible {
            ctx.fill_rect(
                Rect::new(bounds.x(), bounds.y(), self.rail_width, bounds.height()),
                theme.palette.surface,
            );
            ctx.fill_rect(
                Rect::new(
                    bounds.x() + self.rail_width,
                    bounds.y(),
                    1.0,
                    bounds.height(),
                ),
                theme.palette.border,
            );
        }
        if self.rail_visible {
            self.rail.paint(ctx);
        }
        self.content.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(WIDGET_BOOK_SHELL_NAME.to_string());
        ctx.push(node);
        if self.rail_visible {
            self.rail.semantics(ctx);
        }
        self.content.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if self.rail_visible {
            self.rail.visit_children(visitor);
        }
        self.content.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.rail_visible {
            self.rail.visit_children_mut(visitor);
        }
        self.content.visit_children_mut(visitor);
    }
}

struct NamedSection {
    name: String,
    content: SingleChild,
}

impl NamedSection {
    fn new(name: impl Into<String>, content: impl Widget + 'static) -> Self {
        Self {
            name: name.into(),
            content: SingleChild::new(content),
        }
    }
}

impl Widget for NamedSection {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.content.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        ctx.push(node);
        self.content.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
    }
}

fn theme_preview_card(
    theme: DefaultTheme,
    title: &'static str,
    action_label: &'static str,
    input_label: &'static str,
) -> impl Widget {
    let theme_name = theme.colors.name.replace('-', " ");
    let body = Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch)
        .with_child(Label::new(format!("{title} theme")).style(demo_text_style(
            theme,
            DemoTextRole::Emphasis,
            theme.palette.text,
        )))
        .with_child(MaximumWidth::new(
            520.0,
            Label::new(format!(
                "{} base surface with {} accent for primary actions.",
                theme_name, theme_name
            ))
            .style(demo_text_style(
                theme,
                DemoTextRole::Supporting,
                theme.palette.placeholder,
            )),
        ))
        .with_child(
            TextInput::new(input_label)
                .placeholder("Find layer, panel, or asset")
                .theme(theme),
        )
        .with_child(Button::primary(action_label).theme(theme))
        .with_child(
            Switch::new(format!("{title} preview live updates"))
                .on(true)
                .theme(theme),
        )
        .with_child(
            Stack::horizontal()
                .spacing(10.0)
                .alignment(Alignment::Center)
                .with_child(
                    ColorSwatch::new(format!("{title} base swatch"), theme.colors.base_200)
                        .size(Size::new(58.0, 28.0)),
                )
                .with_child(
                    ColorSwatch::new(format!("{title} primary swatch"), theme.colors.primary)
                        .size(Size::new(58.0, 28.0)),
                )
                .with_child(
                    ColorSwatch::new(format!("{title} secondary swatch"), theme.colors.secondary)
                        .size(Size::new(58.0, 28.0)),
                ),
        );

    ThemePreviewCardFrame::new(theme, body)
}

struct ThemePreviewCardFrame {
    theme: DefaultTheme,
    padding: Insets,
    child: SingleChild,
}

impl ThemePreviewCardFrame {
    fn new<W>(theme: DefaultTheme, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme,
            padding: Insets::all(16.0),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for ThemePreviewCardFrame {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child_constraints = Constraints::new(
            Size::new(
                (constraints.min.width - self.padding.left - self.padding.right).max(0.0),
                (constraints.min.height - self.padding.top - self.padding.bottom).max(0.0),
            ),
            Size::new(
                (constraints.max.width - self.padding.left - self.padding.right).max(0.0),
                (constraints.max.height - self.padding.top - self.padding.bottom).max(0.0),
            ),
        );
        let child_size = self.child.measure(ctx, child_constraints);
        constraints.clamp(Size::new(
            child_size.width + self.padding.left + self.padding.right,
            child_size.height + self.padding.top + self.padding.bottom,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let measured = self.child.child().measured_size();
        let child_bounds = Rect::new(
            bounds.x() + self.padding.left,
            bounds.y() + self.padding.top,
            (bounds.width() - self.padding.left - self.padding.right)
                .max(0.0)
                .min(measured.width),
            (bounds.height() - self.padding.top - self.padding.bottom)
                .max(0.0)
                .min(measured.height),
        );
        self.child.arrange(ctx, child_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let border = self.theme.palette.border.with_alpha(0.92);
        let background = self.theme.palette.surface_raised;
        ctx.fill(Path::rounded_rect(bounds, 10.0), background);
        ctx.stroke(
            Path::rounded_rect(bounds, 10.0),
            border,
            StrokeStyle::new(1.0),
        );
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct WidgetBookSummary {
    theme_reader: WidgetBookThemeReader,
    state: Rc<RefCell<WidgetBookState>>,
    last_seen_state: WidgetBookState,
}

impl WidgetBookSummary {
    fn new(state: Rc<RefCell<WidgetBookState>>, theme_reader: WidgetBookThemeReader) -> Self {
        let last_seen_state = state.borrow().clone();
        Self {
            theme_reader,
            state,
            last_seen_state,
        }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        (self.theme_reader)()
    }
}

struct LivePerformancePanel {
    display: Rc<RefCell<LivePerformanceDisplay>>,
}

impl LivePerformancePanel {
    const WIDTH: f32 = 340.0;
    const HEIGHT: f32 = 162.0;
    const PADDING_X: f32 = 12.0;
    const PADDING_Y: f32 = 10.0;
    const CORNER_RADIUS: f32 = 8.0;
    const HEADER_HEIGHT: f32 = 36.0;
    const GRAPH_HEIGHT: f32 = 72.0;
    const LEGEND_HEIGHT: f32 = 28.0;
    const BAR_GAP: f32 = 1.0;

    #[cfg(test)]
    fn new() -> Self {
        Self::with_display(Rc::new(RefCell::new(LivePerformanceDisplay::default())))
    }

    fn with_display(display: Rc<RefCell<LivePerformanceDisplay>>) -> Self {
        Self { display }
    }

    fn caption_text_style(color: Color) -> TextStyle {
        let theme = DefaultTheme::default();
        widget_book_theme_text_style(theme, theme.text.xs, color)
    }

    fn headline_text_style(color: Color) -> TextStyle {
        let theme = DefaultTheme::default();
        widget_book_theme_text_style(theme, theme.text._2xl, color)
    }

    fn graph_bounds(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + Self::PADDING_X,
            bounds.y() + Self::PADDING_Y + Self::HEADER_HEIGHT,
            (bounds.width() - Self::PADDING_X * 2.0).max(1.0),
            Self::GRAPH_HEIGHT,
        )
    }

    fn legend_bounds(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + Self::PADDING_X,
            bounds.max_y() - Self::PADDING_Y - Self::LEGEND_HEIGHT,
            (bounds.width() - Self::PADDING_X * 2.0).max(1.0),
            Self::LEGEND_HEIGHT,
        )
    }

    fn frame_cost_scale(samples: &[LivePerformanceFrameSample]) -> f32 {
        samples
            .iter()
            .map(|sample| sample.total_time_ms)
            .fold(16.67, f32::max)
            .clamp(16.67, 66.67)
    }

    fn stage_short_label(phase: FramePhase) -> &'static str {
        match phase {
            FramePhase::Event => "evt",
            FramePhase::Redraw => "redraw",
            FramePhase::MeasureArrange => "layout",
            FramePhase::HitTest => "hit",
            FramePhase::Paint => "paint",
            FramePhase::Semantics => "a11y",
            FramePhase::Renderer => "rend",
            FramePhase::SurfaceWait => "wait",
            FramePhase::Diagnostics => "diag",
        }
    }

    fn stage_color(phase: FramePhase, alpha: f32) -> Color {
        let alpha = alpha.clamp(0.0, 1.0);
        match phase {
            FramePhase::Event => Color::rgba(0.24, 0.78, 0.68, alpha),
            FramePhase::Redraw => Color::rgba(0.50, 0.68, 0.95, alpha),
            FramePhase::MeasureArrange => Color::rgba(0.96, 0.68, 0.30, alpha),
            FramePhase::HitTest => Color::rgba(0.68, 0.56, 0.92, alpha),
            FramePhase::Paint => Color::rgba(0.94, 0.42, 0.54, alpha),
            FramePhase::Semantics => Color::rgba(0.72, 0.82, 0.36, alpha),
            FramePhase::Renderer => Color::rgba(0.36, 0.78, 0.96, alpha),
            FramePhase::SurfaceWait => Color::rgba(0.68, 0.72, 0.78, alpha),
            FramePhase::Diagnostics => Color::rgba(0.78, 0.80, 0.86, alpha),
        }
    }

    fn paint_budget_line(ctx: &mut PaintCtx, graph: Rect, scale_ms: f32, budget_ms: f32) {
        if budget_ms > scale_ms {
            return;
        }

        let y = graph.max_y() - graph.height() * (budget_ms / scale_ms);
        let mut path = Path::builder();
        path.move_to(Point::new(graph.x(), y));
        path.line_to(Point::new(graph.max_x(), y));
        ctx.stroke(
            path.build(),
            Color::rgba(0.98, 1.0, 1.0, 0.28),
            StrokeStyle::new(1.0),
        );
    }

    fn paint_graph(&self, ctx: &mut PaintCtx, display: &LivePerformanceDisplay, graph: Rect) {
        ctx.fill_rect(graph, Color::rgba(0.0, 0.0, 0.0, 0.24));
        ctx.stroke_rect(
            graph,
            Color::rgba(0.98, 1.0, 1.0, 0.22),
            StrokeStyle::new(1.0),
        );

        let scale_ms = Self::frame_cost_scale(&display.samples);
        Self::paint_budget_line(ctx, graph, scale_ms, 16.67);
        Self::paint_budget_line(ctx, graph, scale_ms, 33.33);

        if display.samples.is_empty() {
            let style = Self::caption_text_style(Color::rgba(0.92, 0.96, 1.0, 0.72));
            paint_single_line_aligned_text(
                ctx,
                graph,
                "waiting for frames",
                &style,
                style.line_height,
                0.5,
            );
            return;
        }

        let slot_width = (graph.width() / LIVE_PERFORMANCE_HISTORY_LIMIT as f32).max(2.0);
        let bar_width = (slot_width - Self::BAR_GAP).max(1.0);
        let visible_count = ((graph.width() / slot_width).floor() as usize)
            .min(display.samples.len())
            .max(1);
        let samples = &display.samples[display.samples.len() - visible_count..];
        let start_x = graph.max_x() - slot_width * samples.len() as f32;

        ctx.push_clip_rect(graph);
        for (sample_index, sample) in samples.iter().enumerate() {
            let x = start_x + sample_index as f32 * slot_width;
            let mut y = graph.max_y();
            for phase in LIVE_PERFORMANCE_GRAPH_PHASES {
                let duration = sample.stage_costs[frame_phase_index(phase)];
                if duration <= 0.0 {
                    continue;
                }

                let height = (graph.height() * (duration / scale_ms)).max(0.5);
                y = (y - height).max(graph.y());
                ctx.fill_rect(
                    Rect::new(x, y, bar_width, (graph.max_y() - y).min(height)),
                    Self::stage_color(phase, 0.88),
                );
            }
        }
        ctx.pop_clip();

        let scale_style = Self::caption_text_style(Color::rgba(0.92, 0.96, 1.0, 0.62));
        let scale_label = format!("{scale_ms:.0} ms");
        paint_single_line_aligned_text(
            ctx,
            Rect::new(
                graph.x() + 4.0,
                graph.y() + 2.0,
                48.0,
                scale_style.line_height,
            ),
            &scale_label,
            &scale_style,
            scale_style.line_height,
            0.0,
        );
        paint_single_line_aligned_text(
            ctx,
            Rect::new(
                graph.x() + 4.0,
                graph.max_y() - scale_style.line_height - 2.0,
                56.0,
                scale_style.line_height,
            ),
            "16.7 ms",
            &scale_style,
            scale_style.line_height,
            0.0,
        );
    }

    fn paint_legend(&self, ctx: &mut PaintCtx, bounds: Rect) {
        let mut x = bounds.x();
        let label_style = Self::caption_text_style(Color::rgba(0.94, 0.97, 1.0, 0.74));
        let y = bounds.y() + (bounds.height() - label_style.line_height) * 0.5;
        for phase in LIVE_PERFORMANCE_GRAPH_PHASES {
            let label = Self::stage_short_label(phase);
            let label_width = match phase {
                FramePhase::MeasureArrange => 38.0,
                FramePhase::Redraw => 42.0,
                FramePhase::Renderer | FramePhase::SurfaceWait => 34.0,
                _ => 30.0,
            };
            if x + label_width > bounds.max_x() {
                break;
            }

            ctx.fill_rect(
                Rect::new(x, y + 4.0, 7.0, 7.0),
                Self::stage_color(phase, 0.95),
            );
            paint_single_line_aligned_text(
                ctx,
                Rect::new(x + 10.0, y, label_width - 10.0, label_style.line_height),
                label,
                &label_style,
                label_style.line_height,
                0.0,
            );
            x += label_width;
        }
    }

    fn snapshot_phase_duration(snapshot: &WindowPerformanceSnapshot, phase: FramePhase) -> f64 {
        snapshot
            .phase_timings
            .iter()
            .filter(|sample| sample.phase == phase)
            .map(|sample| sample.duration_ms)
            .sum()
    }
}

const LIVE_PERFORMANCE_GRAPH_PHASES: [FramePhase; LIVE_PERFORMANCE_STAGE_COUNT] = [
    FramePhase::Event,
    FramePhase::Redraw,
    FramePhase::MeasureArrange,
    FramePhase::HitTest,
    FramePhase::Paint,
    FramePhase::Semantics,
    FramePhase::Renderer,
    FramePhase::SurfaceWait,
    FramePhase::Diagnostics,
];

impl Widget for LivePerformancePanel {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width.min(Self::WIDTH)
        } else {
            Self::WIDTH
        };
        constraints.clamp(Size::new(width, Self::HEIGHT))
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Overlay,
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let display = self.display.borrow().clone();
        let frame = rounded_rect_path(ctx.bounds(), Self::CORNER_RADIUS);
        ctx.fill(frame.clone(), Color::rgba(0.015, 0.025, 0.035, 0.50));
        ctx.stroke(
            frame,
            Color::rgba(0.98, 1.0, 1.0, 0.18),
            StrokeStyle::new(1.0),
        );

        let header_y = ctx.bounds().y() + Self::PADDING_Y;
        let (fps_text, frame_text, slowest_text) = if let Some(snapshot) = &display.snapshot {
            let fps = if display.idle {
                "0 fps".to_string()
            } else {
                format_fps(snapshot.total_time_ms)
            };
            let frame = if display.idle {
                "idle".to_string()
            } else {
                format!(
                    "frame {} | {}",
                    snapshot.frame_index,
                    format_duration_ms(snapshot.total_time_ms)
                )
            };
            let renderer_work_ms = Self::snapshot_phase_duration(snapshot, FramePhase::Renderer);
            let surface_wait_ms = Self::snapshot_phase_duration(snapshot, FramePhase::SurfaceWait);
            let slowest = if renderer_work_ms > 0.0 || surface_wait_ms > 0.0 {
                format!(
                    "rend {} | wait {}",
                    format_duration_ms(renderer_work_ms),
                    format_duration_ms(surface_wait_ms),
                )
            } else {
                snapshot
                    .slowest_phase()
                    .map(|sample| {
                        format!(
                            "{} {}",
                            Self::stage_short_label(sample.phase),
                            format_duration_ms(sample.duration_ms)
                        )
                    })
                    .unwrap_or_else(|| "waiting for phases".to_string())
            };
            (fps, frame, slowest)
        } else {
            (
                "0 fps".to_string(),
                "waiting".to_string(),
                "waiting for first frame".to_string(),
            )
        };

        let fps_style = Self::headline_text_style(Color::rgba(0.98, 1.0, 1.0, 0.96));
        paint_single_line_aligned_text(
            ctx,
            Rect::new(
                ctx.bounds().x() + Self::PADDING_X,
                header_y,
                118.0,
                Self::HEADER_HEIGHT,
            ),
            &fps_text,
            &fps_style,
            fps_style.line_height,
            0.0,
        );
        let detail_style = Self::caption_text_style(Color::rgba(0.92, 0.96, 1.0, 0.76));
        paint_single_line_aligned_text(
            ctx,
            Rect::new(
                ctx.bounds().x() + 136.0,
                header_y,
                ctx.bounds().width() - 148.0,
                detail_style.line_height,
            ),
            &frame_text,
            &detail_style,
            detail_style.line_height,
            0.0,
        );
        let muted_detail_style = Self::caption_text_style(Color::rgba(0.92, 0.96, 1.0, 0.66));
        paint_single_line_aligned_text(
            ctx,
            Rect::new(
                ctx.bounds().x() + 136.0,
                header_y + detail_style.line_height,
                ctx.bounds().width() - 148.0,
                muted_detail_style.line_height,
            ),
            &slowest_text,
            &muted_detail_style,
            muted_detail_style.line_height,
            0.0,
        );

        self.paint_graph(ctx, &display, Self::graph_bounds(ctx.bounds()));
        self.paint_legend(ctx, Self::legend_bounds(ctx.bounds()));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let display = self.display.borrow();
        let value = display
            .snapshot
            .as_ref()
            .map(|snapshot| {
                format!(
                    "{} | {} | {} samples",
                    if display.idle {
                        "0 fps".to_string()
                    } else {
                        format_fps(snapshot.total_time_ms)
                    },
                    format_duration_ms(snapshot.total_time_ms),
                    display.samples.len()
                )
            })
            .unwrap_or_else(|| "waiting for frames".to_string());
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some("Live performance overlay".to_string());
        node.description =
            Some("Transparent FPS overlay with rolling stacked frame phase costs.".to_string());
        node.value = Some(SemanticsValue::Text(value));
        ctx.push(node);
    }
}

fn rounded_rect_path(rect: Rect, radius: f32) -> Path {
    Path::rounded_rect(rect, radius.min(rect.width().min(rect.height()) * 0.5))
}

fn format_fps(total_time_ms: f64) -> String {
    if total_time_ms <= 0.0 {
        "idle".to_string()
    } else {
        format!("{:.0} fps", 1000.0 / total_time_ms)
    }
}

fn format_duration_ms(duration_ms: f64) -> String {
    format!("{duration_ms:.1} ms")
}

impl Widget for WidgetBookSummary {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !matches!(event, Event::Window(WindowEvent::RedrawRequested)) {
            return;
        }

        let current_state = self.state.borrow().clone();
        if current_state != self.last_seen_state {
            self.last_seen_state = current_state;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            320.0
        };
        constraints.clamp(Size::new(width, 270.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let state = self.state.borrow();
        let lines = [
            if state.name.trim().is_empty() {
                "Hello, stranger".to_string()
            } else {
                format!("Hello, {}", state.name)
            },
            format!(
                "buttons: primary={} icon={}",
                state.button_presses, state.icon_button_presses
            ),
            format!(
                "password: {} chars | scheduled: {}",
                state.password.chars().count(),
                if state.scheduled_for.is_empty() {
                    "unset"
                } else {
                    state.scheduled_for.as_str()
                }
            ),
            format!(
                "subscription: {} | snapping: {}",
                if state.subscribed { "on" } else { "off" },
                if state.switch_on { "on" } else { "off" }
            ),
            format!(
                "radio: standalone={} group={}",
                if state.standalone_radio_selected {
                    "selected"
                } else {
                    "idle"
                },
                if state.radio_choice.is_empty() {
                    "unset"
                } else {
                    state.radio_choice.as_str()
                }
            ),
            format!(
                "opacity: {:.0} | brush size: {:.0} | mode: {}",
                state.slider_value,
                state.number_value,
                if state.mode.is_empty() {
                    "unset"
                } else {
                    state.mode.as_str()
                }
            ),
            format!(
                "tabs: bar={} panel={}",
                if state.tab_bar_choice.is_empty() {
                    "unset"
                } else {
                    state.tab_bar_choice.as_str()
                },
                if state.tabs_choice.is_empty() {
                    "unset"
                } else {
                    state.tabs_choice.as_str()
                }
            ),
            format!(
                "menu: {} | context: {} | dialog apply: {}",
                if state.last_menu_action.is_empty() {
                    "idle"
                } else {
                    state.last_menu_action.as_str()
                },
                if state.last_context_action.is_empty() {
                    "idle"
                } else {
                    state.last_context_action.as_str()
                },
                state.dialog_apply_count,
            ),
            format!("notes lines: {}", state.notes.lines().count().max(1)),
        ];

        ctx.fill_bounds(palette.surface_raised);
        ctx.stroke_bounds(
            palette.border,
            StrokeStyle::new(theme.metrics.border_width.max(1.0)),
        );
        for (index, line) in lines.into_iter().enumerate() {
            ctx.label(
                Rect::new(
                    ctx.bounds().x() + 14.0,
                    ctx.bounds().y() + 14.0 + (index as f32 * 28.0),
                    ctx.bounds().width() - 28.0,
                    22.0,
                ),
                line,
                if index == 0 {
                    palette.text
                } else {
                    palette.text_muted
                },
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let state = self.state.borrow();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(SUMMARY_NAME.to_string());
        node.description = Some(format!(
            "name: {}; password length: {}; scheduled for: {}; subscription: {}; button presses: {}; icon actions: {}; switch: {}; standalone radio: {}; radio choice: {}; slider: {:.0}; brush size: {:.0}; mode: {}; tab bar: {}; tabs: {}; menu: {}; context menu: {}; dialog apply: {}; notes lines: {}",
            if state.name.is_empty() {
                "stranger"
            } else {
                state.name.as_str()
            },
            state.password.chars().count(),
            if state.scheduled_for.is_empty() {
                "unset"
            } else {
                state.scheduled_for.as_str()
            },
            if state.subscribed { "on" } else { "off" },
            state.button_presses,
            state.icon_button_presses,
            if state.switch_on { "on" } else { "off" },
            if state.standalone_radio_selected {
                "selected"
            } else {
                "off"
            },
            if state.radio_choice.is_empty() {
                "unset"
            } else {
                state.radio_choice.as_str()
            },
            state.slider_value,
            state.number_value,
            if state.mode.is_empty() {
                "unset"
            } else {
                state.mode.as_str()
            },
            if state.tab_bar_choice.is_empty() {
                "unset"
            } else {
                state.tab_bar_choice.as_str()
            },
            if state.tabs_choice.is_empty() {
                "unset"
            } else {
                state.tabs_choice.as_str()
            },
            if state.last_menu_action.is_empty() {
                "idle"
            } else {
                state.last_menu_action.as_str()
            },
            if state.last_context_action.is_empty() {
                "idle"
            } else {
                state.last_context_action.as_str()
            },
            state.dialog_apply_count,
            state.notes.lines().count().max(1),
        ));
        ctx.push(node);
    }
}

fn option_index(options: &[&str], value: &str) -> Option<usize> {
    options.iter().position(|option| *option == value)
}

#[cfg(test)]
mod tests;
