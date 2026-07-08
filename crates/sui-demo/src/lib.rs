mod animation_demo;
mod app;
mod drag_drop_demo;
mod layout_demo;
#[cfg(feature = "markdown")]
mod markdown_demo;
mod paint_demo;
mod vector_demo;
pub mod widget_book;

#[cfg(not(target_arch = "wasm32"))]
use app::{DesktopAutomationMode, build_dev_application_with_automation};
pub use app::{build_dev_application, build_dev_application_with_widget_book_bounds};

#[cfg(not(target_arch = "wasm32"))]
use std::env;
#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
use std::{
    collections::{HashMap, HashSet},
    io::{self, Stdout},
    time::Duration,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::widget_book::GALLERY_SCROLL_NAME;
use crate::widget_book::{
    build_color_validation_application, build_retained_text_benchmark_application,
    build_text_editing_benchmark_application, build_text_rendering_comparison_application,
    build_widget_book_application, default_widget_book_state,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event as TerminalEvent, KeyCode,
        KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect as TerminalRect},
    style::{Color as TerminalColor, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap, block::Title,
    },
};
use sui::Application;
#[cfg(not(target_arch = "wasm32"))]
use sui::{
    DesktopAutomationAction, DesktopAutomationConfig, DesktopPlatform, SceneStatisticsDetailMode,
    SemanticsRole, set_window_render_options, set_window_scene_statistics_detail_mode,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
use sui::{
    Event, ImeEvent, KeyState, KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent,
    PointerEventKind, Rect as SuiRect, SemanticsAction, SemanticsNode, SemanticsValue, ToggleState,
    Vector,
};
use sui::{
    WindowColorManagementMode, WindowDynamicRangeMode, WindowOutputColorPrimaries,
    WindowRenderOptions, WindowToneMappingMode,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
use sui_testing::TestWindow;
#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
use sui_tui::{TuiLayoutMode, TuiRenderOptions, render_snapshot};

#[cfg(not(target_arch = "wasm32"))]
const DESKTOP_NO_VSYNC_ENV: &str = "SUI_DEMO_NO_VSYNC";
#[cfg(not(target_arch = "wasm32"))]
const DESKTOP_AUTOMATION_ENV: &str = "SUI_DEMO_AUTOMATION";
// WebGPU HDR canvas output treats 1.0 as ordinary browser white and allows
// values above 1.0 for HDR highlights, so the web default keeps SDR UI at 1.0.
const DEFAULT_WEB_SDR_CONTENT_BRIGHTNESS_NITS: f32 = 80.0;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DesktopLaunchMode {
    vsync_enabled: bool,
    automation: Option<DesktopLaunchAutomation>,
    #[cfg(feature = "tui")]
    tui: Option<DesktopTuiLaunchMode>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for DesktopLaunchMode {
    fn default() -> Self {
        Self {
            vsync_enabled: true,
            automation: None,
            #[cfg(feature = "tui")]
            tui: None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopLaunchAutomation {
    WidgetBookScroll,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopTuiLaunchKind {
    Interactive,
    DumpAccessibility,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DesktopTuiLaunchMode {
    kind: DesktopTuiLaunchKind,
    layout: TuiLayoutMode,
    show_hidden: bool,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
impl Default for DesktopTuiLaunchMode {
    fn default() -> Self {
        Self {
            kind: DesktopTuiLaunchKind::Interactive,
            layout: TuiLayoutMode::Spatial,
            show_hidden: false,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_desktop_automation(
    raw_value: Option<&str>,
) -> sui::Result<Option<DesktopLaunchAutomation>> {
    match raw_value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        None => Ok(None),
        Some("widget-book-scroll") => Ok(Some(DesktopLaunchAutomation::WidgetBookScroll)),
        Some(other) => Err(sui::Error::new(format!(
            "unsupported sui-demo automation `{other}`; supported values: widget-book-scroll"
        ))),
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn parse_tui_layout(raw_value: &str) -> sui::Result<TuiLayoutMode> {
    match raw_value {
        "structured" => Ok(TuiLayoutMode::Structured),
        "spatial" => Ok(TuiLayoutMode::Spatial),
        other => Err(sui::Error::new(format!(
            "unsupported sui-demo TUI layout `{other}`; supported values: structured, spatial"
        ))),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn app_automation_mode(mode: Option<DesktopLaunchAutomation>) -> Option<DesktopAutomationMode> {
    match mode {
        Some(DesktopLaunchAutomation::WidgetBookScroll) => {
            Some(DesktopAutomationMode::WidgetBookScroll)
        }
        None => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_automation_config(
    mode: Option<DesktopLaunchAutomation>,
) -> Option<DesktopAutomationConfig> {
    match mode {
        Some(DesktopLaunchAutomation::WidgetBookScroll) => Some(DesktopAutomationConfig {
            label: "widget-book-scroll".to_string(),
            target_role: SemanticsRole::ScrollView,
            target_name: GALLERY_SCROLL_NAME.to_string(),
            action: DesktopAutomationAction::ScrollPixels {
                delta: sui::Vector::new(0.0, -48.0),
            },
            step_interval: std::time::Duration::from_millis(8),
            duration: std::time::Duration::from_secs(4),
            report_interval: std::time::Duration::from_millis(500),
            startup_timeout: std::time::Duration::from_secs(2),
        }),
        None => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn env_requests_no_vsync(raw_value: Option<&str>) -> bool {
    raw_value.is_some_and(|value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        )
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_desktop_launch_mode<I, S>(
    args: I,
    env_disables_vsync: bool,
    env_automation: Option<DesktopLaunchAutomation>,
) -> sui::Result<DesktopLaunchMode>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut mode = DesktopLaunchMode {
        vsync_enabled: !env_disables_vsync,
        automation: env_automation,
        #[cfg(feature = "tui")]
        tui: None,
    };

    for arg in args {
        match arg.as_ref() {
            "--no-vsync" => mode.vsync_enabled = false,
            "--vsync" => mode.vsync_enabled = true,
            #[cfg(feature = "tui")]
            "--tui" => {
                mode.tui = Some(DesktopTuiLaunchMode {
                    kind: DesktopTuiLaunchKind::Interactive,
                    ..mode.tui.unwrap_or_default()
                });
            }
            #[cfg(feature = "tui")]
            "--tui-dump-accessibility" => {
                mode.tui = Some(DesktopTuiLaunchMode {
                    kind: DesktopTuiLaunchKind::DumpAccessibility,
                    ..mode.tui.unwrap_or_default()
                });
            }
            #[cfg(feature = "tui")]
            "--tui-show-hidden" => {
                mode.tui = Some(DesktopTuiLaunchMode {
                    show_hidden: true,
                    ..mode.tui.unwrap_or_default()
                });
            }
            #[cfg(not(feature = "tui"))]
            "--tui" | "--tui-dump-accessibility" | "--tui-show-hidden" => {
                return Err(tui_feature_disabled_error(arg.as_ref()));
            }
            #[cfg(feature = "tui")]
            value if value.starts_with("--tui-layout=") => {
                let layout = parse_tui_layout(value.split_once('=').map(|(_, rhs)| rhs).unwrap())?;
                mode.tui = Some(DesktopTuiLaunchMode {
                    layout,
                    ..mode.tui.unwrap_or_default()
                });
            }
            #[cfg(not(feature = "tui"))]
            value if value.starts_with("--tui-layout=") => {
                return Err(tui_feature_disabled_error(value));
            }
            value if value.starts_with("--automation=") => {
                mode.automation =
                    parse_desktop_automation(value.split_once('=').map(|(_, rhs)| rhs))?;
            }
            "" => {}
            other => {
                return Err(sui::Error::new(format!(
                    "unsupported sui-demo argument `{other}`; supported flags: --no-vsync, --vsync, --automation=<widget-book-scroll>, --tui, --tui-dump-accessibility, --tui-layout=<structured|spatial>, --tui-show-hidden"
                )));
            }
        }
    }

    Ok(mode)
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "tui")))]
fn tui_feature_disabled_error(flag: &str) -> sui::Error {
    sui::Error::new(format!(
        "unsupported sui-demo argument `{flag}`; this binary was built without the `tui` feature"
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn current_desktop_launch_mode() -> sui::Result<DesktopLaunchMode> {
    parse_desktop_launch_mode(
        env::args().skip(1),
        env_requests_no_vsync(env::var(DESKTOP_NO_VSYNC_ENV).ok().as_deref()),
        parse_desktop_automation(env::var(DESKTOP_AUTOMATION_ENV).ok().as_deref())?,
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_desktop_with_vsync(vsync_enabled: bool) -> sui::Result<()> {
    let app = build_dev_application();
    run_desktop_application(app, vsync_enabled)
}

#[cfg(not(target_arch = "wasm32"))]
fn run_desktop_application(app: Application, vsync_enabled: bool) -> sui::Result<()> {
    let feathering_enabled = app.feathering_enabled();
    let feather_width = app.feather_width();
    let initial_window_render_options = app.initial_window_render_options();
    let runtime = app.build()?;
    let platform = DesktopPlatform::new()
        .with_feathering_enabled(feathering_enabled)
        .with_feather_width(feather_width)
        .with_vsync_enabled(vsync_enabled);

    if let Some(options) = initial_window_render_options {
        for window_id in runtime.window_ids() {
            set_window_render_options(window_id, options);
        }
    }

    let _ = platform.run(runtime)?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn run_desktop_application_with_mode(launch_mode: DesktopLaunchMode) -> sui::Result<()> {
    #[cfg(feature = "tui")]
    if let Some(tui) = launch_mode.tui {
        return run_tui_application(tui);
    }

    let app = build_dev_application_with_automation(app_automation_mode(launch_mode.automation));
    let feathering_enabled = app.feathering_enabled();
    let feather_width = app.feather_width();
    let initial_window_render_options = app.initial_window_render_options();
    let runtime = app.build()?;
    let mut platform = DesktopPlatform::new()
        .with_feathering_enabled(feathering_enabled)
        .with_feather_width(feather_width)
        .with_vsync_enabled(launch_mode.vsync_enabled);

    if let Some(options) = initial_window_render_options {
        for window_id in runtime.window_ids() {
            set_window_render_options(window_id, options);
        }
    }

    if let Some(automation) = platform_automation_config(launch_mode.automation) {
        platform = platform.with_automation(automation);
    }
    if launch_mode.automation.is_some() {
        for window_id in runtime.window_ids() {
            set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);
        }
    }
    let _ = platform.run(runtime)?;
    Ok(())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn run_tui_application(tui: DesktopTuiLaunchMode) -> sui::Result<()> {
    let app = sui_testing::TestApp::from_runtime(build_dev_application().build()?)?;
    let window = app.main_window()?;
    match tui.kind {
        DesktopTuiLaunchKind::DumpAccessibility => print_tui_snapshot(&window, tui),
        DesktopTuiLaunchKind::Interactive => run_interactive_tui(window, tui),
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn print_tui_snapshot(window: &TestWindow, tui: DesktopTuiLaunchMode) -> sui::Result<()> {
    let snapshot = window.snapshot()?;
    let frame = render_snapshot(
        &snapshot.accessibility,
        TuiRenderOptions {
            width: 120,
            height: 48,
            mode: tui.layout,
            show_hidden: tui.show_hidden,
        },
    );
    println!("{frame}");
    Ok(())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn run_interactive_tui(window: TestWindow, tui: DesktopTuiLaunchMode) -> sui::Result<()> {
    let mut terminal = TuiTerminalSession::new()?;
    let mut selected = 0usize;
    let mut selection_initialized = false;
    let mut spatial_state = TuiSpatialState::default();
    loop {
        let snapshot = window.snapshot()?;
        let actionable = actionable_nodes(&snapshot.accessibility.nodes);
        if actionable.is_empty() {
            selected = 0;
            selection_initialized = false;
        } else if !selection_initialized {
            selected = preferred_initial_actionable_index(&actionable).unwrap_or(0);
            selection_initialized = true;
        } else {
            selected = selected.min(actionable.len().saturating_sub(1));
        }

        let size = terminal.size()?;
        let areas = tui_layout_areas(size);
        sync_tui_spatial_selection(
            &mut spatial_state,
            areas.spatial,
            &snapshot.accessibility.nodes,
            &actionable,
            actionable.get(selected).map(|node| node.id),
        );
        terminal.draw(
            &snapshot.accessibility.nodes,
            &actionable,
            selected,
            tui.show_hidden,
            &spatial_state,
        )?;

        if !terminal_event_ready()? {
            continue;
        }

        let event = read_terminal_event()?;
        match event {
            TerminalEvent::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                        if !actionable.is_empty() {
                            selected = (selected + 1) % actionable.len();
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                        if !actionable.is_empty() {
                            selected = if selected == 0 {
                                actionable.len().saturating_sub(1)
                            } else {
                                selected - 1
                            };
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('a') => {
                        if let Some(node) = actionable.get(selected) {
                            activate_tui_node(&window, node)?;
                        }
                    }
                    KeyCode::PageDown => {
                        if let Some(node) = actionable.get(selected) {
                            scroll_tui_node(&window, node, Vector::new(0.0, -240.0))?;
                        }
                    }
                    KeyCode::PageUp => {
                        if let Some(node) = actionable.get(selected) {
                            scroll_tui_node(&window, node, Vector::new(0.0, 240.0))?;
                        }
                    }
                    KeyCode::Right | KeyCode::Char('+') => {
                        if let Some(node) = actionable.get(selected) {
                            press_tui_node(&window, node, "ArrowRight")?;
                        }
                    }
                    KeyCode::Left | KeyCode::Char('-') => {
                        if let Some(node) = actionable.get(selected) {
                            press_tui_node(&window, node, "ArrowLeft")?;
                        }
                    }
                    KeyCode::Char('e') => {
                        if let Some(node) = actionable.get(selected) {
                            let Some(value) = prompt_for_tui_value(terminal.terminal_mut(), node)?
                            else {
                                continue;
                            };
                            set_tui_node_value(&window, node, &value)?;
                        }
                    }
                    _ => {}
                }
            }
            TerminalEvent::Mouse(mouse) => {
                handle_tui_mouse(
                    &window,
                    mouse,
                    areas,
                    &snapshot.accessibility.nodes,
                    &actionable,
                    &mut selected,
                    &mut spatial_state,
                )?;
            }
            _ => {}
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
struct TuiTerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
impl TuiTerminalSession {
    fn new() -> sui::Result<Self> {
        enable_raw_mode().map_err(to_sui_io_error)?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture).map_err(to_sui_io_error)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).map_err(to_sui_io_error)?;
        terminal.clear().map_err(to_sui_io_error)?;
        Ok(Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    fn size(&self) -> sui::Result<TerminalRect> {
        self.terminal
            .size()
            .map(|size| TerminalRect::new(0, 0, size.width, size.height))
            .map_err(to_sui_io_error)
    }

    fn draw(
        &mut self,
        nodes: &[SemanticsNode],
        actionable: &[SemanticsNode],
        selected: usize,
        show_hidden: bool,
        spatial_state: &TuiSpatialState,
    ) -> sui::Result<()> {
        self.terminal
            .draw(|frame| {
                draw_tui(
                    frame,
                    nodes,
                    actionable,
                    selected,
                    show_hidden,
                    spatial_state,
                )
            })
            .map(|_| ())
            .map_err(to_sui_io_error)
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
impl Drop for TuiTerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_tui(
    frame: &mut Frame<'_>,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected: usize,
    show_hidden: bool,
    spatial_state: &TuiSpatialState,
) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let issue_count = sui_tui::validate_snapshot(&sui::AccessibilitySnapshot {
        window_id: sui::WindowId::new(0),
        root: nodes
            .iter()
            .find(|node| node.parent.is_none())
            .map(|node| node.id),
        focused_widget: nodes
            .iter()
            .find(|node| node.state.focused)
            .map(|node| node.id),
        nodes: nodes.to_vec(),
    })
    .len();
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " sui-demo ",
            Style::default()
                .fg(TerminalColor::Black)
                .bg(TerminalColor::Cyan),
        ),
        Span::raw(" accessibility TUI "),
        Span::styled(
            format!(
                "nodes={} actions={} issues={}",
                nodes.len(),
                actionable.len(),
                issue_count
            ),
            Style::default().fg(TerminalColor::Yellow),
        ),
        Span::raw(if show_hidden { " hidden" } else { "" }),
    ]));
    frame.render_widget(title, root[0]);

    let areas = tui_layout_areas(frame.area());

    draw_spatial_canvas(
        frame,
        areas.spatial,
        nodes,
        actionable,
        actionable.get(selected),
        spatial_state,
    );
    draw_actionable_list(frame, areas.list, nodes, actionable, selected);
    draw_details(frame, areas.details, actionable.get(selected));

    let help = Paragraph::new("q quit | up/down/jk select | enter activate | e edit | +/- adjust | Pg scroll | click/wheel/right-click mouse")
        .style(Style::default().fg(TerminalColor::Gray));
    frame.render_widget(help, root[2]);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
#[derive(Debug, Clone, Copy)]
struct TuiLayoutAreas {
    spatial: TerminalRect,
    list: TerminalRect,
    details: TerminalRect,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
#[derive(Default, Debug, Clone)]
struct TuiSpatialState {
    flow_offsets: HashMap<sui::WidgetId, usize>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
impl TuiSpatialState {
    fn flow_offset(&self, window_id: sui::WidgetId) -> usize {
        self.flow_offsets.get(&window_id).copied().unwrap_or(0)
    }

    fn set_flow_offset(&mut self, window_id: sui::WidgetId, offset: usize) {
        if offset == 0 {
            self.flow_offsets.remove(&window_id);
        } else {
            self.flow_offsets.insert(window_id, offset);
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_layout_areas(area: TerminalRect) -> TuiLayoutAreas {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(root[1]);
    let side = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(body[1]);

    TuiLayoutAreas {
        spatial: body[0],
        list: side[0],
        details: side[1],
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_spatial_canvas(
    frame: &mut Frame<'_>,
    area: TerminalRect,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected: Option<&SemanticsNode>,
    spatial_state: &TuiSpatialState,
) {
    let world = tui_world_bounds(nodes).unwrap_or_else(|| SuiRect::new(0.0, 0.0, 1.0, 1.0));
    let selected_id = selected.map(|node| node.id);
    frame.render_widget(tui_view_block("Spatial Map"), area);
    let Some(inner) = inner_terminal_rect(area) else {
        return;
    };
    let floating_tabs = tui_floating_tabs(nodes, selected_id);
    let mut canvas_nodes = nodes
        .iter()
        .filter(|node| {
            tui_spatial_map_node(node, selected_id)
                && tui_projected_spatial_bounds(node, &floating_tabs)
                    .and_then(|bounds| bounds.intersection(world))
                    .is_some()
        })
        .cloned()
        .collect::<Vec<_>>();
    canvas_nodes.sort_by(|left, right| {
        let left_selected = selected_id == Some(left.id);
        let right_selected = selected_id == Some(right.id);
        left_selected
            .cmp(&right_selected)
            .then_with(|| tui_node_area(right).total_cmp(&tui_node_area(left)))
            .then_with(|| left.id.cmp(&right.id))
    });

    for node in &canvas_nodes {
        let color = tui_node_color(node, selected_id == Some(node.id));
        let Some(bounds) = tui_projected_spatial_bounds(node, &floating_tabs) else {
            continue;
        };
        let Some(rect) = tui_bounds_terminal_rect(bounds, inner, world) else {
            continue;
        };
        draw_tui_solid_outline(
            frame,
            rect,
            color,
            tui_label_node(node).then(|| tui_compact_label(node)),
        );
    }

    let mut widget_nodes = nodes
        .iter()
        .filter(|node| {
            tui_compact_widget_node(node, selected_id)
                && tui_projected_spatial_bounds(node, &floating_tabs)
                    .and_then(|bounds| bounds.intersection(world))
                    .is_some()
        })
        .cloned()
        .collect::<Vec<_>>();
    widget_nodes.sort_by(|left, right| {
        let left_selected = selected_id == Some(left.id);
        let right_selected = selected_id == Some(right.id);
        left_selected
            .cmp(&right_selected)
            .then_with(|| tui_node_area(right).total_cmp(&tui_node_area(left)))
            .then_with(|| left.id.cmp(&right.id))
    });
    for node in &widget_nodes {
        let Some(bounds) = tui_projected_spatial_bounds(node, &floating_tabs) else {
            continue;
        };
        let Some(rect) = tui_bounds_terminal_rect(bounds, inner, world) else {
            continue;
        };
        draw_tui_compact_widget(frame, rect, node, selected_id == Some(node.id));
    }
    draw_tui_accessibility_flow(
        frame,
        inner,
        world,
        nodes,
        actionable,
        selected_id,
        &floating_tabs,
        spatial_state,
    );
    draw_tui_floating_tabs(frame, inner, world, &floating_tabs);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_actionable_list(
    frame: &mut Frame<'_>,
    area: TerminalRect,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected: usize,
) {
    let rows = tui_action_tree_rows(nodes, actionable);
    let selected_row = tui_selected_action_tree_row(&rows, selected).unwrap_or(0);
    let items = rows
        .iter()
        .map(|row| {
            let actionable_style = if row.actionable_index.is_some() {
                Style::default().fg(TerminalColor::Cyan)
            } else {
                Style::default().fg(TerminalColor::DarkGray)
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    row.prefix.clone(),
                    Style::default().fg(TerminalColor::DarkGray),
                ),
                Span::styled(tui_role_label(&row.node.role), actionable_style),
                Span::raw(" "),
                Span::raw(row.node.name.as_deref().unwrap_or("<unnamed>").to_string()),
            ]))
        })
        .collect::<Vec<_>>();
    let item_count = items.len();
    let list = List::new(items)
        .block(tui_view_block(format!(
            "Action Tree {}/{}",
            selected.saturating_add(1).min(actionable.len()),
            actionable.len()
        )))
        .highlight_style(
            Style::default()
                .fg(TerminalColor::Black)
                .bg(TerminalColor::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    let offset = tui_list_offset(selected_row, area, rows.len());
    let mut state = ListState::default().with_offset(offset);
    if item_count > 0 {
        state.select(Some(selected_row.min(item_count.saturating_sub(1))));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
#[derive(Clone)]
struct TuiActionTreeRow {
    node: SemanticsNode,
    actionable_index: Option<usize>,
    prefix: String,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_action_tree_rows(
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
) -> Vec<TuiActionTreeRow> {
    let node_by_id = nodes
        .iter()
        .map(|node| (node.id, node))
        .collect::<HashMap<_, _>>();
    let actionable_by_id = actionable
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect::<HashMap<_, _>>();
    let mut relevant = actionable_by_id.keys().copied().collect::<HashSet<_>>();

    for node in actionable {
        let mut parent = node.parent;
        while let Some(parent_id) = parent {
            if !relevant.insert(parent_id) {
                break;
            }
            parent = node_by_id.get(&parent_id).and_then(|node| node.parent);
        }
    }

    let mut roots = Vec::new();
    let mut children = HashMap::<sui::WidgetId, Vec<sui::WidgetId>>::new();
    for node in nodes.iter().filter(|node| relevant.contains(&node.id)) {
        if let Some(parent) = node.parent
            && relevant.contains(&parent)
        {
            children.entry(parent).or_default().push(node.id);
            continue;
        }
        roots.push(node.id);
    }

    let mut rows = Vec::new();
    for (index, root) in roots.iter().copied().enumerate() {
        let is_last = index + 1 == roots.len();
        push_tui_action_tree_row(
            root,
            "",
            is_last,
            true,
            &node_by_id,
            &actionable_by_id,
            &children,
            &mut rows,
        );
    }
    rows
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn push_tui_action_tree_row(
    node_id: sui::WidgetId,
    ancestor_prefix: &str,
    is_last: bool,
    is_root: bool,
    node_by_id: &HashMap<sui::WidgetId, &SemanticsNode>,
    actionable_by_id: &HashMap<sui::WidgetId, usize>,
    children: &HashMap<sui::WidgetId, Vec<sui::WidgetId>>,
    rows: &mut Vec<TuiActionTreeRow>,
) {
    let Some(node) = node_by_id.get(&node_id) else {
        return;
    };
    let prefix = if is_root {
        String::new()
    } else {
        format!("{}{}", ancestor_prefix, if is_last { "└─ " } else { "├─ " })
    };
    rows.push(TuiActionTreeRow {
        node: (**node).clone(),
        actionable_index: actionable_by_id.get(&node_id).copied(),
        prefix,
    });

    let child_prefix = if is_root {
        String::new()
    } else {
        format!("{}{}", ancestor_prefix, if is_last { "   " } else { "│  " })
    };
    if let Some(child_ids) = children.get(&node_id) {
        for (index, child_id) in child_ids.iter().copied().enumerate() {
            push_tui_action_tree_row(
                child_id,
                &child_prefix,
                index + 1 == child_ids.len(),
                false,
                node_by_id,
                actionable_by_id,
                children,
                rows,
            );
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_selected_action_tree_row(rows: &[TuiActionTreeRow], selected: usize) -> Option<usize> {
    rows.iter()
        .position(|row| row.actionable_index == Some(selected))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_details(frame: &mut Frame<'_>, area: TerminalRect, selected: Option<&SemanticsNode>) {
    let text = selected.map(format_tui_details).unwrap_or_else(|| {
        "No actionable nodes are available in the current accessibility snapshot.".to_string()
    });
    let details = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .block(tui_view_block("Details"));
    frame.render_widget(details, area);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_view_block(title: impl Into<Title<'static>>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(TerminalColor::Gray))
        .title(title)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
#[derive(Clone)]
struct TuiFloatingTabs {
    parent: SemanticsNode,
    windows: Vec<SemanticsNode>,
    active_window_id: sui::WidgetId,
    active_index: usize,
    node_window: HashMap<sui::WidgetId, sui::WidgetId>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_floating_tabs(
    nodes: &[SemanticsNode],
    selected_id: Option<sui::WidgetId>,
) -> Option<TuiFloatingTabs> {
    let node_by_id = nodes
        .iter()
        .map(|node| (node.id, node))
        .collect::<HashMap<_, _>>();
    let mut windows_by_parent = HashMap::<sui::WidgetId, Vec<SemanticsNode>>::new();
    for node in nodes
        .iter()
        .filter(|node| node.role == SemanticsRole::Window && node.parent.is_some())
    {
        if let Some(parent) = node.parent {
            windows_by_parent
                .entry(parent)
                .or_default()
                .push(node.clone());
        }
    }

    let (parent_id, mut windows) = windows_by_parent
        .into_iter()
        .filter(|(_, windows)| windows.len() > 1)
        .filter(|(parent_id, _)| {
            node_by_id.get(parent_id).is_some_and(|parent| {
                parent.name.as_deref() == Some("Development workspace")
                    || parent.role == SemanticsRole::GenericContainer
            })
        })
        .max_by_key(|(_, windows)| windows.len())?;
    windows.sort_by(|left, right| {
        left.bounds
            .x()
            .total_cmp(&right.bounds.x())
            .then(left.id.cmp(&right.id))
    });
    let parent = (*node_by_id.get(&parent_id)?).clone();
    let window_ids = windows
        .iter()
        .map(|window| window.id)
        .collect::<HashSet<_>>();
    let mut node_window = HashMap::new();
    for node in nodes {
        let mut current = Some(node.id);
        while let Some(id) = current {
            if window_ids.contains(&id) {
                node_window.insert(node.id, id);
                break;
            }
            current = node_by_id.get(&id).and_then(|node| node.parent);
        }
    }
    let active_index = selected_id
        .and_then(|id| {
            windows
                .iter()
                .position(|window| tui_node_has_ancestor(id, window.id, &node_by_id))
        })
        .unwrap_or(0);
    let active_window_id = windows.get(active_index)?.id;

    Some(TuiFloatingTabs {
        parent,
        windows,
        active_window_id,
        active_index,
        node_window,
    })
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_node_has_ancestor(
    node_id: sui::WidgetId,
    ancestor_id: sui::WidgetId,
    node_by_id: &HashMap<sui::WidgetId, &SemanticsNode>,
) -> bool {
    let mut current = Some(node_id);
    while let Some(id) = current {
        if id == ancestor_id {
            return true;
        }
        current = node_by_id.get(&id).and_then(|node| node.parent);
    }
    false
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_projected_spatial_bounds(
    node: &SemanticsNode,
    tabs: &Option<TuiFloatingTabs>,
) -> Option<SuiRect> {
    let Some(tabs) = tabs else {
        return Some(node.bounds);
    };
    if node.id == tabs.parent.id {
        return Some(node.bounds);
    }
    if tabs.windows.iter().any(|window| window.id == node.id) {
        return None;
    }

    if tabs.node_window.contains_key(&node.id) {
        return None;
    }
    Some(node.bounds)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_tui_floating_tabs(
    frame: &mut Frame<'_>,
    inner: TerminalRect,
    world: SuiRect,
    tabs: &Option<TuiFloatingTabs>,
) {
    let Some(tabs) = tabs else {
        return;
    };
    let Some(tab_area) = tui_floating_tab_area(inner, world, tabs) else {
        return;
    };
    fill_tui_rect(
        frame.buffer_mut(),
        tab_area,
        Style::default().bg(TerminalColor::DarkGray),
    );
    for tab in tui_floating_tab_rects(inner, world, tabs) {
        let active = tab.index == tabs.active_index;
        let style = if active {
            Style::default()
                .fg(TerminalColor::Black)
                .bg(TerminalColor::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(TerminalColor::White)
                .bg(TerminalColor::DarkGray)
        };
        frame.buffer_mut().set_stringn(
            tab.rect.x,
            tab.rect.y,
            tab.label,
            tab.rect.width as usize,
            style,
        );
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
#[derive(Clone)]
struct TuiFloatingTabRect {
    window_id: sui::WidgetId,
    index: usize,
    rect: TerminalRect,
    label: String,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_floating_tab_area(
    inner: TerminalRect,
    world: SuiRect,
    tabs: &TuiFloatingTabs,
) -> Option<TerminalRect> {
    let parent_rect = tui_bounds_terminal_rect(tabs.parent.bounds, inner, world)?;
    Some(TerminalRect::new(
        parent_rect.x,
        parent_rect.y,
        parent_rect.width,
        1,
    ))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_floating_tab_rects(
    inner: TerminalRect,
    world: SuiRect,
    tabs: &TuiFloatingTabs,
) -> Vec<TuiFloatingTabRect> {
    let Some(tab_area) = tui_floating_tab_area(inner, world, tabs) else {
        return Vec::new();
    };
    let mut x = tab_area.x.saturating_add(1);
    let max_x = tab_area.x.saturating_add(tab_area.width);
    let mut rects = Vec::new();
    for (index, window) in tabs.windows.iter().enumerate() {
        if x >= max_x {
            break;
        }
        let label = format!(" {} ", window.name.as_deref().unwrap_or("Window"));
        let width = (label.chars().count() as u16).min(max_x.saturating_sub(x));
        rects.push(TuiFloatingTabRect {
            window_id: window.id,
            index,
            rect: TerminalRect::new(x, tab_area.y, width, 1),
            label,
        });
        x = x.saturating_add(width).saturating_add(1);
    }
    rects
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
#[derive(Clone)]
struct TuiAccessibilityFlowItem {
    node: SemanticsNode,
    rect: TerminalRect,
    actionable_index: Option<usize>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
#[derive(Clone)]
struct TuiAccessibilityFlowVirtualItem {
    node: SemanticsNode,
    column: u16,
    row: usize,
    width: u16,
    actionable_index: Option<usize>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_tui_accessibility_flow(
    frame: &mut Frame<'_>,
    inner: TerminalRect,
    world: SuiRect,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected_id: Option<sui::WidgetId>,
    tabs: &Option<TuiFloatingTabs>,
    spatial_state: &TuiSpatialState,
) {
    let Some(tabs) = tabs else {
        return;
    };
    let Some(area) = tui_active_tab_content_area(inner, world, tabs) else {
        return;
    };
    fill_tui_rect(frame.buffer_mut(), area, Style::default());
    let virtual_items = tui_accessibility_flow_virtual_layout(nodes, actionable, tabs, area.width);
    let offset = tui_accessibility_flow_clamped_offset(
        &virtual_items,
        area.height,
        spatial_state.flow_offset(tabs.active_window_id),
    );
    for item in tui_accessibility_flow_layout_from_virtual(virtual_items, area, offset) {
        draw_tui_compact_widget(
            frame,
            item.rect,
            &item.node,
            selected_id == Some(item.node.id),
        );
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_active_tab_content_area(
    inner: TerminalRect,
    world: SuiRect,
    tabs: &TuiFloatingTabs,
) -> Option<TerminalRect> {
    let parent_rect = tui_bounds_terminal_rect(tabs.parent.bounds, inner, world)?;
    if parent_rect.height <= 1 || parent_rect.width == 0 {
        return None;
    }
    Some(TerminalRect::new(
        parent_rect.x,
        parent_rect.y.saturating_add(1),
        parent_rect.width,
        parent_rect.height.saturating_sub(1),
    ))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_accessibility_flow_layout(
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    tabs: &TuiFloatingTabs,
    area: TerminalRect,
    offset: usize,
) -> Vec<TuiAccessibilityFlowItem> {
    if area.width == 0 || area.height == 0 {
        return Vec::new();
    }

    let virtual_items = tui_accessibility_flow_virtual_layout(nodes, actionable, tabs, area.width);
    let offset = tui_accessibility_flow_clamped_offset(&virtual_items, area.height, offset);
    tui_accessibility_flow_layout_from_virtual(virtual_items, area, offset)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_accessibility_flow_layout_from_virtual(
    virtual_items: Vec<TuiAccessibilityFlowVirtualItem>,
    area: TerminalRect,
    offset: usize,
) -> Vec<TuiAccessibilityFlowItem> {
    virtual_items
        .into_iter()
        .filter_map(|item| {
            let visible_row = item.row.checked_sub(offset)?;
            if visible_row >= area.height as usize {
                return None;
            }
            let available = area.width.saturating_sub(item.column);
            if available == 0 {
                return None;
            }
            Some(TuiAccessibilityFlowItem {
                node: item.node,
                rect: TerminalRect::new(
                    area.x.saturating_add(item.column),
                    area.y.saturating_add(visible_row as u16),
                    item.width.min(available),
                    1,
                ),
                actionable_index: item.actionable_index,
            })
        })
        .collect()
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_accessibility_flow_virtual_layout(
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    tabs: &TuiFloatingTabs,
    area_width: u16,
) -> Vec<TuiAccessibilityFlowVirtualItem> {
    if area_width == 0 {
        return Vec::new();
    }

    let actionable_by_id = actionable
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect::<HashMap<_, _>>();
    let mut items = Vec::new();
    let mut x = 0u16;
    let mut row = 0usize;

    for node in nodes.iter().filter(|node| {
        tabs.node_window.get(&node.id) == Some(&tabs.active_window_id)
            && tui_accessibility_flow_node(node)
    }) {
        let inline = tui_accessibility_flow_inline_node(node);
        let item_width = if inline {
            tui_accessibility_flow_cell_width(node, area_width)
        } else {
            area_width
        };

        if inline {
            if x > 0 && x.saturating_add(item_width) > area_width {
                x = 0;
                row = row.saturating_add(1);
            }
            let available = area_width.saturating_sub(x);
            if available == 0 {
                x = 0;
                row = row.saturating_add(1);
                continue;
            }
            let item_width = item_width.min(available);
            items.push(TuiAccessibilityFlowVirtualItem {
                node: node.clone(),
                column: x,
                row,
                width: item_width,
                actionable_index: actionable_by_id.get(&node.id).copied(),
            });
            x = x.saturating_add(item_width).saturating_add(1);
        } else {
            if x != 0 {
                x = 0;
                row = row.saturating_add(1);
            }
            items.push(TuiAccessibilityFlowVirtualItem {
                node: node.clone(),
                column: 0,
                row,
                width: item_width,
                actionable_index: actionable_by_id.get(&node.id).copied(),
            });
            row = row.saturating_add(1);
        }
    }

    items
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_accessibility_flow_clamped_offset(
    items: &[TuiAccessibilityFlowVirtualItem],
    visible_height: u16,
    offset: usize,
) -> usize {
    let visible_rows = visible_height as usize;
    let total_rows = tui_accessibility_flow_total_rows(items);
    if visible_rows == 0 || total_rows <= visible_rows {
        return 0;
    }

    offset.min(total_rows.saturating_sub(visible_rows))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_accessibility_flow_total_rows(items: &[TuiAccessibilityFlowVirtualItem]) -> usize {
    items
        .iter()
        .map(|item| item.row.saturating_add(1))
        .max()
        .unwrap_or(0)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn sync_tui_spatial_selection(
    state: &mut TuiSpatialState,
    area: TerminalRect,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected_id: Option<sui::WidgetId>,
) {
    let Some(selected_id) = selected_id else {
        return;
    };
    let Some((tabs, flow_area, items)) =
        tui_spatial_flow_items(area, nodes, actionable, Some(selected_id))
    else {
        return;
    };
    let Some(selected_item) = items.iter().find(|item| item.node.id == selected_id) else {
        return;
    };

    let visible_rows = flow_area.height as usize;
    if visible_rows == 0 {
        return;
    }

    let mut offset = tui_accessibility_flow_clamped_offset(
        &items,
        flow_area.height,
        state.flow_offset(tabs.active_window_id),
    );
    if selected_item.row < offset {
        offset = selected_item.row;
    } else if selected_item.row >= offset.saturating_add(visible_rows) {
        offset = selected_item
            .row
            .saturating_add(1)
            .saturating_sub(visible_rows);
    }
    state.set_flow_offset(
        tabs.active_window_id,
        tui_accessibility_flow_clamped_offset(&items, flow_area.height, offset),
    );
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_spatial_flow_items(
    area: TerminalRect,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected_id: Option<sui::WidgetId>,
) -> Option<(
    TuiFloatingTabs,
    TerminalRect,
    Vec<TuiAccessibilityFlowVirtualItem>,
)> {
    let inner = inner_terminal_rect(area)?;
    let world = tui_world_bounds(nodes)?;
    let tabs = tui_floating_tabs(nodes, selected_id)?;
    let flow_area = tui_active_tab_content_area(inner, world, &tabs)?;
    let items = tui_accessibility_flow_virtual_layout(nodes, actionable, &tabs, flow_area.width);
    Some((tabs, flow_area, items))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn scroll_tui_spatial_flow(
    mouse: MouseEvent,
    area: TerminalRect,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected: usize,
    state: &mut TuiSpatialState,
    row_delta: isize,
) -> Option<usize> {
    let selected_id = actionable.get(selected).map(|node| node.id);
    let (tabs, flow_area, items) = tui_spatial_flow_items(area, nodes, actionable, selected_id)?;
    if !terminal_rect_contains(flow_area, mouse.column, mouse.row) {
        return None;
    }

    let current = tui_accessibility_flow_clamped_offset(
        &items,
        flow_area.height,
        state.flow_offset(tabs.active_window_id),
    );
    let requested = if row_delta.is_negative() {
        current.saturating_sub(row_delta.unsigned_abs())
    } else {
        current.saturating_add(row_delta as usize)
    };
    let offset = tui_accessibility_flow_clamped_offset(&items, flow_area.height, requested);
    state.set_flow_offset(tabs.active_window_id, offset);

    let target_row = offset.saturating_add(mouse.row.saturating_sub(flow_area.y) as usize);
    tui_visible_flow_actionable_index_near(&items, offset, flow_area.height, target_row)
        .or(Some(selected))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_visible_flow_actionable_index_near(
    items: &[TuiAccessibilityFlowVirtualItem],
    offset: usize,
    visible_height: u16,
    target_row: usize,
) -> Option<usize> {
    let visible_end = offset.saturating_add(visible_height as usize);
    items
        .iter()
        .filter(|item| item.row >= offset && item.row < visible_end)
        .filter_map(|item| item.actionable_index.map(|index| (index, item.row)))
        .min_by_key(|(_, row)| row.abs_diff(target_row))
        .map(|(index, _)| index)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn mouse_hit_spatial_tab(
    mouse: MouseEvent,
    area: TerminalRect,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected_id: Option<sui::WidgetId>,
) -> Option<usize> {
    if !terminal_rect_contains(area, mouse.column, mouse.row) {
        return None;
    }
    let inner = inner_terminal_rect(area)?;
    let world = tui_world_bounds(nodes)?;
    let tabs = tui_floating_tabs(nodes, selected_id)?;
    let window_id = tui_floating_tab_rects(inner, world, &tabs)
        .into_iter()
        .find(|tab| terminal_rect_contains(tab.rect, mouse.column, mouse.row))
        .map(|tab| tab.window_id)?;
    actionable
        .iter()
        .position(|node| tabs.node_window.get(&node.id) == Some(&window_id))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_accessibility_flow_node(node: &SemanticsNode) -> bool {
    !node.state.hidden
        && tui_compact_widget_node(node, None)
        && node.name.as_deref() != Some("Floating view content")
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_accessibility_flow_inline_node(node: &SemanticsNode) -> bool {
    matches!(
        node.role,
        SemanticsRole::Button | SemanticsRole::MenuItem | SemanticsRole::ColorSwatch
    )
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_accessibility_flow_cell_width(node: &SemanticsNode, area_width: u16) -> u16 {
    let text_width = tui_widget_text(node).chars().count() as u16;
    let max_width = area_width.min(24).max(1);
    let min_width = 6.min(max_width);
    text_width.saturating_add(2).clamp(min_width, max_width)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_bounds_terminal_rect(
    bounds: SuiRect,
    inner: TerminalRect,
    world: SuiRect,
) -> Option<TerminalRect> {
    if inner.width == 0 || inner.height == 0 || world.width() <= 0.0 || world.height() <= 0.0 {
        return None;
    }

    let x0 = world_to_tui_column(bounds.x(), inner, world);
    let x1 = world_to_tui_column(bounds.max_x(), inner, world);
    let y0 = world_to_tui_row(bounds.max_y(), inner, world);
    let y1 = world_to_tui_row(bounds.y(), inner, world);
    let left = x0.min(x1);
    let right = x0.max(x1);
    let top = y0.min(y1);
    let bottom = y0.max(y1);

    Some(TerminalRect::new(
        left,
        top,
        right.saturating_sub(left).saturating_add(1),
        bottom.saturating_sub(top).saturating_add(1),
    ))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn world_to_tui_column(x: f32, inner: TerminalRect, world: SuiRect) -> u16 {
    let span = inner.width.saturating_sub(1).max(1) as f32;
    let ratio = ((x - world.x()) / world.width()).clamp(0.0, 1.0);
    inner.x + (ratio * span).round() as u16
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn world_to_tui_row(y: f32, inner: TerminalRect, world: SuiRect) -> u16 {
    let span = inner.height.saturating_sub(1).max(1) as f32;
    let ratio = ((y - world.y()) / world.height()).clamp(0.0, 1.0);
    inner.y + (ratio * span).round() as u16
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_tui_solid_outline(
    frame: &mut Frame<'_>,
    rect: TerminalRect,
    color: TerminalColor,
    label: Option<String>,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    let style = Style::default().fg(color);
    let right = rect.x.saturating_add(rect.width.saturating_sub(1));
    let bottom = rect.y.saturating_add(rect.height.saturating_sub(1));
    let buffer = frame.buffer_mut();

    if rect.height == 1 {
        for x in rect.x..=right {
            set_tui_symbol(buffer, x, rect.y, "─", style);
        }
    } else if rect.width == 1 {
        for y in rect.y..=bottom {
            set_tui_symbol(buffer, rect.x, y, "│", style);
        }
    } else {
        for x in rect.x.saturating_add(1)..right {
            set_tui_symbol(buffer, x, rect.y, "─", style);
            set_tui_symbol(buffer, x, bottom, "─", style);
        }
        for y in rect.y.saturating_add(1)..bottom {
            set_tui_symbol(buffer, rect.x, y, "│", style);
            set_tui_symbol(buffer, right, y, "│", style);
        }
        set_tui_symbol(buffer, rect.x, rect.y, "┌", style);
        set_tui_symbol(buffer, right, rect.y, "┐", style);
        set_tui_symbol(buffer, rect.x, bottom, "└", style);
        set_tui_symbol(buffer, right, bottom, "┘", style);
    }

    if let Some(label) = label
        && rect.width > 4
    {
        let max_width = rect.width.saturating_sub(2) as usize;
        buffer.set_stringn(rect.x + 1, rect.y, label, max_width, style);
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_tui_compact_widget(
    frame: &mut Frame<'_>,
    rect: TerminalRect,
    node: &SemanticsNode,
    selected: bool,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    match node.role {
        SemanticsRole::Text => {
            draw_tui_clipped_text(
                frame,
                rect,
                &tui_widget_text(node),
                Style::default().fg(TerminalColor::White),
                false,
            );
        }
        SemanticsRole::Button | SemanticsRole::MenuItem => {
            let style = if selected {
                Style::default()
                    .fg(TerminalColor::Black)
                    .bg(TerminalColor::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(TerminalColor::Black)
                    .bg(TerminalColor::Cyan)
            };
            draw_tui_filled_label(frame, rect, &tui_widget_text(node), style);
        }
        SemanticsRole::CheckBox | SemanticsRole::Switch | SemanticsRole::RadioButton => {
            let marker = if tui_node_checked(node) { "[x]" } else { "[ ]" };
            let text = format!("{marker} {}", tui_widget_text(node));
            let style = if selected {
                Style::default()
                    .fg(TerminalColor::Black)
                    .bg(TerminalColor::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TerminalColor::Cyan)
            };
            draw_tui_clipped_text(frame, rect, &text, style, false);
        }
        SemanticsRole::TextInput | SemanticsRole::ComboBox | SemanticsRole::SpinBox => {
            let text = tui_widget_value_text(node).unwrap_or_else(|| tui_widget_text(node));
            let style = if selected {
                Style::default()
                    .fg(TerminalColor::Black)
                    .bg(TerminalColor::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(TerminalColor::White)
                    .bg(TerminalColor::DarkGray)
            };
            draw_tui_filled_label(frame, rect, &text, style);
        }
        SemanticsRole::Slider | SemanticsRole::ProgressBar => {
            draw_tui_compact_range(frame, rect, node, selected);
        }
        SemanticsRole::ColorSwatch | SemanticsRole::ColorPicker => {
            let style = if selected {
                Style::default()
                    .fg(TerminalColor::Black)
                    .bg(TerminalColor::Yellow)
            } else {
                Style::default()
                    .fg(TerminalColor::Black)
                    .bg(TerminalColor::Magenta)
            };
            draw_tui_filled_label(frame, rect, &tui_widget_text(node), style);
        }
        SemanticsRole::BusyIndicator => {
            draw_tui_clipped_text(
                frame,
                rect,
                "busy",
                Style::default().fg(TerminalColor::Magenta),
                false,
            );
        }
        _ => {}
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_tui_filled_label(frame: &mut Frame<'_>, rect: TerminalRect, text: &str, style: Style) {
    fill_tui_rect(frame.buffer_mut(), rect, style);
    draw_tui_clipped_text(frame, rect, text, style, true);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_tui_clipped_text(
    frame: &mut Frame<'_>,
    rect: TerminalRect,
    text: &str,
    style: Style,
    center_vertical: bool,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let y = if center_vertical {
        rect.y + rect.height.saturating_sub(1) / 2
    } else {
        rect.y
    };
    let text_x = rect
        .x
        .saturating_add(1)
        .min(rect.x + rect.width.saturating_sub(1));
    let max_width = rect
        .width
        .saturating_sub(if rect.width > 2 { 2 } else { 0 }) as usize;
    if max_width == 0 {
        return;
    }
    frame
        .buffer_mut()
        .set_stringn(text_x, y, text, max_width, style);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn draw_tui_compact_range(
    frame: &mut Frame<'_>,
    rect: TerminalRect,
    node: &SemanticsNode,
    selected: bool,
) {
    let style = if selected {
        Style::default()
            .fg(TerminalColor::Black)
            .bg(TerminalColor::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(TerminalColor::White)
            .bg(TerminalColor::DarkGray)
    };
    fill_tui_rect(frame.buffer_mut(), rect, style);
    let y = rect.y + rect.height.saturating_sub(1) / 2;
    let fill_width = tui_range_fill_width(node, rect.width);
    for x in rect.x..rect.x.saturating_add(fill_width) {
        set_tui_symbol(
            frame.buffer_mut(),
            x,
            y,
            "━",
            Style::default().fg(TerminalColor::Cyan),
        );
    }
    draw_tui_clipped_text(frame, rect, &tui_widget_text(node), style, true);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn fill_tui_rect(buffer: &mut Buffer, rect: TerminalRect, style: Style) {
    for y in rect.y..rect.y.saturating_add(rect.height) {
        for x in rect.x..rect.x.saturating_add(rect.width) {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_symbol(" ").set_style(style);
            }
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn set_tui_symbol(buffer: &mut Buffer, x: u16, y: u16, symbol: &str, style: Style) {
    if let Some(cell) = buffer.cell_mut((x, y)) {
        cell.set_symbol(symbol).set_style(style);
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn handle_tui_mouse(
    window: &TestWindow,
    mouse: MouseEvent,
    areas: TuiLayoutAreas,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected: &mut usize,
    spatial_state: &mut TuiSpatialState,
) -> sui::Result<()> {
    let selected_id = actionable.get(*selected).map(|node| node.id);
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(index) =
                mouse_hit_action_tree(mouse, areas.list, nodes, actionable, *selected)
            {
                *selected = index;
            } else if let Some(index) =
                mouse_hit_spatial_tab(mouse, areas.spatial, nodes, actionable, selected_id)
            {
                *selected = index;
            } else if let Some(index) = mouse_hit_spatial_node(
                mouse,
                areas.spatial,
                nodes,
                actionable,
                selected_id,
                spatial_state,
            ) {
                *selected = index;
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            if let Some(index) = mouse_hit_action_tree(
                mouse, areas.list, nodes, actionable, *selected,
            )
            .or_else(|| {
                mouse_hit_spatial_node(
                    mouse,
                    areas.spatial,
                    nodes,
                    actionable,
                    selected_id,
                    spatial_state,
                )
            }) {
                *selected = index;
                if let Some(node) = actionable.get(index) {
                    activate_tui_node(window, node)?;
                }
            } else if let Some(node) = actionable.get(*selected) {
                activate_tui_node(window, node)?;
            }
        }
        MouseEventKind::ScrollDown => {
            if terminal_rect_contains(areas.list, mouse.column, mouse.row) {
                *selected = (*selected)
                    .saturating_add(3)
                    .min(actionable.len().saturating_sub(1));
            } else if let Some(index) = scroll_tui_spatial_flow(
                mouse,
                areas.spatial,
                nodes,
                actionable,
                *selected,
                spatial_state,
                3,
            ) {
                *selected = index;
            } else {
                let index = mouse_hit_spatial_node(
                    mouse,
                    areas.spatial,
                    nodes,
                    actionable,
                    selected_id,
                    spatial_state,
                )
                .unwrap_or(*selected);
                if let Some(node) = actionable.get(index) {
                    *selected = index;
                    scroll_tui_node(window, node, Vector::new(0.0, -120.0))?;
                }
            }
        }
        MouseEventKind::ScrollUp => {
            if terminal_rect_contains(areas.list, mouse.column, mouse.row) {
                *selected = (*selected).saturating_sub(3);
            } else if let Some(index) = scroll_tui_spatial_flow(
                mouse,
                areas.spatial,
                nodes,
                actionable,
                *selected,
                spatial_state,
                -3,
            ) {
                *selected = index;
            } else {
                let index = mouse_hit_spatial_node(
                    mouse,
                    areas.spatial,
                    nodes,
                    actionable,
                    selected_id,
                    spatial_state,
                )
                .unwrap_or(*selected);
                if let Some(node) = actionable.get(index) {
                    *selected = index;
                    scroll_tui_node(window, node, Vector::new(0.0, 120.0))?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn mouse_hit_action_tree(
    mouse: MouseEvent,
    area: TerminalRect,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected: usize,
) -> Option<usize> {
    if !terminal_rect_contains(area, mouse.column, mouse.row) || area.height <= 2 {
        return None;
    }
    let row = mouse.row.checked_sub(area.y + 1)? as usize;
    let visible_rows = area.height.saturating_sub(2) as usize;
    let rows = tui_action_tree_rows(nodes, actionable);
    let selected_row = tui_selected_action_tree_row(&rows, selected).unwrap_or(0);
    let offset = tui_list_offset(selected_row, area, rows.len());
    let index = offset.saturating_add(row);
    if row >= visible_rows || index >= rows.len() {
        return None;
    }
    rows[index].actionable_index
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_list_offset(selected: usize, area: TerminalRect, item_count: usize) -> usize {
    let visible_rows = area.height.saturating_sub(2) as usize;
    if visible_rows == 0 || item_count <= visible_rows {
        return 0;
    }

    let selected = selected.min(item_count.saturating_sub(1));
    selected
        .saturating_sub(visible_rows / 2)
        .min(item_count.saturating_sub(visible_rows))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn mouse_hit_spatial_node(
    mouse: MouseEvent,
    area: TerminalRect,
    nodes: &[SemanticsNode],
    actionable: &[SemanticsNode],
    selected_id: Option<sui::WidgetId>,
    spatial_state: &TuiSpatialState,
) -> Option<usize> {
    if !terminal_rect_contains(area, mouse.column, mouse.row) || area.width <= 2 || area.height <= 2
    {
        return None;
    }
    let inner = inner_terminal_rect(area)?;
    if !terminal_rect_contains(inner, mouse.column, mouse.row) {
        return None;
    }
    let world = tui_world_bounds(nodes)?;
    let tabs = tui_floating_tabs(nodes, selected_id);
    if let Some(tabs) = tabs.as_ref()
        && let Some(flow_area) = tui_active_tab_content_area(inner, world, tabs)
        && terminal_rect_contains(flow_area, mouse.column, mouse.row)
    {
        let offset = spatial_state.flow_offset(tabs.active_window_id);
        return tui_accessibility_flow_layout(nodes, actionable, tabs, flow_area, offset)
            .into_iter()
            .find(|item| terminal_rect_contains(item.rect, mouse.column, mouse.row))
            .and_then(|item| item.actionable_index);
    }
    let point = mouse_to_world_point(mouse, inner, world)?;

    actionable
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            tui_projected_spatial_bounds(node, &tabs).is_some_and(|bounds| bounds.contains(point))
        })
        .min_by(|(_, left), (_, right)| {
            tui_node_area(left)
                .total_cmp(&tui_node_area(right))
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|(index, _)| index)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn mouse_to_world_point(mouse: MouseEvent, inner: TerminalRect, world: SuiRect) -> Option<Point> {
    if inner.width <= 1 || inner.height <= 1 || world.width() <= 0.0 || world.height() <= 0.0 {
        return None;
    }
    let x_ratio = (mouse.column.saturating_sub(inner.x)) as f32 / (inner.width - 1) as f32;
    let y_ratio = (mouse.row.saturating_sub(inner.y)) as f32 / (inner.height - 1) as f32;
    Some(Point::new(
        world.x() + world.width() * x_ratio.clamp(0.0, 1.0),
        world.y() + world.height() * y_ratio.clamp(0.0, 1.0),
    ))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn terminal_rect_contains(area: TerminalRect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn inner_terminal_rect(area: TerminalRect) -> Option<TerminalRect> {
    if area.width <= 2 || area.height <= 2 {
        return None;
    }
    Some(TerminalRect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width - 2,
        height: area.height - 2,
    })
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_node_area(node: &SemanticsNode) -> f32 {
    node.bounds.width().max(0.0) * node.bounds.height().max(0.0)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn terminal_event_ready() -> sui::Result<bool> {
    event::poll(Duration::from_millis(120)).map_err(to_sui_io_error)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn read_terminal_event() -> sui::Result<TerminalEvent> {
    event::read().map_err(to_sui_io_error)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn prompt_for_tui_value(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    node: &SemanticsNode,
) -> sui::Result<Option<String>> {
    disable_raw_mode().map_err(to_sui_io_error)?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(to_sui_io_error)?;
    println!(
        "Set value for #{} {:?} {}:",
        node.id,
        node.role,
        node.name.as_deref().unwrap_or("<unnamed>")
    );
    let mut value = String::new();
    let read = io::stdin().read_line(&mut value).map_err(to_sui_io_error)?;
    execute!(terminal.backend_mut(), EnterAlternateScreen).map_err(to_sui_io_error)?;
    enable_raw_mode().map_err(to_sui_io_error)?;
    terminal.clear().map_err(to_sui_io_error)?;
    if read == 0 {
        Ok(None)
    } else {
        Ok(Some(value.trim_end().to_string()))
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn to_sui_io_error(error: io::Error) -> sui::Error {
    sui::Error::new(error.to_string())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_world_bounds(nodes: &[SemanticsNode]) -> Option<SuiRect> {
    nodes
        .iter()
        .find(|node| {
            node.parent.is_none()
                && matches!(node.role, SemanticsRole::Window | SemanticsRole::Root)
                && tui_spatial_bounds(node).is_some()
        })
        .and_then(tui_spatial_bounds)
        .or_else(|| {
            nodes
                .iter()
                .filter_map(tui_spatial_bounds)
                .reduce(|left, right| left.union(right))
        })
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_spatial_bounds(node: &SemanticsNode) -> Option<SuiRect> {
    let bounds = node.bounds;
    if bounds.is_empty()
        || !bounds.x().is_finite()
        || !bounds.y().is_finite()
        || !bounds.width().is_finite()
        || !bounds.height().is_finite()
    {
        None
    } else {
        Some(bounds)
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_spatial_map_node(node: &SemanticsNode, selected_id: Option<sui::WidgetId>) -> bool {
    selected_id == Some(node.id)
        || node.state.focused
        || matches!(
            node.role,
            SemanticsRole::Window
                | SemanticsRole::Root
                | SemanticsRole::Separator
                | SemanticsRole::List
                | SemanticsRole::ListItem
                | SemanticsRole::Tree
                | SemanticsRole::Table
                | SemanticsRole::Splitter
                | SemanticsRole::TabBar
                | SemanticsRole::Tabs
                | SemanticsRole::Menu
                | SemanticsRole::ContextMenu
                | SemanticsRole::Dialog
                | SemanticsRole::Popover
                | SemanticsRole::ScrollView
                | SemanticsRole::Image
                | SemanticsRole::Canvas
        )
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_compact_widget_node(node: &SemanticsNode, selected_id: Option<sui::WidgetId>) -> bool {
    selected_id == Some(node.id)
        || matches!(
            node.role,
            SemanticsRole::Button
                | SemanticsRole::CheckBox
                | SemanticsRole::Switch
                | SemanticsRole::RadioButton
                | SemanticsRole::MenuItem
                | SemanticsRole::Slider
                | SemanticsRole::ProgressBar
                | SemanticsRole::BusyIndicator
                | SemanticsRole::Text
                | SemanticsRole::TextInput
                | SemanticsRole::SpinBox
                | SemanticsRole::ComboBox
                | SemanticsRole::ColorSwatch
                | SemanticsRole::ColorPicker
        )
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_label_node(node: &SemanticsNode) -> bool {
    node.state.focused
        || tui_interactive_role(node)
        || matches!(
            node.role,
            SemanticsRole::Window
                | SemanticsRole::Root
                | SemanticsRole::Splitter
                | SemanticsRole::List
                | SemanticsRole::ListItem
                | SemanticsRole::Dialog
                | SemanticsRole::Popover
                | SemanticsRole::ScrollView
        )
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_interactive_role(node: &SemanticsNode) -> bool {
    matches!(
        node.role,
        SemanticsRole::Button
            | SemanticsRole::CheckBox
            | SemanticsRole::Switch
            | SemanticsRole::RadioButton
            | SemanticsRole::MenuItem
            | SemanticsRole::Slider
            | SemanticsRole::TextInput
            | SemanticsRole::SpinBox
            | SemanticsRole::ComboBox
            | SemanticsRole::ColorPicker
            | SemanticsRole::ScrollView
    )
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_node_color(node: &SemanticsNode, selected: bool) -> TerminalColor {
    if selected {
        TerminalColor::Yellow
    } else if node.state.focused {
        TerminalColor::Magenta
    } else if tui_interactive_role(node) {
        TerminalColor::Cyan
    } else if matches!(node.role, SemanticsRole::Window | SemanticsRole::Root) {
        TerminalColor::Green
    } else if matches!(
        node.role,
        SemanticsRole::GenericContainer | SemanticsRole::ScrollView
    ) {
        TerminalColor::Blue
    } else if matches!(
        node.role,
        SemanticsRole::Splitter | SemanticsRole::Separator
    ) {
        TerminalColor::Gray
    } else {
        TerminalColor::DarkGray
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_widget_text(node: &SemanticsNode) -> String {
    node.name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| tui_widget_value_text(node))
        .unwrap_or_else(|| tui_role_label(&node.role).to_string())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_widget_value_text(node: &SemanticsNode) -> Option<String> {
    node.value.as_ref().map(format_semantics_value)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_node_checked(node: &SemanticsNode) -> bool {
    matches!(
        node.state.checked,
        Some(ToggleState::Checked | ToggleState::Mixed)
    )
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_range_fill_width(node: &SemanticsNode, width: u16) -> u16 {
    let ratio = match node.value {
        Some(SemanticsValue::Range { value, min, max }) if max > min => {
            ((value - min) / (max - min)).clamp(0.0, 1.0)
        }
        Some(SemanticsValue::Number(value)) => value.clamp(0.0, 1.0),
        _ => 0.5,
    };
    ((width as f64) * ratio).round().clamp(0.0, width as f64) as u16
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_compact_label(node: &SemanticsNode) -> String {
    let role = match node.role {
        SemanticsRole::GenericContainer => "Group",
        SemanticsRole::ListItem => "Item",
        SemanticsRole::TextInput => "Input",
        SemanticsRole::ScrollView => "Scroll",
        SemanticsRole::Splitter => "Split",
        SemanticsRole::Separator => "Sep",
        SemanticsRole::ColorSwatch => "Swatch",
        SemanticsRole::RadioButton => "Radio",
        SemanticsRole::ProgressBar => "Progress",
        SemanticsRole::BusyIndicator => "Busy",
        _ => tui_role_label(&node.role),
    };
    format!("{role}:{}", node.name.as_deref().unwrap_or("<unnamed>"))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn format_tui_details(node: &SemanticsNode) -> String {
    format!(
        "#{id}\nrole: {role:?}\nname: {name}\nvalue: {value}\nstate: {state}\nactions: {actions}\nbounds: ({x:.0}, {y:.0}, {w:.0}, {h:.0})\n\nEnter/space activates. e edits SetValue controls. +/- adjust slider-like controls.",
        id = node.id.get(),
        role = node.role,
        name = node.name.as_deref().unwrap_or("<unnamed>"),
        value = node
            .value
            .as_ref()
            .map(format_semantics_value)
            .unwrap_or_else(|| "none".to_string()),
        state = format_tui_state(node),
        actions = if node.actions.is_empty() {
            "none".to_string()
        } else {
            node.actions
                .iter()
                .map(|action| format!("{action:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        },
        x = node.bounds.x(),
        y = node.bounds.y(),
        w = node.bounds.width(),
        h = node.bounds.height(),
    )
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn format_semantics_value(value: &SemanticsValue) -> String {
    match value {
        SemanticsValue::Text(text) => text.clone(),
        SemanticsValue::Number(number) => format!("{number:.2}"),
        SemanticsValue::Range { value, min, max } => format!("{value:.2} [{min:.2}..{max:.2}]"),
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn format_tui_state(node: &SemanticsNode) -> String {
    let mut states = Vec::new();
    if node.state.disabled {
        states.push("disabled".to_string());
    }
    if node.state.focused {
        states.push("focused".to_string());
    }
    if node.state.hidden {
        states.push("hidden".to_string());
    }
    if node.state.hovered {
        states.push("hovered".to_string());
    }
    if let Some(checked) = node.state.checked {
        states.push(
            match checked {
                ToggleState::Unchecked => "unchecked",
                ToggleState::Checked => "checked",
                ToggleState::Mixed => "mixed",
            }
            .to_string(),
        );
    }
    if node.state.selected {
        states.push("selected".to_string());
    }
    if let Some(expanded) = node.state.expanded {
        states.push(if expanded { "expanded" } else { "collapsed" }.to_string());
    }
    if node.state.busy {
        states.push("busy".to_string());
    }
    if states.is_empty() {
        "normal".to_string()
    } else {
        states.join(", ")
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_role_label(role: &SemanticsRole) -> &'static str {
    match role {
        SemanticsRole::Window => "Window",
        SemanticsRole::Root => "Root",
        SemanticsRole::GenericContainer => "Group",
        SemanticsRole::Separator => "Separator",
        SemanticsRole::List => "List",
        SemanticsRole::ListItem => "ListItem",
        SemanticsRole::Tree => "Tree",
        SemanticsRole::Table => "Table",
        SemanticsRole::Splitter => "Splitter",
        SemanticsRole::Breadcrumb => "Breadcrumb",
        SemanticsRole::TabBar => "TabBar",
        SemanticsRole::Tabs => "Tabs",
        SemanticsRole::Button => "Button",
        SemanticsRole::Link => "Link",
        SemanticsRole::CheckBox => "CheckBox",
        SemanticsRole::Switch => "Switch",
        SemanticsRole::RadioButton => "Radio",
        SemanticsRole::RadioGroup => "RadioGroup",
        SemanticsRole::Menu => "Menu",
        SemanticsRole::MenuItem => "MenuItem",
        SemanticsRole::ContextMenu => "ContextMenu",
        SemanticsRole::Tooltip => "Tooltip",
        SemanticsRole::Dialog => "Dialog",
        SemanticsRole::Popover => "Popover",
        SemanticsRole::Slider => "Slider",
        SemanticsRole::ProgressBar => "Progress",
        SemanticsRole::BusyIndicator => "Busy",
        SemanticsRole::Text => "Text",
        SemanticsRole::TextInput => "Input",
        SemanticsRole::SpinBox => "SpinBox",
        SemanticsRole::ComboBox => "ComboBox",
        SemanticsRole::Image => "Image",
        SemanticsRole::ColorSwatch => "Swatch",
        SemanticsRole::ColorPicker => "ColorPicker",
        SemanticsRole::Canvas => "Canvas",
        SemanticsRole::ScrollView => "Scroll",
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn actionable_nodes(nodes: &[SemanticsNode]) -> Vec<SemanticsNode> {
    nodes
        .iter()
        .filter(|node| {
            !node.state.hidden
                && !node.state.disabled
                && (node.actions.iter().any(|action| {
                    matches!(
                        action,
                        SemanticsAction::Activate
                            | SemanticsAction::Focus
                            | SemanticsAction::Increment
                            | SemanticsAction::Decrement
                            | SemanticsAction::SetValue
                    )
                }) || matches!(
                    node.role,
                    SemanticsRole::Button
                        | SemanticsRole::CheckBox
                        | SemanticsRole::Switch
                        | SemanticsRole::TextInput
                        | SemanticsRole::Slider
                        | SemanticsRole::SpinBox
                        | SemanticsRole::ScrollView
                ))
        })
        .cloned()
        .collect()
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn preferred_initial_actionable_index(actionable: &[SemanticsNode]) -> Option<usize> {
    actionable.iter().position(|node| {
        node.actions
            .iter()
            .any(|action| matches!(action, SemanticsAction::Activate))
            || matches!(
                node.role,
                SemanticsRole::Button
                    | SemanticsRole::CheckBox
                    | SemanticsRole::Switch
                    | SemanticsRole::TextInput
                    | SemanticsRole::Slider
                    | SemanticsRole::SpinBox
                    | SemanticsRole::ComboBox
                    | SemanticsRole::ColorPicker
            )
    })
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn activate_tui_node(window: &TestWindow, node: &SemanticsNode) -> sui::Result<()> {
    if node
        .actions
        .iter()
        .any(|action| matches!(action, SemanticsAction::Activate))
        || matches!(
            node.role,
            SemanticsRole::Button
                | SemanticsRole::CheckBox
                | SemanticsRole::Switch
                | SemanticsRole::RadioButton
                | SemanticsRole::MenuItem
        )
    {
        return click_tui_node(window, node);
    }

    if matches!(node.role, SemanticsRole::ScrollView) {
        return scroll_tui_node(window, node, Vector::new(0.0, -80.0));
    }

    if node
        .actions
        .iter()
        .any(|action| matches!(action, SemanticsAction::Increment))
    {
        return press_tui_node(window, node, "ArrowRight");
    }

    click_tui_node(window, node)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn set_tui_node_value(window: &TestWindow, node: &SemanticsNode, text: &str) -> sui::Result<()> {
    if node
        .actions
        .iter()
        .any(|action| matches!(action, SemanticsAction::SetValue))
        || matches!(node.role, SemanticsRole::TextInput)
    {
        fill_tui_node(window, node, text)
    } else {
        Err(sui::Error::new(format!(
            "node #{} {:?} does not expose SetValue",
            node.id, node.role
        )))
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn click_tui_node(window: &TestWindow, node: &SemanticsNode) -> sui::Result<()> {
    let point = tui_action_point(node);
    dispatch_tui_event(
        window,
        Event::Pointer(PointerEvent::new(PointerEventKind::Move, point)),
    )?;

    let mut down = PointerEvent::new(PointerEventKind::Down, point);
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    dispatch_tui_event(window, Event::Pointer(down))?;

    let mut up = PointerEvent::new(PointerEventKind::Up, point);
    up.button = Some(PointerButton::Primary);
    dispatch_tui_event(window, Event::Pointer(up))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn scroll_tui_node(window: &TestWindow, node: &SemanticsNode, delta: Vector) -> sui::Result<()> {
    let point = tui_action_point(node);
    dispatch_tui_event(
        window,
        Event::Pointer(PointerEvent::new(PointerEventKind::Move, point)),
    )?;

    let mut scroll = PointerEvent::new(PointerEventKind::Scroll, point);
    scroll.scroll_delta = Some(sui::ScrollDelta::Pixels(delta));
    dispatch_tui_event(window, Event::Pointer(scroll))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn press_tui_node(window: &TestWindow, node: &SemanticsNode, key: &str) -> sui::Result<()> {
    click_tui_node(window, node)?;
    dispatch_tui_event(
        window,
        Event::Keyboard(KeyboardEvent::new(key.to_string(), KeyState::Pressed)),
    )?;
    dispatch_tui_event(
        window,
        Event::Keyboard(KeyboardEvent::new(key.to_string(), KeyState::Released)),
    )
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn fill_tui_node(window: &TestWindow, node: &SemanticsNode, text: &str) -> sui::Result<()> {
    click_tui_node(window, node)?;
    dispatch_tui_event(window, Event::Ime(ImeEvent::CompositionStart))?;
    dispatch_tui_event(
        window,
        Event::Ime(ImeEvent::CompositionUpdate {
            text: text.to_string(),
            cursor_range: None,
        }),
    )?;
    dispatch_tui_event(
        window,
        Event::Ime(ImeEvent::CompositionCommit {
            text: text.to_string(),
        }),
    )?;
    dispatch_tui_event(window, Event::Ime(ImeEvent::CompositionEnd))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn dispatch_tui_event(window: &TestWindow, event: Event) -> sui::Result<()> {
    window.root().dispatch_event(event)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
fn tui_action_point(node: &SemanticsNode) -> Point {
    if matches!(node.role, SemanticsRole::ScrollView) {
        Point::new(
            node.bounds.x() + node.bounds.width().min(48.0),
            node.bounds.y() + node.bounds.height() * 0.5,
        )
    } else {
        Point::new(
            node.bounds.x() + node.bounds.width() * 0.5,
            node.bounds.y() + node.bounds.height() * 0.5,
        )
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebBenchmarkKind {
    RetainedText,
    TextEditing,
    TextComparison,
    ColorValidation,
    WidgetBook,
    DevWorkspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebCanvasFormatPreference {
    Auto,
    Rgba8UnormSrgb,
    Rgba16Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebCanvasColorSpacePreference {
    Auto,
    Srgb,
    DisplayP3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebCanvasToneMappingPreference {
    Auto,
    Standard,
    Extended,
}

#[derive(Debug, Clone, PartialEq)]
struct WebLaunchMode {
    benchmark: Option<WebBenchmarkKind>,
    dev_initial_demo: Option<String>,
    frames: usize,
    warmup_frames: usize,
    canvas_format: WebCanvasFormatPreference,
    canvas_color_space: WebCanvasColorSpacePreference,
    canvas_tone_mapping: WebCanvasToneMappingPreference,
    color_management_mode: WindowColorManagementMode,
    output_primaries: WindowOutputColorPrimaries,
    dynamic_range: WindowDynamicRangeMode,
    tone_mapping: WindowToneMappingMode,
    sdr_content_brightness_nits: f32,
    use_system_sdr_content_brightness: bool,
}

impl Default for WebLaunchMode {
    fn default() -> Self {
        Self {
            benchmark: None,
            dev_initial_demo: None,
            frames: 180,
            warmup_frames: 60,
            canvas_format: WebCanvasFormatPreference::Auto,
            canvas_color_space: WebCanvasColorSpacePreference::Auto,
            canvas_tone_mapping: WebCanvasToneMappingPreference::Auto,
            color_management_mode: WindowColorManagementMode::Automatic,
            output_primaries: WindowOutputColorPrimaries::Automatic,
            dynamic_range: WindowDynamicRangeMode::Automatic,
            tone_mapping: WindowToneMappingMode::Automatic,
            sdr_content_brightness_nits: DEFAULT_WEB_SDR_CONTENT_BRIGHTNESS_NITS,
            use_system_sdr_content_brightness: true,
        }
    }
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn parse_positive_nits(value: &str) -> Option<f32> {
    value
        .parse::<f32>()
        .ok()
        .filter(|nits| nits.is_finite() && *nits > 0.0)
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn parse_bool_query_value(value: &str) -> Option<bool> {
    match value {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn parse_web_launch_mode(query: &str) -> WebLaunchMode {
    let mut mode = WebLaunchMode::default();

    for pair in query.trim_start_matches('?').split('&') {
        if pair.is_empty() {
            continue;
        }

        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        match key {
            "benchmark" => {
                mode.benchmark = match value {
                    "retained-text" => Some(WebBenchmarkKind::RetainedText),
                    "text-editing" => Some(WebBenchmarkKind::TextEditing),
                    "text-comparison" | "comparison-surface" => {
                        Some(WebBenchmarkKind::TextComparison)
                    }
                    "color-validation" | "wide-gamut-validation" => {
                        Some(WebBenchmarkKind::ColorValidation)
                    }
                    "widget-book" => Some(WebBenchmarkKind::WidgetBook),
                    "dev" | "workspace" => Some(WebBenchmarkKind::DevWorkspace),
                    _ => None,
                };
            }
            "demo" | "dev-demo" => {
                let slug = value.to_ascii_lowercase();
                if app::dev_demo_label_for_slug(&slug).is_some() {
                    mode.dev_initial_demo = Some(slug);
                }
            }
            "frames" => {
                mode.frames = value
                    .parse::<usize>()
                    .unwrap_or(mode.frames)
                    .clamp(1, 10_000);
            }
            "warmup" | "warmup-frames" => {
                mode.warmup_frames = value
                    .parse::<usize>()
                    .unwrap_or(mode.warmup_frames)
                    .clamp(0, 2_000);
            }
            "canvas-format" => {
                mode.canvas_format = match value {
                    "rgba8unorm-srgb" | "srgb" => WebCanvasFormatPreference::Rgba8UnormSrgb,
                    "rgba16float" | "float16" | "hdr" => WebCanvasFormatPreference::Rgba16Float,
                    _ => WebCanvasFormatPreference::Auto,
                };
            }
            "canvas-color-space" => {
                mode.canvas_color_space = match value {
                    "srgb" => WebCanvasColorSpacePreference::Srgb,
                    "display-p3" | "p3" => WebCanvasColorSpacePreference::DisplayP3,
                    _ => WebCanvasColorSpacePreference::Auto,
                };
            }
            "canvas-tone-mapping" => {
                mode.canvas_tone_mapping = match value {
                    "standard" => WebCanvasToneMappingPreference::Standard,
                    "extended" | "hdr" => WebCanvasToneMappingPreference::Extended,
                    _ => WebCanvasToneMappingPreference::Auto,
                };
            }
            "color-management" => {
                mode.color_management_mode = match value {
                    "force-sdr" => WindowColorManagementMode::ForceSdr,
                    "prefer-wide-gamut" => WindowColorManagementMode::PreferWideGamut,
                    "prefer-hdr" => WindowColorManagementMode::PreferHdr,
                    _ => WindowColorManagementMode::Automatic,
                };
            }
            "output-primaries" => {
                mode.output_primaries = match value {
                    "srgb" => WindowOutputColorPrimaries::Srgb,
                    "display-p3" | "p3" => WindowOutputColorPrimaries::DisplayP3,
                    _ => WindowOutputColorPrimaries::Automatic,
                };
            }
            "dynamic-range" => {
                mode.dynamic_range = match value {
                    "sdr" | "standard" => WindowDynamicRangeMode::StandardDynamicRange,
                    "hdr" | "high" => WindowDynamicRangeMode::HighDynamicRange,
                    _ => WindowDynamicRangeMode::Automatic,
                };
            }
            "tone-mapping" => {
                mode.tone_mapping = match value {
                    "clamp" => WindowToneMappingMode::Clamp,
                    "reinhard" => WindowToneMappingMode::Reinhard,
                    _ => WindowToneMappingMode::Automatic,
                };
            }
            "sdr-content-brightness" | "sdr-content-brightness-nits" => {
                if let Some(nits) = parse_positive_nits(value) {
                    mode.sdr_content_brightness_nits = nits;
                }
            }
            "use-system-sdr-brightness" | "use-system-sdr-content-brightness" => {
                if let Some(enabled) = parse_bool_query_value(value) {
                    mode.use_system_sdr_content_brightness = enabled;
                }
            }
            _ => {}
        }
    }

    if web_uses_hdr_canvas(&mode) {
        mode.canvas_color_space = WebCanvasColorSpacePreference::Srgb;
        mode.output_primaries = WindowOutputColorPrimaries::Srgb;
    }

    mode
}

#[derive(Debug, Clone, PartialEq)]
struct WebBrowserProbe {
    current_path: String,
    user_agent: String,
    language: String,
    device_pixel_ratio: f64,
    canvas_count: u32,
    document_title: String,
}

#[derive(Debug, Clone, PartialEq)]
struct WebCanvasCapture {
    canvas_count: u32,
    first_canvas_id: String,
    first_canvas_width: u32,
    first_canvas_height: u32,
    first_canvas_data_url_len: usize,
}

fn web_benchmark_slug(benchmark: Option<WebBenchmarkKind>) -> &'static str {
    match benchmark {
        Some(WebBenchmarkKind::RetainedText) => "retained-text",
        Some(WebBenchmarkKind::TextEditing) => "text-editing",
        Some(WebBenchmarkKind::TextComparison) => "text-comparison",
        Some(WebBenchmarkKind::ColorValidation) => "color-validation",
        Some(WebBenchmarkKind::WidgetBook) => "widget-book",
        Some(WebBenchmarkKind::DevWorkspace) | None => "dev",
    }
}

fn web_canvas_format_slug(format: WebCanvasFormatPreference) -> &'static str {
    match format {
        WebCanvasFormatPreference::Auto => "auto",
        WebCanvasFormatPreference::Rgba8UnormSrgb => "rgba8unorm-srgb",
        WebCanvasFormatPreference::Rgba16Float => "rgba16float",
    }
}

fn web_canvas_color_space_slug(color_space: WebCanvasColorSpacePreference) -> &'static str {
    match color_space {
        WebCanvasColorSpacePreference::Auto => "auto",
        WebCanvasColorSpacePreference::Srgb => "srgb",
        WebCanvasColorSpacePreference::DisplayP3 => "display-p3",
    }
}

fn web_uses_hdr_canvas(mode: &WebLaunchMode) -> bool {
    matches!(mode.canvas_format, WebCanvasFormatPreference::Rgba16Float)
        || matches!(
            mode.canvas_tone_mapping,
            WebCanvasToneMappingPreference::Extended
        )
        || matches!(
            mode.color_management_mode,
            WindowColorManagementMode::PreferHdr
        )
        || matches!(mode.dynamic_range, WindowDynamicRangeMode::HighDynamicRange)
}

fn web_canvas_color_space_slug_for_mode(mode: &WebLaunchMode) -> &'static str {
    if web_uses_hdr_canvas(mode) {
        "srgb"
    } else {
        web_canvas_color_space_slug(mode.canvas_color_space)
    }
}

fn web_canvas_tone_mapping_slug(tone_mapping: WebCanvasToneMappingPreference) -> &'static str {
    match tone_mapping {
        WebCanvasToneMappingPreference::Auto => "auto",
        WebCanvasToneMappingPreference::Standard => "standard",
        WebCanvasToneMappingPreference::Extended => "extended",
    }
}

fn web_color_management_slug(mode: WindowColorManagementMode) -> &'static str {
    match mode {
        WindowColorManagementMode::Automatic => "automatic",
        WindowColorManagementMode::ForceSdr => "force-sdr",
        WindowColorManagementMode::PreferWideGamut => "prefer-wide-gamut",
        WindowColorManagementMode::PreferHdr => "prefer-hdr",
    }
}

fn web_output_primaries_slug(primaries: WindowOutputColorPrimaries) -> &'static str {
    match primaries {
        WindowOutputColorPrimaries::Automatic => "automatic",
        WindowOutputColorPrimaries::Srgb => "srgb",
        WindowOutputColorPrimaries::DisplayP3 => "display-p3",
    }
}

fn web_output_primaries_for_mode(mode: &WebLaunchMode) -> WindowOutputColorPrimaries {
    if web_uses_hdr_canvas(mode) {
        WindowOutputColorPrimaries::Srgb
    } else {
        mode.output_primaries
    }
}

fn web_output_primaries_slug_for_mode(mode: &WebLaunchMode) -> &'static str {
    web_output_primaries_slug(web_output_primaries_for_mode(mode))
}

fn web_dynamic_range_slug(dynamic_range: WindowDynamicRangeMode) -> &'static str {
    match dynamic_range {
        WindowDynamicRangeMode::Automatic => "automatic",
        WindowDynamicRangeMode::StandardDynamicRange => "sdr",
        WindowDynamicRangeMode::HighDynamicRange => "hdr",
    }
}

fn web_tone_mapping_slug(tone_mapping: WindowToneMappingMode) -> &'static str {
    match tone_mapping {
        WindowToneMappingMode::Automatic => "automatic",
        WindowToneMappingMode::Clamp => "clamp",
        WindowToneMappingMode::Reinhard => "reinhard",
    }
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_canvas_capture_report(mode: &WebLaunchMode, capture: &WebCanvasCapture) -> String {
    format!(
        "route={}; canvas_count={}; first_canvas_id={}; first_canvas_size={}x{}; first_canvas_data_url_len={}",
        web_benchmark_slug(mode.benchmark),
        capture.canvas_count,
        capture.first_canvas_id,
        capture.first_canvas_width,
        capture.first_canvas_height,
        capture.first_canvas_data_url_len,
    )
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_validation_url_for_path(path: &str, mode: &WebLaunchMode) -> String {
    let normalized_path = if path.is_empty() { "/" } else { path };
    format!("{}?{}", normalized_path, web_validation_query(mode))
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn non_hdr_capture_mode(mode: &WebLaunchMode) -> WebLaunchMode {
    let mut capture = mode.clone();
    capture.canvas_format = WebCanvasFormatPreference::Rgba8UnormSrgb;
    capture.canvas_color_space = WebCanvasColorSpacePreference::Srgb;
    capture.canvas_tone_mapping = WebCanvasToneMappingPreference::Standard;
    capture.color_management_mode = WindowColorManagementMode::ForceSdr;
    capture.output_primaries = WindowOutputColorPrimaries::Srgb;
    capture.dynamic_range = WindowDynamicRangeMode::StandardDynamicRange;
    capture.tone_mapping = WindowToneMappingMode::Clamp;
    capture.use_system_sdr_content_brightness = false;
    capture
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_non_hdr_capture_url_for_path(path: &str, mode: &WebLaunchMode) -> String {
    web_validation_url_for_path(path, &non_hdr_capture_mode(mode))
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_browser_probe_report(mode: &WebLaunchMode, probe: &WebBrowserProbe) -> String {
    format!(
        "route={}; path={}; document_title={}; language={}; device_pixel_ratio={}; canvas_count={}; user_agent={}; validation_url={}",
        web_benchmark_slug(mode.benchmark),
        probe.current_path,
        probe.document_title,
        probe.language,
        probe.device_pixel_ratio,
        probe.canvas_count,
        probe.user_agent,
        web_validation_url_for_path(&probe.current_path, mode),
    )
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_validation_query(mode: &WebLaunchMode) -> String {
    let mut query = format!(
        "benchmark={}&frames={}&warmup={}&canvas-format={}&canvas-color-space={}&canvas-tone-mapping={}&color-management={}&output-primaries={}&dynamic-range={}&tone-mapping={}&sdr-content-brightness={:.0}&use-system-sdr-brightness={}",
        web_benchmark_slug(mode.benchmark),
        mode.frames,
        mode.warmup_frames,
        web_canvas_format_slug(mode.canvas_format),
        web_canvas_color_space_slug_for_mode(mode),
        web_canvas_tone_mapping_slug(mode.canvas_tone_mapping),
        web_color_management_slug(mode.color_management_mode),
        web_output_primaries_slug_for_mode(mode),
        web_dynamic_range_slug(mode.dynamic_range),
        web_tone_mapping_slug(mode.tone_mapping),
        mode.sdr_content_brightness_nits,
        mode.use_system_sdr_content_brightness,
    );
    if let Some(demo) = mode.dev_initial_demo.as_deref() {
        query.push_str("&demo=");
        query.push_str(demo);
    }
    query
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_validation_report(mode: &WebLaunchMode) -> String {
    format!(
        "route={}; canvas_format={}; canvas_color_space={}; canvas_tone_mapping={}; color_management={}; output_primaries={}; dynamic_range={}; tone_mapping={}; sdr_content_brightness={:.0}; use_system_sdr_brightness={}; query=?{}",
        web_benchmark_slug(mode.benchmark),
        web_canvas_format_slug(mode.canvas_format),
        web_canvas_color_space_slug_for_mode(mode),
        web_canvas_tone_mapping_slug(mode.canvas_tone_mapping),
        web_color_management_slug(mode.color_management_mode),
        web_output_primaries_slug_for_mode(mode),
        web_dynamic_range_slug(mode.dynamic_range),
        web_tone_mapping_slug(mode.tone_mapping),
        mode.sdr_content_brightness_nits,
        mode.use_system_sdr_content_brightness,
        web_validation_query(mode),
    )
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn web_window_render_options(mode: &WebLaunchMode) -> WindowRenderOptions {
    app::apply_demo_small_text_rendering_profile(
        WindowRenderOptions::new(true, 1.0)
            .with_color_management_mode(mode.color_management_mode)
            .with_output_color_primaries(web_output_primaries_for_mode(mode))
            .with_dynamic_range_mode(mode.dynamic_range)
            .with_tone_mapping_mode(mode.tone_mapping)
            .with_sdr_content_brightness_nits(mode.sdr_content_brightness_nits)
            .with_system_sdr_content_brightness_enabled(mode.use_system_sdr_content_brightness),
    )
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn build_application_for_web_mode(mode: &WebLaunchMode) -> Application {
    let render_options = web_window_render_options(mode);
    let application = match mode.benchmark {
        Some(WebBenchmarkKind::RetainedText) => build_retained_text_benchmark_application(),
        Some(WebBenchmarkKind::TextEditing) => build_text_editing_benchmark_application(),
        Some(WebBenchmarkKind::TextComparison) => build_text_rendering_comparison_application(),
        Some(WebBenchmarkKind::ColorValidation) => build_color_validation_application(),
        Some(WebBenchmarkKind::WidgetBook) => {
            build_widget_book_application(default_widget_book_state())
        }
        Some(WebBenchmarkKind::DevWorkspace) | None => {
            let initial_demo = mode
                .dev_initial_demo
                .as_deref()
                .and_then(app::dev_demo_label_for_slug);
            app::build_dev_application_with_initial_demo_and_render_options(
                initial_demo,
                render_options,
            )
        }
    };
    application.with_window_render_options(render_options)
}

pub fn run_desktop() -> sui::Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        build_dev_application().run()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let launch_mode = current_desktop_launch_mode()?;
        run_desktop_application_with_mode(launch_mode)
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
fn current_web_launch_mode() -> WebLaunchMode {
    let query = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .unwrap_or_default();
    parse_web_launch_mode(&query)
}

#[cfg(target_arch = "wasm32")]
fn current_web_browser_probe() -> WebBrowserProbe {
    let Some(window) = web_sys::window() else {
        return WebBrowserProbe {
            current_path: "/".to_string(),
            user_agent: String::new(),
            language: String::new(),
            device_pixel_ratio: 1.0,
            canvas_count: 0,
            document_title: String::new(),
        };
    };

    let location = window.location();
    let document = window.document();
    let navigator = window.navigator();
    let current_path = location
        .pathname()
        .ok()
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "/".to_string());
    let canvas_count = document
        .as_ref()
        .map(|document| document.get_elements_by_tag_name("canvas").length())
        .unwrap_or(0);

    WebBrowserProbe {
        current_path,
        user_agent: navigator.user_agent().unwrap_or_default(),
        language: navigator.language().unwrap_or_default(),
        device_pixel_ratio: window.device_pixel_ratio(),
        canvas_count,
        document_title: document
            .map(|document| document.title())
            .unwrap_or_default(),
    }
}

#[cfg(target_arch = "wasm32")]
fn first_web_canvas() -> Option<web_sys::HtmlCanvasElement> {
    use wasm_bindgen::JsCast;

    web_sys::window()?
        .document()?
        .get_elements_by_tag_name("canvas")
        .item(0)?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .ok()
}

#[cfg(target_arch = "wasm32")]
fn web_canvas_sdr_png_data_url(canvas: &web_sys::HtmlCanvasElement) -> String {
    use wasm_bindgen::JsCast;

    let fallback = || canvas.to_data_url().unwrap_or_default();
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return fallback();
    };
    let Ok(element) = document.create_element("canvas") else {
        return fallback();
    };
    let Ok(sdr_canvas) = element.dyn_into::<web_sys::HtmlCanvasElement>() else {
        return fallback();
    };
    sdr_canvas.set_width(canvas.width().max(1));
    sdr_canvas.set_height(canvas.height().max(1));

    let Ok(Some(context)) = sdr_canvas.get_context("2d") else {
        return fallback();
    };
    let Ok(context) = context.dyn_into::<web_sys::CanvasRenderingContext2d>() else {
        return fallback();
    };
    context.set_image_smoothing_enabled(false);
    if context
        .draw_image_with_html_canvas_element(canvas, 0.0, 0.0)
        .is_err()
    {
        return fallback();
    }
    if !web_canvas_context_has_visible_pixels(&context, sdr_canvas.width(), sdr_canvas.height()) {
        return fallback();
    }

    sdr_canvas.to_data_url().unwrap_or_else(|_| fallback())
}

#[cfg(target_arch = "wasm32")]
fn web_canvas_context_has_visible_pixels(
    context: &web_sys::CanvasRenderingContext2d,
    width: u32,
    height: u32,
) -> bool {
    let x = width.max(1) / 2;
    let y = height.max(1) / 2;
    context
        .get_image_data(x as f64, y as f64, 1.0, 1.0)
        .ok()
        .and_then(|image_data| image_data.data().0.get(3).copied())
        .is_some_and(|alpha| alpha > 0)
}

#[cfg(target_arch = "wasm32")]
fn current_web_canvas_capture() -> WebCanvasCapture {
    let Some(window) = web_sys::window() else {
        return WebCanvasCapture {
            canvas_count: 0,
            first_canvas_id: String::new(),
            first_canvas_width: 0,
            first_canvas_height: 0,
            first_canvas_data_url_len: 0,
        };
    };
    let Some(document) = window.document() else {
        return WebCanvasCapture {
            canvas_count: 0,
            first_canvas_id: String::new(),
            first_canvas_width: 0,
            first_canvas_height: 0,
            first_canvas_data_url_len: 0,
        };
    };

    let canvases = document.get_elements_by_tag_name("canvas");
    let canvas_count = canvases.length();
    let Some(canvas) = first_web_canvas() else {
        return WebCanvasCapture {
            canvas_count,
            first_canvas_id: String::new(),
            first_canvas_width: 0,
            first_canvas_height: 0,
            first_canvas_data_url_len: 0,
        };
    };
    let data_url_len = web_canvas_sdr_png_data_url(&canvas).len();

    WebCanvasCapture {
        canvas_count,
        first_canvas_id: canvas.id(),
        first_canvas_width: canvas.width(),
        first_canvas_height: canvas.height(),
        first_canvas_data_url_len: data_url_len,
    }
}

#[cfg(target_arch = "wasm32")]
fn current_web_validation_url() -> String {
    let probe = current_web_browser_probe();
    web_validation_url_for_path(&probe.current_path, &current_web_launch_mode())
}

#[cfg(target_arch = "wasm32")]
fn current_web_non_hdr_capture_url() -> String {
    let probe = current_web_browser_probe();
    web_non_hdr_capture_url_for_path(&probe.current_path, &current_web_launch_mode())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_validation_query() -> String {
    web_validation_query(&current_web_launch_mode())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_validation_report() -> String {
    web_validation_report(&current_web_launch_mode())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_browser_probe_report() -> String {
    web_browser_probe_report(&current_web_launch_mode(), &current_web_browser_probe())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_canvas_capture_report() -> String {
    web_canvas_capture_report(&current_web_launch_mode(), &current_web_canvas_capture())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_canvas_capture_data_url() -> String {
    first_web_canvas()
        .map(|canvas| web_canvas_sdr_png_data_url(&canvas))
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_validation_url() -> String {
    current_web_validation_url()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_non_hdr_canvas_capture_url() -> String {
    current_web_non_hdr_capture_url()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let mode = current_web_launch_mode();
    build_application_for_web_mode(&mode)
        .run()
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_web_launch_mode() {
        assert_eq!(parse_web_launch_mode("").benchmark, None);
        assert_eq!(parse_web_launch_mode("benchmark=unknown").benchmark, None);
    }

    #[test]
    fn parses_dev_workspace_initial_demo_slug() {
        let mode = parse_web_launch_mode("benchmark=dev&demo=paint");

        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::DevWorkspace));
        assert_eq!(mode.dev_initial_demo.as_deref(), Some("paint"));
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn parses_dev_workspace_markdown_initial_demo_slug() {
        let mode = parse_web_launch_mode("benchmark=dev&demo=markdown-render");

        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::DevWorkspace));
        assert_eq!(mode.dev_initial_demo.as_deref(), Some("markdown-render"));
    }

    #[test]
    fn parses_text_comparison_web_benchmark_mode() {
        let mode = parse_web_launch_mode("benchmark=text-comparison&frames=240&warmup=30");
        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::TextComparison));
        assert_eq!(mode.frames, 240);
        assert_eq!(mode.warmup_frames, 30);
    }

    #[test]
    fn parses_comparison_surface_alias() {
        let mode = parse_web_launch_mode("benchmark=comparison-surface");
        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::TextComparison));
    }

    #[test]
    fn parses_color_validation_and_web_output_preferences() {
        let mode = parse_web_launch_mode(
            "benchmark=color-validation&canvas-format=float16&canvas-color-space=display-p3&canvas-tone-mapping=extended&color-management=prefer-hdr&output-primaries=display-p3&dynamic-range=hdr&tone-mapping=reinhard&sdr-content-brightness=260&use-system-sdr-brightness=false",
        );
        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::ColorValidation));
        assert_eq!(mode.canvas_format, WebCanvasFormatPreference::Rgba16Float);
        assert_eq!(mode.canvas_color_space, WebCanvasColorSpacePreference::Srgb);
        assert_eq!(
            mode.canvas_tone_mapping,
            WebCanvasToneMappingPreference::Extended
        );
        assert_eq!(
            mode.color_management_mode,
            WindowColorManagementMode::PreferHdr
        );
        assert_eq!(mode.output_primaries, WindowOutputColorPrimaries::Srgb);
        assert_eq!(mode.dynamic_range, WindowDynamicRangeMode::HighDynamicRange);
        assert_eq!(mode.tone_mapping, WindowToneMappingMode::Reinhard);
        assert_eq!(mode.sdr_content_brightness_nits, 260.0);
        assert!(!mode.use_system_sdr_content_brightness);
    }

    #[test]
    fn web_window_render_options_reflect_launch_mode_preferences() {
        let mode = parse_web_launch_mode(
            "color-management=prefer-wide-gamut&output-primaries=display-p3&dynamic-range=hdr&tone-mapping=clamp&sdr-content-brightness=180&use-system-sdr-brightness=off",
        );
        let options = web_window_render_options(&mode);

        assert_eq!(
            options.color_management_mode,
            WindowColorManagementMode::PreferWideGamut
        );
        assert_eq!(
            options.output_color_primaries,
            WindowOutputColorPrimaries::Srgb
        );
        assert_eq!(
            options.dynamic_range_mode,
            WindowDynamicRangeMode::HighDynamicRange
        );
        assert_eq!(options.tone_mapping_mode, WindowToneMappingMode::Clamp);
        assert_eq!(options.sdr_content_brightness_nits, 180.0);
        assert!(!options.use_system_sdr_content_brightness);
        assert!(matches!(
            options.stem_darkening.normalized(),
            sui::WindowStemDarkening::Enabled { max_ppem, amount }
                if (max_ppem - app::DEMO_SMALL_TEXT_STEM_DARKENING_MAX_PPEM).abs()
                    < f32::EPSILON
                    && (amount - app::DEMO_SMALL_TEXT_STEM_DARKENING_AMOUNT).abs()
                        < f32::EPSILON
        ));
    }

    #[test]
    fn web_validation_query_normalizes_phase4_preferences() {
        let mode = parse_web_launch_mode(
            "benchmark=wide-gamut-validation&canvas-format=hdr&canvas-color-space=p3&canvas-tone-mapping=hdr&color-management=prefer-hdr&output-primaries=p3&dynamic-range=high&tone-mapping=reinhard&frames=240&warmup=24",
        );

        assert_eq!(
            web_validation_query(&mode),
            "benchmark=color-validation&frames=240&warmup=24&canvas-format=rgba16float&canvas-color-space=srgb&canvas-tone-mapping=extended&color-management=prefer-hdr&output-primaries=srgb&dynamic-range=hdr&tone-mapping=reinhard&sdr-content-brightness=80&use-system-sdr-brightness=true"
        );
    }

    #[test]
    fn web_validation_query_preserves_dev_demo_slug() {
        let mode = parse_web_launch_mode("benchmark=dev&demo=paint&frames=5&warmup=1");

        assert!(web_validation_query(&mode).contains("&demo=paint"));
    }

    #[test]
    fn non_hdr_capture_url_overrides_hdr_preferences_for_png_capture() {
        let mode = parse_web_launch_mode(
            "benchmark=widget-book&canvas-format=rgba16float&canvas-color-space=display-p3&canvas-tone-mapping=extended&color-management=prefer-hdr&output-primaries=display-p3&dynamic-range=hdr&tone-mapping=reinhard&frames=5&warmup=1",
        );
        let url = web_non_hdr_capture_url_for_path("/review", &mode);

        assert!(url.starts_with("/review?benchmark=widget-book"));
        assert!(url.contains("frames=5"));
        assert!(url.contains("warmup=1"));
        assert!(url.contains("canvas-format=rgba8unorm-srgb"));
        assert!(url.contains("canvas-color-space=srgb"));
        assert!(url.contains("canvas-tone-mapping=standard"));
        assert!(url.contains("color-management=force-sdr"));
        assert!(url.contains("output-primaries=srgb"));
        assert!(url.contains("dynamic-range=sdr"));
        assert!(url.contains("tone-mapping=clamp"));
        assert!(url.contains("use-system-sdr-brightness=false"));
    }

    #[test]
    fn web_validation_report_summarizes_browser_validation_configuration() {
        let mode = parse_web_launch_mode(
            "benchmark=color-validation&canvas-format=float16&canvas-color-space=display-p3&canvas-tone-mapping=extended&color-management=prefer-wide-gamut&output-primaries=display-p3&dynamic-range=hdr&tone-mapping=clamp",
        );
        let report = web_validation_report(&mode);

        assert!(report.contains("route=color-validation"));
        assert!(report.contains("canvas_format=rgba16float"));
        assert!(report.contains("canvas_color_space=srgb"));
        assert!(report.contains("canvas_tone_mapping=extended"));
        assert!(report.contains("color_management=prefer-wide-gamut"));
        assert!(report.contains("output_primaries=srgb"));
        assert!(report.contains("dynamic_range=hdr"));
        assert!(report.contains("tone_mapping=clamp"));
        assert!(report.contains("sdr_content_brightness=80"));
        assert!(report.contains("use_system_sdr_brightness=true"));
    }

    #[test]
    fn web_default_sdr_brightness_matches_browser_sdr_white() {
        let mode = parse_web_launch_mode(
            "benchmark=widget-book&canvas-format=rgba16float&canvas-tone-mapping=extended&color-management=prefer-hdr&dynamic-range=hdr",
        );
        let options = web_window_render_options(&mode);

        assert_eq!(mode.sdr_content_brightness_nits, 80.0);
        assert_eq!(options.sdr_content_brightness_nits, 80.0);
        assert!(mode.use_system_sdr_content_brightness);
    }

    #[test]
    fn web_browser_probe_report_includes_live_browser_context() {
        let mode = parse_web_launch_mode(
            "benchmark=color-validation&canvas-format=float16&canvas-color-space=display-p3&canvas-tone-mapping=extended&color-management=prefer-hdr&output-primaries=display-p3&dynamic-range=hdr&tone-mapping=reinhard",
        );
        let probe = WebBrowserProbe {
            current_path: "/sui-demo".to_string(),
            user_agent: "ExampleBrowser/1.0".to_string(),
            language: "en-US".to_string(),
            device_pixel_ratio: 2.0,
            canvas_count: 2,
            document_title: "SUI Demo Validation".to_string(),
        };
        let report = web_browser_probe_report(&mode, &probe);

        assert!(report.contains("path=/sui-demo"));
        assert!(report.contains("document_title=SUI Demo Validation"));
        assert!(report.contains("language=en-US"));
        assert!(report.contains("device_pixel_ratio=2"));
        assert!(report.contains("canvas_count=2"));
        assert!(report.contains("user_agent=ExampleBrowser/1.0"));
        assert!(report.contains("validation_url=/sui-demo?benchmark=color-validation"));
        assert!(report.contains("canvas-format=rgba16float"));
    }

    #[test]
    fn web_canvas_capture_report_describes_first_canvas_snapshot() {
        let mode = parse_web_launch_mode(
            "benchmark=color-validation&canvas-format=float16&canvas-color-space=display-p3&canvas-tone-mapping=extended",
        );
        let capture = WebCanvasCapture {
            canvas_count: 2,
            first_canvas_id: "sui-main-canvas".to_string(),
            first_canvas_width: 1920,
            first_canvas_height: 1080,
            first_canvas_data_url_len: 128,
        };
        let report = web_canvas_capture_report(&mode, &capture);

        assert!(report.contains("route=color-validation"));
        assert!(report.contains("canvas_count=2"));
        assert!(report.contains("first_canvas_id=sui-main-canvas"));
        assert!(report.contains("first_canvas_size=1920x1080"));
        assert!(report.contains("first_canvas_data_url_len=128"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_defaults_to_vsync() {
        let mode = parse_desktop_launch_mode(Vec::<&str>::new(), false, None).unwrap();
        assert!(mode.vsync_enabled);
        assert_eq!(mode.automation, None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_honors_no_vsync_sources() {
        let cli_mode = parse_desktop_launch_mode(["--no-vsync"], false, None).unwrap();
        let env_mode = parse_desktop_launch_mode(Vec::<&str>::new(), true, None).unwrap();

        assert!(!cli_mode.vsync_enabled);
        assert!(!env_mode.vsync_enabled);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_allows_cli_override_back_to_vsync() {
        let mode = parse_desktop_launch_mode(["--vsync"], true, None).unwrap();
        assert!(mode.vsync_enabled);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_accepts_automation_sources() {
        let cli_mode =
            parse_desktop_launch_mode(["--automation=widget-book-scroll"], false, None).unwrap();
        let env_mode = parse_desktop_launch_mode(
            Vec::<&str>::new(),
            false,
            Some(DesktopLaunchAutomation::WidgetBookScroll),
        )
        .unwrap();

        assert_eq!(
            cli_mode.automation,
            Some(DesktopLaunchAutomation::WidgetBookScroll)
        );
        assert_eq!(
            env_mode.automation,
            Some(DesktopLaunchAutomation::WidgetBookScroll)
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_accepts_widget_book_scroll_automation() {
        let mode =
            parse_desktop_launch_mode(["--automation=widget-book-scroll"], false, None).unwrap();
        assert_eq!(
            mode.automation,
            Some(DesktopLaunchAutomation::WidgetBookScroll)
        );
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
    #[test]
    fn desktop_launch_mode_accepts_tui_flags() {
        let mode = parse_desktop_launch_mode(["--tui", "--tui-show-hidden"], false, None).unwrap();

        let tui = mode.tui.expect("tui mode parsed");
        assert_eq!(tui.kind, DesktopTuiLaunchKind::Interactive);
        assert_eq!(tui.layout, TuiLayoutMode::Spatial);
        assert!(tui.show_hidden);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
    #[test]
    fn desktop_launch_mode_accepts_tui_dump_mode() {
        let mode = parse_desktop_launch_mode(["--tui-dump-accessibility"], false, None).unwrap();

        let tui = mode.tui.expect("tui mode parsed");
        assert_eq!(tui.kind, DesktopTuiLaunchKind::DumpAccessibility);
        assert_eq!(tui.layout, TuiLayoutMode::Spatial);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
    #[test]
    fn desktop_launch_mode_rejects_unknown_tui_layout() {
        let error = parse_desktop_launch_mode(["--tui-layout=diagonal"], false, None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unsupported sui-demo TUI layout")
        );
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "tui")))]
    #[test]
    fn desktop_launch_mode_rejects_tui_flags_without_tui_feature() {
        let error = parse_desktop_launch_mode(["--tui"], false, None).unwrap_err();
        assert!(error.to_string().contains("without the `tui` feature"));
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
    #[test]
    fn generated_tui_includes_major_dev_workspace_views() -> sui::Result<()> {
        let app = sui_testing::TestApp::from_runtime(build_dev_application().build()?)?;
        let window = app.main_window()?;
        window
            .get_by_role(SemanticsRole::Button)
            .with_name("SUI menu")
            .click()?;
        let snapshot = window.snapshot()?;
        let frame = render_snapshot(
            &snapshot.accessibility,
            TuiRenderOptions {
                width: 160,
                height: 2000,
                mode: TuiLayoutMode::Structured,
                show_hidden: true,
            },
        )
        .to_string();

        for label in [
            "Widget book",
            "HDR validation",
            "Layout",
            "Drag and drop",
            "Paint",
            "Vector editor",
            "Open demo",
            "Theme mode",
            "Settings",
        ] {
            assert!(frame.contains(label), "missing generated TUI label {label}");
        }

        Ok(())
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
    #[test]
    fn generated_tui_mouse_hit_tests_list_and_spatial_nodes() {
        let list_area = TerminalRect::new(20, 3, 24, 8);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 22,
            row: 5,
            modifiers: KeyModifiers::empty(),
        };

        let root = SemanticsNode::new(
            sui::WidgetId::new(1),
            SemanticsRole::Window,
            SuiRect::new(0.0, 0.0, 100.0, 100.0),
        );
        let mut first = SemanticsNode::new(
            sui::WidgetId::new(2),
            SemanticsRole::Button,
            SuiRect::new(10.0, 10.0, 20.0, 20.0),
        );
        first.parent = Some(root.id);
        first.name = Some("First".to_string());
        let mut second = SemanticsNode::new(
            sui::WidgetId::new(3),
            SemanticsRole::Button,
            SuiRect::new(60.0, 60.0, 20.0, 20.0),
        );
        second.parent = Some(root.id);
        second.name = Some("Second".to_string());
        let actionable = vec![first.clone(), second.clone()];
        let tree_nodes = vec![root.clone(), first.clone(), second.clone()];
        let rows = tui_action_tree_rows(&tree_nodes, &actionable);

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].actionable_index, None);
        assert_eq!(rows[1].actionable_index, Some(0));
        assert_eq!(rows[2].actionable_index, Some(1));

        assert_eq!(
            mouse_hit_action_tree(mouse, list_area, &tree_nodes, &actionable, 0),
            Some(0)
        );
        let mouse_second_tree_row = MouseEvent { row: 6, ..mouse };
        assert_eq!(
            mouse_hit_action_tree(
                mouse_second_tree_row,
                list_area,
                &tree_nodes,
                &actionable,
                0
            ),
            Some(1)
        );
        let long_actionable = (0..40)
            .map(|index| {
                let mut node = SemanticsNode::new(
                    sui::WidgetId::new(10 + index as u64),
                    SemanticsRole::Button,
                    SuiRect::new(0.0, 0.0, 10.0, 10.0),
                );
                node.name = Some(format!("Button {index}"));
                node
            })
            .collect::<Vec<_>>();
        assert_eq!(tui_list_offset(20, list_area, long_actionable.len()), 17);
        assert_eq!(
            mouse_hit_action_tree(mouse, list_area, &long_actionable, &long_actionable, 20),
            Some(18)
        );

        let spatial_area = TerminalRect::new(0, 0, 12, 12);
        let spatial_inner = inner_terminal_rect(spatial_area).expect("inner spatial area");
        let world = SuiRect::new(0.0, 0.0, 100.0, 100.0);
        assert!(
            world_to_tui_row(10.0, spatial_inner, world)
                < world_to_tui_row(80.0, spatial_inner, world),
            "spatial map should preserve top-left GUI coordinate orientation"
        );
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 7,
            row: 7,
            modifiers: KeyModifiers::empty(),
        };

        assert_eq!(
            mouse_hit_spatial_node(
                mouse,
                spatial_area,
                &tree_nodes,
                &actionable,
                None,
                &TuiSpatialState::default()
            ),
            Some(1)
        );
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
    #[test]
    fn generated_tui_spatial_widgets_render_compact_clipped_text() -> sui::Result<()> {
        let mut root = SemanticsNode::new(
            sui::WidgetId::new(1),
            SemanticsRole::Window,
            SuiRect::new(0.0, 0.0, 100.0, 40.0),
        );
        root.name = Some("Root".to_string());
        let mut button = SemanticsNode::new(
            sui::WidgetId::new(2),
            SemanticsRole::Button,
            SuiRect::new(10.0, 10.0, 80.0, 16.0),
        );
        button.parent = Some(root.id);
        button.name = Some("Very long button label".to_string());
        let actionable = vec![button.clone()];
        let nodes = vec![root, button];
        let mut terminal =
            Terminal::new(ratatui::backend::TestBackend::new(24, 8)).map_err(to_sui_io_error)?;

        terminal
            .draw(|frame| {
                draw_tui(
                    frame,
                    &nodes,
                    &actionable,
                    0,
                    false,
                    &TuiSpatialState::default(),
                )
            })
            .map_err(to_sui_io_error)?;
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Very"));
        assert!(!rendered.contains("Very long button label"));
        Ok(())
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
    #[test]
    fn generated_tui_renders_floating_view_from_accessibility_tree_flow() {
        let mut workspace = SemanticsNode::new(
            sui::WidgetId::new(1),
            SemanticsRole::GenericContainer,
            SuiRect::new(0.0, 0.0, 300.0, 200.0),
        );
        workspace.name = Some("Development workspace".to_string());
        let mut first_window = SemanticsNode::new(
            sui::WidgetId::new(2),
            SemanticsRole::Window,
            SuiRect::new(20.0, 20.0, 120.0, 100.0),
        );
        first_window.parent = Some(workspace.id);
        first_window.name = Some("First".to_string());
        let mut second_window = SemanticsNode::new(
            sui::WidgetId::new(3),
            SemanticsRole::Window,
            SuiRect::new(80.0, 60.0, 120.0, 100.0),
        );
        second_window.parent = Some(workspace.id);
        second_window.name = Some("Second".to_string());
        let mut first_button = SemanticsNode::new(
            sui::WidgetId::new(4),
            SemanticsRole::Button,
            SuiRect::new(40.0, 40.0, 40.0, 20.0),
        );
        first_button.parent = Some(first_window.id);
        first_button.name = Some("First button".to_string());
        let mut second_button = SemanticsNode::new(
            sui::WidgetId::new(5),
            SemanticsRole::Button,
            SuiRect::new(100.0, 80.0, 40.0, 20.0),
        );
        second_button.parent = Some(second_window.id);
        second_button.name = Some("Second button".to_string());
        let mut second_content = SemanticsNode::new(
            sui::WidgetId::new(6),
            SemanticsRole::ScrollView,
            SuiRect::new(100.0, 80.0, 100.0, 80.0),
        );
        second_content.parent = Some(second_window.id);
        second_content.name = Some("Floating view content".to_string());
        second_button.parent = Some(second_content.id);
        let second_window_id = second_window.id;
        let nodes = vec![
            workspace,
            first_window,
            second_window,
            first_button.clone(),
            second_content,
            second_button.clone(),
        ];
        let actionable = vec![first_button.clone(), second_button.clone()];
        let tabs = tui_floating_tabs(&nodes, Some(second_button.id)).expect("floating tabs");
        let flow_area = TerminalRect::new(0, 1, 40, 8);
        let flow_items = tui_accessibility_flow_layout(&nodes, &actionable, &tabs, flow_area, 0);

        assert_eq!(tabs.active_window_id, second_window_id);
        assert!(tui_projected_spatial_bounds(&first_button, &Some(tabs.clone())).is_none());
        assert!(tui_projected_spatial_bounds(&second_button, &Some(tabs)).is_none());
        assert_eq!(flow_items.len(), 1);
        assert_eq!(flow_items[0].node.id, second_button.id);
        assert_eq!(flow_items[0].actionable_index, Some(1));
        assert_eq!(flow_items[0].rect.x, 0);
        assert_eq!(flow_items[0].rect.y, 1);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
    #[test]
    fn generated_tui_scrolls_accessibility_flow_to_selected_node() {
        let mut workspace = SemanticsNode::new(
            sui::WidgetId::new(1),
            SemanticsRole::GenericContainer,
            SuiRect::new(0.0, 0.0, 300.0, 200.0),
        );
        workspace.name = Some("Development workspace".to_string());
        let mut first_window = SemanticsNode::new(
            sui::WidgetId::new(2),
            SemanticsRole::Window,
            SuiRect::new(0.0, 0.0, 120.0, 100.0),
        );
        first_window.parent = Some(workspace.id);
        first_window.name = Some("First".to_string());
        let mut second_window = SemanticsNode::new(
            sui::WidgetId::new(3),
            SemanticsRole::Window,
            SuiRect::new(120.0, 0.0, 120.0, 100.0),
        );
        second_window.parent = Some(workspace.id);
        second_window.name = Some("Second".to_string());

        let mut nodes = vec![workspace, first_window, second_window.clone()];
        let mut actionable = Vec::new();
        for index in 0..12 {
            let mut button = SemanticsNode::new(
                sui::WidgetId::new(10 + index),
                SemanticsRole::Button,
                SuiRect::new(0.0, index as f32 * 10.0, 40.0, 8.0),
            );
            button.parent = Some(second_window.id);
            button.name = Some(format!("Button {index}"));
            actionable.push(button.clone());
            nodes.push(button);
        }
        let selected = actionable[10].id;
        let tabs = tui_floating_tabs(&nodes, Some(selected)).expect("floating tabs");
        let flow_area = TerminalRect::new(0, 4, 10, 3);
        let mut state = TuiSpatialState::default();
        sync_tui_spatial_selection(
            &mut state,
            TerminalRect::new(0, 0, 10, 5),
            &nodes,
            &actionable,
            Some(selected),
        );
        let flow_items = tui_accessibility_flow_layout(
            &nodes,
            &actionable,
            &tabs,
            flow_area,
            state.flow_offset(tabs.active_window_id),
        );

        assert!(flow_items.iter().any(|item| item.node.id == selected));
        assert!(flow_items.iter().all(|item| terminal_rect_contains(
            flow_area,
            item.rect.x,
            item.rect.y
        )));
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
    #[test]
    fn generated_tui_mouse_click_switches_spatial_tabs() {
        let mut workspace = SemanticsNode::new(
            sui::WidgetId::new(1),
            SemanticsRole::GenericContainer,
            SuiRect::new(0.0, 0.0, 300.0, 200.0),
        );
        workspace.name = Some("Development workspace".to_string());
        let mut first_window = SemanticsNode::new(
            sui::WidgetId::new(2),
            SemanticsRole::Window,
            SuiRect::new(0.0, 0.0, 120.0, 100.0),
        );
        first_window.parent = Some(workspace.id);
        first_window.name = Some("First".to_string());
        let mut second_window = SemanticsNode::new(
            sui::WidgetId::new(3),
            SemanticsRole::Window,
            SuiRect::new(120.0, 0.0, 120.0, 100.0),
        );
        second_window.parent = Some(workspace.id);
        second_window.name = Some("Second".to_string());
        let mut first_button = SemanticsNode::new(
            sui::WidgetId::new(4),
            SemanticsRole::Button,
            SuiRect::new(0.0, 0.0, 40.0, 8.0),
        );
        first_button.parent = Some(first_window.id);
        first_button.name = Some("First button".to_string());
        let mut second_button = SemanticsNode::new(
            sui::WidgetId::new(5),
            SemanticsRole::Button,
            SuiRect::new(0.0, 0.0, 40.0, 8.0),
        );
        second_button.parent = Some(second_window.id);
        second_button.name = Some("Second button".to_string());
        let nodes = vec![
            workspace,
            first_window,
            second_window,
            first_button.clone(),
            second_button.clone(),
        ];
        let actionable = vec![first_button.clone(), second_button.clone()];
        let spatial_area = TerminalRect::new(0, 0, 48, 12);
        let inner = inner_terminal_rect(spatial_area).expect("inner spatial area");
        let world = tui_world_bounds(&nodes).expect("world bounds");
        let tabs = tui_floating_tabs(&nodes, Some(second_button.id)).expect("floating tabs");
        let first_tab = tui_floating_tab_rects(inner, world, &tabs)
            .into_iter()
            .find(|tab| tab.window_id == first_button.parent.unwrap())
            .expect("first tab rect");
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: first_tab.rect.x,
            row: first_tab.rect.y,
            modifiers: KeyModifiers::empty(),
        };

        assert_eq!(
            mouse_hit_spatial_tab(
                mouse,
                spatial_area,
                &nodes,
                &actionable,
                Some(second_button.id)
            ),
            Some(0)
        );
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tui"))]
    #[test]
    fn generated_tui_mouse_wheel_scrolls_spatial_flow() {
        let mut workspace = SemanticsNode::new(
            sui::WidgetId::new(1),
            SemanticsRole::GenericContainer,
            SuiRect::new(0.0, 0.0, 300.0, 200.0),
        );
        workspace.name = Some("Development workspace".to_string());
        let mut first_window = SemanticsNode::new(
            sui::WidgetId::new(2),
            SemanticsRole::Window,
            SuiRect::new(0.0, 0.0, 120.0, 100.0),
        );
        first_window.parent = Some(workspace.id);
        first_window.name = Some("First".to_string());
        let mut second_window = SemanticsNode::new(
            sui::WidgetId::new(3),
            SemanticsRole::Window,
            SuiRect::new(120.0, 0.0, 120.0, 100.0),
        );
        second_window.parent = Some(workspace.id);
        second_window.name = Some("Second".to_string());

        let mut nodes = vec![workspace, first_window, second_window.clone()];
        let mut actionable = Vec::new();
        for index in 0..12 {
            let mut button = SemanticsNode::new(
                sui::WidgetId::new(10 + index),
                SemanticsRole::Button,
                SuiRect::new(0.0, index as f32 * 10.0, 40.0, 8.0),
            );
            button.parent = Some(second_window.id);
            button.name = Some(format!("Button {index}"));
            actionable.push(button.clone());
            nodes.push(button);
        }

        let spatial_area = TerminalRect::new(0, 0, 12, 8);
        let (tabs, flow_area, _) =
            tui_spatial_flow_items(spatial_area, &nodes, &actionable, Some(actionable[0].id))
                .expect("spatial flow items");
        let mouse = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: flow_area.x,
            row: flow_area.y,
            modifiers: KeyModifiers::empty(),
        };
        let mut state = TuiSpatialState::default();

        let selected =
            scroll_tui_spatial_flow(mouse, spatial_area, &nodes, &actionable, 0, &mut state, 3)
                .expect("spatial flow scroll consumed");

        assert_eq!(state.flow_offset(tabs.active_window_id), 3);
        assert_eq!(selected, 3);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_rejects_unknown_flags() {
        let error = parse_desktop_launch_mode(["--bogus"], false, None).unwrap_err();
        assert!(error.to_string().contains("unsupported sui-demo argument"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_rejects_unknown_automation() {
        let error = parse_desktop_automation(Some("bogus")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unsupported sui-demo automation")
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_no_vsync_env_parser_understands_falsey_values() {
        assert!(!env_requests_no_vsync(None));
        assert!(!env_requests_no_vsync(Some("0")));
        assert!(!env_requests_no_vsync(Some("false")));
        assert!(env_requests_no_vsync(Some("1")));
        assert!(env_requests_no_vsync(Some("true")));
    }

    #[test]
    fn clamps_invalid_frame_counts() {
        let mode = parse_web_launch_mode("benchmark=retained-text&frames=0&warmup=999999");
        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::RetainedText));
        assert_eq!(mode.frames, 1);
        assert_eq!(mode.warmup_frames, 2000);
    }
}
