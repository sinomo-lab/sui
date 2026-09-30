//! The board's data: cards in three columns, and the moves the page makes
//! to them.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Column {
    Todo,
    Doing,
    Done,
}

impl Column {
    pub(super) const ALL: [Self; 3] = [Self::Todo, Self::Doing, Self::Done];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Todo => "To do",
            Self::Doing => "Doing",
            Self::Done => "Done",
        }
    }

    pub(super) fn index(self) -> usize {
        match self {
            Self::Todo => 0,
            Self::Doing => 1,
            Self::Done => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AssetKind {
    Image,
    Palette,
    Scene,
}

impl AssetKind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::Palette => "Palette",
            Self::Scene => "Scene",
        }
    }
}

/// Something on the shelf that can be attached to a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Asset {
    pub(super) name: &'static str,
    pub(super) kind: AssetKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Card {
    pub(super) id: u32,
    pub(super) title: String,
    pub(super) note: String,
    pub(super) attachments: Vec<Asset>,
}

impl Card {
    fn new(id: u32, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            note: String::new(),
            attachments: Vec::new(),
        }
    }
}

/// A card the trash holds until the next deletion, for undo.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Deleted {
    card: Card,
    column: Column,
    index: usize,
}

/// A keyboard move of one card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Nudge {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Board {
    columns: [Vec<Card>; 3],
    next_id: u32,
    deleted: Option<Deleted>,
}

impl Board {
    pub(super) fn new() -> Self {
        let mut board = Self {
            columns: [Vec::new(), Vec::new(), Vec::new()],
            next_id: 1,
            deleted: None,
        };
        let notes = board.add_card(Column::Todo, "Write launch notes");
        board.add_note(notes, "Outline is in the shared doc");
        board.add_card(Column::Todo, "Record the demo video");
        let preview = board.add_card(Column::Doing, "Polish the drag preview");
        board.attach(
            preview,
            Asset {
                name: "Hero.png",
                kind: AssetKind::Image,
            },
        );
        board.add_card(Column::Doing, "Review keyboard moves");
        board.add_card(Column::Done, "Sketch the board");
        board
    }

    pub(super) fn column(&self, column: Column) -> &[Card] {
        &self.columns[column.index()]
    }

    pub(super) fn find(&self, id: u32) -> Option<(Column, usize)> {
        Column::ALL.into_iter().find_map(|column| {
            self.column(column)
                .iter()
                .position(|card| card.id == id)
                .map(|index| (column, index))
        })
    }

    pub(super) fn card(&self, id: u32) -> Option<&Card> {
        let (column, index) = self.find(id)?;
        self.columns[column.index()].get(index)
    }

    fn card_mut(&mut self, id: u32) -> Option<&mut Card> {
        let (column, index) = self.find(id)?;
        self.columns[column.index()].get_mut(index)
    }

    /// Add a card called `title` at the end of `column`.
    pub(super) fn add_card(&mut self, column: Column, title: impl Into<String>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.columns[column.index()].push(Card::new(id, title));
        id
    }

    /// Put card `id` where a card now at `index` of `column` is, or at the
    /// end past the last one, moving it or placing a copy. Returns the id of
    /// the card placed.
    pub(super) fn place(
        &mut self,
        id: u32,
        column: Column,
        index: usize,
        copy: bool,
    ) -> Option<u32> {
        let (from, from_index) = self.find(id)?;
        let mut index = index.min(self.column(column).len());
        let card = if copy {
            let mut card = self.columns[from.index()][from_index].clone();
            card.id = self.next_id;
            self.next_id += 1;
            card.title.push_str(" (copy)");
            card
        } else {
            let card = self.columns[from.index()].remove(from_index);
            if from == column && from_index < index {
                index -= 1;
            }
            card
        };
        let placed = card.id;
        self.columns[column.index()].insert(index, card);
        Some(placed)
    }

    /// Put card `id` just before card `before`.
    pub(super) fn place_before(&mut self, id: u32, before: u32, copy: bool) -> Option<u32> {
        if id == before && !copy {
            return Some(id);
        }
        let (column, index) = self.find(before)?;
        self.place(id, column, index, copy)
    }

    /// Put card `id` at the end of `column`.
    pub(super) fn append(&mut self, id: u32, column: Column, copy: bool) -> Option<u32> {
        self.place(id, column, usize::MAX, copy)
    }

    /// Attach `asset` to card `id`, unless it already is.
    pub(super) fn attach(&mut self, id: u32, asset: Asset) -> bool {
        let Some(card) = self.card_mut(id) else {
            return false;
        };
        if card.attachments.contains(&asset) {
            return false;
        }
        card.attachments.push(asset);
        true
    }

    /// Add `text` to the end of card `id`'s note.
    pub(super) fn add_note(&mut self, id: u32, text: &str) -> bool {
        let Some(card) = self.card_mut(id) else {
            return false;
        };
        if !card.note.is_empty() {
            card.note.push_str(" · ");
        }
        card.note.push_str(text);
        true
    }

    /// Delete card `id`, keeping it for [`Self::undo_delete`].
    pub(super) fn delete(&mut self, id: u32) -> Option<&Card> {
        let (column, index) = self.find(id)?;
        let card = self.columns[column.index()].remove(index);
        self.deleted = Some(Deleted {
            card,
            column,
            index,
        });
        self.deleted.as_ref().map(|deleted| &deleted.card)
    }

    pub(super) fn deleted(&self) -> Option<&Card> {
        self.deleted.as_ref().map(|deleted| &deleted.card)
    }

    /// Put the last deleted card back where it was.
    pub(super) fn undo_delete(&mut self) -> Option<u32> {
        let Deleted {
            card,
            column,
            index,
        } = self.deleted.take()?;
        let id = card.id;
        let cards = &mut self.columns[column.index()];
        cards.insert(index.min(cards.len()), card);
        Some(id)
    }

    /// Move card `id` a step: up or down its column, or to the column on
    /// either side at the same height. Returns where it ends up, or `None`
    /// when it cannot go that way.
    pub(super) fn nudge(&mut self, id: u32, nudge: Nudge) -> Option<(Column, usize)> {
        let (column, index) = self.find(id)?;
        let (to, to_index) = match nudge {
            Nudge::Up => (column, index.checked_sub(1)?),
            Nudge::Down if index + 1 < self.column(column).len() => (column, index + 1),
            Nudge::Down => return None,
            Nudge::Left => (*Column::ALL.get(column.index().checked_sub(1)?)?, index),
            Nudge::Right => (*Column::ALL.get(column.index() + 1)?, index),
        };
        let card = self.columns[column.index()].remove(index);
        let cards = &mut self.columns[to.index()];
        let to_index = to_index.min(cards.len());
        cards.insert(to_index, card);
        Some((to, to_index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(board: &Board, column: Column) -> Vec<&str> {
        board
            .column(column)
            .iter()
            .map(|card| card.title.as_str())
            .collect()
    }

    fn id_of(board: &Board, title: &str) -> u32 {
        Column::ALL
            .into_iter()
            .flat_map(|column| board.column(column))
            .find(|card| card.title == title)
            .map(|card| card.id)
            .unwrap()
    }

    #[test]
    fn moves_land_before_the_card_dropped_on() {
        let mut board = Board::new();
        let video = id_of(&board, "Record the demo video");
        let review = id_of(&board, "Review keyboard moves");
        board.place_before(video, review, false);
        assert_eq!(titles(&board, Column::Todo), ["Write launch notes"]);
        assert_eq!(
            titles(&board, Column::Doing),
            [
                "Polish the drag preview",
                "Record the demo video",
                "Review keyboard moves"
            ]
        );

        // Down its own column: before a card further down.
        let polish = id_of(&board, "Polish the drag preview");
        board.place_before(polish, review, false);
        assert_eq!(
            titles(&board, Column::Doing),
            [
                "Record the demo video",
                "Polish the drag preview",
                "Review keyboard moves"
            ]
        );
        board.append(polish, Column::Doing, false);
        assert_eq!(
            titles(&board, Column::Doing).last(),
            Some(&"Polish the drag preview")
        );
    }

    #[test]
    fn copies_leave_the_original_and_carry_its_attachments() {
        let mut board = Board::new();
        let polish = id_of(&board, "Polish the drag preview");
        let copy = board.append(polish, Column::Done, true).unwrap();
        assert_ne!(copy, polish);
        assert_eq!(
            board.find(polish).map(|(column, _)| column),
            Some(Column::Doing)
        );
        let copied = board.card(copy).unwrap();
        assert_eq!(copied.title, "Polish the drag preview (copy)");
        assert_eq!(copied.attachments.len(), 1);
    }

    #[test]
    fn attachments_notes_and_deletion_undo() {
        let mut board = Board::new();
        let video = id_of(&board, "Record the demo video");
        let palette = Asset {
            name: "Dusk",
            kind: AssetKind::Palette,
        };
        assert!(board.attach(video, palette));
        assert!(!board.attach(video, palette), "attached once");
        board.add_note(video, "Ship it Friday");
        board.add_note(video, "Needs review");
        assert_eq!(
            board.card(video).unwrap().note,
            "Ship it Friday · Needs review"
        );

        assert_eq!(board.delete(video).map(|card| card.id), Some(video));
        assert_eq!(board.find(video), None);
        assert_eq!(board.undo_delete(), Some(video));
        assert_eq!(board.find(video), Some((Column::Todo, 1)));
        assert_eq!(board.undo_delete(), None);
    }

    #[test]
    fn nudges_move_within_and_across_columns() {
        let mut board = Board::new();
        let video = id_of(&board, "Record the demo video");
        assert_eq!(board.nudge(video, Nudge::Up), Some((Column::Todo, 0)));
        assert_eq!(board.nudge(video, Nudge::Up), None, "already first");
        assert_eq!(
            board.nudge(video, Nudge::Left),
            None,
            "no column to the left"
        );
        assert_eq!(board.nudge(video, Nudge::Right), Some((Column::Doing, 0)));
        assert_eq!(board.nudge(video, Nudge::Right), Some((Column::Done, 0)));
        assert_eq!(board.nudge(video, Nudge::Down), Some((Column::Done, 1)));
        assert_eq!(board.nudge(video, Nudge::Down), None, "already last");
    }
}
