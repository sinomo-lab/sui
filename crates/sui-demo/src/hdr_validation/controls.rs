//! The window output controls. Settings and the HDR validation page build
//! them from the same state, so each shows what the other changed.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::OnceLock,
};

use sui::prelude::*;
use sui::{
    Signal, WindowColorManagementMode, WindowDynamicRangeMode, WindowId,
    WindowOutputColorPrimaries, WindowRenderOptions, WindowToneMappingMode,
    set_window_render_options, window_render_options,
};

use crate::app::{DevThemeReader, clone_dev_theme_reader, default_render_options};

pub(crate) const COLOR_MANAGEMENT_MODE_NAME: &str = "Color management";
pub(crate) const OUTPUT_PRIMARIES_NAME: &str = "Output primaries";
pub(crate) const DYNAMIC_RANGE_MODE_NAME: &str = "Dynamic range";
pub(crate) const TONE_MAPPING_MODE_NAME: &str = "Tone mapping";
pub(crate) const SDR_CONTENT_BRIGHTNESS_NAME: &str = "SDR content brightness";
pub(crate) const USE_SYSTEM_SDR_BRIGHTNESS_LABEL: &str = "Use system SDR brightness";

const COLOR_MANAGEMENT_MODE_OPTIONS: [&str; 4] =
    ["Automatic", "Force SDR", "Prefer wide gamut", "Prefer HDR"];
const OUTPUT_PRIMARIES_OPTIONS: [&str; 3] = ["Automatic", "sRGB", "Display P3"];
const DYNAMIC_RANGE_MODE_OPTIONS: [&str; 3] = ["Automatic", "SDR", "HDR"];
const TONE_MAPPING_MODE_OPTIONS: [&str; 3] = ["Automatic", "Clamp", "Reinhard"];

const SDR_CONTENT_BRIGHTNESS_MIN_NITS: f64 = 48.0;
const SDR_CONTENT_BRIGHTNESS_MAX_NITS: f64 = 1000.0;

/// Changes whenever output options change through the controls. Widgets
/// that describe the output observe it to look at the output again.
pub(crate) fn output_options_changes() -> &'static Signal<u64> {
    static CHANGES: OnceLock<Signal<u64>> = OnceLock::new();
    CHANGES.get_or_init(|| Signal::named("Output options changes", 0))
}

/// Render options for one window, as the output controls edit them. Clones
/// share the options, so Settings and the HDR validation page edit the same.
#[derive(Clone)]
pub(crate) struct OutputOptions {
    options: Rc<RefCell<WindowRenderOptions>>,
    window: Rc<Cell<Option<WindowId>>>,
    /// Whether to take the window's options when first bound to it, for a
    /// page that is not sharing them with Settings.
    adopt_window: Rc<Cell<bool>>,
}

impl OutputOptions {
    /// Options that already match the window.
    pub(crate) fn shared(options: Rc<RefCell<WindowRenderOptions>>) -> Self {
        Self {
            options,
            window: Rc::new(Cell::new(None)),
            adopt_window: Rc::new(Cell::new(false)),
        }
    }

    /// Options that start as whatever the window uses.
    pub(crate) fn from_window() -> Self {
        Self {
            options: Rc::new(RefCell::new(default_render_options())),
            window: Rc::new(Cell::new(None)),
            adopt_window: Rc::new(Cell::new(true)),
        }
    }

    /// All the window's render options, for editors of the ones not shown
    /// here.
    pub(crate) fn state(&self) -> Rc<RefCell<WindowRenderOptions>> {
        Rc::clone(&self.options)
    }

    pub(crate) fn get(&self) -> WindowRenderOptions {
        *self.options.borrow()
    }

    /// Change the options and apply them to the window.
    pub(crate) fn update(&self, update: impl FnOnce(&mut WindowRenderOptions)) {
        update(&mut self.options.borrow_mut());
        if let Some(window_id) = self.window.get() {
            set_window_render_options(window_id, self.get().clamped());
        }
        output_options_changes().update(|changes| *changes = changes.wrapping_add(1));
    }

    /// Apply changes to `window_id` from now on. Options made with
    /// [`from_window`](Self::from_window) take the window's options first.
    pub(crate) fn bind_window(&self, window_id: WindowId) {
        if self.window.replace(Some(window_id)) == Some(window_id) {
            return;
        }
        if self.adopt_window.replace(false)
            && let Some(options) = window_render_options(window_id)
        {
            *self.options.borrow_mut() = options;
        }
    }
}

pub(crate) fn color_management_select(
    theme_reader: &DevThemeReader,
    options: &OutputOptions,
) -> Select {
    let read = options.clone();
    let write = options.clone();
    Select::new(COLOR_MANAGEMENT_MODE_NAME)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .options(COLOR_MANAGEMENT_MODE_OPTIONS)
        .selected_when(move || {
            Some(match read.get().color_management_mode {
                WindowColorManagementMode::Automatic => 0,
                WindowColorManagementMode::ForceSdr => 1,
                WindowColorManagementMode::PreferWideGamut => 2,
                WindowColorManagementMode::PreferHdr => 3,
            })
        })
        .on_change(move |index, _| {
            write.update(|options| {
                options.color_management_mode = match index {
                    1 => WindowColorManagementMode::ForceSdr,
                    2 => WindowColorManagementMode::PreferWideGamut,
                    3 => WindowColorManagementMode::PreferHdr,
                    _ => WindowColorManagementMode::Automatic,
                };
            });
        })
}

pub(crate) fn output_primaries_select(
    theme_reader: &DevThemeReader,
    options: &OutputOptions,
) -> Select {
    let read = options.clone();
    let write = options.clone();
    Select::new(OUTPUT_PRIMARIES_NAME)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .options(OUTPUT_PRIMARIES_OPTIONS)
        .selected_when(move || {
            Some(match read.get().output_color_primaries {
                WindowOutputColorPrimaries::Automatic => 0,
                WindowOutputColorPrimaries::Srgb => 1,
                WindowOutputColorPrimaries::DisplayP3 => 2,
            })
        })
        .on_change(move |index, _| {
            write.update(|options| {
                options.output_color_primaries = match index {
                    1 => WindowOutputColorPrimaries::Srgb,
                    2 => WindowOutputColorPrimaries::DisplayP3,
                    _ => WindowOutputColorPrimaries::Automatic,
                };
            });
        })
}

pub(crate) fn dynamic_range_select(
    theme_reader: &DevThemeReader,
    options: &OutputOptions,
) -> Select {
    let read = options.clone();
    let write = options.clone();
    Select::new(DYNAMIC_RANGE_MODE_NAME)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .options(DYNAMIC_RANGE_MODE_OPTIONS)
        .selected_when(move || {
            Some(match read.get().dynamic_range_mode {
                WindowDynamicRangeMode::Automatic => 0,
                WindowDynamicRangeMode::StandardDynamicRange => 1,
                WindowDynamicRangeMode::HighDynamicRange => 2,
            })
        })
        .on_change(move |index, _| {
            write.update(|options| {
                options.dynamic_range_mode = match index {
                    1 => WindowDynamicRangeMode::StandardDynamicRange,
                    2 => WindowDynamicRangeMode::HighDynamicRange,
                    _ => WindowDynamicRangeMode::Automatic,
                };
            });
        })
}

pub(crate) fn tone_mapping_select(
    theme_reader: &DevThemeReader,
    options: &OutputOptions,
) -> Select {
    let read = options.clone();
    let write = options.clone();
    Select::new(TONE_MAPPING_MODE_NAME)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .options(TONE_MAPPING_MODE_OPTIONS)
        .selected_when(move || {
            Some(match read.get().tone_mapping_mode {
                WindowToneMappingMode::Automatic => 0,
                WindowToneMappingMode::Clamp => 1,
                WindowToneMappingMode::Reinhard => 2,
            })
        })
        .on_change(move |index, _| {
            write.update(|options| {
                options.tone_mapping_mode = match index {
                    1 => WindowToneMappingMode::Clamp,
                    2 => WindowToneMappingMode::Reinhard,
                    _ => WindowToneMappingMode::Automatic,
                };
            });
        })
}

pub(crate) fn sdr_content_brightness_input(
    theme_reader: &DevThemeReader,
    options: &OutputOptions,
) -> NumberInput {
    let read = options.clone();
    let write = options.clone();
    NumberInput::new(SDR_CONTENT_BRIGHTNESS_NAME)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .range(
            SDR_CONTENT_BRIGHTNESS_MIN_NITS,
            SDR_CONTENT_BRIGHTNESS_MAX_NITS,
        )
        .step(1.0)
        .precision(0)
        .value(f64::from(options.get().sdr_content_brightness_nits))
        .value_when(move || f64::from(read.get().sdr_content_brightness_nits))
        .on_change(move |value| {
            write.update(|options| {
                options.sdr_content_brightness_nits = value.clamp(
                    SDR_CONTENT_BRIGHTNESS_MIN_NITS,
                    SDR_CONTENT_BRIGHTNESS_MAX_NITS,
                ) as f32;
            });
        })
}

pub(crate) fn system_sdr_brightness_checkbox(
    theme_reader: &DevThemeReader,
    options: &OutputOptions,
) -> Checkbox {
    let read = options.clone();
    let write = options.clone();
    Checkbox::new(USE_SYSTEM_SDR_BRIGHTNESS_LABEL)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .checked_when(move || read.get().use_system_sdr_content_brightness)
        .on_toggle(move |checked| {
            write.update(|options| options.use_system_sdr_content_brightness = checked);
        })
}
