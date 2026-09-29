use sui::prelude::*;
use sui::{SemanticTone, StatusBadge};

use super::super::registry::{Category, Story, StoryCtx};
use super::super::specimen::{Section, example};
use crate::demo_support::paint_table_cell;

pub(super) const STORIES: [Story; 8] = [
    Story {
        id: "list-view",
        title: "List view",
        api: "ListView, ListItem",
        summary: "Selectable rows with detail text, accents, trailing values, and custom content.",
        keywords: "rows list selection items",
        category: Category::Data,
        build: list_view,
    },
    Story {
        id: "tree-view",
        title: "Tree view",
        api: "TreeView, TreeItem",
        summary: "Expandable hierarchy with detail text and custom row content.",
        keywords: "hierarchy outline scene nested",
        category: Category::Data,
        build: tree_view,
    },
    Story {
        id: "table",
        title: "Table and data grid",
        api: "Table, DataGrid",
        summary: "Columnar rows with aligned numeric columns and a selected row.",
        keywords: "grid columns rows cells",
        category: Category::Data,
        build: table,
    },
    Story {
        id: "virtual-table",
        title: "Virtual table",
        api: "VirtualTable",
        summary: "Painter-driven rows for very large tables; only visible rows are drawn.",
        keywords: "virtualized large table rows painter",
        category: Category::Data,
        build: virtual_table,
    },
    Story {
        id: "virtual-list",
        title: "Virtual list",
        api: "VirtualList",
        summary: "Keyed widget rows realized on demand from a collection model.",
        keywords: "virtualized keyed collection rows",
        category: Category::Data,
        build: virtual_list,
    },
    Story {
        id: "layer-list",
        title: "Layer list",
        api: "LayerList",
        summary: "Layers with thumbnails, visibility, lock, and drag-to-reorder.",
        keywords: "layers visibility lock reorder",
        category: Category::Data,
        build: layer_list,
    },
    Story {
        id: "reorderable-list",
        title: "Reorderable list",
        api: "ReorderableList",
        summary: "Arbitrary child rows that can be dragged into a new order.",
        keywords: "drag reorder sortable",
        category: Category::Data,
        build: reorderable_list,
    },
    Story {
        id: "drag-and-drop",
        title: "Drag and drop",
        api: "DragDropHost, Draggable, DropTarget",
        summary: "Typed payloads moved between a source and a target within one scope.",
        keywords: "drag drop payload target source",
        category: Category::Data,
        build: drag_and_drop,
    },
];

fn list_view(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let accent = theme.decorative.get(DecorativeHue::Blue).solid;
    let warm = theme.decorative.get(DecorativeHue::Orange).solid;
    vec![example(
        "",
        SizedBox::new().width(360.0).height(236.0).with_child(
            ListView::new("Assets list")
                .padding(Insets::all(8.0))
                .items([
                    ListItem::new("Hero texture")
                        .detail("2048 x 2048 RGBA")
                        .accent(accent),
                    ListItem::new("Normals atlas").detail("Streaming mip chain"),
                    ListItem::new("Glass material").with_content(
                        Stack::horizontal()
                            .spacing(10.0)
                            .alignment(Alignment::Center)
                            .with_child(ctx.text("Glass material"))
                            .with_child(
                                StatusBadge::new("3 prefabs")
                                    .tone(SemanticTone::Accent)
                                    .theme(theme),
                            ),
                    ),
                    ListItem::new("UI icon sheet")
                        .detail("Tagged for export")
                        .trailing("12 MB")
                        .accent(warm),
                    ListItem::new("Archive cache")
                        .detail("Read only")
                        .disabled(),
                ])
                .selected(1)
                .theme(theme),
        ),
    )]
}

fn tree_view(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![example(
        "",
        SizedBox::new().width(380.0).height(236.0).with_child(
            TreeView::new("Scene tree")
                .padding(Insets::all(8.0))
                .items([TreeItem::new("Scene")
                    .expanded(true)
                    .with_child(
                        TreeItem::new("Environment")
                            .expanded(true)
                            .with_child(TreeItem::new("Sky dome").detail("Visible"))
                            .with_child(TreeItem::new("Fog volume").detail("Animated")),
                    )
                    .with_child(
                        TreeItem::new("Characters")
                            .expanded(true)
                            .with_child(
                                TreeItem::new("Pilot").with_content(
                                    Stack::horizontal()
                                        .spacing(10.0)
                                        .alignment(Alignment::Center)
                                        .with_child(ctx.text("Pilot"))
                                        .with_child(
                                            StatusBadge::new("Selected")
                                                .tone(SemanticTone::Accent)
                                                .theme(theme),
                                        ),
                                ),
                            )
                            .with_child(TreeItem::new("Companion drone")),
                    )
                    .with_child(TreeItem::new("FX").detail("Collapsed group"))])
                .theme(theme),
        ),
    )]
}

fn table(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![
        example(
            "Table",
            SizedBox::new().width(620.0).height(230.0).with_child(
                Table::new("Material table")
                    .columns([
                        TableColumn::new("Material"),
                        TableColumn::new("Domain").width(110.0),
                        TableColumn::new("Shader").width(150.0),
                        TableColumn::new("Passes")
                            .width(80.0)
                            .alignment(TableColumnAlignment::End),
                        TableColumn::new("Last edit").width(110.0),
                    ])
                    .rows([
                        TableRow::new([
                            "ClearCoat_Glass",
                            "Surface",
                            "pbr.clearcoat",
                            "3",
                            "2 min ago",
                        ]),
                        TableRow::new([
                            "Terrain_Master",
                            "Surface",
                            "terrain.layered",
                            "5",
                            "11 min ago",
                        ]),
                        TableRow::new([
                            "UI_Highlight",
                            "Overlay",
                            "ui.gradient",
                            "1",
                            "24 min ago",
                        ]),
                        TableRow::new(["CloudShadow", "Decal", "fx.projected", "2", "1 hour ago"]),
                        TableRow::new(["Water_Foam", "Surface", "water.foam", "4", "yesterday"]),
                    ])
                    .selected(2)
                    .theme(theme),
            ),
        ),
        example(
            "Data grid",
            SizedBox::new().width(420.0).height(190.0).with_child(
                DataGrid::new("Asset grid")
                    .columns([
                        TableColumn::new("Asset"),
                        TableColumn::new("Type").width(92.0),
                        TableColumn::new("State").width(100.0),
                    ])
                    .rows([
                        TableRow::new(["hero.png", "Image", "Ready"]),
                        TableRow::new(["glass.mat", "Material", "Dirty"]),
                        TableRow::new(["rig.skel", "Rig", "Cached"]),
                    ])
                    .selected(0)
                    .theme(theme),
            ),
        ),
    ]
}

fn virtual_table(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let table = VirtualTable::new("Virtual asset table")
        .columns([
            VirtualTableColumn::new("Row").width(80.0),
            VirtualTableColumn::new("Asset").width(180.0),
            VirtualTableColumn::new("Status").width(120.0),
        ])
        .row_count(10_000)
        .selected(2)
        .row_name(|row| format!("Virtual row {row}"))
        .row_description(|row| format!("Asset record {row}"))
        .row_painter(move |ctx, row| {
            let body = theme.body_text_style();
            let muted = TextStyle {
                color: theme.palette.text_muted,
                ..body.clone()
            };
            let status_tone = match row.row_index % 4 {
                0 => (theme.palette.success, "Ready"),
                1 => (theme.palette.warning, "Dirty"),
                2 => (theme.palette.text_muted, "Cached"),
                _ => (theme.palette.accent, "Streaming"),
            };
            let status = TextStyle {
                color: status_tone.0,
                ..body.clone()
            };
            let cells = [
                (format!("#{:04}", row.row_index), &muted),
                (format!("asset_{:05}.png", row.row_index), &body),
                (status_tone.1.to_string(), &status),
            ];
            for (rect, (text, style)) in row.column_rects.iter().zip(cells.iter()) {
                paint_table_cell(ctx, *rect, text, style, 0.0);
            }
        })
        .theme(theme);
    vec![example(
        "10,000 rows",
        SizedBox::new().width(400.0).height(190.0).with_child(table),
    )]
}

fn virtual_list(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let rows = VirtualCollectionModel::from_items(
        "Widget book virtual rows",
        (0_u64..2_000).map(|index| (index, format!("Retained row {index}"))),
    )
    .expect("widget-book row keys are unique");
    vec![example(
        "2,000 keyed rows",
        SizedBox::new().width(360.0).height(150.0).with_child(
            VirtualList::new("Virtual retained rows", rows, move |_key, text| {
                Surface::field(Label::new("").text_from(text))
                    .padding(Insets {
                        left: 8.0,
                        top: 5.0,
                        right: 8.0,
                        bottom: 5.0,
                    })
                    .theme(theme)
            })
            .estimated_row_height(30.0)
            .row_name(|key, _| format!("Virtual retained row {key}")),
        ),
    )]
}

fn layer_list(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let decorative = |hue| theme.decorative.get(hue).solid;
    vec![example(
        "",
        SizedBox::new().width(340.0).height(170.0).with_child(
            LayerList::new("Layer stack")
                .layers([
                    LayerListItem::new("Highlights")
                        .detail("Screen 72%")
                        .thumbnail(decorative(DecorativeHue::Amber)),
                    LayerListItem::new("Glass")
                        .detail("Normal")
                        .thumbnail(decorative(DecorativeHue::Blue)),
                    LayerListItem::new("Guides")
                        .detail("Hidden")
                        .thumbnail(decorative(DecorativeHue::Teal))
                        .visible(false),
                    LayerListItem::new("Shadow")
                        .detail("Multiply")
                        .thumbnail(Color::rgba(0.08, 0.1, 0.16, 1.0))
                        .locked(true),
                ])
                .selected(1)
                .theme(theme),
        ),
    )]
}

fn reorderable_list(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let row = |label: &str| {
        Surface::field(ctx.text(label))
            .padding(Insets::all(10.0))
            .theme(theme)
    };
    vec![example(
        "",
        SizedBox::new().width(320.0).with_child(
            ReorderableList::new("Reorderable task list")
                .item(row("Capture screenshots"))
                .item(row("Review contrast"))
                .item(row("Update docs"))
                .theme(theme),
        ),
    )]
}

fn drag_and_drop(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let scope = DragDropScope::new();
    vec![example(
        "",
        DragDropHost::new(
            scope.clone(),
            Stack::horizontal()
                .spacing(16.0)
                .alignment(Alignment::Center)
                .with_child(
                    Draggable::new(
                        Surface::field(ctx.text("hero.png"))
                            .padding(Insets::all(12.0))
                            .theme(theme),
                    )
                    .scope(scope.clone())
                    .payload(|| DragPayload::text("asset://hero"))
                    .effect(DropEffect::Copy)
                    .preview_label("hero.png"),
                )
                .with_child(ctx.muted("drag to"))
                .with_child(
                    DropTarget::new(
                        Surface::panel(ctx.text("Drop target slot"))
                            .padding(Insets::all(12.0))
                            .theme(theme),
                    )
                    .scope(scope)
                    .accept(|_| DropEffect::Copy),
                ),
        )
        .theme(theme),
    )]
}
