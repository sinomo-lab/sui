use sui::prelude::*;

use super::super::registry::{Category, Story, StoryCtx};
use super::super::specimen::{Section, boxed, example, strip};
use super::selection::size_strip;
use super::sized;

pub(super) const STORIES: [Story; 4] = [
    Story {
        id: "tab-bar",
        title: "Tab bar",
        api: "TabBar",
        summary: "Switches between peer views; the selected tab carries an accent underline.",
        keywords: "tabs workspace switcher strip",
        category: Category::Navigation,
        build: tab_bar,
    },
    Story {
        id: "tabs",
        title: "Tabs",
        api: "Tabs",
        summary: "A tab bar that owns its panels and shows the selected one.",
        keywords: "tab panel inspector pages",
        category: Category::Navigation,
        build: tabs,
    },
    Story {
        id: "browser-tab-bar",
        title: "Browser tab bar",
        api: "BrowserTabBar",
        summary: "Closable document tabs for editor and browser chrome.",
        keywords: "documents close tabs editor",
        category: Category::Navigation,
        build: browser_tab_bar,
    },
    Story {
        id: "breadcrumb",
        title: "Breadcrumb",
        api: "Breadcrumb",
        summary: "Shows the path to the current location; earlier segments navigate back.",
        keywords: "path bar location hierarchy pathbar",
        category: Category::Navigation,
        build: breadcrumb,
    },
];

fn tab_bar(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let tabs = ["Canvas", "Inspector", "Export"];
    vec![
        strip(
            theme,
            "Selection",
            vec![
                (
                    "First selected",
                    boxed(sized(
                        300.0,
                        TabBar::new("Workspace tabs")
                            .tabs(tabs)
                            .selected(0)
                            .theme(theme),
                    )),
                ),
                (
                    "Last selected",
                    boxed(sized(
                        300.0,
                        TabBar::new("Workspace tabs, export")
                            .tabs(tabs)
                            .selected(2)
                            .theme(theme),
                    )),
                ),
                (
                    "Disabled",
                    boxed(sized(
                        300.0,
                        TabBar::new("Workspace tabs, disabled")
                            .tabs(tabs)
                            .selected(0)
                            .enabled(false)
                            .theme(theme),
                    )),
                ),
            ],
        ),
        size_strip(ctx, |size_theme| {
            sized(
                240.0,
                TabBar::new("Sized tabs")
                    .tabs(["Canvas", "Inspect"])
                    .selected(1)
                    .theme(size_theme),
            )
        }),
    ]
}

fn tabs(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let panel = |title: &str, detail: &str| {
        Padding::all(
            4.0,
            Stack::vertical()
                .spacing(6.0)
                .alignment(Alignment::Stretch)
                .with_child(ctx.text(title))
                .with_child(ctx.muted(detail)),
        )
    };
    vec![example(
        "",
        SizedBox::new().width(460.0).height(150.0).with_child(
            Tabs::new("Inspector tabs")
                .selected(1)
                .theme(theme)
                .tab(
                    "Layout",
                    panel("Layout", "Alignment, spacing, and geometry."),
                )
                .tab(
                    "Data",
                    panel(
                        "Selection: 4 layers, 2 masks",
                        "Bound to the active document.",
                    ),
                )
                .tab("History", panel("History", "Undo groups and checkpoints.")),
        ),
    )]
}

fn browser_tab_bar(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![example(
        "",
        sized(
            560.0,
            BrowserTabBar::new("Open documents")
                .tabs(["main.rs", "theme.rs", "widget_book.rs", "README.md"])
                .selected(Some(1))
                .theme(theme),
        ),
    )]
}

fn breadcrumb(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let items = |labels: &[&str]| {
        labels
            .iter()
            .map(|label| BreadcrumbItem::new(*label))
            .collect::<Vec<_>>()
    };
    vec![strip(
        theme,
        "",
        vec![
            (
                "Short",
                boxed(
                    Breadcrumb::new("Project path")
                        .items(items(&["Workspace", "Starfall"]))
                        .current(1)
                        .theme(theme),
                ),
            ),
            (
                "Deep",
                boxed(
                    Breadcrumb::new("Material path")
                        .items(items(&[
                            "Workspace",
                            "Projects",
                            "Starfall",
                            "Materials",
                            "Glass",
                        ]))
                        .current(4)
                        .theme(theme),
                ),
            ),
            (
                "Path bar",
                boxed(
                    Breadcrumb::new("Asset path bar")
                        .items(items(&["Assets", "Textures", "hero.png"]))
                        .current(2)
                        .theme(theme),
                ),
            ),
        ],
    )]
}
