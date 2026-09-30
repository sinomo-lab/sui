//! The Drag and drop page: a board of cards to reorder and move between
//! columns, a shelf of assets and snippets to drop on them, drop zones that
//! accept or refuse, and a log of every step of each drag.

mod board;
#[cfg(test)]
mod tests;
mod widgets;

use std::{cell::Cell, rc::Rc};

use sui::{
    DragDropScope, DragEvent, DragOutcome, DragPayload, DragPreview, DropEffect, DropEffects,
    DropHover, PointerButton, Signal, prelude::*,
};

use self::board::{Asset, AssetKind, Board, Card, Column, Nudge};
use self::widgets::{CardFrame, CardHint, CardSlot, ZoneFrame};
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, dev_theme_color};
#[cfg(test)]
use crate::demo_support::default_theme_reader;
use crate::demo_support::{DemoTextColor, demo_label, demo_mono_label};

pub(crate) const DRAG_DROP_TAB_LABEL: &str = "Drag and drop";
pub(crate) const DRAG_DROP_DEMO_SCROLL_NAME: &str = "Drag and drop demo scroll";
const SUMMARY: &str = "Drag cards between columns, drop assets and snippets on them, and follow each drag in the event log.";

pub(crate) const EVENT_LOG_NAME: &str = "Drag events";
pub(crate) const SHELF_NAME: &str = "Shelf";
pub(crate) const TRASH_NAME: &str = "Trash";
pub(crate) const UNDO_LABEL: &str = "Undo delete";
pub(crate) const TEXT_FIELD_NAME: &str = "Text only field";
pub(crate) const LOCKED_WELL_NAME: &str = "Locked palette well";
pub(crate) const LOCKED_SWATCH_NAME: &str = "Brand palette";
pub(crate) const FILES_NAME: &str = "Desktop files";

const CARD_KIND: &str = "sui-demo.board-card";
const ASSET_KIND: &str = "sui-demo.asset";
const SWATCH_KIND: &str = "sui-demo.locked-swatch";
const LOG_LINES: usize = 6;

/// A card being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CardRef {
    id: u32,
}

const SHELF_ASSETS: [Asset; 5] = [
    Asset {
        name: "Hero.png",
        kind: AssetKind::Image,
    },
    Asset {
        name: "Dusk.png",
        kind: AssetKind::Image,
    },
    Asset {
        name: "Sunset",
        kind: AssetKind::Palette,
    },
    Asset {
        name: "Ocean",
        kind: AssetKind::Palette,
    },
    Asset {
        name: "Studio.scene",
        kind: AssetKind::Scene,
    },
];
const SNIPPETS: [&str; 3] = ["Ship it Friday", "Needs review", "Blocked on design"];

/// The key that turns a drag into a copy here.
fn copy_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "Option"
    } else {
        "Ctrl"
    }
}

fn effect_name(effect: DropEffect) -> &'static str {
    match effect {
        DropEffect::None => "none",
        DropEffect::Copy => "copy",
        DropEffect::Move => "move",
        DropEffect::Link => "link",
    }
}

fn asset_icon(kind: AssetKind) -> IconGlyph {
    match kind {
        AssetKind::Image => IconGlyph::Camera,
        AssetKind::Palette => IconGlyph::Palette,
        AssetKind::Scene => IconGlyph::Blocks,
    }
}

/// What the page's parts share: the board, the log, and the zones' state.
#[derive(Clone)]
struct Page {
    theme_reader: DevThemeReader,
    scope: DragDropScope,
    board: Signal<Board>,
    log: Signal<Vec<String>>,
    /// The drag in progress, which log lines about it are numbered by.
    session: Rc<Cell<u64>>,
    /// A card to focus once it is laid out in a column after a move.
    focus_request: Rc<Cell<Option<(u32, Column)>>>,
    field_text: Signal<Vec<String>>,
    files_hover: Signal<DropHover>,
}

impl Page {
    fn new(theme_reader: DevThemeReader) -> Self {
        Self {
            theme_reader,
            scope: DragDropScope::new(),
            board: Signal::named("Board", Board::new()),
            log: Signal::named(EVENT_LOG_NAME, Vec::new()),
            session: Rc::new(Cell::new(0)),
            focus_request: Rc::new(Cell::new(None)),
            field_text: Signal::named(TEXT_FIELD_NAME, Vec::new()),
            files_hover: Signal::named(FILES_NAME, DropHover::Idle),
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

    /// Log a step of the drag in progress.
    fn log_drag(&self, step: impl AsRef<str>) {
        self.log(format!("#{:<3} {}", self.session.get(), step.as_ref()));
    }

    fn title(&self, id: u32) -> String {
        self.board
            .get()
            .card(id)
            .map_or_else(String::new, |card| card.title.clone())
    }

    fn started(&self, preview: &DragPreview, what: &str) {
        self.session.set(preview.session_id.get());
        let mut effects = vec![effect_name(preview.allowed_effect)];
        for effect in [DropEffect::Copy, DropEffect::Move, DropEffect::Link] {
            if effect != preview.allowed_effect && preview.allowed_effects.contains(effect) {
                effects.push(effect_name(effect));
            }
        }
        self.log_drag(format!("start   {what} · {}", effects.join(", ")));
    }

    fn ended(&self, drag: &DragEvent) {
        if matches!(drag.outcome, Some(DragOutcome::Cancelled)) {
            self.log_drag("cancel  Esc, or released where nothing takes it");
        }
    }

    fn hovered(&self, target: &str, state: DropHover) {
        match state {
            DropHover::Accepting(effect) => {
                self.log_drag(format!("over    {target} · {}", effect_name(effect)));
            }
            DropHover::Refusing => self.log_drag(format!("refused {target}")),
            DropHover::Idle => {}
        }
    }
}

pub(crate) fn build_drag_drop_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let page = Page::new(theme_reader);
    let theme_reader = &page.theme_reader;
    let content = Stack::vertical()
        .spacing(24.0)
        .alignment(Alignment::Stretch)
        .with_child(header(theme_reader))
        .with_child(event_log(&page))
        .with_child(
            Flex::horizontal()
                .gap(20.0)
                .wrap(FlexWrap::Wrap)
                .align_items(Alignment::Start)
                .with_item(shelf(&page), FlexItem::new().basis(250.0).min_width(220.0))
                .with_item(
                    board_view(&page),
                    FlexItem::new().grow(1.0).basis(640.0).min_width(300.0),
                ),
        )
        .with_child(zones(&page));
    let scroll = Background::new(
        theme_reader().palette.surface,
        ScrollView::vertical(Padding::all(24.0, content))
            .name(DRAG_DROP_DEMO_SCROLL_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader)),
    )
    .brush_when(dev_theme_color(theme_reader, |theme| theme.palette.surface));
    host(&page, scroll)
}

/// The page on its own, for tests.
#[cfg(test)]
pub(crate) fn build_drag_drop_application() -> Application {
    App::new()
        .window(
            Window::new(DRAG_DROP_TAB_LABEL)
                .root(build_drag_drop_demo_with_theme(default_theme_reader())),
        )
        .into_application()
}

/// The host for the page's drags: it draws their previews and turns files
/// dropped from the desktop into cards.
fn host<W>(page: &Page, child: W) -> DragDropHost
where
    W: Widget + 'static,
{
    let preview = page.clone();
    let hover = page.clone();
    let drop = page.clone();
    let cancel = page.clone();
    DragDropHost::new(page.scope.clone(), child)
        .theme_when(clone_dev_theme_reader(&page.theme_reader))
        .preview(move |dragged| drag_preview(&preview, dragged))
        .on_external_file_hover(move |_, paths| {
            hover
                .files_hover
                .set(DropHover::Accepting(DropEffect::Copy));
            hover.log(format!(
                "file    {} over the page",
                if paths.len() == 1 {
                    "1 file".to_string()
                } else {
                    format!("{} files", paths.len())
                }
            ));
        })
        .on_external_file_drop(move |_, path| {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            drop.board.update(|board| {
                board.add_card(Column::Todo, name.clone());
            });
            drop.files_hover.set(DropHover::Idle);
            drop.log(format!("file    dropped {name} · new card in To do"));
        })
        .on_external_file_hover_cancelled(move |_| {
            cancel.files_hover.set(DropHover::Idle);
            cancel.log("file    left the page");
        })
}

/// A lifted card or asset chip under the pointer; text keeps the label.
fn drag_preview(page: &Page, dragged: &DragPreview) -> Option<Box<dyn Widget>> {
    let theme_reader = &page.theme_reader;
    match dragged.payload.custom_kind() {
        Some(CARD_KIND) => {
            let id = dragged.payload.custom_data::<CardRef>()?.id;
            let title = page.board.get().card(id)?.title.clone();
            Some(Box::new(
                SizedBox::new().width(220.0).with_child(
                    Surface::panel(
                        Stack::vertical()
                            .spacing(4.0)
                            .alignment(Alignment::Stretch)
                            .with_child(demo_label(
                                theme_reader,
                                title,
                                DemoTextRole::Body,
                                DemoTextColor::Text,
                            ))
                            .with_child(demo_label(
                                theme_reader,
                                format!("Moves · hold {} to copy", copy_key()),
                                DemoTextRole::Metadata,
                                DemoTextColor::Muted,
                            )),
                    )
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .elevation(SurfaceElevation::Medium)
                    .padding(Insets::all(12.0))
                    .fill_width(),
                ),
            ))
        }
        Some(ASSET_KIND) => {
            let asset = *dragged.payload.custom_data::<Asset>()?;
            Some(Box::new(
                SizedBox::new().width(200.0).with_child(
                    Surface::panel(chip_row(
                        theme_reader,
                        asset_icon(asset.kind),
                        asset.name,
                        "Attach",
                    ))
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .elevation(SurfaceElevation::Medium)
                    .padding(Insets::all(10.0))
                    .fill_width(),
                ),
            ))
        }
        _ => None,
    }
}

fn header(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(demo_label(
            theme_reader,
            DRAG_DROP_TAB_LABEL,
            DemoTextRole::PageTitle,
            DemoTextColor::Text,
        ))
        .with_child(demo_label(
            theme_reader,
            SUMMARY,
            DemoTextRole::Supporting,
            DemoTextColor::Muted,
        ))
        .with_child(demo_label(
            theme_reader,
            format!(
                "Hold {} while dragging to copy · Esc cancels a drag · Alt and an arrow key move a focused card · Delete trashes it",
                copy_key()
            ),
            DemoTextRole::Metadata,
            DemoTextColor::Muted,
        ))
}

fn event_log(page: &Page) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    Surface::field(
        SizedBox::new().height(128.0).with_child(
            Stack::vertical()
                .spacing(6.0)
                .alignment(Alignment::Stretch)
                .with_child(demo_label(
                    theme_reader,
                    "Event log",
                    DemoTextRole::CardTitle,
                    DemoTextColor::Text,
                ))
                .with_child(
                    demo_mono_label(theme_reader, "", DemoTextRole::Metadata, |theme| {
                        theme.palette.text
                    })
                    .semantic_name(EVENT_LOG_NAME)
                    .text_from(page.log.select(|lines| {
                        if lines.is_empty() {
                            "Start a drag and each step shows up here.".to_string()
                        } else {
                            lines.join("\n")
                        }
                    })),
                ),
        ),
    )
    .theme_when(clone_dev_theme_reader(theme_reader))
    .padding(Insets::all(12.0))
    .fill_width()
}

/// An icon, a name, and a badge in a row.
fn chip_row(
    theme_reader: &DevThemeReader,
    icon: IconGlyph,
    name: &str,
    badge: &str,
) -> impl Widget + use<> {
    let text = Rc::clone(theme_reader);
    Flex::horizontal()
        .gap(8.0)
        .align_items(Alignment::Center)
        .with_child(
            Icon::new(icon)
                .size(16.0)
                .color_when(move || text().palette.text),
        )
        .with_item(
            demo_label(theme_reader, name, DemoTextRole::Body, DemoTextColor::Text),
            FlexItem::flex(1.0),
        )
        .with_child(demo_label(
            theme_reader,
            badge,
            DemoTextRole::Metadata,
            DemoTextColor::Muted,
        ))
}

/// A shelf item: a drag handle and its row, on a field surface.
fn shelf_chip(theme_reader: &DevThemeReader, icon: IconGlyph, name: &str, badge: &str) -> Surface {
    let muted = Rc::clone(theme_reader);
    Surface::field(
        Flex::horizontal()
            .gap(8.0)
            .align_items(Alignment::Center)
            .with_child(
                Icon::new(IconGlyph::Move)
                    .size(14.0)
                    .color_when(move || muted().palette.text_muted),
            )
            .with_item(
                chip_row(theme_reader, icon, name, badge),
                FlexItem::flex(1.0),
            ),
    )
    .name(format!("Shelf item {name}"))
    .theme_when(clone_dev_theme_reader(theme_reader))
    .padding(Insets::all(8.0))
    .fill_width()
}

fn shelf(page: &Page) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let mut items = Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(demo_label(
            theme_reader,
            SHELF_NAME,
            DemoTextRole::CardTitle,
            DemoTextColor::Text,
        ))
        .with_child(demo_label(
            theme_reader,
            "Drop an asset on a card to attach it.",
            DemoTextRole::Metadata,
            DemoTextColor::Muted,
        ));
    for asset in SHELF_ASSETS {
        let start = page.clone();
        let end = page.clone();
        items.push(
            Draggable::new(shelf_chip(
                theme_reader,
                asset_icon(asset.kind),
                asset.name,
                asset.kind.name(),
            ))
            .scope(page.scope.clone())
            .payload(move || DragPayload::custom(ASSET_KIND, asset))
            .effects(DropEffects::COPY | DropEffects::LINK)
            .preview_label(asset.name)
            .on_drag_start(move |_, preview| start.started(preview, asset.name))
            .on_drag_end(move |_, drag| end.ended(drag)),
        );
    }
    items.push(demo_label(
        theme_reader,
        "Drop a snippet on a card to add it to the note.",
        DemoTextRole::Metadata,
        DemoTextColor::Muted,
    ));
    for snippet in SNIPPETS {
        let start = page.clone();
        let end = page.clone();
        items.push(
            Draggable::new(shelf_chip(theme_reader, IconGlyph::Type, snippet, "Text"))
                .scope(page.scope.clone())
                .payload(move || DragPayload::text(snippet))
                .effect(DropEffect::Copy)
                .preview_label(format!("“{snippet}”"))
                .on_drag_start(move |_, preview| {
                    start.started(preview, &format!("“{snippet}”"));
                })
                .on_drag_end(move |_, drag| end.ended(drag)),
        );
    }
    Surface::panel(items)
        .name(SHELF_NAME)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .padding(Insets::all(14.0))
        .fill_width()
}

fn board_view(page: &Page) -> impl Widget + use<> {
    let mut columns = Flex::horizontal()
        .gap(14.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Start);
    for column in Column::ALL {
        columns.push_item(
            column_view(page, column),
            FlexItem::new().grow(1.0).basis(200.0).min_width(180.0),
        );
    }
    columns
}

fn column_view(page: &Page, column: Column) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let cards = page
        .board
        .select(move |board| board.column(column).to_vec());
    let count = page
        .board
        .select(move |board| board.column(column).len().to_string());
    let builder = page.clone();
    Surface::panel(
        Stack::vertical()
            .spacing(2.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Flex::horizontal()
                    .gap(8.0)
                    .align_items(Alignment::Center)
                    .with_item(
                        demo_label(
                            theme_reader,
                            column.name(),
                            DemoTextRole::CardTitle,
                            DemoTextColor::Text,
                        ),
                        FlexItem::flex(1.0),
                    )
                    .with_child(
                        demo_label(
                            theme_reader,
                            "",
                            DemoTextRole::Metadata,
                            DemoTextColor::Muted,
                        )
                        .semantic_name(format!("{} count", column.name()))
                        .text_from(count),
                    ),
            )
            .with_child(KeyedStack::vertical(
                cards,
                |card: &Card| card.id,
                move |_, card| card_view(&builder, column, card),
            ))
            .with_child(SizedBox::new().height(CARD_GAP_BELOW))
            .with_child(column_tail(page, column)),
    )
    .name(format!("{} column", column.name()))
    .theme_when(clone_dev_theme_reader(theme_reader))
    .padding(Insets::all(12.0))
    .fill_width()
}

/// Space between the last card and the column's drop zone.
const CARD_GAP_BELOW: f32 = 10.0;

/// What a drag over card `id` would do, and the effect to accept it with.
fn card_accepts(board: &Board, id: u32, drag: &DragEvent) -> (DropEffect, CardHint) {
    match drag.payload.custom_kind() {
        Some(CARD_KIND) => {
            let Some(dragged) = drag.payload.custom_data::<CardRef>() else {
                return (DropEffect::None, CardHint::None);
            };
            let effect = drag.preferred_effect();
            if dragged.id == id && effect != DropEffect::Copy {
                (DropEffect::None, CardHint::None)
            } else {
                (effect, CardHint::InsertBefore)
            }
        }
        Some(ASSET_KIND) => {
            let attached = drag.payload.custom_data::<Asset>().is_some_and(|asset| {
                board
                    .card(id)
                    .is_some_and(|card| card.attachments.contains(asset))
            });
            if attached || !drag.allowed_effects.contains(DropEffect::Link) {
                (DropEffect::None, CardHint::None)
            } else {
                (DropEffect::Link, CardHint::Attach)
            }
        }
        _ if drag.payload.as_text().is_some() => (DropEffect::Copy, CardHint::Note),
        _ => (DropEffect::None, CardHint::None),
    }
}

fn card_view(page: &Page, column: Column, card: Signal<Card>) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let id = card.get().id;
    let title = card.get().title;
    let hint = Signal::new(CardHint::None);
    let pending = Rc::new(Cell::new(CardHint::None));

    let nudge = {
        let page = page.clone();
        Rc::new(move |_: &mut EventCtx, id: u32, nudge: Nudge| nudge_card(&page, id, nudge))
    };
    let delete = {
        let page = page.clone();
        Rc::new(move |_: &mut EventCtx, id: u32| delete_card(&page, id))
    };
    let frame = CardFrame::new(
        theme_reader,
        &card,
        column,
        &hint,
        &page.focus_request,
        nudge,
        delete,
        card_content(page, &card),
    );
    let start = page.clone();
    let end = page.clone();
    let source = Draggable::new(frame)
        .scope(page.scope.clone())
        .payload(move || DragPayload::custom(CARD_KIND, CardRef { id }))
        .effects(DropEffects::MOVE | DropEffects::COPY)
        .preview_label(title.clone())
        .on_drag_start(move |_, preview| start.started(preview, &format!("“{}”", start.title(id))))
        .on_drag_end(move |_, drag| end.ended(drag));

    let accept_board = page.board.clone();
    let accept_pending = Rc::clone(&pending);
    let hover_page = page.clone();
    let drop_page = page.clone();
    DropTarget::new(CardSlot::new(theme_reader, &hint, source))
        .scope(page.scope.clone())
        .accept(move |drag| {
            let (effect, kind) = card_accepts(&accept_board.get(), id, drag);
            accept_pending.set(kind);
            effect
        })
        .on_hover_state(move |state| {
            let kind = if state.is_accepting() {
                pending.get()
            } else {
                CardHint::None
            };
            hint.set(kind);
            let title = hover_page.title(id);
            let target = match kind {
                CardHint::InsertBefore => format!("before “{title}”"),
                CardHint::Attach => format!("attach to “{title}”"),
                CardHint::Note => format!("note of “{title}”"),
                // A card refuses itself, and assets it already has; that
                // is not worth a line.
                CardHint::None => return,
            };
            hover_page.hovered(&target, state);
        })
        .on_drop(move |_, drag| drop_on_card(&drop_page, id, drag))
}

fn drop_on_card(page: &Page, id: u32, drag: &DragEvent) {
    let title = page.title(id);
    let effect = effect_name(drag.accepted_effect);
    if let Some(dragged) = drag.payload.custom_data::<CardRef>() {
        let copy = drag.accepted_effect == DropEffect::Copy;
        let dragged_title = page.title(dragged.id);
        page.board.update(|board| {
            board.place_before(dragged.id, id, copy);
        });
        page.log_drag(format!(
            "drop    “{dragged_title}” before “{title}” · {effect}"
        ));
    } else if let Some(asset) = drag.payload.custom_data::<Asset>() {
        let asset = *asset;
        page.board.update(|board| {
            board.attach(id, asset);
        });
        page.log_drag(format!("drop    {} on “{title}” · {effect}", asset.name));
    } else if let Some(text) = drag.payload.as_text() {
        page.board.update(|board| {
            board.add_note(id, text);
        });
        page.log_drag(format!(
            "drop    “{text}” into the note of “{title}” · {effect}"
        ));
    }
}

fn nudge_card(page: &Page, id: u32, nudge: Nudge) {
    let title = page.title(id);
    let before = page.board.get().find(id);
    let mut moved = None;
    page.board.update(|board| moved = board.nudge(id, nudge));
    let Some((column, index)) = moved else {
        return;
    };
    // A card moved to another column is built again there.
    if before.map(|(from, _)| from) != Some(column) {
        page.focus_request.set(Some((id, column)));
    }
    let count = page.board.get().column(column).len();
    page.log(format!(
        "key     “{title}” to {}, {} of {count}",
        column.name(),
        index + 1
    ));
}

fn move_card_to(page: &Page, id: u32, column: Column) {
    let title = page.title(id);
    page.board.update(|board| {
        board.append(id, column, false);
    });
    page.focus_request.set(Some((id, column)));
    page.log(format!("menu    “{title}” to {}", column.name()));
}

fn delete_card(page: &Page, id: u32) {
    let title = page.title(id);
    page.board.update(|board| {
        board.delete(id);
    });
    page.log(format!("delete  “{title}” is in the trash"));
}

fn card_content(page: &Page, card: &Signal<Card>) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let title = card.select(|card| card.title.clone());
    let note = card.select(|card| {
        if card.note.is_empty() {
            "Drop a snippet to add a note".to_string()
        } else {
            card.note.clone()
        }
    });
    let attachments = card.select(|card| card.attachments.clone());
    let chips = Rc::clone(theme_reader);
    Padding::all(
        12.0,
        Stack::vertical()
            .spacing(6.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Flex::horizontal()
                    .gap(6.0)
                    .align_items(Alignment::Start)
                    .with_item(
                        demo_label(theme_reader, "", DemoTextRole::Body, DemoTextColor::Text)
                            .text_from(title),
                        FlexItem::flex(1.0),
                    )
                    .with_child(card_menu(page, card)),
            )
            .with_child(
                demo_label(
                    theme_reader,
                    "",
                    DemoTextRole::Metadata,
                    DemoTextColor::Muted,
                )
                .text_from(note),
            )
            .with_child(RebuildOnChange::new_observable(
                attachments,
                move |attachments| WidgetPod::new(attachment_chips(&chips, attachments)),
            )),
    )
}

fn attachment_chips(theme_reader: &DevThemeReader, attachments: &[Asset]) -> Flex {
    let mut chips = Flex::horizontal().gap(6.0).wrap(FlexWrap::Wrap);
    for asset in attachments {
        let color = Rc::clone(theme_reader);
        chips.push(
            Surface::field(
                Stack::horizontal()
                    .spacing(4.0)
                    .alignment(Alignment::Center)
                    .with_child(
                        Icon::new(asset_icon(asset.kind))
                            .size(12.0)
                            .color_when(move || color().palette.text_muted),
                    )
                    .with_child(demo_label(
                        theme_reader,
                        asset.name,
                        DemoTextRole::Metadata,
                        DemoTextColor::Text,
                    )),
            )
            .theme_when(clone_dev_theme_reader(theme_reader))
            .padding(Insets {
                left: 6.0,
                top: 2.0,
                right: 6.0,
                bottom: 2.0,
            }),
        );
    }
    chips
}

/// The "Move to…" menu on each card.
fn card_menu(page: &Page, card: &Signal<Card>) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let id = card.get().id;
    let title = card.get().title;
    let board = page.board.clone();
    let actions = page.clone();
    ContextMenu::new(
        format!("Move “{title}”"),
        IconButton::new(IconGlyph::MoreHorizontal, format!("Move “{title}” to…"))
            .theme_when(clone_dev_theme_reader(theme_reader))
            .size(24.0)
            .icon_size(14.0),
    )
    .theme_when(clone_dev_theme_reader(theme_reader))
    .activation_button(PointerButton::Primary)
    .items_when(move || {
        let current = board.get().find(id).map(|(column, _)| column);
        let mut items: Vec<MenuItem> = Column::ALL
            .into_iter()
            .map(|column| {
                let item = MenuItem::new(format!("Move to {}", column.name()));
                if Some(column) == current {
                    item.disabled()
                } else {
                    item
                }
            })
            .collect();
        items.push(MenuItem::new("Move up").separator_before());
        items.push(MenuItem::new("Move down"));
        items.push(MenuItem::new("Delete").separator_before().destructive());
        items
    })
    .on_activate(move |index, _| match index {
        0..=2 => move_card_to(&actions, id, Column::ALL[index]),
        3 => nudge_card(&actions, id, Nudge::Up),
        4 => nudge_card(&actions, id, Nudge::Down),
        _ => delete_card(&actions, id),
    })
}

/// The zone below a column's cards: cards dropped here go last, and a
/// snippet dropped here becomes a new card.
fn column_tail(page: &Page, column: Column) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let hover = Signal::new(DropHover::Idle);
    let label = hover.select(|hover| {
        match hover {
            DropHover::Accepting(_) => "Release to drop here",
            DropHover::Refusing => "Assets attach to cards",
            DropHover::Idle => "Drop cards or snippets here",
        }
        .to_string()
    });
    let hover_page = page.clone();
    let hover_state = hover.clone();
    let drop_page = page.clone();
    DropTarget::new(
        ZoneFrame::new(
            theme_reader,
            format!("End of {}", column.name()),
            &hover,
            Align::center(Padding::all(
                8.0,
                demo_label(
                    theme_reader,
                    "",
                    DemoTextRole::Metadata,
                    DemoTextColor::Muted,
                )
                .text_from(label),
            )),
        )
        .min_height(48.0),
    )
    .scope(page.scope.clone())
    .accept(|drag| match drag.payload.custom_kind() {
        Some(CARD_KIND) => drag.preferred_effect(),
        _ if drag.payload.as_text().is_some() => DropEffect::Copy,
        _ => DropEffect::None,
    })
    .on_hover_state(move |state| {
        hover_state.set(state);
        hover_page.hovered(&format!("end of {}", column.name()), state);
    })
    .on_drop(move |_, drag| {
        let effect = effect_name(drag.accepted_effect);
        if let Some(dragged) = drag.payload.custom_data::<CardRef>() {
            let copy = drag.accepted_effect == DropEffect::Copy;
            let title = drop_page.title(dragged.id);
            drop_page.board.update(|board| {
                board.append(dragged.id, column, copy);
            });
            drop_page.log_drag(format!(
                "drop    “{title}” at the end of {} · {effect}",
                column.name()
            ));
        } else if let Some(text) = drag.payload.as_text() {
            drop_page.board.update(|board| {
                board.add_card(column, text);
            });
            drop_page.log_drag(format!(
                "drop    “{text}” as a new card in {} · {effect}",
                column.name()
            ));
        }
    })
}

fn zones(page: &Page) -> impl Widget + use<> {
    let item = || FlexItem::new().grow(1.0).basis(240.0).min_width(220.0);
    Flex::horizontal()
        .gap(16.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Stretch)
        .with_item(trash(page), item())
        .with_item(text_field(page), item())
        .with_item(locked_palette(page), item())
        .with_item(desktop_files(page), item())
}

/// A zone's heading: an icon and a title.
fn zone_heading(
    theme_reader: &DevThemeReader,
    icon: IconGlyph,
    title: &str,
) -> impl Widget + use<> {
    let color = Rc::clone(theme_reader);
    Stack::horizontal()
        .spacing(8.0)
        .alignment(Alignment::Center)
        .with_child(
            Icon::new(icon)
                .size(16.0)
                .color_when(move || color().palette.text),
        )
        .with_child(demo_label(
            theme_reader,
            title,
            DemoTextRole::CardTitle,
            DemoTextColor::Text,
        ))
}

fn trash(page: &Page) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let hover = Signal::new(DropHover::Idle);
    let holds = page.board.select(|board| match board.deleted() {
        Some(card) => format!("Holds “{}”", card.title),
        None => "Drop a card to delete it".to_string(),
    });
    let can_undo = page.board.clone();
    let undo = page.clone();
    let hover_page = page.clone();
    let hover_state = hover.clone();
    let drop_page = page.clone();
    DropTarget::new(
        ZoneFrame::new(
            theme_reader,
            TRASH_NAME,
            &hover,
            Padding::all(
                14.0,
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Start)
                    .with_child(zone_heading(theme_reader, IconGlyph::Trash, TRASH_NAME))
                    .with_child(
                        demo_label(
                            theme_reader,
                            "",
                            DemoTextRole::Metadata,
                            DemoTextColor::Muted,
                        )
                        .text_from(holds),
                    )
                    .with_child(
                        Button::new(UNDO_LABEL)
                            .theme_when(clone_dev_theme_reader(theme_reader))
                            .enabled_when(move || can_undo.get().deleted().is_some())
                            .on_press(move || {
                                let mut restored = None;
                                undo.board.update(|board| restored = board.undo_delete());
                                if let Some(id) = restored {
                                    undo.log(format!("undo    “{}” is back", undo.title(id)));
                                }
                            }),
                    ),
            ),
        )
        .min_height(140.0),
    )
    .scope(page.scope.clone())
    .accept(|drag| {
        if drag.payload.custom_kind() == Some(CARD_KIND) {
            DropEffect::Move
        } else {
            DropEffect::None
        }
    })
    .on_hover_state(move |state| {
        hover_state.set(state);
        hover_page.hovered("the trash", state);
    })
    .on_drop(move |_, drag| {
        if let Some(dragged) = drag.payload.custom_data::<CardRef>() {
            let title = drop_page.title(dragged.id);
            drop_page.board.update(|board| {
                board.delete(dragged.id);
            });
            drop_page.log_drag(format!("drop    “{title}” in the trash · move"));
        }
    })
}

fn text_field(page: &Page) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let hover = Signal::new(DropHover::Idle);
    let contents = page.field_text.select(|texts| {
        if texts.is_empty() {
            "Snippets land here; nothing else does.".to_string()
        } else {
            texts.join(" · ")
        }
    });
    let status = hover.select(|hover| {
        match hover {
            DropHover::Accepting(_) => "Release to paste",
            DropHover::Refusing => "Only text can go here",
            DropHover::Idle => "Text only",
        }
        .to_string()
    });
    let hover_page = page.clone();
    let hover_state = hover.clone();
    let drop_page = page.clone();
    DropTarget::new(
        ZoneFrame::new(
            theme_reader,
            TEXT_FIELD_NAME,
            &hover,
            Padding::all(
                14.0,
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Stretch)
                    .with_child(zone_heading(theme_reader, IconGlyph::Type, "Text field"))
                    .with_child(
                        demo_label(
                            theme_reader,
                            "",
                            DemoTextRole::Metadata,
                            DemoTextColor::Muted,
                        )
                        .semantic_name(format!("{TEXT_FIELD_NAME} status"))
                        .text_from(status),
                    )
                    .with_child(
                        demo_label(theme_reader, "", DemoTextRole::Body, DemoTextColor::Text)
                            .semantic_name(format!("{TEXT_FIELD_NAME} contents"))
                            .text_from(contents),
                    ),
            ),
        )
        .min_height(140.0),
    )
    .scope(page.scope.clone())
    .accept(|drag| {
        if drag.payload.as_text().is_some() {
            DropEffect::Copy
        } else {
            DropEffect::None
        }
    })
    .on_hover_state(move |state| {
        hover_state.set(state);
        hover_page.hovered("the text field", state);
    })
    .on_drop(move |_, drag| {
        if let Some(text) = drag.payload.as_text() {
            let text = text.to_string();
            drop_page
                .field_text
                .update(|texts| texts.push(text.clone()));
            drop_page.log_drag(format!("drop    “{text}” in the text field · copy"));
        }
    })
}

/// A palette in a scope of its own: it can only be dropped in its well, and
/// drags from the board pass over the well untouched.
fn locked_palette(page: &Page) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let scope = DragDropScope::new();
    let hover = Signal::new(DropHover::Idle);
    let held = Signal::new(false);
    let well_text = held.select(|held| {
        if *held {
            "Holds the brand palette".to_string()
        } else {
            "Drop the palette here".to_string()
        }
    });
    let start = page.clone();
    let end = page.clone();
    let hover_page = page.clone();
    let hover_state = hover.clone();
    let drop_page = page.clone();
    let drop_held = held.clone();
    let swatch = Draggable::new(
        Surface::field(
            Stack::horizontal()
                .spacing(4.0)
                .alignment(Alignment::Center)
                .with_child(swatch_dot(theme_reader, DecorativeHue::Violet))
                .with_child(swatch_dot(theme_reader, DecorativeHue::Magenta))
                .with_child(swatch_dot(theme_reader, DecorativeHue::Orange))
                .with_child(swatch_dot(theme_reader, DecorativeHue::Amber))
                .with_child(Padding::all(
                    4.0,
                    demo_label(
                        theme_reader,
                        LOCKED_SWATCH_NAME,
                        DemoTextRole::Metadata,
                        DemoTextColor::Text,
                    ),
                )),
        )
        .name(LOCKED_SWATCH_NAME)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .padding(Insets::all(8.0)),
    )
    .scope(scope.clone())
    .payload(|| DragPayload::custom(SWATCH_KIND, ()))
    .effect(DropEffect::Copy)
    .preview_label(LOCKED_SWATCH_NAME)
    .on_drag_start(move |_, preview| start.started(preview, "brand palette, locked scope"))
    .on_drag_end(move |_, drag| end.ended(drag));
    let well = DropTarget::new(
        ZoneFrame::new(
            theme_reader,
            LOCKED_WELL_NAME,
            &hover,
            Align::center(Padding::all(
                10.0,
                demo_label(
                    theme_reader,
                    "",
                    DemoTextRole::Metadata,
                    DemoTextColor::Muted,
                )
                .text_from(well_text),
            )),
        )
        .min_height(52.0),
    )
    .scope(scope.clone())
    .accept(|drag| {
        if drag.payload.custom_kind() == Some(SWATCH_KIND) {
            DropEffect::Copy
        } else {
            DropEffect::None
        }
    })
    .on_hover_state(move |state| {
        hover_state.set(state);
        hover_page.hovered("the palette well", state);
    })
    .on_drop(move |_, _| {
        drop_held.set(true);
        drop_page.log_drag("drop    brand palette in its well · copy");
    });

    DragDropHost::new(
        scope,
        Surface::panel(
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(zone_heading(
                    theme_reader,
                    IconGlyph::Lock,
                    "Locked palette",
                ))
                .with_child(demo_label(
                    theme_reader,
                    "Its own scope: the palette can't leave, and board drags don't get in.",
                    DemoTextRole::Metadata,
                    DemoTextColor::Muted,
                ))
                .with_child(swatch)
                .with_child(well),
        )
        .theme_when(clone_dev_theme_reader(theme_reader))
        .padding(Insets::all(14.0))
        .fill_width(),
    )
    .theme_when(clone_dev_theme_reader(theme_reader))
}

fn swatch_dot(theme_reader: &DevThemeReader, hue: DecorativeHue) -> impl Widget + use<> {
    Background::new(
        theme_reader().decorative.get(hue).solid,
        SizedBox::new().width(14.0).height(14.0),
    )
    .brush_when(dev_theme_color(theme_reader, move |theme| {
        theme.decorative.get(hue).solid
    }))
}

fn desktop_files(page: &Page) -> impl Widget + use<> {
    let theme_reader = &page.theme_reader;
    let note = if cfg!(target_arch = "wasm32") {
        "Dropping files from your desktop works in the desktop app."
    } else {
        "Drop files from your desktop anywhere on this page; each becomes a card in To do."
    };
    ZoneFrame::new(
        theme_reader,
        FILES_NAME,
        &page.files_hover,
        Padding::all(
            14.0,
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(zone_heading(theme_reader, IconGlyph::File, FILES_NAME))
                .with_child(demo_label(
                    theme_reader,
                    note,
                    DemoTextRole::Metadata,
                    DemoTextColor::Muted,
                )),
        ),
    )
    .min_height(140.0)
}
