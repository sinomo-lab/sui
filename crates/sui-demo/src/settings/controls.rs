//! The settings controls. Settings and the HDR validation page build them
//! from the same options, so each shows what the other changed.

use sui::prelude::*;
use sui::{
    HdrThemeMode, WindowColorManagementMode, WindowDynamicRangeMode, WindowOutputColorPrimaries,
    WindowRenderOptions, WindowStemDarkening, WindowTextCoveragePolicy, WindowTextHinting,
    WindowToneMappingMode,
};

use super::options::RenderOptions;
use crate::app::{DevThemeReader, clone_dev_theme_reader};
use crate::hdr_theme_mode::{hdr_theme_mode, set_hdr_theme_mode};

pub(crate) const COLOR_MANAGEMENT_MODE_NAME: &str = "Color management";
pub(crate) const OUTPUT_PRIMARIES_NAME: &str = "Output primaries";
pub(crate) const DYNAMIC_RANGE_MODE_NAME: &str = "Dynamic range";
pub(crate) const TONE_MAPPING_MODE_NAME: &str = "Tone mapping";
pub(crate) const SDR_CONTENT_BRIGHTNESS_NAME: &str = "SDR content brightness";
pub(crate) const USE_SYSTEM_SDR_BRIGHTNESS_LABEL: &str = "Use system SDR brightness";
pub(crate) const HDR_THEME_MODE_NAME: &str = "HDR theme mode";

pub(crate) const TEXT_COVERAGE_POLICY_NAME: &str = "Coverage policy";
pub(crate) const TEXT_COVERAGE_GAMMA_NAME: &str = "Coverage gamma";
pub(crate) const TEXT_HINTING_LABEL: &str = "Slight hinting";
pub(crate) const TEXT_HINTING_MAX_PPEM_NAME: &str = "Hinting max ppem";
pub(crate) const STEM_DARKENING_LABEL: &str = "Stem darkening";
pub(crate) const SYSTEM_TEXT_SMOOTHING_LABEL: &str = "Use system ClearType";
pub(crate) const STEM_DARKENING_AMOUNT_NAME: &str = "Darkening amount";
pub(crate) const STEM_DARKENING_MAX_PPEM_NAME: &str = "Darkening max ppem";
pub(crate) const OPTICAL_CENTERING_LABEL: &str = "Optical vertical centering";

pub(crate) const FEATHERING_LABEL: &str = "Feathering";
pub(crate) const FEATHER_WIDTH_NAME: &str = "Feather width";

const SDR_CONTENT_BRIGHTNESS_NITS: (f64, f64) = (48.0, 1000.0);
const TEXT_COVERAGE_GAMMA: (f64, f64) = (0.25, 4.0);
const DEFAULT_TEXT_COVERAGE_GAMMA: f32 = 1.6;
const DEFAULT_TEXT_COVERAGE_BOOST: f32 = 0.75;
pub(crate) const TEXT_HINTING_MAX_PPEM_LIMIT: f32 = 96.0;
const STEM_DARKENING_AMOUNT: (f64, f64) = (0.0, 1.0);
const STEM_DARKENING_MAX_PPEM: (f64, f64) = (1.0, 64.0);
const DEFAULT_STEM_DARKENING: WindowStemDarkening = WindowStemDarkening::Enabled {
    max_ppem: 18.0,
    amount: 0.08,
};
const FEATHER_WIDTH: (f64, f64) = (0.0, 8.0);

const HDR_THEME_MODES: [HdrThemeMode; 4] = [
    HdrThemeMode::Disabled,
    HdrThemeMode::WideGamutOnly,
    HdrThemeMode::ConstrainedHdr,
    HdrThemeMode::FullHdr,
];

/// Where controls are. Settings floats over pages that show some of the same
/// controls, so its controls say so in their accessible names, which keeps
/// each name in a window unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Page,
    Settings,
}

impl Place {
    /// The accessible name of the control labeled `label` here.
    pub(crate) fn name(self, label: &str) -> String {
        match self {
            Self::Page => label.to_string(),
            Self::Settings => format!("{label} in Settings"),
        }
    }
}

pub(crate) fn hdr_theme_mode_label(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => "Disabled (SDR baseline)",
        HdrThemeMode::WideGamutOnly => "Wide-gamut only",
        HdrThemeMode::ConstrainedHdr => "Constrained HDR",
        HdrThemeMode::FullHdr => "Full HDR",
    }
}

/// `control` under its `label`, `width` wide, for pages that lay controls
/// out in columns.
pub(crate) fn labeled_control<W>(
    theme_reader: &DevThemeReader,
    label: &'static str,
    width: f32,
    control: W,
) -> PropertyRow
where
    W: Widget + 'static,
{
    PropertyRow::new(label, control)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .control_width(width)
}

/// A select showing which of `labels` the options hold, and writing the
/// choice back with `update`.
fn option_select(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
    name: &'static str,
    labels: &'static [&'static str],
    index: fn(&WindowRenderOptions) -> usize,
    update: fn(&mut WindowRenderOptions, usize),
) -> Select {
    let read = options.clone();
    let write = options.clone();
    Select::new(place.name(name))
        .theme_when(clone_dev_theme_reader(theme_reader))
        .options(labels.iter().copied())
        .selected_when(move || Some(index(&read.get())))
        .on_change(move |selected, _| write.update(|options| update(options, selected)))
}

/// A switch showing `on` for the options, and writing a toggle back with
/// `set`.
fn option_switch(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
    label: &'static str,
    on: fn(&WindowRenderOptions) -> bool,
    set: fn(&mut WindowRenderOptions, bool),
) -> Switch {
    let read = options.clone();
    let write = options.clone();
    Switch::new(label)
        .semantic_name(place.name(label))
        .theme_when(clone_dev_theme_reader(theme_reader))
        .checked_when(move || on(&read.get()))
        .on_change(move |value| write.update(|options| set(options, value)))
}

/// A number input showing `value` for the options within `range`, and
/// writing a change back with `set`.
#[allow(clippy::too_many_arguments)]
fn option_number(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
    name: &'static str,
    (min, max): (f64, f64),
    step: f64,
    precision: usize,
    value: fn(&WindowRenderOptions) -> f64,
    set: fn(&mut WindowRenderOptions, f64),
) -> NumberInput {
    let read = options.clone();
    let write = options.clone();
    NumberInput::new(place.name(name))
        .theme_when(clone_dev_theme_reader(theme_reader))
        .range(min, max)
        .step(step)
        .precision(precision)
        .value(value(&options.get()))
        .value_when(move || value(&read.get()))
        .on_change(move |new_value| {
            write.update(|options| set(options, new_value.clamp(min, max)));
        })
}

pub(crate) fn color_management_select(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Select {
    option_select(
        theme_reader,
        options,
        place,
        COLOR_MANAGEMENT_MODE_NAME,
        &["Automatic", "Force SDR", "Prefer wide gamut", "Prefer HDR"],
        |options| match options.color_management_mode {
            WindowColorManagementMode::Automatic => 0,
            WindowColorManagementMode::ForceSdr => 1,
            WindowColorManagementMode::PreferWideGamut => 2,
            WindowColorManagementMode::PreferHdr => 3,
        },
        |options, index| {
            options.color_management_mode = match index {
                1 => WindowColorManagementMode::ForceSdr,
                2 => WindowColorManagementMode::PreferWideGamut,
                3 => WindowColorManagementMode::PreferHdr,
                _ => WindowColorManagementMode::Automatic,
            };
        },
    )
}

pub(crate) fn output_primaries_select(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Select {
    option_select(
        theme_reader,
        options,
        place,
        OUTPUT_PRIMARIES_NAME,
        &["Automatic", "sRGB", "Display P3"],
        |options| match options.output_color_primaries {
            WindowOutputColorPrimaries::Automatic => 0,
            WindowOutputColorPrimaries::Srgb => 1,
            WindowOutputColorPrimaries::DisplayP3 => 2,
        },
        |options, index| {
            options.output_color_primaries = match index {
                1 => WindowOutputColorPrimaries::Srgb,
                2 => WindowOutputColorPrimaries::DisplayP3,
                _ => WindowOutputColorPrimaries::Automatic,
            };
        },
    )
}

pub(crate) fn dynamic_range_select(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Select {
    option_select(
        theme_reader,
        options,
        place,
        DYNAMIC_RANGE_MODE_NAME,
        &["Automatic", "SDR", "HDR"],
        |options| match options.dynamic_range_mode {
            WindowDynamicRangeMode::Automatic => 0,
            WindowDynamicRangeMode::StandardDynamicRange => 1,
            WindowDynamicRangeMode::HighDynamicRange => 2,
        },
        |options, index| {
            options.dynamic_range_mode = match index {
                1 => WindowDynamicRangeMode::StandardDynamicRange,
                2 => WindowDynamicRangeMode::HighDynamicRange,
                _ => WindowDynamicRangeMode::Automatic,
            };
        },
    )
}

pub(crate) fn tone_mapping_select(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Select {
    option_select(
        theme_reader,
        options,
        place,
        TONE_MAPPING_MODE_NAME,
        &["Automatic", "Clamp", "Reinhard"],
        |options| match options.tone_mapping_mode {
            WindowToneMappingMode::Automatic => 0,
            WindowToneMappingMode::Clamp => 1,
            WindowToneMappingMode::Reinhard => 2,
        },
        |options, index| {
            options.tone_mapping_mode = match index {
                1 => WindowToneMappingMode::Clamp,
                2 => WindowToneMappingMode::Reinhard,
                _ => WindowToneMappingMode::Automatic,
            };
        },
    )
}

pub(crate) fn sdr_content_brightness_input(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> NumberInput {
    option_number(
        theme_reader,
        options,
        place,
        SDR_CONTENT_BRIGHTNESS_NAME,
        SDR_CONTENT_BRIGHTNESS_NITS,
        1.0,
        0,
        |options| f64::from(options.sdr_content_brightness_nits),
        |options, nits| options.sdr_content_brightness_nits = nits as f32,
    )
}

pub(crate) fn system_sdr_brightness_switch(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Switch {
    option_switch(
        theme_reader,
        options,
        place,
        USE_SYSTEM_SDR_BRIGHTNESS_LABEL,
        |options| options.use_system_sdr_content_brightness,
        |options, on| options.use_system_sdr_content_brightness = on,
    )
}

/// The HDR theme mode widgets preview, which falls back to what the output
/// can show.
pub(crate) fn hdr_theme_mode_select(theme_reader: &DevThemeReader, place: Place) -> Select {
    Select::new(place.name(HDR_THEME_MODE_NAME))
        .theme_when(clone_dev_theme_reader(theme_reader))
        .options(HDR_THEME_MODES.map(hdr_theme_mode_label))
        .selected_when(|| {
            HDR_THEME_MODES
                .iter()
                .position(|mode| *mode == hdr_theme_mode())
        })
        .on_change(|index, _| {
            if let Some(mode) = HDR_THEME_MODES.get(index) {
                set_hdr_theme_mode(*mode);
            }
        })
}

pub(crate) fn text_coverage_policy_select(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Select {
    option_select(
        theme_reader,
        options,
        place,
        TEXT_COVERAGE_POLICY_NAME,
        &[
            "Perceptual",
            "Linear",
            "Gamma",
            "Coverage boost",
            "2c - c^2",
        ],
        |options| match options.text_coverage_policy.normalized() {
            WindowTextCoveragePolicy::Perceptual => 0,
            WindowTextCoveragePolicy::Linear => 1,
            WindowTextCoveragePolicy::Gamma(_) => 2,
            WindowTextCoveragePolicy::CoverageBoost(_) => 3,
            WindowTextCoveragePolicy::TwoCoverageMinusCoverageSq => 4,
        },
        |options, index| {
            options.text_coverage_policy = match index {
                0 => WindowTextCoveragePolicy::Perceptual,
                1 => WindowTextCoveragePolicy::Linear,
                2 => WindowTextCoveragePolicy::Gamma(text_coverage_gamma(options) as f32),
                3 => WindowTextCoveragePolicy::CoverageBoost(
                    match options.text_coverage_policy.normalized() {
                        WindowTextCoveragePolicy::CoverageBoost(amount) => amount,
                        _ => DEFAULT_TEXT_COVERAGE_BOOST,
                    },
                ),
                _ => WindowTextCoveragePolicy::TwoCoverageMinusCoverageSq,
            };
        },
    )
}

/// Whether text coverage follows a gamma curve, which the gamma input sets.
pub(crate) fn uses_text_coverage_gamma(options: &WindowRenderOptions) -> bool {
    matches!(
        options.text_coverage_policy.normalized(),
        WindowTextCoveragePolicy::Gamma(_)
    )
}

fn text_coverage_gamma(options: &WindowRenderOptions) -> f64 {
    match options.text_coverage_policy.normalized() {
        WindowTextCoveragePolicy::Gamma(gamma) => f64::from(gamma),
        _ => f64::from(DEFAULT_TEXT_COVERAGE_GAMMA),
    }
}

pub(crate) fn text_coverage_gamma_input(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> NumberInput {
    option_number(
        theme_reader,
        options,
        place,
        TEXT_COVERAGE_GAMMA_NAME,
        TEXT_COVERAGE_GAMMA,
        0.05,
        2,
        text_coverage_gamma,
        |options, gamma| {
            options.text_coverage_policy = WindowTextCoveragePolicy::Gamma(gamma as f32);
        },
    )
}

pub(crate) fn uses_text_hinting(options: &WindowRenderOptions) -> bool {
    !matches!(options.text_hinting.normalized(), WindowTextHinting::None)
}

fn text_hinting_max_ppem(options: &WindowRenderOptions) -> f32 {
    match options.text_hinting.normalized() {
        WindowTextHinting::Slight { max_ppem } => max_ppem,
        WindowTextHinting::None => TEXT_HINTING_MAX_PPEM_LIMIT,
    }
}

pub(crate) fn text_hinting_switch(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Switch {
    option_switch(
        theme_reader,
        options,
        place,
        TEXT_HINTING_LABEL,
        uses_text_hinting,
        |options, on| {
            options.text_hinting = if on {
                WindowTextHinting::Slight {
                    max_ppem: text_hinting_max_ppem(options),
                }
            } else {
                WindowTextHinting::None
            };
        },
    )
}

pub(crate) fn text_hinting_max_ppem_input(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> NumberInput {
    option_number(
        theme_reader,
        options,
        place,
        TEXT_HINTING_MAX_PPEM_NAME,
        (1.0, f64::from(TEXT_HINTING_MAX_PPEM_LIMIT)),
        0.5,
        1,
        |options| f64::from(text_hinting_max_ppem(options)),
        |options, max_ppem| {
            options.text_hinting = WindowTextHinting::Slight {
                max_ppem: max_ppem as f32,
            };
        },
    )
}

pub(crate) fn uses_stem_darkening(options: &WindowRenderOptions) -> bool {
    !matches!(
        options.stem_darkening.normalized(),
        WindowStemDarkening::None
    )
}

/// The stem darkening the options use, or the default one when it is off.
fn stem_darkening(options: &WindowRenderOptions) -> (f32, f32) {
    match options.stem_darkening.normalized() {
        WindowStemDarkening::Enabled { max_ppem, amount } => (max_ppem, amount),
        WindowStemDarkening::None => match DEFAULT_STEM_DARKENING {
            WindowStemDarkening::Enabled { max_ppem, amount } => (max_ppem, amount),
            WindowStemDarkening::None => (0.0, 0.0),
        },
    }
}

/// LCD text in the system's ClearType subpixel order.
pub(crate) fn system_text_smoothing_switch(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Switch {
    option_switch(
        theme_reader,
        options,
        place,
        SYSTEM_TEXT_SMOOTHING_LABEL,
        |options| options.use_system_text_smoothing,
        |options, on| options.use_system_text_smoothing = on,
    )
}

pub(crate) fn stem_darkening_switch(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Switch {
    option_switch(
        theme_reader,
        options,
        place,
        STEM_DARKENING_LABEL,
        uses_stem_darkening,
        |options, on| {
            let (max_ppem, amount) = stem_darkening(options);
            options.stem_darkening = if on {
                WindowStemDarkening::Enabled { max_ppem, amount }
            } else {
                WindowStemDarkening::None
            };
        },
    )
}

pub(crate) fn stem_darkening_amount_input(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> NumberInput {
    option_number(
        theme_reader,
        options,
        place,
        STEM_DARKENING_AMOUNT_NAME,
        STEM_DARKENING_AMOUNT,
        0.01,
        2,
        |options| f64::from(stem_darkening(options).1),
        |options, amount| {
            let (max_ppem, _) = stem_darkening(options);
            options.stem_darkening = WindowStemDarkening::Enabled {
                max_ppem,
                amount: amount as f32,
            };
        },
    )
}

pub(crate) fn stem_darkening_max_ppem_input(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> NumberInput {
    option_number(
        theme_reader,
        options,
        place,
        STEM_DARKENING_MAX_PPEM_NAME,
        STEM_DARKENING_MAX_PPEM,
        0.5,
        1,
        |options| f64::from(stem_darkening(options).0),
        |options, max_ppem| {
            let (_, amount) = stem_darkening(options);
            options.stem_darkening = WindowStemDarkening::Enabled {
                max_ppem: max_ppem as f32,
                amount,
            };
        },
    )
}

pub(crate) fn optical_centering_switch(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Switch {
    option_switch(
        theme_reader,
        options,
        place,
        OPTICAL_CENTERING_LABEL,
        |options| options.optical_vertical_text_alignment_enabled,
        |options, on| options.optical_vertical_text_alignment_enabled = on,
    )
}

pub(crate) fn feathering_switch(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> Switch {
    option_switch(
        theme_reader,
        options,
        place,
        FEATHERING_LABEL,
        |options| options.feathering_enabled,
        |options, on| options.feathering_enabled = on,
    )
}

pub(crate) fn feather_width_input(
    theme_reader: &DevThemeReader,
    options: &RenderOptions,
    place: Place,
) -> NumberInput {
    option_number(
        theme_reader,
        options,
        place,
        FEATHER_WIDTH_NAME,
        FEATHER_WIDTH,
        0.05,
        2,
        |options| f64::from(options.feather_width),
        |options, width| options.feather_width = width as f32,
    )
}
