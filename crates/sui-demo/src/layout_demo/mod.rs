//! The Layout page: flex, grid, responsive containers, panes and safe
//! areas, with the examples shown in frames whose width you choose.

mod flex;
mod frame;
mod phone;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use sui::{
    AdaptiveClass, GridTrackMax, Rect, ResponsiveSidebarMode, Signal, WidgetPodMutVisitor,
    WidgetPodVisitor, prelude::*,
};

use self::flex::{ALIGN, FlexPlayground, FlexSettings, JUSTIFY, flex_code};
use self::frame::{
    BreakpointTicks, DeviceFrame, FrameWidth, MAX_FRAME_WIDTH, MIN_FRAME_WIDTH, ShowSize, SizeTag,
    WidthProbe,
};
use self::phone::{PhoneMock, insets_text, phone_insets};
use crate::app::{
    DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style_when, dev_theme_color,
};
#[cfg(test)]
use crate::demo_support::default_theme_reader;
use crate::demo_support::{DemoTextColor, NamedSection, demo_label, demo_mono_label};
use crate::settings::controls::labeled_control;

pub(crate) const LAYOUT_TAB_LABEL: &str = "Layout";
pub(crate) const LAYOUT_DEMO_SCROLL_NAME: &str = "Layout demo scroll";
const LAYOUT_SUMMARY: &str =
    "Flex, grid, container queries, panes, and safe areas, shown in frames you can resize.";

pub(crate) const FRAME_WIDTH_NAME: &str = "Frame width";
pub(crate) const FRAME_PRESET_NAME: &str = "Frame preset";
pub(crate) const FRAME_READOUT_NAME: &str = "Frame readout";
pub(crate) const SHOW_SIZES_LABEL: &str = "Show sizes";
const PRESETS: [(&str, f32); 3] = [("Phone", 360.0), ("Tablet", 768.0), ("Desktop", 1180.0)];

const SIDEBAR_DOCKS_AT: f32 = 520.0;
const MASTER_DETAIL_SPLITS_AT: f32 = 600.0;
const QUERY_WIDE_AT: f32 = 680.0;
/// Where the responsive examples change, marked under the width slider.
const BREAKPOINTS: [(f32, &str); 3] = [
    (SIDEBAR_DOCKS_AT, "the sidebar docks"),
    (
        MASTER_DETAIL_SPLITS_AT,
        "master and detail sit side by side",
    ),
    (QUERY_WIDE_AT, "the container query goes wide"),
];

pub(crate) const FLEX_DIRECTION_NAME: &str = "Direction";
pub(crate) const FLEX_JUSTIFY_NAME: &str = "Justify";
pub(crate) const FLEX_ALIGN_NAME: &str = "Align items";
pub(crate) const FLEX_GAP_NAME: &str = "Gap";
pub(crate) const FLEX_WRAP_LABEL: &str = "Wrap";
pub(crate) const FLEX_CODE_NAME: &str = "Flex code";
pub(crate) const GRID_COLUMN_NAMES: [&str; 3] = ["Column 1", "Column 2", "Column 3"];
pub(crate) const GRID_GAP_NAME: &str = "Grid gap";
pub(crate) const GRID_CODE_NAME: &str = "Grid code";
pub(crate) const QUERY_RULE_NAME: &str = "Matched rule";
pub(crate) const SIDEBAR_MODE_NAME: &str = "Sidebar mode";
pub(crate) const SIDEBAR_TOGGLE_LABEL: &str = "Toggle sidebar";
pub(crate) const OPEN_DETAIL_LABEL: &str = "Open release notes";
pub(crate) const BACK_LABEL: &str = "Back to documents";
pub(crate) const RESET_SPLIT_LABEL: &str = "Reset split";
pub(crate) const KEYBOARD_LABEL: &str = "Keyboard";
pub(crate) const INSETS_NAME: &str = "Safe area insets";

const CONTROL_WIDTH: f32 = 180.0;
const SLIDER_WIDTH: f32 = 400.0;
const SPLIT_START: f32 = 240.0;

/// What the page's controls chose, shared by the sections.
#[derive(Clone)]
struct PageState {
    frame: Signal<FrameWidth>,
    show_sizes: Signal<bool>,
    flex: Signal<FlexSettings>,
    grid: Signal<GridSettings>,
    keyboard: Signal<bool>,
}

impl PageState {
    fn new() -> Self {
        Self {
            frame: Signal::named(FRAME_WIDTH_NAME, FrameWidth::new(PRESETS[1].1)),
            show_sizes: Signal::named(SHOW_SIZES_LABEL, false),
            flex: Signal::named("Flex settings", FlexSettings::default()),
            grid: Signal::named("Grid settings", GridSettings::default()),
            keyboard: Signal::named(KEYBOARD_LABEL, false),
        }
    }
}

pub(crate) fn build_layout_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let page = PageState::new();
    Background::new(
        theme_reader().palette.surface,
        ScrollView::vertical(Padding::all(
            24.0,
            Stack::vertical()
                .spacing(32.0)
                .alignment(Alignment::Stretch)
                .with_child(header(&theme_reader))
                .with_child(frame_bar(&theme_reader, &page))
                .with_child(flex_section(&theme_reader, &page))
                .with_child(grid_section(&theme_reader, &page))
                .with_child(responsive_section(&theme_reader, &page))
                .with_child(panes_section(&theme_reader, &page))
                .with_child(safe_area_section(&theme_reader, &page)),
        ))
        .name(LAYOUT_DEMO_SCROLL_NAME),
    )
    .brush_when(dev_theme_color(&theme_reader, |theme| {
        theme.palette.surface
    }))
}

/// The page on its own, for tests.
#[cfg(test)]
pub(crate) fn build_layout_application() -> Application {
    App::new()
        .window(
            Window::new(LAYOUT_TAB_LABEL)
                .initial_size(Size::new(1100.0, 900.0))
                .root(build_layout_demo_with_theme(default_theme_reader())),
        )
        .into_application()
}

fn header(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(demo_label(
            theme_reader,
            LAYOUT_TAB_LABEL,
            DemoTextRole::PageTitle,
            DemoTextColor::Text,
        ))
        .with_child(demo_label(
            theme_reader,
            LAYOUT_SUMMARY,
            DemoTextRole::Supporting,
            DemoTextColor::Muted,
        ))
}

/// Width slider with the breakpoints marked, presets, and the size toggle.
fn frame_bar(theme_reader: &DevThemeReader, page: &PageState) -> impl Widget + use<> {
    let slider = {
        let read = page.frame.clone();
        let write = page.frame.clone();
        Slider::new(FRAME_WIDTH_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .range(f64::from(MIN_FRAME_WIDTH), f64::from(MAX_FRAME_WIDTH))
            .step(10.0)
            .value_when(move || f64::from(read.get().requested))
            .on_change(move |width| {
                write.update(|frame| frame.requested = width as f32);
            })
    };
    let presets = {
        let read = page.frame.clone();
        let write = page.frame.clone();
        SegmentedControl::new(FRAME_PRESET_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .segments(PRESETS.map(|(label, _)| label))
            .selected_when(move || {
                let requested = read.get().requested;
                PRESETS
                    .iter()
                    .position(|(_, width)| (requested - width).abs() < 0.5)
            })
            .on_change(move |index, _| {
                write.update(|frame| frame.requested = PRESETS[index].1);
            })
    };
    let show_sizes = {
        let read = page.show_sizes.clone();
        let write = page.show_sizes.clone();
        Switch::new(SHOW_SIZES_LABEL)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .on_when(move || read.get())
            .on_toggle(move |on| {
                write.set(on);
            })
    };
    let readout = demo_label(theme_reader, "", DemoTextRole::Body, DemoTextColor::Text)
        .semantic_name(FRAME_READOUT_NAME)
        .text_from(page.frame.select(|frame| frame.readout()));
    let legend = BREAKPOINTS
        .iter()
        .map(|(width, what)| format!("{width} px: {what}"))
        .collect::<Vec<_>>()
        .join(" · ");

    Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Flex::horizontal()
                .gap(24.0)
                .wrap(FlexWrap::Wrap)
                .align_items(Alignment::End)
                .with_child(labeled_control(
                    theme_reader,
                    FRAME_WIDTH_NAME,
                    SLIDER_WIDTH,
                    Stack::vertical()
                        .spacing(2.0)
                        .alignment(Alignment::Stretch)
                        .with_child(slider)
                        .with_child(BreakpointTicks::new(
                            theme_reader,
                            &page.frame,
                            &BREAKPOINTS,
                        )),
                ))
                .with_child(labeled_control(
                    theme_reader,
                    FRAME_PRESET_NAME,
                    260.0,
                    presets,
                ))
                .with_child(lift(22.0, readout))
                .with_child(lift(20.0, show_sizes)),
        )
        .with_child(demo_label(
            theme_reader,
            format!("Breakpoints — {legend}. Drag a frame's right edge to resize it too."),
            DemoTextRole::Metadata,
            DemoTextColor::Muted,
        ))
}

fn section<W>(
    theme_reader: &DevThemeReader,
    title: &str,
    summary: &str,
    body: W,
) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    NamedSection::new(
        title,
        Stack::vertical()
            .spacing(12.0)
            .alignment(Alignment::Stretch)
            .with_child(demo_label(
                theme_reader,
                title,
                DemoTextRole::SectionTitle,
                DemoTextColor::Text,
            ))
            .with_child(demo_label(
                theme_reader,
                summary,
                DemoTextRole::Supporting,
                DemoTextColor::Muted,
            ))
            .with_child(body),
    )
}

/// `child` raised by `bottom`, to sit level with the controls beside it.
fn lift<W>(bottom: f32, child: W) -> Padding
where
    W: Widget + 'static,
{
    Padding::new(
        Insets {
            bottom,
            ..Insets::all(0.0)
        },
        child,
    )
}

/// Controls in a row that wraps on narrow pages.
fn control_row() -> Flex {
    Flex::horizontal()
        .gap(20.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::End)
}

/// Builder code in a panel, following `code`.
fn code_panel<O>(theme_reader: &DevThemeReader, name: &str, code: O) -> impl Widget + use<O>
where
    O: sui::Observable<String> + 'static,
{
    LayoutTile::new(
        dev_theme_color(theme_reader, |theme| theme.palette.control),
        Some(dev_theme_color(theme_reader, |theme| theme.palette.border)),
        Padding::all(
            14.0,
            demo_mono_label(theme_reader, "", DemoTextRole::Metadata, |theme| {
                theme.palette.text
            })
            .semantic_name(name)
            .text_from(code),
        ),
    )
}

fn flex_section(theme_reader: &DevThemeReader, page: &PageState) -> impl Widget + use<> {
    let settings = &page.flex;
    let direction = {
        let read = settings.clone();
        let write = settings.clone();
        SegmentedControl::new(FLEX_DIRECTION_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .segments(["Row", "Column"])
            .selected_when(move || Some(usize::from(read.get().column)))
            .on_change(move |index, _| {
                write.update(|settings| settings.column = index == 1);
            })
    };
    let justify = {
        let read = settings.clone();
        let write = settings.clone();
        Select::new(FLEX_JUSTIFY_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .options(JUSTIFY.map(|(_, label, _)| label))
            .selected_when(move || Some(read.get().justify))
            .on_change(move |index, _| {
                write.update(|settings| settings.justify = index);
            })
    };
    let align = {
        let read = settings.clone();
        let write = settings.clone();
        Select::new(FLEX_ALIGN_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .options(ALIGN.map(|(_, label, _)| label))
            .selected_when(move || Some(read.get().align))
            .on_change(move |index, _| {
                write.update(|settings| settings.align = index);
            })
    };
    let gap = {
        let read = settings.clone();
        let write = settings.clone();
        Slider::new(FLEX_GAP_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .range(0.0, 32.0)
            .step(4.0)
            .value_when(move || f64::from(read.get().gap))
            .on_change(move |gap| {
                write.update(|settings| settings.gap = gap as f32);
            })
    };
    let wrap = {
        let read = settings.clone();
        let write = settings.clone();
        Switch::new(FLEX_WRAP_LABEL)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .on_when(move || read.get().wrap)
            .on_toggle(move |on| {
                write.update(|settings| settings.wrap = on);
            })
    };

    section(
        theme_reader,
        "Flex",
        "Lay items along a row or column. Press a tile to switch it between a fixed size, growing into free space, and growing up to a cap.",
        Stack::vertical()
            .spacing(14.0)
            .alignment(Alignment::Stretch)
            .with_child(
                control_row()
                    .with_child(labeled_control(
                        theme_reader,
                        FLEX_DIRECTION_NAME,
                        CONTROL_WIDTH,
                        direction,
                    ))
                    .with_child(labeled_control(
                        theme_reader,
                        FLEX_JUSTIFY_NAME,
                        CONTROL_WIDTH,
                        justify,
                    ))
                    .with_child(labeled_control(
                        theme_reader,
                        FLEX_ALIGN_NAME,
                        CONTROL_WIDTH,
                        align,
                    ))
                    .with_child(labeled_control(
                        theme_reader,
                        FLEX_GAP_NAME,
                        CONTROL_WIDTH,
                        gap,
                    ))
                    .with_child(lift(8.0, wrap)),
            )
            .with_child(DeviceFrame::new(
                theme_reader,
                &page.frame,
                Padding::all(
                    12.0,
                    FlexPlayground::new(theme_reader, settings, &page.show_sizes),
                ),
            ))
            .with_child(code_panel(
                theme_reader,
                FLEX_CODE_NAME,
                settings.select(flex_code),
            )),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrackKind {
    Fixed,
    Auto,
    Fraction,
    MinMax,
}

const TRACK_KINDS: [(TrackKind, &str); 4] = [
    (TrackKind::Fixed, "Fixed 160"),
    (TrackKind::Auto, "Auto"),
    (TrackKind::Fraction, "1fr"),
    (TrackKind::MinMax, "minmax(0, 1fr)"),
];

impl TrackKind {
    fn track(self) -> GridTrack {
        match self {
            Self::Fixed => GridTrack::Fixed(160.0),
            Self::Auto => GridTrack::Auto,
            Self::Fraction => GridTrack::Fraction(1.0),
            Self::MinMax => GridTrack::MinMax {
                min: 0.0,
                max: GridTrackMax::Fraction(1.0),
            },
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Fixed => "GridTrack::Fixed(160.0)",
            Self::Auto => "GridTrack::Auto",
            Self::Fraction => "GridTrack::Fraction(1.0)",
            Self::MinMax => "GridTrack::MinMax { min: 0.0, max: GridTrackMax::Fraction(1.0) }",
        }
    }
}

/// The grid section's choices: a track kind, by index into
/// [`TRACK_KINDS`], for each column, and the gap.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GridSettings {
    tracks: [usize; 3],
    gap: f32,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            tracks: [0, 1, 2],
            gap: 12.0,
        }
    }
}

fn grid_code(settings: &GridSettings) -> String {
    let tracks = settings
        .tracks
        .map(|index| TRACK_KINDS[index].0.code())
        .join(",\n    ");
    format!("Grid::new([\n    {tracks},\n])\n.gap({:.1})", settings.gap)
}

fn grid_section(theme_reader: &DevThemeReader, page: &PageState) -> impl Widget + use<> {
    let settings = &page.grid;
    let mut controls = control_row();
    for (column, name) in GRID_COLUMN_NAMES.iter().enumerate() {
        let read = settings.clone();
        let write = settings.clone();
        controls.push(labeled_control(
            theme_reader,
            name,
            CONTROL_WIDTH,
            Select::new(*name)
                .theme_when(clone_dev_theme_reader(theme_reader))
                .options(TRACK_KINDS.map(|(_, label)| label))
                .selected_when(move || Some(read.get().tracks[column]))
                .on_change(move |index, _| {
                    write.update(|settings| settings.tracks[column] = index);
                }),
        ));
    }
    let gap = {
        let read = settings.clone();
        let write = settings.clone();
        Slider::new(GRID_GAP_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .range(0.0, 32.0)
            .step(4.0)
            .value_when(move || f64::from(read.get().gap))
            .on_change(move |gap| {
                write.update(|settings| settings.gap = gap as f32);
            })
    };
    controls.push(labeled_control(
        theme_reader,
        GRID_GAP_NAME,
        CONTROL_WIDTH,
        gap,
    ));

    let reader = Rc::clone(theme_reader);
    let show_sizes = page.show_sizes.clone();
    let example = RebuildOnChange::new_observable(settings.clone(), move |settings| {
        WidgetPod::new(grid_example(&reader, settings, &show_sizes))
    });

    section(
        theme_reader,
        "Grid",
        "Choose how each column is sized. The top row reads each track's resolved width; the media cell keeps a 16:9 ratio at any width.",
        Stack::vertical()
            .spacing(14.0)
            .alignment(Alignment::Stretch)
            .with_child(controls)
            .with_child(DeviceFrame::new(
                theme_reader,
                &page.frame,
                Padding::all(12.0, example),
            ))
            .with_child(code_panel(
                theme_reader,
                GRID_CODE_NAME,
                settings.select(grid_code),
            )),
    )
}

fn grid_example(
    theme_reader: &DevThemeReader,
    settings: &GridSettings,
    show_sizes: &Signal<bool>,
) -> impl Widget + use<> {
    let kinds = settings.tracks.map(|index| TRACK_KINDS[index]);
    let mut grid = Grid::new(kinds.map(|(kind, _)| kind.track())).gap(settings.gap);
    for (column, (_, label)) in kinds.iter().enumerate() {
        grid.push(SizeTag::new(
            theme_reader,
            *label,
            ShowSize::Always,
            soft_tile(theme_reader, GRID_HUES[column]).min_size(Size::new(48.0, 36.0)),
        ));
    }
    grid.push(SizeTag::new(
        theme_reader,
        "",
        ShowSize::When(show_sizes.clone()),
        tile(theme_reader, "Navigation", GRID_HUES[0]),
    ));
    grid.push(SizeTag::new(
        theme_reader,
        "",
        ShowSize::When(show_sizes.clone()),
        AspectRatio::new(
            16.0 / 9.0,
            tile(theme_reader, "16:9 media", GRID_HUES[1]).min_size(Size::new(96.0, 40.0)),
        ),
    ));
    grid.push(SizeTag::new(
        theme_reader,
        "",
        ShowSize::When(show_sizes.clone()),
        tile(theme_reader, "Notes sized by their words", GRID_HUES[2]),
    ));
    grid
}

const GRID_HUES: [DecorativeHue; 3] = [
    DecorativeHue::Cyan,
    DecorativeHue::Violet,
    DecorativeHue::Green,
];

fn sidebar_mode_name(mode: ResponsiveSidebarMode) -> String {
    match mode {
        ResponsiveSidebarMode::OverlayClosed => "Overlay, closed",
        ResponsiveSidebarMode::OverlayOpen => "Overlay, open",
        ResponsiveSidebarMode::Rail => "Rail",
        ResponsiveSidebarMode::Inline => "Inline",
    }
    .to_string()
}

fn responsive_section(theme_reader: &DevThemeReader, page: &PageState) -> impl Widget + use<> {
    section(
        theme_reader,
        "Responsive",
        "These examples follow the frame's width, not the window's: a container query, an adaptive sidebar, master-detail navigation, and a toolbar that wraps.",
        DeviceFrame::new(
            theme_reader,
            &page.frame,
            Padding::all(
                12.0,
                Stack::vertical()
                    .spacing(20.0)
                    .alignment(Alignment::Stretch)
                    .with_child(example(
                        theme_reader,
                        "Container query",
                        container_query_example(theme_reader),
                    ))
                    .with_child(example(
                        theme_reader,
                        "Adaptive sidebar",
                        sidebar_example(theme_reader),
                    ))
                    .with_child(example(
                        theme_reader,
                        "Master-detail",
                        master_detail_example(theme_reader),
                    ))
                    .with_child(example(
                        theme_reader,
                        "Wrapping toolbar",
                        toolbar_example(theme_reader),
                    )),
            ),
        ),
    )
}

fn example<W>(theme_reader: &DevThemeReader, title: &str, body: W) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(demo_label(
            theme_reader,
            title,
            DemoTextRole::CardTitle,
            DemoTextColor::Text,
        ))
        .with_child(body)
}

fn rule_label(theme_reader: &DevThemeReader, rule: &str) -> Label {
    demo_label(
        theme_reader,
        rule,
        DemoTextRole::Metadata,
        DemoTextColor::Muted,
    )
    .semantic_name(QUERY_RULE_NAME)
}

fn container_query_example(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    ConstraintView::new(
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(rule_label(
                theme_reader,
                "Default rule: stacked, below 680 px",
            ))
            .with_child(tile(theme_reader, "Header", DecorativeHue::Amber))
            .with_child(tile(theme_reader, "Content", DecorativeHue::Blue)),
    )
    .when(
        ConstraintQuery::new().min_width(QUERY_WIDE_AT),
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(rule_label(theme_reader, "min_width(680): side by side"))
            .with_child(
                Grid::new([GridTrack::Fixed(180.0), GridTrack::Fraction(1.0)])
                    .gap(10.0)
                    .with_child(tile(theme_reader, "Header", DecorativeHue::Amber))
                    .with_child(tile(theme_reader, "Content", DecorativeHue::Blue)),
            ),
    )
}

fn sidebar_example(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let state = ResponsiveSidebarState::new();
    let mode = Signal::named(SIDEBAR_MODE_NAME, ResponsiveSidebarMode::Inline);
    let toggle = {
        let state = state.clone();
        let mode = mode.clone();
        live_button(theme_reader, SIDEBAR_TOGGLE_LABEL).on_press(move || match mode.get() {
            ResponsiveSidebarMode::OverlayClosed | ResponsiveSidebarMode::OverlayOpen => {
                state.toggle_overlay();
            }
            ResponsiveSidebarMode::Rail | ResponsiveSidebarMode::Inline => {
                state.toggle_expanded();
            }
        })
    };
    let badge = demo_label(theme_reader, "", DemoTextRole::Body, DemoTextColor::Text)
        .semantic_name(SIDEBAR_MODE_NAME)
        .text_from(mode.select(|mode| sidebar_mode_name(*mode)));
    let record = mode.clone();

    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Stack::horizontal()
                .spacing(12.0)
                .alignment(Alignment::Center)
                .with_child(toggle)
                .with_child(badge),
        )
        .with_child(
            SizedBox::new().height(168.0).with_child(
                ResponsiveSidebar::new(
                    live_fill(
                        theme_reader,
                        |theme| theme.palette.control,
                        Padding::all(
                            12.0,
                            Stack::vertical()
                                .spacing(8.0)
                                .alignment(Alignment::Stretch)
                                .with_child(live_label(theme_reader, "Files"))
                                .with_child(live_button(theme_reader, "src"))
                                .with_child(live_button(theme_reader, "docs")),
                        ),
                    ),
                    live_fill(
                        theme_reader,
                        |theme| theme.palette.surface_raised,
                        Align::center(live_label(
                            theme_reader,
                            "Docked from 520 px; an overlay below",
                        )),
                    ),
                )
                .name("Adaptive sidebar preview")
                .theme_when(clone_dev_theme_reader(theme_reader))
                .breakpoints(AdaptiveBreakpoints::new(SIDEBAR_DOCKS_AT, 900.0))
                .state(state)
                .split_state(SplitState::pixels(180.0))
                .rail_width(56.0)
                .overlay_width(240.0)
                .on_mode_change(move |mode| {
                    record.set(mode);
                }),
            ),
        )
}

fn master_detail_example(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let breakpoints = AdaptiveBreakpoints::new(MASTER_DETAIL_SPLITS_AT, 900.0);
    let compact = Signal::named("Master-detail compact", true);
    let navigation = MasterDetailState::default();
    let show_detail = navigation.clone();
    let show_master = navigation.clone();
    let record = compact.clone();

    // The back button only makes sense while one pane shows at a time.
    let back = SwitchView::new()
        .selected_from(compact.select(|compact| usize::from(*compact)))
        .with_child(SizedBox::new())
        .with_child(live_button(theme_reader, BACK_LABEL).on_press(move || {
            show_master.show_master();
        }));

    WidthProbe::new(
        move |width| {
            record.set(breakpoints.classify(width) == AdaptiveClass::Compact);
        },
        SizedBox::new().height(176.0).with_child(
            MasterDetail::new(
                live_fill(
                    theme_reader,
                    |theme| theme.palette.control,
                    Padding::all(
                        12.0,
                        Stack::vertical()
                            .spacing(8.0)
                            .alignment(Alignment::Stretch)
                            .with_child(live_label(theme_reader, "Documents"))
                            .with_child(live_button(theme_reader, OPEN_DETAIL_LABEL).on_press(
                                move || {
                                    show_detail.show_detail();
                                },
                            )),
                    ),
                ),
                live_fill(
                    theme_reader,
                    |theme| theme.palette.surface_raised,
                    Padding::all(
                        12.0,
                        Stack::vertical()
                            .spacing(8.0)
                            .alignment(Alignment::Stretch)
                            .with_child(live_label(theme_reader, "Release notes"))
                            .with_child(back),
                    ),
                ),
            )
            .state(navigation)
            .breakpoints(breakpoints)
            .split_state(SplitState::pixels(220.0)),
        ),
    )
}

fn toolbar_example(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let mut toolbar = Toolbar::horizontal()
        .theme_when(clone_dev_theme_reader(theme_reader))
        .wrapping()
        .line_spacing(8.0)
        .divider(false)
        .with_child(Button::primary("Run").theme_when(clone_dev_theme_reader(theme_reader)));
    for action in [
        "Format",
        "Inspect",
        "Share",
        "Duplicate",
        "Rename",
        "Export",
        "History",
        "Compare",
        "Archive",
        "More actions",
    ] {
        toolbar = toolbar.with_child(live_button(theme_reader, action));
    }
    toolbar
}

fn panes_section(theme_reader: &DevThemeReader, page: &PageState) -> impl Widget + use<> {
    let split = SplitState::pixels(SPLIT_START);
    let reset = split.clone();
    section(
        theme_reader,
        "Panes",
        "Drag the divider. Each pane keeps its minimum, 160 px and 240 px, and the split state outlives the widget.",
        Stack::vertical()
            .spacing(14.0)
            .alignment(Alignment::Stretch)
            .with_child(control_row().with_child(
                live_button(theme_reader, RESET_SPLIT_LABEL).on_press(move || {
                    reset.set_pixels(SPLIT_START);
                }),
            ))
            .with_child(DeviceFrame::new(
                theme_reader,
                &page.frame,
                SizedBox::new().height(180.0).with_child(
                    SplitView::horizontal(
                        SizeTag::new(
                            theme_reader,
                            "Sidebar",
                            ShowSize::Always,
                            live_fill(
                                theme_reader,
                                |theme| theme.palette.control,
                                Align::center(live_label(theme_reader, "At least 160 px")),
                            ),
                        ),
                        SizeTag::new(
                            theme_reader,
                            "Content",
                            ShowSize::Always,
                            live_fill(
                                theme_reader,
                                |theme| theme.palette.surface_raised,
                                Align::center(live_label(theme_reader, "At least 240 px")),
                            ),
                        ),
                    )
                    .name("Split view preview")
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .state(split)
                    .min_first(160.0)
                    .min_second(240.0),
                ),
            )),
    )
}

fn safe_area_section(theme_reader: &DevThemeReader, page: &PageState) -> impl Widget + use<> {
    let keyboard = {
        let read = page.keyboard.clone();
        let write = page.keyboard.clone();
        Switch::new(KEYBOARD_LABEL)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .on_when(move || read.get())
            .on_toggle(move |on| {
                write.set(on);
            })
    };
    let insets = demo_label(theme_reader, "", DemoTextRole::Body, DemoTextColor::Text)
        .semantic_name(INSETS_NAME)
        .text_from(
            page.keyboard
                .select(|keyboard| insets_text(phone_insets(*keyboard))),
        );
    let reader = Rc::clone(theme_reader);
    let screen = RebuildOnChange::new_observable(page.keyboard.clone(), move |keyboard| {
        WidgetPod::new(
            SafeArea::new(phone_content(&reader))
                .edges(SafeAreaEdges::ALL)
                .minimum(phone_insets(*keyboard)),
        )
    });

    section(
        theme_reader,
        "Safe area",
        "Content stays clear of the status bar, home indicator, and keyboard. Here SafeArea::minimum stands in for the insets a phone reports; SafeArea uses whichever is larger.",
        Flex::horizontal()
            .gap(32.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Start)
            .with_child(PhoneMock::new(theme_reader, &page.keyboard, screen))
            .with_child(
                Stack::vertical()
                    .spacing(12.0)
                    .alignment(Alignment::Start)
                    .with_child(keyboard)
                    .with_child(insets)
                    .with_child(demo_label(
                        theme_reader,
                        "Tinted bands are the insets. The message field rides above the keyboard.",
                        DemoTextRole::Supporting,
                        DemoTextColor::Muted,
                    )),
            ),
    )
}

fn phone_content(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    live_fill(
        theme_reader,
        |theme| theme.palette.surface_raised,
        Padding::all(
            10.0,
            Flex::vertical()
                .gap(8.0)
                .align_items(Alignment::Stretch)
                .with_child(demo_label(
                    theme_reader,
                    "Messages",
                    DemoTextRole::CardTitle,
                    DemoTextColor::Text,
                ))
                .with_child(
                    soft_tile(theme_reader, DecorativeHue::Blue).min_size(Size::new(0.0, 44.0)),
                )
                .with_child(
                    soft_tile(theme_reader, DecorativeHue::Green).min_size(Size::new(0.0, 44.0)),
                )
                .with_child(
                    soft_tile(theme_reader, DecorativeHue::Violet).min_size(Size::new(0.0, 44.0)),
                )
                .spacer()
                .with_child(
                    LayoutTile::new(
                        dev_theme_color(theme_reader, |theme| theme.palette.field),
                        Some(dev_theme_color(theme_reader, |theme| theme.palette.border)),
                        Padding::all(
                            10.0,
                            demo_label(
                                theme_reader,
                                "Message",
                                DemoTextRole::Body,
                                DemoTextColor::Muted,
                            ),
                        ),
                    )
                    .min_size(Size::new(0.0, 40.0)),
                ),
        ),
    )
}

/// A pane filled with a live theme role, so it follows theme switches after
/// the demo is built.
fn live_fill<W>(
    theme_reader: &DevThemeReader,
    role: fn(&DefaultTheme) -> Color,
    child: W,
) -> Background
where
    W: Widget + 'static,
{
    let reader = Rc::clone(theme_reader);
    Background::new(role(&theme_reader()), child).brush_when(move || role(&reader()))
}

fn live_label(theme_reader: &DevThemeReader, text: &str) -> Label {
    Label::new(text).style_when(demo_text_style_when(
        theme_reader,
        DemoTextRole::Body,
        |theme| theme.palette.text,
    ))
}

fn live_button(theme_reader: &DevThemeReader, label: &str) -> Button {
    Button::new(label).theme_when(clone_dev_theme_reader(theme_reader))
}

/// A solid tile in `hue`, labeled `label`.
fn tile(theme_reader: &DevThemeReader, label: &str, hue: DecorativeHue) -> LayoutTile {
    LayoutTile::new(
        dev_theme_color(theme_reader, move |theme| theme.decorative.get(hue).solid),
        None::<fn() -> Color>,
        Align::center(Padding::all(
            8.0,
            Label::new(label).style_when(demo_text_style_when(
                theme_reader,
                DemoTextRole::Metadata,
                move |theme| theme.decorative.get(hue).on_solid,
            )),
        )),
    )
    .min_size(Size::new(72.0, 44.0))
}

/// A quiet tile in `hue` with nothing in it.
fn soft_tile(theme_reader: &DevThemeReader, hue: DecorativeHue) -> LayoutTile {
    LayoutTile::new(
        dev_theme_color(theme_reader, move |theme| theme.decorative.get(hue).soft),
        Some(dev_theme_color(theme_reader, move |theme| {
            theme.decorative.get(hue).border
        })),
        SizedBox::new(),
    )
}

/// A rounded, filled box around `child`, at least `min_size`.
struct LayoutTile {
    fill: Box<dyn Fn() -> Color>,
    border: Option<Box<dyn Fn() -> Color>>,
    min_size: Size,
    child: SingleChild,
}

impl LayoutTile {
    fn new<W, F, B>(fill: F, border: Option<B>, child: W) -> Self
    where
        W: Widget + 'static,
        F: Fn() -> Color + 'static,
        B: Fn() -> Color + 'static,
    {
        Self {
            fill: Box::new(fill),
            border: border.map(|border| Box::new(border) as Box<dyn Fn() -> Color>),
            min_size: Size::ZERO,
            child: SingleChild::new(child),
        }
    }

    fn min_size(mut self, min_size: Size) -> Self {
        self.min_size = min_size;
        self
    }
}

impl Widget for LayoutTile {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child = self.child.measure(ctx, constraints.loosen());
        constraints.clamp(Size::new(
            child.width.max(self.min_size.width),
            child.height.max(self.min_size.height),
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let path = Path::rounded_rect(bounds, 8.0);
        ctx.fill(path.clone(), (self.fill)());
        if let Some(border) = &self.border {
            ctx.stroke(
                Path::rounded_rect(bounds.inflate(-0.5, -0.5), 7.5),
                border(),
                StrokeStyle::new(1.0),
            );
        }
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
