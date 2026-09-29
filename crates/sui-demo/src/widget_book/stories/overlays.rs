use sui::PopoverAlignment;
use sui::prelude::*;

use super::super::registry::{Category, Story, StoryCtx};
use super::super::specimen::{Section, boxed, strip};
use super::sized;

pub(super) const STORIES: [Story; 6] = [
    Story {
        id: "menu",
        title: "Menu",
        api: "Menu, MenuItem",
        summary: "A list of commands with shortcuts, separators, disabled, and destructive items.",
        keywords: "commands dropdown items shortcut",
        category: Category::Overlays,
        build: menu,
    },
    Story {
        id: "context-menu",
        title: "Context menu",
        api: "ContextMenu",
        summary: "Opens on secondary click with nested submenus. Shown open in place here.",
        keywords: "right click submenu nested menu",
        category: Category::Overlays,
        build: context_menu,
    },
    Story {
        id: "tooltip",
        title: "Tooltip",
        api: "Tooltip",
        summary: "A short hint that appears while hovering a control. Shown open in place here.",
        keywords: "hint hover help",
        category: Category::Overlays,
        build: tooltip,
    },
    Story {
        id: "popover",
        title: "Popover",
        api: "Popover",
        summary: "A lightweight panel anchored to its trigger. Shown open in place here.",
        keywords: "flyout panel anchored inspector",
        category: Category::Overlays,
        build: popover,
    },
    Story {
        id: "dialog",
        title: "Dialog",
        api: "Dialog",
        summary: "A titled surface for confirmations and settings with primary and secondary actions.",
        keywords: "modal confirm settings sheet",
        category: Category::Overlays,
        build: dialog,
    },
    Story {
        id: "notification",
        title: "Notification",
        api: "NotificationHost, TransientNotification",
        summary: "Transient toasts for background results; assertive notifications interrupt.",
        keywords: "toast snackbar alert message",
        category: Category::Overlays,
        build: notification,
    },
];

fn command_items() -> [MenuItem; 4] {
    [
        MenuItem::new("New tab").shortcut("Ctrl+T"),
        MenuItem::new("Duplicate").shortcut("Ctrl+D"),
        MenuItem::new("Bake preview").disabled(),
        MenuItem::new("Delete layer")
            .shortcut("Del")
            .separator_before()
            .destructive(),
    ]
}

fn menu(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![strip(
        theme,
        "",
        vec![
            (
                "Resting",
                boxed(sized(
                    240.0,
                    Menu::new("Command menu")
                        .items(command_items())
                        .theme(theme),
                )),
            ),
            (
                "Highlighted item",
                boxed(sized(
                    240.0,
                    Menu::new("Command menu, highlighted")
                        .items(command_items())
                        .highlighted(1)
                        .theme(theme),
                )),
            ),
        ],
    )]
}

fn context_menu(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let target = |label: &str| {
        Surface::field(ctx.muted(label))
            .padding(Insets::all(12.0))
            .theme(theme)
    };
    let items = || {
        [
            MenuItem::new("Rename"),
            MenuItem::new("Duplicate").shortcut("Ctrl+D"),
            MenuItem::new("Move to").submenu([
                MenuItem::new("Archive"),
                MenuItem::new("Shared").submenu([
                    MenuItem::new("Team workspace"),
                    MenuItem::new("Project workspace"),
                ]),
            ]),
            MenuItem::new("Delete").separator_before().destructive(),
        ]
    };
    vec![strip(
        theme,
        "",
        vec![
            (
                "Open",
                boxed(
                    ContextMenu::new("Layer context menu", target("Layer 08"))
                        .items(items())
                        .theme(theme)
                        .show_inline()
                        .highlighted_path([1]),
                ),
            ),
            (
                "Submenu open",
                boxed(
                    ContextMenu::new("Nested layer context menu", target("Layer 12"))
                        .items(items())
                        .theme(theme)
                        .show_inline()
                        .highlighted_path([2, 1]),
                ),
            ),
        ],
    )]
}

fn tooltip(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let trigger = |label: &str| Button::new(label).theme(theme);
    vec![strip(
        theme,
        "",
        vec![
            (
                "Above",
                boxed(
                    Tooltip::new("Copy link to clipboard", trigger("Share"))
                        .theme(theme)
                        .show_inline(),
                ),
            ),
            (
                "Below",
                boxed(
                    Tooltip::new("Undo the last edit", trigger("Undo"))
                        .placement(TooltipPlacement::Below)
                        .theme(theme)
                        .show_inline(),
                ),
            ),
            (
                "Start aligned",
                boxed(
                    Tooltip::new("Enter sends, Shift+Enter adds a line", trigger("Send"))
                        .alignment(TooltipAlignment::Start)
                        .placement(TooltipPlacement::Below)
                        .theme(theme)
                        .show_inline(),
                ),
            ),
        ],
    )]
}

fn popover(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let content = |title: &str, detail: &str| {
        Stack::vertical()
            .spacing(6.0)
            .alignment(Alignment::Start)
            .with_child(ctx.text(title))
            .with_child(ctx.muted(detail))
    };
    vec![strip(
        theme,
        "",
        vec![
            (
                "Start aligned",
                boxed(
                    Popover::new(
                        "Layer details",
                        Button::new("Details").theme(theme),
                        content("Blend: Screen at 72%", "Mask feather: 8 px"),
                    )
                    .theme(theme)
                    .show_inline(),
                ),
            ),
            (
                "End aligned",
                boxed(
                    Popover::new(
                        "Share options",
                        Button::primary("Share").theme(theme),
                        content("Anyone with the link", "Can view and comment"),
                    )
                    .alignment(PopoverAlignment::End)
                    .theme(theme)
                    .show_inline(),
                ),
            ),
        ],
    )]
}

fn dialog(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let settings = Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch)
        .with_child(ctx.text("Autosave every 90 seconds"))
        .with_child(ctx.text("Export color profile: Display P3"));
    vec![strip(
        theme,
        "",
        vec![
            (
                "Settings",
                boxed(sized(
                    400.0,
                    Dialog::new("Project settings", settings)
                        .description("Applies to every document in this project.")
                        .secondary_action("Cancel", || {})
                        .primary_action("Apply", || {})
                        .theme(theme)
                        .show_inline(),
                )),
            ),
            (
                "Confirmation",
                boxed(sized(
                    340.0,
                    Dialog::new(
                        "Delete 3 layers?",
                        ctx.muted("Deleted layers stay in history until you close the document."),
                    )
                    .header_action(
                        IconButton::new(IconGlyph::Close, "Close dialog")
                            .appearance(ButtonAppearance::Ghost)
                            .theme(theme),
                    )
                    .secondary_action("Keep layers", || {})
                    .action(Button::danger("Delete").theme(theme))
                    .theme(theme)
                    .show_inline(),
                )),
            ),
        ],
    )]
}

fn notification(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let host = |notification: TransientNotification| {
        let center = NotificationCenter::new();
        center.push(notification.persistent());
        SizedBox::new()
            .width(340.0)
            .height(96.0)
            .with_child(NotificationHost::new(center).theme(theme).width(320.0))
    };
    vec![strip(
        theme,
        "",
        vec![
            (
                "Polite",
                boxed(host(TransientNotification::new(
                    "Workspace indexed",
                    "42 files are ready for search.",
                ))),
            ),
            (
                "Assertive",
                boxed(host(
                    TransientNotification::new("Export failed", "The disk is full.")
                        .urgency(NotificationUrgency::Assertive),
                )),
            ),
        ],
    )]
}
