//! Settings: the window's render options in sections, under rows summing up
//! what its output shows. The HDR validation page shows the display options
//! too, and both edit the same [`RenderOptions`].

pub(crate) mod controls;
mod options;
mod summary;

use std::rc::Rc;

use sui::prelude::*;
use sui::{EventCtx, HdrThemeMode};

use crate::app::{DevThemeReader, clone_dev_theme_reader};
use crate::hdr_theme_mode::set_hdr_theme_mode;
use controls::*;
use options::OptionFlag;
pub(crate) use options::{RenderOptions, RenderOptionsScope, default_render_options};
use summary::OutputSummaryRows;
#[cfg(test)]
pub(crate) use summary::{HDR_THEME_ROW_NAME, OUTPUT_ROW_NAME, SDR_WHITE_ROW_NAME};

pub(crate) const SETTINGS_TITLE: &str = "Settings";
pub(crate) const DISPLAY_SECTION_NAME: &str = "Display";
pub(crate) const TEXT_SECTION_NAME: &str = "Text";
pub(crate) const SHAPES_SECTION_NAME: &str = "Shapes";
pub(crate) const DEVELOPER_SECTION_NAME: &str = "Developer";
pub(crate) const PERFORMANCE_OVERLAY_LABEL: &str = "Performance overlay";
pub(crate) const OPEN_HDR_VALIDATION_LABEL: &str = "Open HDR validation";
pub(crate) const RESET_LABEL: &str = "Reset to defaults";
pub(crate) const SETTINGS_SCROLL_NAME: &str = "Settings controls";

const PADDING: f32 = 16.0;
const SECTION_GAP: f32 = 20.0;
const ROW_GAP: f32 = 10.0;
const LABEL_WIDTH: f32 = 150.0;

/// What Settings needs from the app around it.
pub(crate) struct SettingsHost {
    pub(crate) performance_overlay_visible: Rc<dyn Fn() -> bool>,
    pub(crate) show_performance_overlay: Rc<dyn Fn(bool)>,
    pub(crate) open_hdr_validation: Rc<dyn Fn(&mut EventCtx)>,
}

/// Settings, editing `options` for the window it is in.
pub(crate) fn settings_view(
    options: RenderOptions,
    theme_reader: &DevThemeReader,
    host: SettingsHost,
) -> impl Widget + use<> {
    let reset_options = options.clone();
    let reset = Button::new(RESET_LABEL)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .appearance(ButtonAppearance::Outline)
        .on_press(move || {
            reset_options.update(|options| *options = default_render_options());
            set_hdr_theme_mode(HdrThemeMode::Disabled);
        });
    let content = Padding::all(
        PADDING,
        Stack::vertical()
            .spacing(SECTION_GAP)
            .alignment(Alignment::Stretch)
            .with_child(display_section(theme_reader, &options, &host))
            .with_child(text_section(theme_reader, &options))
            .with_child(shapes_section(theme_reader, &options))
            .with_child(developer_section(theme_reader, &host))
            .with_child(Align::new(Alignment::End, Alignment::Start, reset)),
    );
    // The floating view fits its content to the view, so Settings scrolls
    // itself.
    RenderOptionsScope::new(
        options,
        ScrollView::vertical(content)
            .name(SETTINGS_SCROLL_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader)),
    )
}

fn display_section(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    host: &SettingsHost,
) -> PanelSection {
    let open_hdr_validation = Rc::clone(&host.open_hdr_validation);
    section(
        theme_reader,
        DISPLAY_SECTION_NAME,
        rows()
            .with_child(OutputSummaryRows::new(theme_reader))
            .with_child(row(
                theme_reader,
                COLOR_MANAGEMENT_MODE_NAME,
                color_management_select(theme_reader, options, Place::Settings),
            ))
            .with_child(row(
                theme_reader,
                OUTPUT_PRIMARIES_NAME,
                output_primaries_select(theme_reader, options, Place::Settings),
            ))
            .with_child(row(
                theme_reader,
                DYNAMIC_RANGE_MODE_NAME,
                dynamic_range_select(theme_reader, options, Place::Settings),
            ))
            .with_child(row(
                theme_reader,
                TONE_MAPPING_MODE_NAME,
                tone_mapping_select(theme_reader, options, Place::Settings),
            ))
            .with_child(row(
                theme_reader,
                SDR_CONTENT_BRIGHTNESS_NAME,
                sdr_content_brightness_input(theme_reader, options, Place::Settings),
            ))
            .with_child(system_sdr_brightness_switch(
                theme_reader,
                options,
                Place::Settings,
            ))
            .with_child(row(
                theme_reader,
                HDR_THEME_MODE_NAME,
                hdr_theme_mode_select(theme_reader, Place::Settings),
            ))
            .with_child(Align::new(
                Alignment::Start,
                Alignment::Start,
                Button::new(OPEN_HDR_VALIDATION_LABEL)
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .appearance(ButtonAppearance::Ghost)
                    .on_press_with_ctx(move |ctx| open_hdr_validation(ctx)),
            )),
    )
}

fn text_section(theme_reader: &DevThemeReader, options: &RenderOptions) -> PanelSection {
    section(
        theme_reader,
        TEXT_SECTION_NAME,
        text_controls(theme_reader, options, Place::Settings),
    )
}

/// The window's text settings, as Settings shows them and the Text rendering
/// page does beside its samples.
pub(crate) fn text_controls(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Stack {
    rows()
        .with_child(with_details(
            row(
                theme_reader,
                TEXT_COVERAGE_POLICY_NAME,
                text_coverage_policy_select(theme_reader, options, place),
            ),
            options.flag(
                "Text coverage follows a gamma curve",
                uses_text_coverage_gamma,
            ),
            rows().with_child(row(
                theme_reader,
                TEXT_COVERAGE_GAMMA_NAME,
                text_coverage_gamma_input(theme_reader, options, place),
            )),
        ))
        .with_child(with_details(
            text_hinting_switch(theme_reader, options, place),
            options.flag("Text hinting is on", uses_text_hinting),
            rows().with_child(row(
                theme_reader,
                TEXT_HINTING_MAX_PPEM_NAME,
                text_hinting_max_ppem_input(theme_reader, options, place),
            )),
        ))
        .with_child(with_details(
            stem_darkening_switch(theme_reader, options, place),
            options.flag("Stem darkening is on", uses_stem_darkening),
            rows()
                .with_child(row(
                    theme_reader,
                    STEM_DARKENING_AMOUNT_NAME,
                    stem_darkening_amount_input(theme_reader, options, place),
                ))
                .with_child(row(
                    theme_reader,
                    STEM_DARKENING_MAX_PPEM_NAME,
                    stem_darkening_max_ppem_input(theme_reader, options, place),
                )),
        ))
        .with_child(optical_centering_switch(theme_reader, options, place))
}

fn shapes_section(theme_reader: &DevThemeReader, options: &RenderOptions) -> PanelSection {
    section(
        theme_reader,
        SHAPES_SECTION_NAME,
        rows().with_child(with_details(
            feathering_switch(theme_reader, options, Place::Settings),
            options.flag("Feathering is on", |options| options.feathering_enabled),
            rows().with_child(row(
                theme_reader,
                FEATHER_WIDTH_NAME,
                feather_width_input(theme_reader, options, Place::Settings),
            )),
        )),
    )
}

fn developer_section(theme_reader: &DevThemeReader, host: &SettingsHost) -> PanelSection {
    let visible = Rc::clone(&host.performance_overlay_visible);
    let show = Rc::clone(&host.show_performance_overlay);
    section(
        theme_reader,
        DEVELOPER_SECTION_NAME,
        rows().with_child(
            Switch::new(PERFORMANCE_OVERLAY_LABEL)
                .theme_when(clone_dev_theme_reader(theme_reader))
                .on_when(move || visible())
                .on_toggle(move |on| show(on)),
        ),
    )
}

fn section(theme_reader: &DevThemeReader, title: &'static str, rows: Stack) -> PanelSection {
    PanelSection::new(title, rows)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .collapsible(true)
}

fn rows() -> Stack {
    Stack::vertical()
        .spacing(ROW_GAP)
        .alignment(Alignment::Stretch)
}

/// `control` beside its `label`.
fn row<W>(theme_reader: &DevThemeReader, label: &'static str, control: W) -> PropertyRow
where
    W: Widget + 'static,
{
    PropertyRow::new(label, control)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .inline()
        .label_width(LABEL_WIDTH)
}

/// `setting`, with `details` under it while `shown` holds: the settings that
/// only matter then.
fn with_details<W>(setting: W, shown: OptionFlag, details: Stack) -> Stack
where
    W: Widget + 'static,
{
    Stack::vertical()
        .alignment(Alignment::Stretch)
        .with_child(setting)
        .with_child(
            Presence::new(Padding::new(
                Insets {
                    top: ROW_GAP,
                    ..Insets::ZERO
                },
                details,
            ))
            .shown_from(shown)
            .collapse(Axis::Vertical),
        )
}
