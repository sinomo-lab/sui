use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;

use crate::app::{
    DevThemeReader, clone_dev_theme_reader, dev_text_style, dev_theme_color, request_window_refresh,
};

pub(crate) const DRAG_DROP_TAB_LABEL: &str = "Drag and drop";
pub(crate) const DRAG_DROP_DEMO_SCROLL_NAME: &str = "Drag and drop demo scroll";

const ASSET_KIND: &str = "sui-demo.asset";
const SCOPE_TOKEN_KIND: &str = "sui-demo.scope-token";
const REORDER_ITEM_COUNT: usize = 5;

fn body_label(label: Label, theme_reader: &DevThemeReader, muted: bool) -> Label {
    let theme = theme_reader();
    let color = if muted {
        theme.palette.text_muted
    } else {
        theme.palette.text
    };
    label
        .style(dev_text_style(theme, theme.text.sm, color))
        .color_when(dev_theme_color(theme_reader, move |theme| {
            if muted {
                theme.palette.text_muted
            } else {
                theme.palette.text
            }
        }))
}

fn meta_label(label: Label, theme_reader: &DevThemeReader) -> Label {
    let theme = theme_reader();
    label
        .style(dev_text_style(
            theme,
            theme.text.xs,
            theme.palette.text_muted,
        ))
        .color_when(dev_theme_color(theme_reader, |theme| {
            theme.palette.text_muted
        }))
}

fn section_title_label(label: Label, theme_reader: &DevThemeReader) -> Label {
    let theme = theme_reader();
    label
        .style(dev_text_style(theme, theme.text.lg, theme.palette.text))
        .color_when(dev_theme_color(theme_reader, |theme| theme.palette.text))
}

const INITIAL_REORDER_ITEMS: [ReorderItem; REORDER_ITEM_COUNT] = [
    ReorderItem {
        id: 1,
        label: "Capture brief",
        detail: "Planning",
    },
    ReorderItem {
        id: 2,
        label: "Sketch flows",
        detail: "Design",
    },
    ReorderItem {
        id: 3,
        label: "Wire callbacks",
        detail: "Implementation",
    },
    ReorderItem {
        id: 4,
        label: "Test pointer paths",
        detail: "QA",
    },
    ReorderItem {
        id: 5,
        label: "Ship example",
        detail: "Release",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DemoAsset {
    name: &'static str,
    group: &'static str,
    size: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScopeToken {
    label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReorderItem {
    id: u32,
    label: &'static str,
    detail: &'static str,
}

#[derive(Clone)]
struct DragDropDemoState {
    inner: Rc<RefCell<DragDropDemoStateInner>>,
}

struct DragDropDemoStateInner {
    text_items: Vec<String>,
    asset_items: Vec<DemoAsset>,
    status: String,
    scope_status: String,
    reorder_items: Vec<ReorderItem>,
    text_hovered: bool,
    asset_hovered: bool,
    text_only_hovered: bool,
    scoped_hovered: bool,
}

impl DragDropDemoState {
    fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(DragDropDemoStateInner {
                text_items: Vec::new(),
                asset_items: Vec::new(),
                status: "Ready".to_string(),
                scope_status: "Shared scope target is empty.".to_string(),
                reorder_items: INITIAL_REORDER_ITEMS.to_vec(),
                text_hovered: false,
                asset_hovered: false,
                text_only_hovered: false,
                scoped_hovered: false,
            })),
        }
    }

    fn push_text(&self, text: impl Into<String>) {
        let mut inner = self.inner.borrow_mut();
        inner.text_items.push(text.into());
        if inner.text_items.len() > 5 {
            inner.text_items.remove(0);
        }
    }

    fn push_asset(&self, asset: DemoAsset) {
        let mut inner = self.inner.borrow_mut();
        inner.asset_items.push(asset);
        if inner.asset_items.len() > 4 {
            inner.asset_items.remove(0);
        }
    }

    fn set_status(&self, status: impl Into<String>) {
        self.inner.borrow_mut().status = status.into();
    }

    fn set_scope_status(&self, status: impl Into<String>) {
        self.inner.borrow_mut().scope_status = status.into();
    }

    fn set_text_hovered(&self, hovered: bool) {
        self.inner.borrow_mut().text_hovered = hovered;
    }

    fn set_asset_hovered(&self, hovered: bool) {
        self.inner.borrow_mut().asset_hovered = hovered;
    }

    fn set_text_only_hovered(&self, hovered: bool) {
        self.inner.borrow_mut().text_only_hovered = hovered;
    }

    fn set_scoped_hovered(&self, hovered: bool) {
        self.inner.borrow_mut().scoped_hovered = hovered;
    }

    fn text_hovered(&self) -> bool {
        self.inner.borrow().text_hovered
    }

    fn asset_hovered(&self) -> bool {
        self.inner.borrow().asset_hovered
    }

    fn text_only_hovered(&self) -> bool {
        self.inner.borrow().text_only_hovered
    }

    fn scoped_hovered(&self) -> bool {
        self.inner.borrow().scoped_hovered
    }

    fn reorder_summary(&self) -> String {
        self.inner
            .borrow()
            .reorder_items
            .iter()
            .enumerate()
            .map(|(position, item)| format!("{}. {}", position + 1, item.label))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn apply_reorder_change(&self, change: ReorderableListChange) -> Option<String> {
        let mut inner = self.inner.borrow_mut();
        if change.from >= inner.reorder_items.len() || change.to >= inner.reorder_items.len() {
            return None;
        }
        if change.from == change.to {
            return None;
        }

        let item = inner.reorder_items.remove(change.from);
        inner.reorder_items.insert(change.to, item);

        Some(format!(
            "Moved {} to position {}",
            item.label,
            change.to + 1
        ))
    }

    fn text_summary(&self) -> String {
        let inner = self.inner.borrow();
        if inner.text_items.is_empty() {
            return "No copied text yet.".to_string();
        }
        inner
            .text_items
            .iter()
            .rev()
            .map(|text| format!("Text: {text}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn asset_summary(&self) -> String {
        let inner = self.inner.borrow();
        if inner.asset_items.is_empty() {
            return "No assets accepted yet.".to_string();
        }
        inner
            .asset_items
            .iter()
            .rev()
            .map(|asset| format!("{}  {}  {}", asset.name, asset.group, asset.size))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn status(&self) -> String {
        self.inner.borrow().status.clone()
    }

    fn scope_status(&self) -> String {
        self.inner.borrow().scope_status.clone()
    }
}

pub(crate) fn build_drag_drop_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let scope = DragDropScope::new();
    let state = DragDropDemoState::new();
    let content = Stack::vertical()
        .spacing(18.0)
        .alignment(Alignment::Stretch)
        .with_child(section(
            "Text payloads",
            "Copy text fragments into a shared text target.",
            build_text_payload_example(Rc::clone(&theme_reader), scope.clone(), state.clone()),
            Rc::clone(&theme_reader),
        ))
        .with_child(section(
            "Typed custom payloads",
            "Move typed asset records into an asset target.",
            build_custom_payload_example(Rc::clone(&theme_reader), scope.clone(), state.clone()),
            Rc::clone(&theme_reader),
        ))
        .with_child(section(
            "List reorder",
            "Animated reorderable container backed by internal drag-and-drop.",
            build_list_reorder_example(Rc::clone(&theme_reader), state.clone()),
            Rc::clone(&theme_reader),
        ))
        .with_child(section(
            "Negotiation and scopes",
            "Targets filter payloads and isolated scopes cancel cleanly.",
            build_negotiation_example(Rc::clone(&theme_reader), scope.clone(), state.clone()),
            Rc::clone(&theme_reader),
        ))
        .with_child(build_status_panel(Rc::clone(&theme_reader), state));

    DragDropHost::new(
        scope,
        Background::new(
            theme_reader().palette.surface,
            ScrollView::vertical(Padding::all(18.0, content)).name(DRAG_DROP_DEMO_SCROLL_NAME),
        )
        .brush_when(dev_theme_color(&theme_reader, |theme| {
            theme.palette.surface
        })),
    )
}

fn build_text_payload_example(
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    state: DragDropDemoState,
) -> impl Widget {
    Flex::horizontal()
        .gap(14.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Stretch)
        .with_item(
            panel(
                "Text drag sources",
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Stretch)
                    .with_child(text_chip(
                        "Invoice #1042",
                        Rc::clone(&theme_reader),
                        scope.clone(),
                        state.clone(),
                    ))
                    .with_child(text_chip(
                        "Design note",
                        Rc::clone(&theme_reader),
                        scope.clone(),
                        state.clone(),
                    ))
                    .with_child(text_chip(
                        "Release tag",
                        Rc::clone(&theme_reader),
                        scope.clone(),
                        state.clone(),
                    )),
                Rc::clone(&theme_reader),
            ),
            FlexItem::new().basis_fraction(0.5).min_width(280.0),
        )
        .with_item(
            text_drop_target(theme_reader, scope, state),
            FlexItem::new().basis_fraction(0.5).min_width(280.0),
        )
}

fn build_custom_payload_example(
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    state: DragDropDemoState,
) -> impl Widget {
    let assets = [
        DemoAsset {
            name: "Hero.png",
            group: "Image",
            size: "240 KB",
        },
        DemoAsset {
            name: "Palette.json",
            group: "Data",
            size: "12 KB",
        },
        DemoAsset {
            name: "Scene.sui",
            group: "Document",
            size: "88 KB",
        },
    ];

    let mut sources = Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch);
    for asset in assets {
        sources = sources.with_child(asset_card(
            asset,
            Rc::clone(&theme_reader),
            scope.clone(),
            state.clone(),
        ));
    }

    Flex::horizontal()
        .gap(14.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Stretch)
        .with_item(
            panel("Asset drag sources", sources, Rc::clone(&theme_reader)),
            FlexItem::new().basis_fraction(0.5).min_width(300.0),
        )
        .with_item(
            asset_drop_target(theme_reader, scope, state),
            FlexItem::new().basis_fraction(0.5).min_width(300.0),
        )
}

fn build_list_reorder_example(
    theme_reader: DevThemeReader,
    state: DragDropDemoState,
) -> impl Widget {
    let reorder_state = state.clone();
    let summary_state = state;
    let mut list = ReorderableList::new("Animated reorder list")
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .spacing(8.0)
        .preview_label("Move step")
        .on_reorder_with_ctx(move |ctx, change| {
            if let Some(status) = reorder_state.apply_reorder_change(change) {
                reorder_state.set_status(status);
                request_window_refresh(ctx, false);
            }
        });
    for item in INITIAL_REORDER_ITEMS {
        list = list.item(reorder_demo_row(item, Rc::clone(&theme_reader)));
    }

    Flex::horizontal()
        .gap(14.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Stretch)
        .with_item(
            panel("Sortable list", list, Rc::clone(&theme_reader)),
            FlexItem::new().basis_fraction(0.6).min_width(320.0),
        )
        .with_item(
            panel(
                "Current order",
                reorder_summary_label(Rc::clone(&theme_reader), summary_state),
                theme_reader,
            ),
            FlexItem::new().basis_fraction(0.4).min_width(240.0),
        )
}

fn reorder_summary_label(theme_reader: DevThemeReader, state: DragDropDemoState) -> impl Widget {
    let label_state = state.clone();
    body_label(
        Label::dynamic(label_state.reorder_summary(), move || {
            label_state.reorder_summary()
        }),
        &theme_reader,
        true,
    )
}

fn reorder_demo_row(item: ReorderItem, theme_reader: DevThemeReader) -> impl Widget {
    Surface::field(
        Stack::vertical()
            .spacing(3.0)
            .alignment(Alignment::Stretch)
            .with_child(body_label(Label::new(item.label), &theme_reader, false))
            .with_child(meta_label(
                Label::new(format!("{}  item {}", item.detail, item.id)),
                &theme_reader,
            )),
    )
    .name(format!("Reorder item {}", item.label))
    .theme_when(clone_dev_theme_reader(&theme_reader))
    .padding(Insets::all(10.0))
    .fill_width()
}

fn build_negotiation_example(
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    state: DragDropDemoState,
) -> impl Widget {
    let isolated_scope = DragDropScope::new();

    Flex::horizontal()
        .gap(14.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Stretch)
        .with_item(
            panel(
                "Mixed sources",
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Stretch)
                    .with_child(text_chip(
                        "Accepted text",
                        Rc::clone(&theme_reader),
                        scope.clone(),
                        state.clone(),
                    ))
                    .with_child(scope_token_card(
                        "Shared token",
                        Rc::clone(&theme_reader),
                        scope.clone(),
                        state.clone(),
                        false,
                    ))
                    .with_child(DragDropHost::new(
                        isolated_scope.clone(),
                        scope_token_card(
                            "Isolated token",
                            Rc::clone(&theme_reader),
                            isolated_scope,
                            state.clone(),
                            true,
                        ),
                    )),
                Rc::clone(&theme_reader),
            ),
            FlexItem::new().basis_fraction(0.45).min_width(280.0),
        )
        .with_item(
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(text_only_drop_target(
                    Rc::clone(&theme_reader),
                    scope.clone(),
                    state.clone(),
                ))
                .with_child(scoped_drop_target(theme_reader, scope, state)),
            FlexItem::new().basis_fraction(0.55).min_width(320.0),
        )
}

fn build_status_panel(theme_reader: DevThemeReader, state: DragDropDemoState) -> impl Widget {
    let status_state = state.clone();
    panel(
        "Drag status",
        body_label(
            Label::dynamic("Ready", move || status_state.status()),
            &theme_reader,
            true,
        ),
        theme_reader,
    )
}

fn text_chip(
    text: &'static str,
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    state: DragDropDemoState,
) -> impl Widget {
    let start_state = state.clone();
    let end_state = state;
    Draggable::new(
        Surface::field(Align::center(Padding::all(
            8.0,
            body_label(Label::new(text), &theme_reader, false),
        )))
        .name(format!("Text source {text}"))
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .padding(Insets::all(8.0))
        .fill_width(),
    )
    .scope(scope)
    .payload(move || DragPayload::text(text))
    .effect(DropEffect::Copy)
    .preview_label(text)
    .on_drag_start(move |ctx, _| {
        start_state.set_status(format!("Dragging text: {text}"));
        request_window_refresh(ctx, false);
    })
    .on_drag_end(move |ctx, drag| {
        if matches!(drag.outcome, Some(DragOutcome::Cancelled)) {
            end_state.set_status(format!("Text drag cancelled: {text}"));
            request_window_refresh(ctx, false);
        }
    })
}

fn asset_card(
    asset: DemoAsset,
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    state: DragDropDemoState,
) -> impl Widget {
    let start_state = state.clone();
    let end_state = state;
    Draggable::new(
        Surface::field(
            Stack::vertical()
                .spacing(3.0)
                .alignment(Alignment::Stretch)
                .with_child(body_label(Label::new(asset.name), &theme_reader, false))
                .with_child(meta_label(
                    Label::new(format!("{}  {}", asset.group, asset.size)),
                    &theme_reader,
                )),
        )
        .name(format!("Asset source {}", asset.name))
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .padding(Insets::all(10.0))
        .fill_width(),
    )
    .scope(scope)
    .payload(move || DragPayload::custom(ASSET_KIND, asset))
    .effect(DropEffect::Move)
    .preview_label(asset.name)
    .on_drag_start(move |ctx, _| {
        start_state.set_status(format!("Dragging asset: {}", asset.name));
        request_window_refresh(ctx, false);
    })
    .on_drag_end(move |ctx, drag| {
        if matches!(drag.outcome, Some(DragOutcome::Cancelled)) {
            end_state.set_status(format!("Asset drag cancelled: {}", asset.name));
            request_window_refresh(ctx, false);
        }
    })
}

fn scope_token_card(
    label: &'static str,
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    state: DragDropDemoState,
    isolated: bool,
) -> impl Widget {
    let start_state = state.clone();
    let end_state = state;
    let caption = if isolated {
        "Own scope"
    } else {
        "Shared scope"
    };
    Draggable::new(
        Surface::field(
            Stack::vertical()
                .spacing(3.0)
                .alignment(Alignment::Stretch)
                .with_child(body_label(Label::new(label), &theme_reader, false))
                .with_child(meta_label(Label::new(caption), &theme_reader)),
        )
        .name(format!("Scope token source {label}"))
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .padding(Insets::all(10.0))
        .fill_width(),
    )
    .scope(scope)
    .payload(move || DragPayload::custom(SCOPE_TOKEN_KIND, ScopeToken { label }))
    .effect(DropEffect::Link)
    .preview_label(label)
    .on_drag_start(move |ctx, _| {
        start_state.set_status(format!("Dragging scope token: {label}"));
        request_window_refresh(ctx, false);
    })
    .on_drag_end(move |ctx, drag| {
        if matches!(drag.outcome, Some(DragOutcome::Cancelled)) {
            end_state.set_scope_status(format!("{label} cancelled outside a matching scope."));
            end_state.set_status(format!("Scope token cancelled: {label}"));
            request_window_refresh(ctx, false);
        }
    })
}

fn text_drop_target(
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    state: DragDropDemoState,
) -> impl Widget {
    let hover_state = state.clone();
    let drop_state = state.clone();
    DropTarget::new(panel(
        "Text drop target",
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(target_heading(
                "Copied text",
                Rc::clone(&theme_reader),
                state.clone(),
                DragDropDemoState::text_hovered,
            ))
            .with_child(body_label(
                Label::dynamic("No copied text yet.", move || state.text_summary()),
                &theme_reader,
                true,
            )),
        theme_reader,
    ))
    .scope(scope)
    .accept(|drag| {
        if drag.payload.as_text().is_some() {
            DropEffect::Copy
        } else {
            DropEffect::None
        }
    })
    .on_hover_change(move |hovered| hover_state.set_text_hovered(hovered))
    .on_drop(move |ctx, drag| {
        if let Some(text) = drag.payload.as_text() {
            drop_state.push_text(text.to_string());
            drop_state.set_status(format!("Copied text: {text}"));
            request_window_refresh(ctx, false);
        }
    })
}

fn asset_drop_target(
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    state: DragDropDemoState,
) -> impl Widget {
    let hover_state = state.clone();
    let drop_state = state.clone();
    DropTarget::new(panel(
        "Asset drop target",
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(target_heading(
                "Accepted assets",
                Rc::clone(&theme_reader),
                state.clone(),
                DragDropDemoState::asset_hovered,
            ))
            .with_child(body_label(
                Label::dynamic("No assets accepted yet.", move || state.asset_summary()),
                &theme_reader,
                true,
            )),
        theme_reader,
    ))
    .scope(scope)
    .accept(|drag| {
        if drag.payload.custom_kind() == Some(ASSET_KIND) {
            DropEffect::Move
        } else {
            DropEffect::None
        }
    })
    .on_hover_change(move |hovered| hover_state.set_asset_hovered(hovered))
    .on_drop(move |ctx, drag| {
        if let Some(asset) = drag.payload.custom_data::<DemoAsset>() {
            drop_state.push_asset(*asset);
            drop_state.set_status(format!("Moved asset: {}", asset.name));
            request_window_refresh(ctx, false);
        }
    })
}

fn text_only_drop_target(
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    state: DragDropDemoState,
) -> impl Widget {
    let hover_state = state.clone();
    let drop_state = state.clone();
    DropTarget::new(panel(
        "Text only target",
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(target_heading(
                "Text only",
                Rc::clone(&theme_reader),
                state.clone(),
                DragDropDemoState::text_only_hovered,
            ))
            .with_child(body_label(
                Label::new("Asset and scope-token payloads are ignored."),
                &theme_reader,
                true,
            )),
        theme_reader,
    ))
    .scope(scope)
    .accept(|drag| {
        if drag.payload.as_text().is_some() {
            DropEffect::Copy
        } else {
            DropEffect::None
        }
    })
    .on_hover_change(move |hovered| hover_state.set_text_only_hovered(hovered))
    .on_drop(move |ctx, drag| {
        if let Some(text) = drag.payload.as_text() {
            drop_state.push_text(text.to_string());
            drop_state.set_status(format!("Filtered target copied text: {text}"));
            request_window_refresh(ctx, false);
        }
    })
}

fn scoped_drop_target(
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    state: DragDropDemoState,
) -> impl Widget {
    let hover_state = state.clone();
    let drop_state = state.clone();
    DropTarget::new(panel(
        "Shared scope target",
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(target_heading(
                "Scope-aware target",
                Rc::clone(&theme_reader),
                state.clone(),
                DragDropDemoState::scoped_hovered,
            ))
            .with_child(body_label(
                Label::dynamic("Shared scope target is empty.", move || {
                    state.scope_status()
                }),
                &theme_reader,
                true,
            )),
        theme_reader,
    ))
    .scope(scope)
    .accept(|drag| {
        if drag.payload.custom_kind() == Some(SCOPE_TOKEN_KIND) {
            DropEffect::Link
        } else {
            DropEffect::None
        }
    })
    .on_hover_change(move |hovered| hover_state.set_scoped_hovered(hovered))
    .on_drop(move |ctx, drag| {
        if let Some(token) = drag.payload.custom_data::<ScopeToken>() {
            drop_state.set_scope_status(format!("Accepted from shared scope: {}", token.label));
            drop_state.set_status(format!("Linked scope token: {}", token.label));
            request_window_refresh(ctx, false);
        }
    })
}

fn section<W>(title: &str, description: &str, body: W, theme_reader: DevThemeReader) -> impl Widget
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(section_title_label(Label::new(title), &theme_reader))
        .with_child(body_label(Label::new(description), &theme_reader, true))
        .with_child(body)
        .with_child(
            Separator::horizontal()
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .inset(0.0),
        )
}

fn panel<W>(name: &str, child: W, theme_reader: DevThemeReader) -> impl Widget
where
    W: Widget + 'static,
{
    Surface::panel(child)
        .name(name)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .padding(Insets::all(14.0))
        .elevation(SurfaceElevation::Small)
        .fill_width()
}

fn target_heading<F>(
    text: &'static str,
    theme_reader: DevThemeReader,
    state: DragDropDemoState,
    hovered: F,
) -> impl Widget
where
    F: Fn(&DragDropDemoState) -> bool + 'static,
{
    Label::new(text)
        .style(dev_text_style(
            theme_reader(),
            theme_reader().text.base,
            theme_reader().palette.text,
        ))
        .color_when(move || {
            let theme = theme_reader();
            if hovered(&state) {
                theme.palette.border_focus
            } else {
                theme.palette.text
            }
        })
}
