//! The Node graphs page: a color lab whose values flow along the edges.
//! Colors and numbers go in, mixes, lightening, and a contrast check work on
//! them, and previews show the results, all recomputed as you drag a slider
//! or rewire the graph. Ports are typed, so a connection a port refuses says
//! why. A toolbar, context menus, and the keyboard add, copy, paste,
//! duplicate, delete, undo, and redo; an inspector edits the selection; a
//! log follows what happened.

#[cfg(test)]
mod benchmarks;
mod inspector;
mod lab;
mod nodes;
#[cfg(test)]
mod tests;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use sui::{
    ContextMenuHandle, PointerButton, Rect, SemanticRegion, Signal, WidgetPodMutVisitor,
    WidgetPodVisitor, prelude::*,
};
use sui_nodes::{
    BackgroundVariant, Connection, Edge, EdgeId, EdgeKind, FitViewOptions, GraphModel,
    GraphSnapshot, NodeChange, NodeGraph, NodeGraphConfig, NodeGraphEvent, NodeGraphHit,
    NodeGraphState, NodeId, NodeMiniMap,
};

use self::lab::{KINDS, LabState, Op, connection_rule, evaluate_snapshot, lab_document, lab_node};
use self::nodes::LabContext;
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, dev_theme_color};
#[cfg(test)]
use crate::demo_support::default_theme_reader;
use crate::demo_support::{DemoTextColor, demo_label, demo_mono_label};

pub(crate) const NODES_TAB_LABEL: &str = "Node graphs";
pub(crate) const NODES_DEMO_NAME: &str = "Color lab";
pub(crate) const NODES_MAIN_GRAPH_NAME: &str = "Color lab graph";
pub(crate) const NODES_MINIMAP_NAME: &str = "Color lab overview";
pub(crate) const NODES_LOG_NAME: &str = "Graph events";
pub(crate) const NODES_ADD_NODE_BUTTON: &str = "Add node";
pub(crate) const NODES_INSPECTOR_NAME: &str = "Inspector";
const SUMMARY: &str = "Colors and numbers flow along the edges and recompute as you drag a slider or rewire. Ports are typed, so a connection that doesn't fit says why.";

const UNDO_LABEL: &str = "Undo";
const REDO_LABEL: &str = "Redo";
const COPY_LABEL: &str = "Copy";
const PASTE_LABEL: &str = "Paste";
const DUPLICATE_LABEL: &str = "Duplicate";
const DELETE_LABEL: &str = "Delete";
const FIT_LABEL: &str = "Fit view";
const EDGE_STYLE_NAME: &str = "Edge style";
const CANVAS_MENU_NAME: &str = "Graph menu";
const ADD_MENU_NAME: &str = "Add node menu";
const LOG_LINES: usize = 4;
const SIDE_PANEL_WIDTH: f32 = 320.0;
const HISTORY_LIMIT: usize = 200;

/// Edge styles, as the style picker and menus list them.
const EDGE_STYLES: [(&str, EdgeKind); 4] = [
    ("Bezier", EdgeKind::Bezier),
    ("Smooth step", EdgeKind::SmoothStep),
    ("Step", EdgeKind::Step),
    ("Straight", EdgeKind::Straight),
];

fn style_index(kind: EdgeKind) -> usize {
    EDGE_STYLES
        .iter()
        .position(|(_, style)| *style == kind)
        .unwrap_or(0)
}

/// What the page's parts share.
#[derive(Clone)]
struct Page {
    theme_reader: DevThemeReader,
    state: LabState,
    lab: LabContext,
    log: Signal<Vec<String>>,
    /// The style new edges get, and the picker shows.
    edge_style: Signal<EdgeKind>,
    menu: ContextMenuHandle,
    /// What the graph's context menu is open for, and where in the flow.
    menu_target: Rc<RefCell<(NodeGraphHit, Point)>>,
    next_id: Rc<Cell<u32>>,
    /// Whether the drag in progress has moved anything.
    dragged: Rc<Cell<bool>>,
}

impl Page {
    fn new(theme_reader: DevThemeReader) -> Self {
        let document = lab_document();
        let graph =
            GraphModel::new(document.nodes, document.edges).expect("the lab document is valid");
        let state =
            NodeGraphState::from_snapshot(GraphSnapshot::new(graph).viewport(document.viewport))
                .with_history(HISTORY_LIMIT);
        let evaluation = state
            .observable()
            .select_named("Color lab evaluation", evaluate_snapshot);
        Self {
            lab: LabContext {
                theme_reader: Rc::clone(&theme_reader),
                state: state.clone(),
                evaluation,
            },
            theme_reader,
            state,
            log: Signal::named(NODES_LOG_NAME, Vec::new()),
            edge_style: Signal::named(EDGE_STYLE_NAME, EdgeKind::Bezier),
            menu: ContextMenuHandle::new(),
            menu_target: Rc::new(RefCell::new((NodeGraphHit::Pane, Point::ZERO))),
            next_id: Rc::new(Cell::new(1)),
            dragged: Rc::new(Cell::new(false)),
        }
    }

    fn log(&self, line: impl Into<String>) {
        let line = line.into();
        self.log.update(|log| {
            log.push(line);
            if log.len() > LOG_LINES {
                log.remove(0);
            }
        });
    }

    fn label(&self, id: &NodeId) -> String {
        self.state
            .node(id)
            .map_or_else(|| id.to_string(), |node| node.label)
    }

    /// A node of kind `op` at `position` in the flow, selected on its own.
    fn add_node(&self, op: Op, position: Point) {
        let snapshot = self.state.snapshot();
        let (id, number) = loop {
            let number = self.next_id.get();
            self.next_id.set(number + 1);
            let id = format!("{}-{number}", op.kind().trim_start_matches("lab-"));
            if snapshot.graph.node(&NodeId::from(id.as_str())).is_none() {
                break (id, number);
            }
        };
        let size = op.size();
        let mut node = lab_node(
            id.as_str(),
            &format!("{} {number}", op.title()),
            op,
            Point::new(
                position.x - size.width * 0.5,
                position.y - size.height * 0.5,
            ),
        );
        node.selected = true;
        self.state.undo_group(|| {
            self.state.clear_selection();
            self.state
                .add_node(node)
                .expect("a fresh id makes a valid node");
        });
        self.log(format!("Added {} {number}", op.title()));
    }

    /// The middle of what the graph shows, in the flow.
    fn visible_center(&self) -> Point {
        let size = self.state.viewport_size();
        self.state
            .screen_to_flow_position(Point::new(size.width * 0.5, size.height * 0.5), None)
    }

    fn selection(&self) -> (Vec<NodeId>, Vec<EdgeId>) {
        let snapshot = self.state.snapshot();
        (
            snapshot.graph.selected_node_ids(),
            snapshot.graph.selected_edge_ids(),
        )
    }

    fn delete_selection(&self) {
        let (nodes, edges) = self.selection();
        let deleted = self.state.delete_elements(&nodes, &edges);
        if !deleted.nodes.is_empty() || !deleted.edges.is_empty() {
            self.log(format!(
                "Deleted {}",
                count_of(deleted.nodes.len(), deleted.edges.len())
            ));
        }
    }

    fn duplicate_selection(&self) {
        if let Ok(pasted) = self.state.duplicate()
            && !pasted.is_empty()
        {
            self.log(format!("Duplicated {}", nodes_count(pasted.len())));
        }
    }

    fn copy_selection(&self) {
        let count = self.selection().0.len();
        if self.state.copy() {
            self.log(format!("Copied {}", nodes_count(count)));
        }
    }

    fn paste(&self) {
        if let Ok(pasted) = self.state.paste()
            && !pasted.is_empty()
        {
            self.log(format!("Pasted {}", nodes_count(pasted.len())));
        }
    }

    /// Give the selected edges `kind`, and new edges too.
    fn set_edge_style(&self, kind: EdgeKind) {
        self.edge_style.set(kind);
        let (_, edges) = self.selection();
        if edges.is_empty() {
            return;
        }
        self.state.undo_group(|| {
            for id in &edges {
                let _ = self.state.update_edge(id, |edge| edge.kind = kind);
            }
        });
        self.log(format!(
            "Restyled {} as {}",
            edges_count(edges.len()),
            EDGE_STYLES[style_index(kind)].0.to_lowercase()
        ));
    }

    fn disconnect_selection(&self) {
        let (nodes, _) = self.selection();
        let edges = self
            .state
            .connected_edges(&nodes)
            .into_iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        if !edges.is_empty() {
            self.state.delete_elements(&[], &edges);
            self.log(format!("Disconnected {}", edges_count(edges.len())));
        }
    }

    fn undo(&self) {
        if self.state.undo() {
            self.log("Undo");
        }
    }

    fn redo(&self) {
        if self.state.redo() {
            self.log("Redo");
        }
    }

    fn fit_view(&self) {
        self.state.fit_view(
            self.state.viewport_size(),
            FitViewOptions::default().padding(28.0).zoom_range(0.3, 1.5),
        );
    }

    /// A readable line for a graph event, when it is worth one.
    fn describe(&self, event: &NodeGraphEvent) -> Option<String> {
        let port = |connection: &Connection| {
            format!(
                "{} › {} {}",
                self.label(&connection.source),
                self.label(&connection.target),
                connection
                    .target_handle
                    .as_ref()
                    .map_or("", |handle| handle.as_str())
            )
        };
        match event {
            NodeGraphEvent::Connect(connection) => Some(format!("Connected {}", port(connection))),
            NodeGraphEvent::ConnectionRefused { connection, reason } => {
                Some(format!("Refused {}: {reason}", port(connection)))
            }
            NodeGraphEvent::EdgesChanged(changes) => {
                changes.iter().find_map(|change| match change {
                    sui_nodes::EdgeChange::Reconnected { connection, .. } => {
                        Some(format!("Reconnected to {}", port(connection)))
                    }
                    _ => None,
                })
            }
            NodeGraphEvent::NodesChanged(changes) => {
                let removed = changes
                    .iter()
                    .filter(|change| matches!(change, NodeChange::Removed { .. }))
                    .count();
                if changes
                    .iter()
                    .any(|change| matches!(change, NodeChange::Position { dragging: true, .. }))
                {
                    self.dragged.set(true);
                }
                (removed > 0).then(|| format!("Deleted {}", nodes_count(removed)))
            }
            NodeGraphEvent::NodeDragStopped(ids) if self.dragged.replace(false) => {
                Some(match ids.as_slice() {
                    [id] => format!("Moved {}", self.label(id)),
                    ids => format!("Moved {}", nodes_count(ids.len())),
                })
            }
            NodeGraphEvent::ContextMenu { target, .. } => Some(match target {
                NodeGraphHit::Node(id) | NodeGraphHit::Handle { node: id, .. } => {
                    format!("Menu for {}", self.label(id))
                }
                NodeGraphHit::Edge(_) => "Menu for an edge".to_string(),
                NodeGraphHit::Pane => "Menu for the canvas".to_string(),
            }),
            NodeGraphEvent::Copied(ids) => Some(format!("Copied {}", nodes_count(ids.len()))),
            NodeGraphEvent::Pasted(ids) => Some(format!("Pasted {}", nodes_count(ids.len()))),
            NodeGraphEvent::Undone => Some("Undo".to_string()),
            NodeGraphEvent::Redone => Some("Redo".to_string()),
            _ => None,
        }
    }
}

fn nodes_count(count: usize) -> String {
    if count == 1 {
        "1 node".to_string()
    } else {
        format!("{count} nodes")
    }
}

fn edges_count(count: usize) -> String {
    if count == 1 {
        "1 edge".to_string()
    } else {
        format!("{count} edges")
    }
}

fn count_of(nodes: usize, edges: usize) -> String {
    match (nodes, edges) {
        (0, edges) => edges_count(edges),
        (nodes, 0) => nodes_count(nodes),
        (nodes, edges) => format!("{} and {}", nodes_count(nodes), edges_count(edges)),
    }
}

pub(crate) fn build_nodes_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let page = Page::new(theme_reader);
    let theme_reader = &page.theme_reader;
    SemanticRegion::new(
        NODES_DEMO_NAME,
        Background::new(
            theme_reader().palette.surface,
            Flex::vertical()
                .align_items(Alignment::Stretch)
                .with_child(header(&page))
                .with_child(toolbar(&page))
                .with_item(
                    Workspace::new(graph(&page), side_panel(&page)),
                    FlexItem::new().grow(1.0).basis(0.0),
                )
                .with_child(event_log(&page)),
        )
        .brush_when(dev_theme_color(theme_reader, |theme| theme.palette.surface)),
    )
    .description(SUMMARY)
}

/// The page on its own, for tests.
#[cfg(test)]
pub(crate) fn build_nodes_application() -> Application {
    App::new()
        .window(
            Window::new(NODES_TAB_LABEL)
                .initial_size(Size::new(1280.0, 820.0))
                .root(build_nodes_demo_with_theme(default_theme_reader())),
        )
        .into_application()
}

fn header(page: &Page) -> impl Widget + use<> {
    Padding::new(
        Insets {
            left: 20.0,
            right: 20.0,
            top: 16.0,
            bottom: 8.0,
        },
        Stack::vertical()
            .spacing(4.0)
            .alignment(Alignment::Stretch)
            .with_child(demo_label(
                &page.theme_reader,
                NODES_TAB_LABEL,
                DemoTextRole::PageTitle,
                DemoTextColor::Text,
            ))
            .with_child(demo_label(
                &page.theme_reader,
                SUMMARY,
                DemoTextRole::Supporting,
                DemoTextColor::Muted,
            )),
    )
}

/// A toolbar button that acts on the graph, leaving focus in it.
fn tool(page: &Page, label: &str, icon: IconGlyph) -> Button {
    Button::new(label)
        .icon(icon)
        .theme_when(clone_dev_theme_reader(&page.theme_reader))
        .focus_on_press(false)
}

fn toolbar(page: &Page) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let history = page.state.history_observable();
    let has_selection = page
        .state
        .observable()
        .select_named("Lab has a selection", |snapshot| {
            !snapshot.graph.selected_node_ids().is_empty()
                || !snapshot.graph.selected_edge_ids().is_empty()
        });
    let has_nodes_selected = page
        .state
        .observable()
        .select_named("Lab has nodes selected", |snapshot| {
            !snapshot.graph.selected_node_ids().is_empty()
        });

    let add = {
        let page = page.clone();
        ContextMenu::new(
            ADD_MENU_NAME,
            Button::primary(NODES_ADD_NODE_BUTTON)
                .icon(IconGlyph::Add)
                .theme_when(clone_dev_theme_reader(theme_reader)),
        )
        .theme_when(clone_dev_theme_reader(theme_reader))
        .activation_button(PointerButton::Primary)
        .items(KINDS.map(|op| MenuItem::new(op.title())))
        .on_activate(move |index, _| {
            page.add_node(KINDS[index], page.visible_center());
        })
    };
    let undo = {
        let page = page.clone();
        tool(&page, UNDO_LABEL, IconGlyph::Undo)
            .enabled_from(history.select_named("Lab can undo", |status| status.can_undo()))
            .on_press(move || page.undo())
    };
    let redo = {
        let page = page.clone();
        tool(&page, REDO_LABEL, IconGlyph::Redo)
            .enabled_from(history.select_named("Lab can redo", |status| status.can_redo()))
            .on_press(move || page.redo())
    };
    let copy = {
        let page = page.clone();
        tool(&page, COPY_LABEL, IconGlyph::File)
            .enabled_from(has_nodes_selected.clone())
            .on_press(move || page.copy_selection())
    };
    let paste = {
        let page = page.clone();
        tool(&page, PASTE_LABEL, IconGlyph::FileText).on_press(move || page.paste())
    };
    let duplicate = {
        let page = page.clone();
        tool(&page, DUPLICATE_LABEL, IconGlyph::Blocks)
            .enabled_from(has_nodes_selected)
            .on_press(move || page.duplicate_selection())
    };
    let delete = {
        let page = page.clone();
        tool(&page, DELETE_LABEL, IconGlyph::Trash)
            .enabled_from(has_selection)
            .on_press(move || page.delete_selection())
    };
    let style = {
        let read = page.edge_style.clone();
        let page = page.clone();
        Select::new(EDGE_STYLE_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .options(EDGE_STYLES.map(|(label, _)| label))
            .selected_when(move || Some(style_index(read.get())))
            .on_change(move |index, _| page.set_edge_style(EDGE_STYLES[index].1))
    };
    let fit = {
        let page = page.clone();
        tool(&page, FIT_LABEL, IconGlyph::FitView).on_press(move || page.fit_view())
    };
    let divider = || {
        SizedBox::new()
            .size(Size::new(1.0, 24.0))
            .with_child(Separator::vertical().theme_when(clone_dev_theme_reader(theme_reader)))
    };
    Padding::symmetric(
        20.0,
        8.0,
        Flex::horizontal()
            .gap(8.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Center)
            .with_child(add)
            .with_child(divider())
            .with_child(undo)
            .with_child(redo)
            .with_child(divider())
            .with_child(copy)
            .with_child(paste)
            .with_child(duplicate)
            .with_child(delete)
            .with_child(divider())
            .with_child(demo_label(
                theme_reader,
                EDGE_STYLE_NAME,
                DemoTextRole::Metadata,
                DemoTextColor::Muted,
            ))
            .with_child(SizedBox::new().width(150.0).with_child(style))
            .with_child(divider())
            .with_child(fit),
    )
}

/// The graph, in a context menu the graph opens when asked.
fn graph(page: &Page) -> impl Widget + use<> {
    let lab = &page.lab;
    let edges = {
        let page = page.clone();
        move |connection: Connection| {
            let snapshot = page.state.snapshot();
            let id = loop {
                let number = page.next_id.get();
                page.next_id.set(number + 1);
                let id = EdgeId::from(format!("edge-{number}"));
                if snapshot.graph.edge(&id).is_none() {
                    break id;
                }
            };
            let mut edge =
                Edge::new(id, connection.source, connection.target, ()).kind(page.edge_style.get());
            edge.source_handle = connection.source_handle;
            edge.target_handle = connection.target_handle;
            edge
        }
    };
    let log = page.clone();
    let open_menu = page.clone();
    let graph = NodeGraph::new(NODES_MAIN_GRAPH_NAME, page.state.clone())
        .theme_when(clone_dev_theme_reader(&page.theme_reader))
        .config(NodeGraphConfig {
            fit_view_on_init: true,
            fit_view: FitViewOptions::default().padding(28.0).zoom_range(0.3, 1.5),
            min_zoom: 0.3,
            max_zoom: 2.5,
            background_variant: BackgroundVariant::Dots,
            snap_to_grid: Some(Size::new(8.0, 8.0)),
            nodes_resizable: false,
            ..NodeGraphConfig::default()
        })
        .connection_rule(connection_rule)
        .edge_factory(edges)
        .node_type("lab-color", {
            let lab = lab.clone();
            move |id, node| nodes::color_node(id, node, &lab)
        })
        .node_type("lab-number", {
            let lab = lab.clone();
            move |id, node| nodes::number_node(id, node, &lab)
        })
        .node_type("lab-mix", {
            let lab = lab.clone();
            move |id, node| nodes::blend_node(id, node, &lab)
        })
        .node_type("lab-lighten", {
            let lab = lab.clone();
            move |id, node| nodes::blend_node(id, node, &lab)
        })
        .node_type("lab-contrast", {
            let lab = lab.clone();
            move |id, node| nodes::contrast_node(id, node, &lab)
        })
        .node_type("lab-preview", {
            let lab = lab.clone();
            move |id, node| nodes::preview_node(id, node, &lab)
        })
        .node_type("lab-gradient", {
            let lab = lab.clone();
            move |id, node| nodes::gradient_node(id, node, &lab)
        })
        .node_type("lab-group", {
            let lab = lab.clone();
            move |_, node| nodes::group_node(node, &lab)
        })
        .on_change(move |event| {
            if let Some(line) = log.describe(&event) {
                log.log(line);
            }
        })
        .on_context_menu(move |ctx, target, position| {
            let origin = ctx.bounds().origin;
            let flow = open_menu.state.screen_to_flow_position(
                Point::new(position.x - origin.x, position.y - origin.y),
                None,
            );
            *open_menu.menu_target.borrow_mut() = (target, flow);
            open_menu.menu.open_at(ctx, position);
        });

    let items = page.clone();
    let act = page.clone();
    ContextMenu::new(CANVAS_MENU_NAME, graph)
        .theme_when(clone_dev_theme_reader(&page.theme_reader))
        .handle(page.menu.clone())
        .items_when(move || menu_items(&items))
        .on_activate_path(move |path, item| act_on_menu(&act, &path, item.label()))
}

/// What the graph's menu lists for what it opened on.
fn menu_items(page: &Page) -> Vec<MenuItem> {
    let (target, _) = page.menu_target.borrow().clone();
    match target {
        NodeGraphHit::Node(_) | NodeGraphHit::Handle { .. } => vec![
            MenuItem::new(DUPLICATE_LABEL).shortcut("Ctrl+D"),
            MenuItem::new(COPY_LABEL).shortcut("Ctrl+C"),
            MenuItem::new("Disconnect"),
            MenuItem::new(DELETE_LABEL)
                .shortcut("Delete")
                .destructive()
                .separator_before(),
        ],
        NodeGraphHit::Edge(_) => vec![
            MenuItem::new("Style").submenu(EDGE_STYLES.map(|(label, _)| MenuItem::new(label))),
            MenuItem::new(DELETE_LABEL)
                .shortcut("Delete")
                .destructive()
                .separator_before(),
        ],
        NodeGraphHit::Pane => {
            let paste = MenuItem::new(PASTE_LABEL).shortcut("Ctrl+V");
            vec![
                MenuItem::new("Add").submenu(KINDS.map(|op| MenuItem::new(op.title()))),
                if page.state.can_paste() {
                    paste
                } else {
                    paste.disabled()
                },
                MenuItem::new(FIT_LABEL).shortcut("Home").separator_before(),
            ]
        }
    }
}

fn act_on_menu(page: &Page, path: &[usize], label: &str) {
    let (target, flow) = page.menu_target.borrow().clone();
    match (target, path) {
        (NodeGraphHit::Pane, [0, kind]) => page.add_node(KINDS[*kind], flow),
        (NodeGraphHit::Edge(_), [0, style]) => page.set_edge_style(EDGE_STYLES[*style].1),
        _ => match label {
            DUPLICATE_LABEL => page.duplicate_selection(),
            COPY_LABEL => page.copy_selection(),
            "Disconnect" => page.disconnect_selection(),
            DELETE_LABEL => page.delete_selection(),
            PASTE_LABEL => page.paste(),
            FIT_LABEL => page.fit_view(),
            _ => {}
        },
    }
}

/// The inspector, the overview, and the shortcuts, in a column that scrolls.
fn side_panel(page: &Page) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let minimap = NodeMiniMap::new(NODES_MINIMAP_NAME, page.state.clone())
        .theme_when(clone_dev_theme_reader(theme_reader))
        .desired_size(Size::new(280.0, 150.0))
        .pannable(true)
        .zoomable(true)
        .zoom_range(0.3, 2.5);
    ScrollView::vertical(Padding::all(
        14.0,
        Stack::vertical()
            .spacing(14.0)
            .alignment(Alignment::Stretch)
            .with_child(card(
                theme_reader,
                NODES_INSPECTOR_NAME,
                inspector::inspector(page),
            ))
            .with_child(card(theme_reader, "Overview", minimap))
            .with_child(card(
                theme_reader,
                "Shortcuts",
                demo_mono_label(theme_reader, SHORTCUTS, DemoTextRole::Metadata, |theme| {
                    theme.palette.text_muted
                }),
            )),
    ))
    .name("Color lab side panel")
    .theme_when(clone_dev_theme_reader(theme_reader))
}

const SHORTCUTS: &str = "Drag an output to an input\nRight-click   menu\nCtrl+Z        undo\nCtrl+Shift+Z  redo\nCtrl+C / V    copy, paste\nCtrl+D        duplicate\nDelete        delete\nShift+drag    select an area\nHome          fit the view";

fn card<W>(theme_reader: &DevThemeReader, title: &str, body: W) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    Surface::panel(
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(demo_label(
                theme_reader,
                title,
                DemoTextRole::CardTitle,
                DemoTextColor::Text,
            ))
            .with_child(body),
    )
    .theme_when(clone_dev_theme_reader(theme_reader))
    .padding(Insets::all(14.0))
    .fill_width()
}

fn event_log(page: &Page) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    Padding::new(
        Insets {
            left: 20.0,
            right: 20.0,
            top: 8.0,
            bottom: 14.0,
        },
        Surface::field(
            demo_mono_label(theme_reader, "", DemoTextRole::Metadata, |theme| {
                theme.palette.text
            })
            .semantic_name(NODES_LOG_NAME)
            .text_from(page.log.select(|lines| {
                let text = if lines.is_empty() {
                    "Connect, drag, or right-click; what happens shows up here.".to_string()
                } else {
                    lines.join("\n")
                };
                // As many lines each time, so the log keeps its height.
                let count = text.lines().count();
                format!("{text}{}", "\n".repeat(LOG_LINES.saturating_sub(count)))
            })),
        )
        .theme_when(clone_dev_theme_reader(theme_reader))
        .padding(Insets::all(10.0))
        .fill_width(),
    )
}

/// The graph beside the side panel, or above it where the page is narrow.
struct Workspace {
    graph: SingleChild,
    panel: SingleChild,
}

impl Workspace {
    const SIDE_BY_SIDE: f32 = 920.0;

    fn new<G, P>(graph: G, panel: P) -> Self
    where
        G: Widget + 'static,
        P: Widget + 'static,
    {
        Self {
            graph: SingleChild::new(graph),
            panel: SingleChild::new(panel),
        }
    }

    fn split(bounds: Rect) -> (Rect, Rect) {
        if bounds.width() >= Self::SIDE_BY_SIDE {
            let graph = Rect::new(
                bounds.x() + 20.0,
                bounds.y(),
                bounds.width() - SIDE_PANEL_WIDTH - 20.0,
                bounds.height(),
            );
            let panel = Rect::new(graph.max_x(), bounds.y(), SIDE_PANEL_WIDTH, bounds.height());
            (graph, panel)
        } else {
            let panel_height = (bounds.height() * 0.4).clamp(160.0, 320.0);
            let graph = Rect::new(
                bounds.x() + 20.0,
                bounds.y(),
                (bounds.width() - 40.0).max(0.0),
                (bounds.height() - panel_height).max(0.0),
            );
            let panel = Rect::new(bounds.x(), graph.max_y(), bounds.width(), panel_height);
            (graph, panel)
        }
    }
}

impl Widget for Workspace {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                1100.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                640.0
            },
        );
        let size = constraints.clamp(size);
        let (graph, panel) = Self::split(Rect::from_origin_size(Point::ZERO, size));
        self.graph.measure(ctx, Constraints::tight(graph.size));
        self.panel.measure(ctx, Constraints::tight(panel.size));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let (graph, panel) = Self::split(bounds);
        self.graph.arrange(ctx, graph);
        self.panel.arrange(ctx, panel);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.graph.paint(ctx);
        self.panel.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.graph.semantics(ctx);
        self.panel.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.graph.visit_children(visitor);
        self.panel.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.graph.visit_children_mut(visitor);
        self.panel.visit_children_mut(visitor);
    }
}
