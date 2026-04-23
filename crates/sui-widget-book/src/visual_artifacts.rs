use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use sui::{
    Error, Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Rect,
    Result, ScrollDelta, SemanticsRole, Vector, WindowColorManagementMode,
    WindowDynamicRangeMode, WindowOutputColorPrimaries, WindowRenderOptions,
    WindowToneMappingMode, window_output_diagnostics,
};
use sui_render_wgpu::{
    DebugCaptureArtifact, DebugCaptureEncoding, DebugCaptureRequest, DebugCaptureStage,
    DebugSdrVisualization,
};
use sui_testing::{
    Locator, Screenshot, TestApp, TestWindow, hdr_clip_mask, hdr_headroom_heatmap,
    hdr_luminance_heatmap, write_hdr_avif, write_hdr_exr,
};

use crate::{
    BREADCRUMB_NAME, COLOR_PICKER_NAME, COLOR_SWATCH_NAME, CONTEXT_MENU_NAME,
    COLOR_VALIDATION_VIEW_TITLE, DEMO_IMAGE_LABEL, DIALOG_TITLE, DIALOG_TRIGGER_LABEL,
    GALLERY_SCROLL_NAME, ICON_BUTTON_LABEL, ICON_LABEL, LIST_VIEW_NAME, MENU_NAME,
    NAME_INPUT_LABEL, NUMBER_INPUT_NAME, POPOVER_NAME, POPOVER_TRIGGER_LABEL,
    PRIMARY_BUTTON_LABEL, PROGRESS_NAME, RADIO_BUTTON_LABEL, RADIO_GROUP_NAME, SELECT_NAME,
    SLIDER_NAME, SPINNER_NAME, SPLIT_VIEW_NAME, SUBSCRIBE_LABEL, SUMMARY_NAME, SWITCH_LABEL,
    TAB_BAR_NAME, TAB_BAR_OPTIONS, TAB_PANEL_OPTIONS, TABLE_NAME, TABS_NAME, TEXT_AREA_LABEL,
    THEME_PREVIEW_NAME, TOOLBAR_SEPARATOR_NAME, TOOLTIP_TEXT, TOOLTIP_TRIGGER_LABEL,
    TREE_VIEW_NAME, WINDOW_TITLE, WidgetBookState, build_color_validation_application,
    build_widget_book_application, default_widget_book_state,
};

#[derive(Clone, Copy)]
pub(crate) enum StoryCase {
    Overview,
    OverviewConfigured,
    Button,
    ButtonHover,
    ButtonPressed,
    Checkbox,
    CheckboxUnchecked,
    FilledInput,
    EmptyInputFocused,
    Icon,
    IconButton,
    Separator,
    Switch,
    RadioButton,
    RadioGroup,
    Slider,
    NumberInput,
    TextArea,
    SelectExpanded,
    TabBar,
    Tabs,
    Menu,
    ContextMenuOpen,
    TooltipVisible,
    PopoverOpen,
    Dialog,
    ProgressBar,
    Spinner,
    ScrollViewScrolled,
    Summary,
    ListView,
    TreeView,
    Table,
    SplitView,
    Breadcrumb,
    ColorSwatch,
    ColorPicker,
    ThemePreview,
    ImageWidget,
}

impl StoryCase {
    pub(crate) const ALL: [Self; 39] = [
        Self::Overview,
        Self::OverviewConfigured,
        Self::Button,
        Self::ButtonHover,
        Self::ButtonPressed,
        Self::Checkbox,
        Self::CheckboxUnchecked,
        Self::FilledInput,
        Self::EmptyInputFocused,
        Self::Icon,
        Self::IconButton,
        Self::Separator,
        Self::Switch,
        Self::RadioButton,
        Self::RadioGroup,
        Self::Slider,
        Self::NumberInput,
        Self::TextArea,
        Self::SelectExpanded,
        Self::TabBar,
        Self::Tabs,
        Self::Menu,
        Self::ContextMenuOpen,
        Self::TooltipVisible,
        Self::PopoverOpen,
        Self::Dialog,
        Self::ProgressBar,
        Self::Spinner,
        Self::ScrollViewScrolled,
        Self::Summary,
        Self::ListView,
        Self::TreeView,
        Self::Table,
        Self::SplitView,
        Self::Breadcrumb,
        Self::ColorSwatch,
        Self::ColorPicker,
        Self::ThemePreview,
        Self::ImageWidget,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::OverviewConfigured => "overview-configured",
            Self::Button => "button",
            Self::ButtonHover => "button-hover",
            Self::ButtonPressed => "button-pressed",
            Self::Checkbox => "checkbox",
            Self::CheckboxUnchecked => "checkbox-unchecked",
            Self::FilledInput => "filled-input",
            Self::EmptyInputFocused => "empty-input-focused",
            Self::Icon => "icon",
            Self::IconButton => "icon-button",
            Self::Separator => "separator",
            Self::Switch => "switch",
            Self::RadioButton => "radio-button",
            Self::RadioGroup => "radio-group",
            Self::Slider => "slider",
            Self::NumberInput => "number-input",
            Self::TextArea => "text-area",
            Self::SelectExpanded => "select-expanded",
            Self::TabBar => "tab-bar",
            Self::Tabs => "tabs",
            Self::Menu => "menu",
            Self::ContextMenuOpen => "context-menu-open",
            Self::TooltipVisible => "tooltip-visible",
            Self::PopoverOpen => "popover-open",
            Self::Dialog => "dialog",
            Self::ProgressBar => "progress-bar",
            Self::Spinner => "spinner",
            Self::ScrollViewScrolled => "scroll-view-scrolled",
            Self::Summary => "summary",
            Self::ListView => "list-view",
            Self::TreeView => "tree-view",
            Self::Table => "table",
            Self::SplitView => "split-view",
            Self::Breadcrumb => "breadcrumb",
            Self::ColorSwatch => "color-swatch",
            Self::ColorPicker => "color-picker",
            Self::ThemePreview => "theme-preview",
            Self::ImageWidget => "image-widget",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Overview => "Whole-window widget book overview screenshot.",
            Self::OverviewConfigured => {
                "Whole-window widget book overview with configured state changes."
            }
            Self::Button => "Primary button crop for direct visual regression review.",
            Self::ButtonHover => "Primary button crop in the hovered state.",
            Self::ButtonPressed => "Primary button crop while the pointer is held down.",
            Self::Checkbox => "Checkbox crop in the checked default state.",
            Self::CheckboxUnchecked => "Checkbox crop in the unchecked configured state.",
            Self::FilledInput => {
                "Text input crop with a configured value for text rendering checks."
            }
            Self::EmptyInputFocused => {
                "Empty text input crop with focus ring and placeholder visible."
            }
            Self::Icon => "Standalone icon crop for compact toolbar glyph review.",
            Self::IconButton => "Icon button crop for titlebar-style actions.",
            Self::Separator => "Separator crop for toolbar and inspector dividers.",
            Self::Switch => "Switch crop for boolean controls distinct from checkbox rows.",
            Self::RadioButton => "Standalone radio button crop.",
            Self::RadioGroup => "Radio group crop for mutually exclusive choices.",
            Self::Slider => "Slider crop for numeric tuning controls.",
            Self::NumberInput => "Number input crop for spinbox-style editing.",
            Self::TextArea => "Text area crop with multiline content.",
            Self::SelectExpanded => "Expanded select crop showing compact option picking.",
            Self::TabBar => "Standalone tab bar crop for editor-style navigation.",
            Self::Tabs => "Tabs crop showing selected panel content.",
            Self::Menu => "Command menu crop for overflow and app menus.",
            Self::ContextMenuOpen => {
                "Open context menu crop anchored to an explicit scene-layer surface."
            }
            Self::TooltipVisible => "Tooltip crop while the trigger is hovered.",
            Self::PopoverOpen => "Open popover crop for inline inspector content.",
            Self::Dialog => "Dialog crop for confirmations and settings.",
            Self::ProgressBar => "Progress bar crop for long-running tasks.",
            Self::Spinner => "Busy indicator crop for indeterminate work.",
            Self::ScrollViewScrolled => {
                "Outer widget-book scroll view after paging down through the gallery."
            }
            Self::Summary => "Composed summary panel showing derived state.",
            Self::ListView => "List view crop for asset browser and inspector collections.",
            Self::TreeView => "Tree view crop for layers, files, and scene hierarchies.",
            Self::Table => "Table crop for structured tool data and data-grid layouts.",
            Self::SplitView => "Split view crop with the resizable divider in an editor shell.",
            Self::Breadcrumb => "Breadcrumb crop for path and project navigation surfaces.",
            Self::ColorSwatch => "Color swatch crop for palette chips and compact property rows.",
            Self::ColorPicker => "Color picker crop for interactive color adjustment workflows.",
            Self::ThemePreview => {
                "Theme preview panel with the light and dark comparison cards visible."
            }
            Self::ImageWidget => "Image widget crop for previews, thumbnails, and asset panels.",
        }
    }

    pub(crate) fn build_app(self) -> Result<TestApp> {
        let state = match self {
            Self::Overview
            | Self::Button
            | Self::ButtonHover
            | Self::ButtonPressed
            | Self::Checkbox
            | Self::Icon
            | Self::IconButton
            | Self::Separator
            | Self::Switch
            | Self::RadioButton
            | Self::RadioGroup
            | Self::Slider
            | Self::NumberInput
            | Self::SelectExpanded
            | Self::TabBar
            | Self::Tabs
            | Self::Menu
            | Self::ContextMenuOpen
            | Self::TooltipVisible
            | Self::PopoverOpen
            | Self::Dialog
            | Self::ProgressBar
            | Self::Spinner
            | Self::ScrollViewScrolled
            | Self::ListView
            | Self::TreeView
            | Self::Table
            | Self::SplitView
            | Self::Breadcrumb
            | Self::ColorSwatch
            | Self::ColorPicker
            | Self::ThemePreview
            | Self::ImageWidget => default_widget_book_state(),
            Self::OverviewConfigured
            | Self::CheckboxUnchecked
            | Self::FilledInput
            | Self::TextArea
            | Self::Summary => configured_widget_book_state(),
            Self::EmptyInputFocused => blank_widget_book_state(),
        };

        TestApp::from_runtime(build_widget_book_application(state).build()?)
    }

    pub(crate) fn prepare(self, window: &TestWindow) -> Result<()> {
        if !matches!(
            self,
            Self::Overview | Self::OverviewConfigured | Self::ScrollViewScrolled
        ) {
            scroll_to_story_target(window, self, 64)?;
        }

        match self {
            Self::Button
            | Self::Checkbox
            | Self::CheckboxUnchecked
            | Self::FilledInput
            | Self::Icon
            | Self::IconButton
            | Self::Separator
            | Self::Switch
            | Self::RadioButton
            | Self::RadioGroup
            | Self::Slider
            | Self::NumberInput
            | Self::TabBar
            | Self::Tabs
            | Self::Menu
            | Self::ProgressBar
            | Self::Spinner
            | Self::Summary
            | Self::ListView
            | Self::TreeView
            | Self::Table
            | Self::SplitView
            | Self::Breadcrumb
            | Self::ColorSwatch
            | Self::ColorPicker
            | Self::ThemePreview
            | Self::ImageWidget
            | Self::TextArea => Ok(()),
            Self::ButtonHover => self.target(window).hover(),
            Self::ButtonPressed => {
                press_target(window, SemanticsRole::Button, PRIMARY_BUTTON_LABEL)
            }
            Self::EmptyInputFocused => self.target(window).focus(),
            Self::SelectExpanded => {
                self.target(window).click()?;
                Ok(())
            }
            Self::ContextMenuOpen | Self::TooltipVisible | Self::PopoverOpen | Self::Dialog => {
                match self {
                    Self::ContextMenuOpen => secondary_click_target(
                        window,
                        SemanticsRole::ContextMenu,
                        CONTEXT_MENU_NAME,
                    ),
                    Self::TooltipVisible => window
                        .get_by_role(SemanticsRole::Button)
                        .with_name(TOOLTIP_TRIGGER_LABEL)
                        .hover(),
                    Self::PopoverOpen => self.target(window).click(),
                    Self::Dialog => {
                        scroll_gallery_by(window, -220.0)?;
                        window
                            .get_by_role(SemanticsRole::Button)
                            .with_name(DIALOG_TRIGGER_LABEL)
                            .click()
                    }
                    _ => Ok(()),
                }
            }
            Self::ScrollViewScrolled => scroll_gallery(window, 1),
            Self::Overview | Self::OverviewConfigured => Ok(()),
        }
    }

    pub(crate) fn target(self, window: &TestWindow) -> Locator {
        match self {
            Self::Overview | Self::OverviewConfigured => window.root(),
            Self::Button | Self::ButtonHover | Self::ButtonPressed => window
                .get_by_role(SemanticsRole::Button)
                .with_name(PRIMARY_BUTTON_LABEL),
            Self::Checkbox | Self::CheckboxUnchecked => window
                .get_by_role(SemanticsRole::CheckBox)
                .with_name(SUBSCRIBE_LABEL),
            Self::FilledInput | Self::EmptyInputFocused => window
                .get_by_role(SemanticsRole::TextInput)
                .with_name(NAME_INPUT_LABEL),
            Self::Icon => window
                .get_by_role(SemanticsRole::Image)
                .with_name(ICON_LABEL),
            Self::IconButton => window
                .get_by_role(SemanticsRole::Button)
                .with_name(ICON_BUTTON_LABEL),
            Self::Separator => window
                .get_by_role(SemanticsRole::Separator)
                .with_name(TOOLBAR_SEPARATOR_NAME),
            Self::Switch => window
                .get_by_role(SemanticsRole::Switch)
                .with_name(SWITCH_LABEL),
            Self::RadioButton => window
                .get_by_role(SemanticsRole::RadioButton)
                .with_name(RADIO_BUTTON_LABEL),
            Self::RadioGroup => window
                .get_by_role(SemanticsRole::RadioGroup)
                .with_name(RADIO_GROUP_NAME),
            Self::Slider => window
                .get_by_role(SemanticsRole::Slider)
                .with_name(SLIDER_NAME),
            Self::NumberInput => window
                .get_by_role(SemanticsRole::SpinBox)
                .with_name(NUMBER_INPUT_NAME),
            Self::TextArea => window
                .get_by_role(SemanticsRole::TextInput)
                .with_name(TEXT_AREA_LABEL),
            Self::SelectExpanded => window
                .get_by_role(SemanticsRole::ComboBox)
                .with_name(SELECT_NAME),
            Self::TabBar => window
                .get_by_role(SemanticsRole::TabBar)
                .with_name(TAB_BAR_NAME),
            Self::Tabs => window.get_by_role(SemanticsRole::Tabs).with_name(TABS_NAME),
            Self::Menu => window.get_by_role(SemanticsRole::Menu).with_name(MENU_NAME),
            Self::ContextMenuOpen => window
                .get_by_role(SemanticsRole::ContextMenu)
                .with_name(CONTEXT_MENU_NAME),
            Self::TooltipVisible => window
                .get_by_role(SemanticsRole::Tooltip)
                .with_name(TOOLTIP_TEXT),
            Self::PopoverOpen => window
                .get_by_role(SemanticsRole::Popover)
                .with_name(POPOVER_NAME),
            Self::Dialog => window
                .get_by_role(SemanticsRole::Dialog)
                .with_name(DIALOG_TITLE),
            Self::ProgressBar => window
                .get_by_role(SemanticsRole::ProgressBar)
                .with_name(PROGRESS_NAME),
            Self::Spinner => window
                .get_by_role(SemanticsRole::BusyIndicator)
                .with_name(SPINNER_NAME),
            Self::ScrollViewScrolled => window
                .get_by_role(SemanticsRole::ScrollView)
                .with_name(GALLERY_SCROLL_NAME),
            Self::Summary => window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name(SUMMARY_NAME),
            Self::ListView => window
                .get_by_role(SemanticsRole::List)
                .with_name(LIST_VIEW_NAME),
            Self::TreeView => window
                .get_by_role(SemanticsRole::Tree)
                .with_name(TREE_VIEW_NAME),
            Self::Table => window
                .get_by_role(SemanticsRole::Table)
                .with_name(TABLE_NAME),
            Self::SplitView => window
                .get_by_role(SemanticsRole::Splitter)
                .with_name(SPLIT_VIEW_NAME),
            Self::Breadcrumb => window
                .get_by_role(SemanticsRole::Breadcrumb)
                .with_name(BREADCRUMB_NAME),
            Self::ColorSwatch => window
                .get_by_role(SemanticsRole::ColorSwatch)
                .with_name(COLOR_SWATCH_NAME),
            Self::ColorPicker => window
                .get_by_role(SemanticsRole::ColorPicker)
                .with_name(COLOR_PICKER_NAME),
            Self::ThemePreview => window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name(THEME_PREVIEW_NAME),
            Self::ImageWidget => window
                .get_by_role(SemanticsRole::Image)
                .with_name(DEMO_IMAGE_LABEL),
        }
    }

    fn capture_target(self) -> (SemanticsRole, Option<&'static str>) {
        match self {
            Self::Overview | Self::OverviewConfigured => {
                (SemanticsRole::Window, Some(WINDOW_TITLE))
            }
            Self::Button | Self::ButtonHover | Self::ButtonPressed => {
                (SemanticsRole::Button, Some(PRIMARY_BUTTON_LABEL))
            }
            Self::Checkbox | Self::CheckboxUnchecked => {
                (SemanticsRole::CheckBox, Some(SUBSCRIBE_LABEL))
            }
            Self::FilledInput | Self::EmptyInputFocused => {
                (SemanticsRole::TextInput, Some(NAME_INPUT_LABEL))
            }
            Self::Icon => (SemanticsRole::Image, Some(ICON_LABEL)),
            Self::IconButton => (SemanticsRole::Button, Some(ICON_BUTTON_LABEL)),
            Self::Separator => (SemanticsRole::Separator, Some(TOOLBAR_SEPARATOR_NAME)),
            Self::Switch => (SemanticsRole::Switch, Some(SWITCH_LABEL)),
            Self::RadioButton => (SemanticsRole::RadioButton, Some(RADIO_BUTTON_LABEL)),
            Self::RadioGroup => (SemanticsRole::RadioGroup, Some(RADIO_GROUP_NAME)),
            Self::Slider => (SemanticsRole::Slider, Some(SLIDER_NAME)),
            Self::NumberInput => (SemanticsRole::SpinBox, Some(NUMBER_INPUT_NAME)),
            Self::TextArea => (SemanticsRole::TextInput, Some(TEXT_AREA_LABEL)),
            Self::SelectExpanded => (SemanticsRole::ComboBox, Some(SELECT_NAME)),
            Self::TabBar => (SemanticsRole::TabBar, Some(TAB_BAR_NAME)),
            Self::Tabs => (SemanticsRole::Tabs, Some(TABS_NAME)),
            Self::Menu => (SemanticsRole::Menu, Some(MENU_NAME)),
            Self::ContextMenuOpen => (SemanticsRole::ContextMenu, Some(CONTEXT_MENU_NAME)),
            Self::TooltipVisible => (SemanticsRole::Tooltip, Some(TOOLTIP_TEXT)),
            Self::PopoverOpen => (SemanticsRole::Popover, Some(POPOVER_NAME)),
            Self::Dialog => (SemanticsRole::Dialog, Some(DIALOG_TITLE)),
            Self::ProgressBar => (SemanticsRole::ProgressBar, Some(PROGRESS_NAME)),
            Self::Spinner => (SemanticsRole::BusyIndicator, Some(SPINNER_NAME)),
            Self::ScrollViewScrolled => (SemanticsRole::ScrollView, Some(GALLERY_SCROLL_NAME)),
            Self::Summary => (SemanticsRole::GenericContainer, Some(SUMMARY_NAME)),
            Self::ListView => (SemanticsRole::List, Some(LIST_VIEW_NAME)),
            Self::TreeView => (SemanticsRole::Tree, Some(TREE_VIEW_NAME)),
            Self::Table => (SemanticsRole::Table, Some(TABLE_NAME)),
            Self::SplitView => (SemanticsRole::Splitter, Some(SPLIT_VIEW_NAME)),
            Self::Breadcrumb => (SemanticsRole::Breadcrumb, Some(BREADCRUMB_NAME)),
            Self::ColorSwatch => (SemanticsRole::ColorSwatch, Some(COLOR_SWATCH_NAME)),
            Self::ColorPicker => (SemanticsRole::ColorPicker, Some(COLOR_PICKER_NAME)),
            Self::ThemePreview => (SemanticsRole::GenericContainer, Some(THEME_PREVIEW_NAME)),
            Self::ImageWidget => (SemanticsRole::Image, Some(DEMO_IMAGE_LABEL)),
        }
    }

    fn story_node(self) -> Option<(SemanticsRole, Option<&'static str>)> {
        match self {
            Self::Button | Self::ButtonHover | Self::ButtonPressed => {
                Some((SemanticsRole::Button, Some(PRIMARY_BUTTON_LABEL)))
            }
            Self::Checkbox | Self::CheckboxUnchecked => {
                Some((SemanticsRole::CheckBox, Some(SUBSCRIBE_LABEL)))
            }
            Self::FilledInput | Self::EmptyInputFocused => {
                Some((SemanticsRole::TextInput, Some(NAME_INPUT_LABEL)))
            }
            Self::Icon => Some((SemanticsRole::Image, Some(ICON_LABEL))),
            Self::IconButton => Some((SemanticsRole::Button, Some(ICON_BUTTON_LABEL))),
            Self::Separator => Some((SemanticsRole::Separator, Some(TOOLBAR_SEPARATOR_NAME))),
            Self::Switch => Some((SemanticsRole::Switch, Some(SWITCH_LABEL))),
            Self::RadioButton => Some((SemanticsRole::RadioButton, Some(RADIO_BUTTON_LABEL))),
            Self::RadioGroup => Some((SemanticsRole::RadioGroup, Some(RADIO_GROUP_NAME))),
            Self::Slider => Some((SemanticsRole::Slider, Some(SLIDER_NAME))),
            Self::NumberInput => Some((SemanticsRole::SpinBox, Some(NUMBER_INPUT_NAME))),
            Self::TextArea => Some((SemanticsRole::TextInput, Some(TEXT_AREA_LABEL))),
            Self::SelectExpanded => Some((SemanticsRole::ComboBox, Some(SELECT_NAME))),
            Self::TabBar => Some((SemanticsRole::TabBar, Some(TAB_BAR_NAME))),
            Self::Tabs => Some((SemanticsRole::Tabs, Some(TABS_NAME))),
            Self::Menu => Some((SemanticsRole::Menu, Some(MENU_NAME))),
            Self::ContextMenuOpen => Some((SemanticsRole::ContextMenu, Some(CONTEXT_MENU_NAME))),
            Self::TooltipVisible => Some((SemanticsRole::Button, Some(TOOLTIP_TRIGGER_LABEL))),
            Self::PopoverOpen => Some((SemanticsRole::Button, Some(POPOVER_TRIGGER_LABEL))),
            Self::Dialog => Some((SemanticsRole::Button, Some(DIALOG_TRIGGER_LABEL))),
            Self::ProgressBar => Some((SemanticsRole::ProgressBar, Some(PROGRESS_NAME))),
            Self::Spinner => Some((SemanticsRole::BusyIndicator, Some(SPINNER_NAME))),
            Self::Summary => Some((SemanticsRole::GenericContainer, Some(SUMMARY_NAME))),
            Self::ListView => Some((SemanticsRole::List, Some(LIST_VIEW_NAME))),
            Self::TreeView => Some((SemanticsRole::Tree, Some(TREE_VIEW_NAME))),
            Self::Table => Some((SemanticsRole::Table, Some(TABLE_NAME))),
            Self::SplitView => Some((SemanticsRole::Splitter, Some(SPLIT_VIEW_NAME))),
            Self::Breadcrumb => Some((SemanticsRole::Breadcrumb, Some(BREADCRUMB_NAME))),
            Self::ColorSwatch => Some((SemanticsRole::ColorSwatch, Some(COLOR_SWATCH_NAME))),
            Self::ColorPicker => Some((SemanticsRole::ColorPicker, Some(COLOR_PICKER_NAME))),
            Self::ThemePreview => Some((SemanticsRole::GenericContainer, Some(THEME_PREVIEW_NAME))),
            Self::ImageWidget => Some((SemanticsRole::Image, Some(DEMO_IMAGE_LABEL))),
            _ => None,
        }
    }
}

pub fn write_visual_artifacts() -> Result<PathBuf> {
    let output_root = artifact_root();
    write_visual_artifacts_to(&output_root)
}

pub(crate) fn write_visual_artifacts_to(output_root: &Path) -> Result<PathBuf> {
    reset_dir(output_root)?;

    for story in StoryCase::ALL {
        let story_dir = output_root.join(story.id());
        create_dir(&story_dir)?;

        let app = story.build_app()?;
        let window = app.main_window()?;
        story.prepare(&window)?;
        let artifacts = window.capture_artifacts()?;
        artifacts.write_to_dir(&story_dir)?;
        rename_window_artifacts(&story_dir)?;

        let screenshot = capture_story_screenshot(story, &window)?;
        screenshot.write_png(story_dir.join("screenshot.png"))?;
        write_text(story_dir.join("story.txt"), story.description())?;
    }

    write_hdr_widget_book_artifacts(output_root)?;

    Ok(output_root.to_path_buf())
}

fn hdr_widget_book_render_options() -> WindowRenderOptions {
    WindowRenderOptions::new(true, 1.0)
        .with_color_management_mode(WindowColorManagementMode::PreferHdr)
        .with_output_color_primaries(WindowOutputColorPrimaries::DisplayP3)
        .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange)
        .with_tone_mapping_mode(WindowToneMappingMode::Automatic)
}

fn write_hdr_widget_book_artifacts(output_root: &Path) -> Result<()> {
    let hdr_dir = output_root.join("hdr-widget-book");
    create_dir(&hdr_dir)?;

    let options = hdr_widget_book_render_options();
    let runtime = build_color_validation_application().build()?;
    for window_id in runtime.window_ids() {
        sui::set_window_render_options(window_id, options);
    }
    let app = TestApp::from_runtime(runtime)?;
    let window = app.main_window()?;

    let artifacts = window.capture_artifacts()?;
    artifacts.write_to_dir(&hdr_dir)?;
    rename_window_artifacts(&hdr_dir)?;
    write_text(
        hdr_dir.join("story.txt"),
        "HDR-configured widget-book validation surface with HDR debug captures.",
    )?;

    let artifact = window.capture_debug_frame(DebugCaptureRequest {
        stage: DebugCaptureStage::HdrIntermediate,
        encoding: DebugCaptureEncoding::Exr,
        sdr_visualization: DebugSdrVisualization::ToneMappedColor,
    })?;
    let DebugCaptureArtifact::HdrLinearRgbaF32(image) = artifact else {
        return Err(Error::new(
            "widget-book HDR artifact capture did not produce an HDR intermediate frame",
        ));
    };

    write_hdr_exr(&image, hdr_dir.join("hdr-intermediate.exr"))?;
    write_hdr_avif(&image, hdr_dir.join("hdr-intermediate.avif"), 1.0)?;
    hdr_luminance_heatmap(&image)?.write_png(hdr_dir.join("luminance-map.png"))?;
    hdr_headroom_heatmap(&image, 1.0)?.write_png(hdr_dir.join("headroom-map.png"))?;
    hdr_clip_mask(&image, 1.0)?.write_png(hdr_dir.join("clip-mask.png"))?;

    let max_channel = image
        .pixels()
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let max_luminance = image
        .pixels()
        .chunks_exact(4)
        .map(|rgba| rgba[0] * 0.2126 + rgba[1] * 0.7152 + rgba[2] * 0.0722)
        .fold(f32::NEG_INFINITY, f32::max);

    let diagnostics_text = if let Some(diagnostics) = window_output_diagnostics(window.id()) {
        format!(
            "view={COLOR_VALIDATION_VIEW_TITLE}\nrequested_color_management_mode={:?}\nrequested_output_primaries={:?}\nrequested_dynamic_range_mode={:?}\nrequested_tone_mapping_mode={:?}\nrequested_sdr_content_brightness_nits={:.0}\nsupports_hdr={}\nnative_hdr_presentation_supported={}\npreferred_dynamic_range={:?}\nactive_output_strategy={:?}\nnotes={}\n",
            diagnostics.requested_color_management_mode,
            diagnostics.requested_output_primaries,
            diagnostics.requested_dynamic_range_mode,
            diagnostics.requested_tone_mapping_mode,
            diagnostics.requested_sdr_content_brightness_nits,
            diagnostics.display_capabilities.supports_hdr,
            diagnostics.display_capabilities.native_hdr_presentation_supported,
            diagnostics.display_capabilities.preferred_dynamic_range,
            diagnostics.active_output_strategy,
            diagnostics.display_capabilities.notes,
        )
    } else {
        format!(
            "view={COLOR_VALIDATION_VIEW_TITLE}\noutput_diagnostics=unavailable\n"
        )
    };
    write_text(hdr_dir.join("output-diagnostics.txt"), &diagnostics_text)?;

    let final_artifact = window.capture_debug_frame(DebugCaptureRequest {
        stage: DebugCaptureStage::FinalComposed,
        encoding: DebugCaptureEncoding::Exr,
        sdr_visualization: DebugSdrVisualization::ToneMappedColor,
    })?;
    let (final_artifact_kind, final_max_channel, final_max_luminance) = match final_artifact {
        DebugCaptureArtifact::HdrLinearRgbaF32(final_image) => {
            write_hdr_exr(&final_image, hdr_dir.join("final-composed.exr"))?;
            write_hdr_avif(&final_image, hdr_dir.join("final-composed.avif"), 1.0)?;
            let max_channel = final_image
                .pixels()
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            let max_luminance = final_image
                .pixels()
                .chunks_exact(4)
                .map(|rgba| rgba[0] * 0.2126 + rgba[1] * 0.7152 + rgba[2] * 0.0722)
                .fold(f32::NEG_INFINITY, f32::max);
            ("hdr", max_channel, max_luminance)
        }
        DebugCaptureArtifact::SdrRgba8(final_image) => {
            Screenshot::new(
                final_image.width(),
                final_image.height(),
                final_image.into_pixels(),
            )?
            .write_png(hdr_dir.join("final-composed.png"))?;
            ("sdr", 1.0, 1.0)
        }
    };

    write_text(
        hdr_dir.join("capture-metrics.txt"),
        &format!(
            "intermediate_max_channel={max_channel}\nintermediate_max_luminance={max_luminance}\nfinal_artifact_kind={final_artifact_kind}\nfinal_max_channel={final_max_channel}\nfinal_max_luminance={final_max_luminance}\n"
        ),
    )?;

    Ok(())
}

pub(crate) fn configured_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
    Rc::new(RefCell::new(WidgetBookState {
        name: "Grace Hopper".to_string(),
        subscribed: false,
        theme_preview_comparison: true,
        button_presses: 1,
        icon_button_presses: 2,
        switch_on: false,
        standalone_radio_selected: true,
        radio_choice: "High".to_string(),
        slider_value: 35.0,
        number_value: 24.0,
        notes: "Line 1\nLine 2".to_string(),
        mode: "Multiply".to_string(),
        tab_bar_choice: "Export".to_string(),
        tabs_choice: "History".to_string(),
        last_menu_action: "Delete layer".to_string(),
        last_context_action: "Duplicate".to_string(),
        dialog_apply_count: 2,
    }))
}

pub(crate) fn blank_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
    Rc::new(RefCell::new(WidgetBookState {
        name: String::new(),
        subscribed: false,
        theme_preview_comparison: true,
        button_presses: 0,
        icon_button_presses: 0,
        switch_on: false,
        standalone_radio_selected: false,
        radio_choice: "Balanced".to_string(),
        slider_value: 50.0,
        number_value: 8.0,
        notes: String::new(),
        mode: String::new(),
        tab_bar_choice: TAB_BAR_OPTIONS[0].to_string(),
        tabs_choice: TAB_PANEL_OPTIONS[0].to_string(),
        last_menu_action: String::new(),
        last_context_action: String::new(),
        dialog_apply_count: 0,
    }))
}

pub(crate) fn scroll_to_story_target(
    window: &TestWindow,
    story: StoryCase,
    max_pages: usize,
) -> Result<()> {
    const SCROLL_STEP: f32 = -180.0;

    let Some((role, name)) = story.story_node() else {
        return Ok(());
    };

    if story_node_is_visible(window, role.clone(), name)? {
        return Ok(());
    }

    for _ in 0..(max_pages * 2) {
        scroll_gallery_by(window, SCROLL_STEP)?;
        if story_node_is_visible(window, role.clone(), name)? {
            return Ok(());
        }
    }

    let snapshot = window.snapshot()?;
    let visible_nodes = snapshot
        .accessibility
        .nodes
        .iter()
        .filter_map(|node| {
            node.name
                .as_deref()
                .map(|name| format!("{:?}:{name}", node.role))
        })
        .collect::<Vec<_>>()
        .join(", ");

    Err(Error::new(format!(
        "failed to scroll story target {:?} {:?} into view; visible nodes: {}",
        role, name, visible_nodes
    )))
}

pub(crate) fn artifact_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("ui-artifacts")
        .join("sui-widget-book")
}

fn reset_dir(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| Error::new(format!("failed to clear {}: {error}", path.display())))?;
    }
    create_dir(path)
}

fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|error| Error::new(format!("failed to create {}: {error}", path.display())))
}

fn write_text(path: PathBuf, contents: &str) -> Result<()> {
    fs::write(&path, contents)
        .map_err(|error| Error::new(format!("failed to write {}: {error}", path.display())))
}

fn rename_window_artifacts(dir: &Path) -> Result<()> {
    rename_if_exists(dir, "screenshot.png", "window.png")?;
    rename_if_exists(dir, "semantics-overlay.png", "window-semantics-overlay.png")?;
    rename_if_exists(dir, "widget-overlay.png", "window-widget-overlay.png")
}

fn rename_if_exists(dir: &Path, from: &str, to: &str) -> Result<()> {
    let from_path = dir.join(from);
    if !from_path.exists() {
        return Ok(());
    }

    let to_path = dir.join(to);
    if to_path.exists() {
        fs::remove_file(&to_path).map_err(|error| {
            Error::new(format!("failed to remove {}: {error}", to_path.display()))
        })?;
    }

    fs::rename(&from_path, &to_path)
        .map_err(|error| Error::new(format!("failed to rename {}: {error}", from_path.display())))
}

fn press_target(window: &TestWindow, role: SemanticsRole, name: &str) -> Result<()> {
    let locator = window.get_by_role(role.clone()).with_name(name);
    let point = node_center(window, role, name)?;

    locator.dispatch_event(Event::Pointer(PointerEvent::new(
        PointerEventKind::Move,
        point,
    )))?;

    let mut down = PointerEvent::new(PointerEventKind::Down, point);
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    locator.dispatch_event(Event::Pointer(down))
}

fn secondary_click_target(window: &TestWindow, role: SemanticsRole, name: &str) -> Result<()> {
    let locator = window.get_by_role(role.clone()).with_name(name);
    let point = node_center(window, role, name)?;

    locator.dispatch_event(Event::Pointer(PointerEvent::new(
        PointerEventKind::Move,
        point,
    )))?;

    let mut down = PointerEvent::new(PointerEventKind::Down, point);
    down.button = Some(PointerButton::Secondary);
    down.buttons = PointerButtons::new(2);
    locator.dispatch_event(Event::Pointer(down))?;

    let mut up = PointerEvent::new(PointerEventKind::Up, point);
    up.button = Some(PointerButton::Secondary);
    locator.dispatch_event(Event::Pointer(up))
}

fn scroll_gallery(window: &TestWindow, pages: usize) -> Result<()> {
    for _ in 0..pages {
        scroll_gallery_by(window, -360.0)?;
    }
    Ok(())
}

fn scroll_gallery_by(window: &TestWindow, delta_y: f32) -> Result<()> {
    let point = gallery_scroll_point(window)?;
    let root = window.root();

    root.dispatch_event(Event::Pointer(PointerEvent::new(
        PointerEventKind::Move,
        point,
    )))?;

    let mut scroll = PointerEvent::new(PointerEventKind::Scroll, point);
    scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, delta_y)));
    root.dispatch_event(Event::Pointer(scroll))
}

fn gallery_scroll_point(window: &TestWindow) -> Result<Point> {
    let snapshot = window.snapshot()?;
    let gallery = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(GALLERY_SCROLL_NAME)
        })
        .ok_or_else(|| Error::new("widget book gallery scroll view is missing"))?;

    Ok(Point::new(
        gallery.bounds.x() + 32.0,
        gallery.bounds.y() + 120.0,
    ))
}

fn story_node_is_visible(
    window: &TestWindow,
    role: SemanticsRole,
    name: Option<&str>,
) -> Result<bool> {
    let snapshot = window.snapshot()?;
    let viewport = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(GALLERY_SCROLL_NAME)
        })
        .or_else(|| {
            snapshot
                .accessibility
                .nodes
                .iter()
                .find(|node| node.role == SemanticsRole::Window)
        })
        .map(|node| node.bounds)
        .unwrap_or(Rect::ZERO);
    Ok(snapshot.accessibility.nodes.iter().any(|node| {
        if node.role != role || node.name.as_deref() != name {
            return false;
        }

        let Some(visible) = node.bounds.intersection(viewport) else {
            return false;
        };

        let node_area = node.bounds.width() * node.bounds.height();
        let visible_area = visible.width() * visible.height();
        node_area > 0.0 && (visible_area / node_area) >= 0.85
    }))
}

fn node_center(window: &TestWindow, role: SemanticsRole, name: &str) -> Result<Point> {
    let snapshot = window.snapshot()?;
    let node = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| node.role == role && node.name.as_deref() == Some(name))
        .ok_or_else(|| Error::new(format!("missing story node {:?} {name}", role)))?;

    Ok(Point::new(
        node.bounds.x() + (node.bounds.width() / 2.0),
        node.bounds.y() + (node.bounds.height() / 2.0),
    ))
}

fn capture_story_screenshot(
    story: StoryCase,
    window: &TestWindow,
) -> Result<sui_testing::Screenshot> {
    let snapshot = window.snapshot()?;
    let screenshot = window.capture_screenshot()?;
    let (role, name) = story.capture_target();
    let bounds = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| node.role == role && node.name.as_deref() == name)
        .map(|node| node.bounds)
        .ok_or_else(|| {
            Error::new(format!(
                "widget book story {} is missing target semantics {:?} {:?}",
                story.id(),
                role,
                name
            ))
        })?;

    let bounds = if let Some(scene) = &snapshot.scene_summary {
        let viewport = scene.viewport;
        if viewport.width > 0.0 && viewport.height > 0.0 {
            let scale_x = screenshot.width() as f32 / viewport.width;
            let scale_y = screenshot.height() as f32 / viewport.height;
            Rect::new(
                bounds.x() * scale_x,
                bounds.y() * scale_y,
                bounds.width() * scale_x,
                bounds.height() * scale_y,
            )
        } else {
            bounds
        }
    } else {
        bounds
    };

    screenshot.crop(bounds).map_err(|error| {
        Error::new(format!(
            "widget book story {} failed to crop screenshot: {}",
            story.id(),
            error
        ))
    })
}
