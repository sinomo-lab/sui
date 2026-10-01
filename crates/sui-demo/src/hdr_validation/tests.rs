use sui::diagnostics::{OutputStrategy, WindowOutputDiagnostics};
use sui::{
    Application, Color, DefaultTheme, DisplayCapabilities, DisplayColorPrimaries, Result,
    SemanticsRole, SemanticsValue, Size, SizedBox, WindowBuilder, WindowColorManagementMode,
    WindowDynamicRangeMode, WindowOutputColorPrimaries, WindowRenderOptions, WindowToneMappingMode,
    window_render_options,
};
use sui_render_wgpu::DynamicRangeMode;
use sui_testing::prelude::*;

use super::live::{HighlightFit, OutputKind};
use super::probes::{clipped_to_srgb, fitted_on_cpu};
use super::report::LightMetrics;
use super::*;
use crate::test_support::*;

/// The page on a simulated `display`, so tests never depend on this
/// machine's screens.
fn page_app(display: DisplayCapabilities, options: Option<WindowRenderOptions>) -> Result<TestApp> {
    TestApp::builder(move || {
        let application = build_color_validation_application();
        match options {
            Some(options) => application.with_window_render_options(options),
            None => application,
        }
    })
    .vsync(false)
    .display_capabilities(display)
    .launch()
}

fn diagnostics(
    strategy: OutputStrategy,
    tone_mapping: WindowToneMappingMode,
) -> WindowOutputDiagnostics {
    let display_capabilities = DisplayCapabilities {
        supports_wide_gamut: true,
        supports_hdr: true,
        preferred_primaries: DisplayColorPrimaries::DisplayP3,
        preferred_dynamic_range: DynamicRangeMode::HighDynamicRange,
        max_luminance_nits: Some(1000.0),
        sdr_white_nits: Some(250.0),
        max_content_headroom: None,
        native_hdr_presentation_supported: true,
        notes: String::new(),
    };
    WindowOutputDiagnostics {
        output_gamut: strategy.gamut(&display_capabilities),
        display_capabilities,
        requested_color_management_mode: WindowColorManagementMode::Automatic,
        requested_output_primaries: WindowOutputColorPrimaries::Automatic,
        requested_dynamic_range_mode: WindowDynamicRangeMode::Automatic,
        requested_tone_mapping_mode: tone_mapping,
        requested_sdr_content_brightness_nits: 250.0,
        configured_sdr_content_brightness_nits: 250.0,
        system_sdr_content_brightness_nits: Some(250.0),
        use_system_sdr_content_brightness: true,
        active_output_strategy: strategy,
    }
}

fn text_starting_with(window: &TestWindow, prefix: &str) -> Option<String> {
    window
        .snapshot()
        .ok()?
        .accessibility
        .nodes
        .iter()
        .filter(|node| node.role == SemanticsRole::Text)
        .filter_map(|node| node.name.clone())
        .find(|name| name.starts_with(prefix))
}

#[test]
fn page_lays_out_every_section_probe_and_control() -> Result<()> {
    let app = page_app(DisplayCapabilities::sdr(), None)?;
    let window = app.main_window()?;
    let snapshot = window.snapshot()?;
    let nodes = &snapshot.accessibility.nodes;
    let has = |role: SemanticsRole, name: &str| {
        nodes
            .iter()
            .any(|node| node.role == role && node.name.as_deref() == Some(name))
    };

    assert!(has(SemanticsRole::ScrollView, COLOR_VALIDATION_SCROLL_NAME));
    for section in [
        OUTPUT_SECTION_NAME,
        OUTPUT_VERDICT_NAME,
        "Brightness headroom",
        "Highlight fitting",
        "Wide gamut",
        "Gradients and banding",
        "HDR in UI",
        "Capture and report",
        HEADROOM_RAMP_NAME,
        HEADROOM_LADDER_NAME,
        NEAR_WHITE_NAME,
        HIGHLIGHT_CURVE_NAME,
        HUE_GRID_NAME,
        GAMUT_TILES_NAME,
        CHROMATICITY_NAME,
        RAMPS_NAME,
        ui_modes::UI_MODES_NAME,
        CAPTURE_STATUS_NAME,
    ] {
        assert!(
            has(SemanticsRole::GenericContainer, section),
            "missing {section:?}"
        );
    }
    for swatch in [
        "White 1×",
        "White 16×",
        "Near white 1.05×",
        "sRGB clipped green",
        "Display P3 green",
        "Full HDR emissive indicator",
    ] {
        assert!(
            has(SemanticsRole::ColorSwatch, swatch),
            "missing swatch {swatch:?}"
        );
    }
    for select in [
        controls::COLOR_MANAGEMENT_MODE_NAME,
        controls::OUTPUT_PRIMARIES_NAME,
        controls::DYNAMIC_RANGE_MODE_NAME,
        controls::TONE_MAPPING_MODE_NAME,
        controls::HDR_THEME_MODE_NAME,
    ] {
        assert!(
            has(SemanticsRole::ComboBox, select),
            "missing control {select:?}"
        );
    }
    assert!(has(
        SemanticsRole::SpinBox,
        controls::SDR_CONTENT_BRIGHTNESS_NAME
    ));
    assert!(has(
        SemanticsRole::Switch,
        controls::USE_SYSTEM_SDR_BRIGHTNESS_LABEL
    ));
    assert!(has(SemanticsRole::Button, CAPTURE_BUTTON_LABEL));
    assert!(has(SemanticsRole::Button, COPY_REPORT_BUTTON_LABEL));
    Ok(())
}

#[test]
fn verdict_and_expectations_describe_a_native_hdr_display() -> Result<()> {
    let app = page_app(DisplayCapabilities::hdr(1000.0, 250.0), None)?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    let verdict = text_starting_with(&window, "Native HDR output.")
        .expect("the verdict describes the HDR display");
    assert!(verdict.contains("1000 nits peak"), "{verdict}");
    assert!(
        verdict.contains("Colors outside sRGB reach it too."),
        "{verdict}"
    );
    let ui_modes = text_starting_with(&window, "On this output: accents in the Constrained")
        .expect("HDR theme modes take effect on an HDR display");
    assert!(ui_modes.contains("glow above SDR white"), "{ui_modes}");
    Ok(())
}

#[test]
fn verdict_and_expectations_follow_the_presented_output() -> Result<()> {
    let app = page_app(DisplayCapabilities::sdr(), None)?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    let verdict = text_starting_with(&window, "SDR output in sRGB.")
        .expect("the verdict describes the presented output");
    assert!(verdict.contains("keeping their hue"), "{verdict}");
    let gamut = text_starting_with(&window, "On this output: both halves")
        .expect("the gamut probe expects sRGB clipping");
    assert!(gamut.contains("clips Display P3 colors to sRGB"), "{gamut}");
    Ok(())
}

#[test]
fn page_starts_from_the_window_options_and_edits_them() -> Result<()> {
    let configured = WindowRenderOptions::new(true, 1.5)
        .with_color_management_mode(WindowColorManagementMode::ForceSdr)
        .with_output_color_primaries(WindowOutputColorPrimaries::DisplayP3)
        .with_tone_mapping_mode(WindowToneMappingMode::Clamp);
    let app = page_app(DisplayCapabilities::sdr(), Some(configured))?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    let tone_mapping = window
        .get_by_role(SemanticsRole::ComboBox)
        .with_name(controls::TONE_MAPPING_MODE_NAME);
    let value = |window: &TestWindow, name: &str| {
        window
            .snapshot()
            .expect("snapshot")
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox && node.name.as_deref() == Some(name))
            .and_then(|node| node.value.clone())
    };
    assert_eq!(
        value(&window, controls::OUTPUT_PRIMARIES_NAME),
        Some(SemanticsValue::Text("Display P3".to_string()))
    );
    assert_eq!(
        value(&window, controls::TONE_MAPPING_MODE_NAME),
        Some(SemanticsValue::Text("Clamp".to_string()))
    );

    tone_mapping.click()?;
    window.focused().press("ArrowDown")?;
    window.focused().press("Enter")?;
    window.run_until_idle()?;

    assert_eq!(
        value(&window, controls::TONE_MAPPING_MODE_NAME),
        Some(SemanticsValue::Text("Reinhard".to_string()))
    );
    let applied = window_render_options(window.id()).expect("the page applied its options");
    assert_eq!(applied.tone_mapping_mode, WindowToneMappingMode::Reinhard);
    // Options the page does not show are left as the app configured them.
    assert_eq!(
        applied.output_color_primaries,
        WindowOutputColorPrimaries::DisplayP3
    );
    assert!(applied.feathering_enabled);
    assert_eq!(applied.feather_width, 1.5);

    let verdict = text_starting_with(&window, "SDR output in sRGB.")
        .expect("the verdict follows the new tone mapping");
    assert!(verdict.contains("roll off toward white"), "{verdict}");
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn capture_writes_a_bundle_and_reports_what_it_found() -> Result<()> {
    let options = WindowRenderOptions::new(true, 1.0)
        .with_color_management_mode(WindowColorManagementMode::PreferHdr)
        .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange);
    let app = page_app(DisplayCapabilities::hdr(1000.0, 250.0), Some(options))?;
    let window = app.main_window()?;
    // The capture panel is at the bottom of the page.
    window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(COLOR_VALIDATION_SCROLL_NAME)
        .scroll_pixels(sui::Vector::new(0.0, -20_000.0))?;
    window
        .get_by_role(SemanticsRole::Button)
        .with_name(CAPTURE_BUTTON_LABEL)
        .click()?;
    window.run_until_idle()?;

    let status = window
        .snapshot()?
        .accessibility
        .nodes
        .iter()
        .find(|node| node.name.as_deref() == Some(CAPTURE_STATUS_NAME))
        .and_then(|node| node.description.clone())
        .expect("the capture status is exposed");
    assert!(
        status.contains("Scene (HDR intermediate): brightest channel"),
        "{status}"
    );
    // On an HDR display the final output keeps extended range too.
    assert!(
        status.contains("Final output: brightest channel"),
        "{status}"
    );
    let dir = status
        .lines()
        .find_map(|line| line.strip_prefix("Files written to "))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| panic!("the capture wrote files: {status}"));
    for file in [
        "hdr-intermediate.exr",
        "luminance-map.png",
        "headroom-map.png",
        "clip-mask.png",
        "output-diagnostics.txt",
        "capture-metrics.txt",
    ] {
        assert!(
            dir.join(file).exists(),
            "missing {file} in {}",
            dir.display()
        );
    }
    let metrics = std::fs::read_to_string(dir.join("capture-metrics.txt"))
        .expect("capture metrics are readable");
    let _ = std::fs::remove_dir_all(&dir);
    let brightest = metrics
        .lines()
        .find_map(|line| line.strip_prefix("intermediate_max_channel="))
        .and_then(|value| value.parse::<f32>().ok())
        .expect("the metrics record the brightest channel");
    // The ramp and ladder reach 16× SDR white.
    assert!(brightest > 1.0, "{metrics}");
    Ok(())
}

#[test]
fn summaries_describe_each_output() {
    let native = OutputSummary::of(&diagnostics(
        OutputStrategy::HdrNativeSurface {
            format: wgpu::TextureFormat::Rgba16Float,
            primaries: DisplayColorPrimaries::Srgb,
            transfer: sui_render_wgpu::DisplayTransferFunction::LinearExtended,
        },
        WindowToneMappingMode::Automatic,
    ));
    assert_eq!(native.kind, OutputKind::NativeHdr);
    assert_eq!(native.fit, HighlightFit::Extended);
    assert_eq!(native.headroom, Some(4.0));
    // scRGB carries wide-gamut colors to a display that has them.
    assert!(native.shows_wide_gamut());
    assert!(native.headline().contains("about 4.0× SDR white"));

    let wide = OutputSummary::of(&diagnostics(
        OutputStrategy::WideGamutSurface {
            format: wgpu::TextureFormat::Rgba16Float,
            primaries: DisplayColorPrimaries::DisplayP3,
        },
        WindowToneMappingMode::Reinhard,
    ));
    assert_eq!(wide.kind, OutputKind::WideGamut);
    assert_eq!(wide.fit, HighlightFit::RollOff);
    assert!(wide.shows_wide_gamut());
    assert_eq!(wide.headroom, Some(1.0));
}

#[test]
fn cpu_fit_matches_the_output_it_describes() {
    let sdr = OutputSummary::of(&diagnostics(
        OutputStrategy::SdrSurface {
            format: wgpu::TextureFormat::Rgba16Float,
        },
        WindowToneMappingMode::Clamp,
    ));
    // Clipping keeps the orange's hue.
    let orange = fitted_on_cpu([4.0, 2.0, 0.5], Some(sdr)).to_linear_srgb();
    assert!((orange.red - 1.0).abs() < 1e-5);
    assert!((orange.green - 0.5).abs() < 1e-5);
    assert!((orange.blue - 0.125).abs() < 1e-5);
    // Colors within SDR range are left alone.
    let dim = fitted_on_cpu([0.5, 0.25, 0.1], Some(sdr)).to_linear_srgb();
    assert!((dim.green - 0.25).abs() < 1e-5);

    // P3 outputs fit in their own primaries: the result stays within them.
    let wide = OutputSummary::of(&diagnostics(
        OutputStrategy::WideGamutSurface {
            format: wgpu::TextureFormat::Rgba16Float,
            primaries: DisplayColorPrimaries::DisplayP3,
        },
        WindowToneMappingMode::Clamp,
    ));
    let fitted = fitted_on_cpu([3.0, 0.2, 0.1], Some(wide));
    assert!(fitted.red <= 1.0 + 1e-5 && fitted.green <= 1.0 && fitted.blue <= 1.0);

    // Native HDR sends colors as they are.
    let native = OutputSummary::of(&diagnostics(
        OutputStrategy::HdrNativeSurface {
            format: wgpu::TextureFormat::Rgba16Float,
            primaries: DisplayColorPrimaries::Srgb,
            transfer: sui_render_wgpu::DisplayTransferFunction::LinearExtended,
        },
        WindowToneMappingMode::Clamp,
    ));
    assert_eq!(fitted_on_cpu([4.0, 2.0, 0.5], Some(native)).red, 4.0);
}

#[test]
fn srgb_clipping_matches_what_an_srgb_output_shows() {
    let green = clipped_to_srgb(Color::display_p3(0.0, 1.0, 0.0, 1.0), None);
    assert_eq!(green.red, 0.0);
    assert_eq!(green.blue, 0.0);
    assert!((green.green - 1.0).abs() < 1e-5);
}

#[test]
fn light_metrics_measure_headroom_use() {
    let image =
        sui::diagnostics::HdrRgbaImage::new(2, 1, vec![4.0, 2.0, 0.5, 1.0, 0.5, 0.5, 0.5, 1.0])
            .unwrap();
    let metrics = LightMetrics::of(&image);
    assert_eq!(metrics.max_channel, 4.0);
    assert_eq!(metrics.above_sdr_white, 0.5);
    assert!(metrics.max_luminance > 2.0);

    let report = report::output_diagnostics_report("view", None);
    assert_eq!(report, "view=view\noutput_diagnostics=unavailable\n");
}

fn build_color_validation_runtime() -> Result<sui::Runtime> {
    build_color_validation_application().build()
}

fn build_narrow_color_validation_runtime() -> Result<sui::Runtime> {
    Application::new()
        .window(
            WindowBuilder::new()
                .title(COLOR_VALIDATION_VIEW_TITLE)
                .root(
                    SizedBox::new()
                        .size(Size::new(430.0, 320.0))
                        .with_child(build_color_validation_surface()),
                ),
        )
        .build()
}

#[test]
fn color_validation_surface_exposes_its_reference_swatches() {
    let mut runtime =
        build_color_validation_runtime().expect("color validation runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("color validation surface should render");

    let semantics = runtime
        .semantics(window_id)
        .expect("color validation semantics should exist");

    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Window
            && node.name.as_deref() == Some(COLOR_VALIDATION_VIEW_TITLE)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::ScrollView
            && node.name.as_deref() == Some(COLOR_VALIDATION_SCROLL_NAME)
    }));

    for swatch_name in [
        "sRGB clipped red",
        "Display P3 red",
        "sRGB clipped green",
        "Display P3 green",
        "sRGB clipped cyan",
        "Display P3 cyan",
        "White 1×",
        "White 2×",
        "White 4×",
        "White 8×",
        "White 16×",
        "Near white 0.9×",
        "Near white 1.05×",
    ] {
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::ColorSwatch && node.name.as_deref() == Some(swatch_name)
        }));
    }
}

#[test]
fn color_validation_surface_keeps_swatches_and_text_readable_when_narrow() {
    let mut runtime = build_narrow_color_validation_runtime()
        .expect("narrow color validation runtime should build");
    let window_id = runtime.window_ids()[0];
    let output = runtime
        .render(window_id)
        .expect("narrow color validation surface should render");

    let scroll = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(COLOR_VALIDATION_SCROLL_NAME)
        })
        .expect("color validation scroll view should be present");
    let horizontal_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME)
        })
        .expect("horizontal color validation scroll bar should be present");
    let vertical_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(COLOR_VALIDATION_VERTICAL_SCROLL_BAR_NAME)
        })
        .expect("vertical color validation scroll bar should be present");
    let brightest_swatch = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ColorSwatch && node.name.as_deref() == Some("White 16×")
        })
        .expect("the brightest ladder swatch should be present");
    let hdr_description = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Text
                && node
                    .name
                    .as_deref()
                    .is_some_and(|name| name.starts_with("White from a quarter of SDR white"))
        })
        .expect("the headroom description should be present");

    let horizontal_max = match horizontal_scroll_bar.value {
        Some(SemanticsValue::Range { max, .. }) => max,
        _ => 0.0,
    };
    let vertical_max = match vertical_scroll_bar.value {
        Some(SemanticsValue::Range { max, .. }) => max,
        _ => 0.0,
    };

    assert!(horizontal_max > 0.0);
    assert!(vertical_max > 0.0);
    assert!(horizontal_scroll_bar.bounds.y() >= scroll.bounds.max_y());
    assert!(vertical_scroll_bar.bounds.x() >= scroll.bounds.max_x());
    // The page keeps its width and scrolls instead of squeezing probes.
    assert!(brightest_swatch.bounds.width() >= 80.0);
    assert!(brightest_swatch.bounds.height() >= 40.0);
    assert!(hdr_description.bounds.height() > 20.0);
    assert!(hdr_description.bounds.width() < 1000.0);
}

#[test]
fn color_validation_scroll_bars_use_themed_metrics() {
    let theme = DefaultTheme::touch();
    let output = render_widget_with_size(
        COLOR_VALIDATION_VIEW_TITLE,
        Size::new(430.0, 320.0),
        build_color_validation_surface_with_theme(theme_reader(theme)),
    );
    let horizontal_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME)
        })
        .expect("horizontal color validation scroll bar should be present");
    let vertical_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(COLOR_VALIDATION_VERTICAL_SCROLL_BAR_NAME)
        })
        .expect("vertical color validation scroll bar should be present");

    assert_eq!(
        vertical_scroll_bar.bounds.width(),
        theme.metrics.scroll_bar_thickness
    );
    assert_eq!(
        horizontal_scroll_bar.bounds.height(),
        theme.metrics.scroll_bar_thickness
    );
}

#[test]
fn color_validation_surface_omits_live_performance_overlay() {
    let mut runtime =
        build_color_validation_runtime().expect("color validation runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("color validation surface should render");

    let semantics = runtime
        .semantics(window_id)
        .expect("color validation semantics should exist");
    assert_semantics_omit_live_performance_overlay(semantics);
}

#[test]
fn color_validation_repaints_when_the_theme_reader_changes() -> Result<()> {
    assert_widget_repaints_after_theme_change(
        COLOR_VALIDATION_VIEW_TITLE,
        Size::new(520.0, 360.0),
        build_color_validation_surface_with_theme,
    )
}
