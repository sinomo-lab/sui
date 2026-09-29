//! The HDR validation page: one scroll showing what the window's output does
//! with light above SDR white and colors outside sRGB, beside the controls
//! that change it and a capture that records it.
//!
//! Every probe states what it should look like on the current output, so a
//! reader can tell a correct result from a broken one without knowing the
//! renderer.

#![forbid(unsafe_code)]

mod capture;
mod live;
mod probes;
pub(crate) mod report;
mod ui_modes;

#[cfg(test)]
mod tests;

use sui::prelude::*;
use sui::{GridTrack, WindowOutputDiagnostics};

#[cfg(test)]
use capture::{CAPTURE_BUTTON_LABEL, CAPTURE_STATUS_NAME, COPY_REPORT_BUTTON_LABEL};
pub(crate) use live::OutputSummary;
use live::{LiveText, WAITING_FOR_OUTPUT, expect, sdr_content_brightness_line};
#[cfg(test)]
use probes::{
    CHROMATICITY_NAME, GAMUT_TILES_NAME, HEADROOM_RAMP_NAME, HIGHLIGHT_CURVE_NAME, HUE_GRID_NAME,
    RAMPS_NAME,
};

use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader};
use crate::demo_support::*;
use crate::live_performance::LivePerformanceRoot;
use crate::settings::{RenderOptions, RenderOptionsScope, controls};

pub const COLOR_VALIDATION_VIEW_TITLE: &str = "SUI HDR and Color Validation";
pub const COLOR_VALIDATION_SCROLL_NAME: &str = "Color validation scroll";
pub const COLOR_VALIDATION_VERTICAL_SCROLL_BAR_NAME: &str = "Color validation vertical scroll bar";
pub const COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME: &str =
    "Color validation horizontal scroll bar";

pub(crate) const OUTPUT_SECTION_NAME: &str = "Output";
pub(crate) const OUTPUT_VERDICT_NAME: &str = "Output verdict";
pub(crate) const HEADROOM_LADDER_NAME: &str = "White ladder";
pub(crate) const NEAR_WHITE_NAME: &str = "Near SDR white";

const PAGE_MIN_WIDTH: f32 = 1000.0;
const CONTROL_WIDTH: f32 = 220.0;

/// The page for a window of its own: its output options start as the
/// window's.
pub fn build_color_validation_surface() -> impl Widget {
    build_color_validation_surface_with_theme(default_theme_reader())
}

pub fn build_color_validation_surface_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    build_hdr_validation_surface(theme_reader, RenderOptions::from_window())
}

pub fn build_color_validation_application() -> Application {
    App::new()
        .window(Window::new(COLOR_VALIDATION_VIEW_TITLE).root(
            LivePerformanceRoot::new(
                COLOR_VALIDATION_VIEW_TITLE,
                "What this window's output does with light above SDR white and colors outside sRGB.",
                build_color_validation_surface(),
            ),
        ))
        .into_application()
}

/// The page, editing `options`.
pub(crate) fn build_hdr_validation_surface(
    theme_reader: DevThemeReader,
    options: RenderOptions,
) -> impl Widget {
    let scroll_state = ScrollState::new();
    let content = MinimumWidth::new(
        PAGE_MIN_WIDTH,
        Padding::all(
            24.0,
            Stack::vertical()
                .spacing(18.0)
                .alignment(Alignment::Stretch)
                .with_child(output_section(&theme_reader, &options))
                .with_child(headroom_section(&theme_reader))
                .with_child(highlight_section(&theme_reader))
                .with_child(gamut_section(&theme_reader))
                .with_child(ramps_section(&theme_reader))
                .with_child(ui_modes_section(&theme_reader))
                .with_child(capture_section(&theme_reader)),
        ),
    );

    RenderOptionsScope::new(
        options,
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
        ),
    )
}

fn section<W>(
    theme_reader: &DevThemeReader,
    name: &'static str,
    subtitle: &'static str,
    body: W,
) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    NamedSection::new(
        name,
        panel_with_theme(std::rc::Rc::clone(theme_reader), name, subtitle, body),
    )
}

fn body() -> Stack {
    Stack::vertical().spacing(14.0).alignment(Alignment::Start)
}

fn expectation(
    theme_reader: &DevThemeReader,
    text: fn(Option<OutputSummary>) -> String,
) -> impl Widget + use<> {
    MaximumWidth::new(
        GALLERY_TEXT_MAX_WIDTH,
        LiveText::new(
            theme_reader,
            DemoTextRole::Body,
            DemoTextColor::Text,
            move |diagnostics| text(diagnostics.map(OutputSummary::of)),
        ),
    )
}

fn output_details(diagnostics: Option<&WindowOutputDiagnostics>) -> String {
    let Some(diagnostics) = diagnostics else {
        return WAITING_FOR_OUTPUT.to_string();
    };
    let capabilities = &diagnostics.display_capabilities;
    let yes_no = |value: bool| if value { "yes" } else { "no" };
    format!(
        "Display: wide gamut {}, HDR {}, native HDR presentation {}; prefers {:?}, {:?}. {}. Strategy: {:?}. {}",
        yes_no(capabilities.supports_wide_gamut),
        yes_no(capabilities.supports_hdr),
        yes_no(capabilities.native_hdr_presentation_supported),
        capabilities.preferred_primaries,
        capabilities.preferred_dynamic_range,
        sdr_content_brightness_line(diagnostics),
        diagnostics.active_output_strategy,
        capabilities.notes,
    )
}

/// `control` under its `label`, in the output section's grid.
fn control_row<W>(theme_reader: &DevThemeReader, label: &'static str, control: W) -> PropertyRow
where
    W: Widget + 'static,
{
    controls::labeled_control(theme_reader, label, CONTROL_WIDTH, control)
}

fn output_section(theme_reader: &DevThemeReader, options: &RenderOptions) -> impl Widget + use<> {
    let controls = Grid::new([GridTrack::Fraction(1.0), GridTrack::Fraction(1.0)])
        .rows([
            GridTrack::Auto,
            GridTrack::Auto,
            GridTrack::Auto,
            GridTrack::Auto,
        ])
        .column_gap(24.0)
        .row_gap(8.0)
        .with_child(control_row(
            theme_reader,
            controls::COLOR_MANAGEMENT_MODE_NAME,
            controls::color_management_select(theme_reader, options),
        ))
        .with_child(control_row(
            theme_reader,
            controls::OUTPUT_PRIMARIES_NAME,
            controls::output_primaries_select(theme_reader, options),
        ))
        .with_child(control_row(
            theme_reader,
            controls::DYNAMIC_RANGE_MODE_NAME,
            controls::dynamic_range_select(theme_reader, options),
        ))
        .with_child(control_row(
            theme_reader,
            controls::TONE_MAPPING_MODE_NAME,
            controls::tone_mapping_select(theme_reader, options),
        ))
        .with_child(control_row(
            theme_reader,
            controls::SDR_CONTENT_BRIGHTNESS_NAME,
            controls::sdr_content_brightness_input(theme_reader, options),
        ))
        .with_child(control_row(
            theme_reader,
            controls::HDR_THEME_MODE_NAME,
            controls::hdr_theme_mode_select(theme_reader),
        ))
        .with_child(controls::system_sdr_brightness_switch(
            theme_reader,
            options,
        ));

    section(
        theme_reader,
        OUTPUT_SECTION_NAME,
        "What this window's output does with light above SDR white and with colors outside sRGB. These controls change it for this window, and the HDR theme mode widgets preview; Settings edits the same options.",
        body()
            .alignment(Alignment::Stretch)
            .with_child(NamedSection::new(
                OUTPUT_VERDICT_NAME,
                MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    LiveText::new(
                        theme_reader,
                        DemoTextRole::Emphasis,
                        DemoTextColor::Text,
                        |diagnostics| {
                            diagnostics.map_or_else(
                                || WAITING_FOR_OUTPUT.to_string(),
                                |diagnostics| OutputSummary::of(diagnostics).headline(),
                            )
                        },
                    ),
                ),
            ))
            .with_child(controls)
            .with_child(MaximumWidth::new(
                GALLERY_TEXT_MAX_WIDTH,
                LiveText::new(
                    theme_reader,
                    DemoTextRole::Metadata,
                    DemoTextColor::Muted,
                    output_details,
                ),
            )),
    )
}

fn headroom_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let mut ladder = probes::SwatchStrip::new(HEADROOM_LADDER_NAME, theme_reader);
    for multiple in [1.0_f32, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0] {
        ladder = ladder.swatch(
            format!("White {multiple}×"),
            format!("{multiple}×"),
            Color::linear_rgba(multiple, multiple, multiple, 1.0),
        );
    }
    let mut near_white = probes::SwatchStrip::new(NEAR_WHITE_NAME, theme_reader);
    for multiple in [0.9_f32, 0.95, 1.0, 1.05, 1.1, 1.25] {
        near_white = near_white.swatch(
            format!("White {multiple}×"),
            format!("{multiple}×"),
            Color::linear_rgba(multiple, multiple, multiple, 1.0),
        );
    }
    section(
        theme_reader,
        "Brightness headroom",
        "White from a quarter of SDR white to 16 times it. HDR output shows more of the ramp as distinct steps, up to the display's peak; SDR output shows everything past SDR white as the same white.",
        body()
            .alignment(Alignment::Stretch)
            .with_child(probes::HeadroomRamp::new(theme_reader))
            .with_child(ladder)
            .with_child(near_white)
            .with_child(expectation(theme_reader, expect::headroom)),
    )
}

fn highlight_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    section(
        theme_reader,
        "Highlight fitting",
        "How colors brighter than SDR white fit an output that cannot show them. Clamp keeps each highlight's hue at full brightness; Reinhard turns brighter highlights toward white so they still read brighter. HDR output sends them as they are. Tone mapping in the Output controls switches between them.",
        body()
            .with_child(probes::HighlightCurvePlot::new(theme_reader))
            .with_child(probes::HueFitGrid::new(theme_reader))
            .with_child(expectation(theme_reader, expect::highlights)),
    )
}

fn gamut_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    section(
        theme_reader,
        "Wide gamut",
        "Display P3 colors outside sRGB. Each tile's left half is the color clipped to sRGB and its right half is the color itself, so the two only differ where the output reaches past sRGB.",
        Stack::horizontal()
            .spacing(24.0)
            .alignment(Alignment::Start)
            .with_child(probes::ChromaticityDiagram::new(theme_reader))
            .with_child(
                body()
                    .with_child(probes::GamutSplitTiles::new(theme_reader))
                    .with_child(expectation(theme_reader, expect::gamut)),
            ),
    )
}

fn ramps_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    section(
        theme_reader,
        "Gradients and banding",
        "Smooth ramps should show no steps. Faint steps in the shadow ramp are normal on 8-bit SDR output; strong steps elsewhere mean precision is lost on the way to the display.",
        body()
            .alignment(Alignment::Stretch)
            .with_child(probes::GradientRamps::new(theme_reader))
            .with_child(expectation(theme_reader, expect::ramps)),
    )
}

fn ui_modes_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    section(
        theme_reader,
        "HDR in UI",
        "The same controls under each HDR theme mode. The theme mode decides how far accents may rise above SDR white; the output decides whether the display shows it.",
        body()
            .alignment(Alignment::Stretch)
            .with_child(ui_modes::ui_mode_columns())
            .with_child(expectation(theme_reader, expect::ui_modes)),
    )
}

fn capture_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    section(
        theme_reader,
        "Capture and report",
        "Record what the renderer produced, independent of the display: the scene before output conversion and the final output as linear EXR, with luminance, headroom, and clip maps. Copy the report into bug reports; it names the requested options, what the display reported, and the strategy in use.",
        capture::CapturePanel::new(theme_reader, COLOR_VALIDATION_VIEW_TITLE),
    )
}
