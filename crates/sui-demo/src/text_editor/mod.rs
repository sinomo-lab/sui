//! The Text editor page: one editor with documents to open, wrap and
//! direction toggles, highlighting that follows edits, and an inspector
//! showing the caret, the selection, the input method's composition, and the
//! lines on screen.

#![forbid(unsafe_code)]

mod documents;
#[cfg(test)]
mod tests;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use sui::Application;
use sui::prelude::*;
use sui::{
    Command, GridTrack, LayerOptions, Rect, TextDirection, TextStyle, TextSurface,
    TextSurfaceStatus, TextWrap, WidgetPodMutVisitor, WidgetPodVisitor,
};

use crate::app::{DevThemeReader, clone_dev_theme_reader};
use crate::demo_support::*;
use crate::live_performance::LivePerformanceRoot;
use crate::settings::controls::labeled_control;
#[cfg(test)]
pub(crate) use documents::LARGE_LINES;
use documents::{DOCUMENTS, Document, highlight};

pub const TEXT_EDITOR_VIEW_TITLE: &str = "SUI Text Editor";
pub const TEXT_EDITOR_NAME: &str = "Text editor document";
pub(crate) const DOCUMENT_NAME: &str = "Document";
pub(crate) const WRAP_LABEL: &str = "Wrap lines";
pub(crate) const DIRECTION_NAME: &str = "Direction";
pub(crate) const CARET_NAME: &str = "Caret";
pub(crate) const SELECTION_NAME: &str = "Selection";
pub(crate) const COMPOSITION_NAME: &str = "Input method";
pub(crate) const ON_SCREEN_NAME: &str = "On screen";
pub(crate) const SIZE_NAME: &str = "Size";

const DIRECTIONS: [(TextDirection, &str); 3] = [
    (TextDirection::Auto, "Automatic"),
    (TextDirection::LeftToRight, "Left to right"),
    (TextDirection::RightToLeft, "Right to left"),
];
const EDITOR_HEIGHT: f32 = 520.0;
const CONTROL_WIDTH: f32 = 260.0;

/// What the toolbar chose, shared with the editor.
#[derive(Clone)]
struct Choices {
    document: Signal<usize>,
    wrap: Signal<bool>,
    direction: Signal<usize>,
}

pub fn build_text_editor_surface_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let choices = Choices {
        document: Signal::named(DOCUMENT_NAME, 0),
        wrap: Signal::named(WRAP_LABEL, Document::Code.wraps()),
        direction: Signal::named(DIRECTION_NAME, 0),
    };
    let status = Signal::named("Text editor status", TextSurfaceStatus::default());
    Padding::all(
        24.0,
        Stack::vertical()
            .spacing(16.0)
            .alignment(Alignment::Stretch)
            .with_child(toolbar(&theme_reader, &choices))
            .with_child(
                SizedBox::new()
                    .height(EDITOR_HEIGHT)
                    .with_child(EditorHost::new(&theme_reader, choices, status.clone())),
            )
            .with_child(Inspector::new(&theme_reader, status)),
    )
}

pub fn build_text_editor_application() -> Application {
    App::new()
        .window(Window::new(TEXT_EDITOR_VIEW_TITLE).root(LivePerformanceRoot::new(
            TEXT_EDITOR_VIEW_TITLE,
            "An editor with documents to open, highlighting that follows edits, and an inspector for the caret, selection, and input method.",
            build_text_editor_surface_with_theme(default_theme_reader()),
        )))
        .into_application()
}

fn toolbar(theme_reader: &DevThemeReader, choices: &Choices) -> impl Widget + use<> {
    let document = {
        let read = choices.document.clone();
        let write = choices.document.clone();
        let wrap = choices.wrap.clone();
        Select::new(DOCUMENT_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .options(DOCUMENTS.map(Document::name))
            .selected_when(move || Some(read.get()))
            .on_change(move |index, _| {
                if let Some(document) = DOCUMENTS.get(index) {
                    wrap.set(document.wraps());
                    write.set(index);
                }
            })
    };
    let wrap = {
        let read = choices.wrap.clone();
        let write = choices.wrap.clone();
        Switch::new(WRAP_LABEL)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .on_when(move || read.get())
            .on_toggle(move |on| {
                write.set(on);
            })
    };
    let direction = {
        let read = choices.direction.clone();
        let write = choices.direction.clone();
        Select::new(DIRECTION_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .options(DIRECTIONS.map(|(_, name)| name))
            .selected_when(move || Some(read.get()))
            .on_change(move |index, _| {
                write.set(index);
            })
    };
    Stack::horizontal()
        .spacing(24.0)
        .alignment(Alignment::End)
        .with_child(labeled_control(
            theme_reader,
            DOCUMENT_NAME,
            CONTROL_WIDTH,
            document,
        ))
        .with_child(labeled_control(
            theme_reader,
            DIRECTION_NAME,
            CONTROL_WIDTH,
            direction,
        ))
        .with_child(wrap)
}

/// The editor, opening the document the toolbar chose and highlighting code
/// as it changes. It is the editor itself as far as the window can tell: it
/// forwards every widget call to the [`TextSurface`] it holds.
struct EditorHost {
    theme_reader: DevThemeReader,
    surface: TextSurface,
    choices: Choices,
    /// What the editor shows now.
    document: Option<Document>,
    wrap: bool,
    direction: TextDirection,
    /// Set by the editor when its text changes.
    edited: Rc<Cell<bool>>,
    /// The theme the highlighting was made for.
    highlighted_for: Option<Color>,
}

impl EditorHost {
    fn new(
        theme_reader: &DevThemeReader,
        choices: Choices,
        status: Signal<TextSurfaceStatus>,
    ) -> Self {
        let edited = Rc::new(Cell::new(false));
        let on_change = Rc::clone(&edited);
        let style_reader = Rc::clone(theme_reader);
        Self {
            theme_reader: Rc::clone(theme_reader),
            surface: TextSurface::new(TEXT_EDITOR_NAME)
                .theme_when(clone_dev_theme_reader(theme_reader))
                .text_style_when(move |_| text_style(style_reader()))
                .status(status)
                .on_change(move |_| on_change.set(true)),
            choices,
            document: None,
            wrap: false,
            direction: TextDirection::Auto,
            edited,
            highlighted_for: None,
        }
    }

    /// Open the chosen document and apply the chosen wrap and direction.
    fn follow_choices(&mut self, ctx: &MeasureCtx) {
        let document = DOCUMENTS
            .get(ctx.observe(&self.choices.document))
            .copied()
            .unwrap_or(Document::Code);
        if self.document != Some(document) {
            self.document = Some(document);
            self.surface.set_value(document.text());
            self.surface.set_selection(0, 0);
            self.highlighted_for = None;
        }
        let wrap = ctx.observe(&self.choices.wrap);
        if self.wrap != wrap {
            self.wrap = wrap;
            self.surface.set_wrap(if wrap {
                TextWrap::Word
            } else {
                TextWrap::NoWrap
            });
        }
        let direction = DIRECTIONS
            .get(ctx.observe(&self.choices.direction))
            .map_or(TextDirection::Auto, |(direction, _)| *direction);
        if self.direction != direction {
            self.direction = direction;
            self.surface.set_direction(direction);
        }
        let theme = (self.theme_reader)();
        if self.highlighted_for != Some(theme.palette.accent) {
            self.highlight(theme);
        }
    }

    fn highlight(&mut self, theme: DefaultTheme) {
        self.highlighted_for = Some(theme.palette.accent);
        let spans = if self.document.is_some_and(Document::highlights) {
            highlight(self.surface.current_value(), &text_style(theme), theme)
        } else {
            Vec::new()
        };
        self.surface.set_style_spans(spans);
    }
}

fn text_style(theme: DefaultTheme) -> TextStyle {
    theme_mono_text_style(theme, theme.text.sm, theme.palette.text)
}

impl Widget for EditorHost {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.surface.event(ctx, event);
        if self.edited.replace(false) && self.document.is_some_and(Document::highlights) {
            self.highlight((self.theme_reader)());
            ctx.request_paint();
        }
    }

    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        self.surface.command(ctx, command);
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.follow_choices(ctx);
        self.surface.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.surface.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.surface.paint(ctx);
    }

    fn layer_options(&self) -> LayerOptions {
        self.surface.layer_options()
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.surface.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.surface.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.surface.focus_changed(ctx, focused);
    }
}

/// What the inspector shows, as of the editor's last update.
#[derive(Default)]
struct Readout {
    caret: String,
    selection: String,
    composition: String,
    on_screen: String,
    size: String,
}

impl Readout {
    fn of(status: &TextSurfaceStatus) -> Self {
        Self {
            caret: format!("Line {}, column {}", status.caret_line, status.caret_column),
            selection: if status.selection.is_empty() {
                "Nothing selected".to_string()
            } else {
                format!(
                    "{} characters, bytes {}–{}",
                    grouped(status.selected_chars),
                    grouped(status.selection.start),
                    grouped(status.selection.end)
                )
            },
            composition: status.composition.as_ref().map_or_else(
                || "Not composing".to_string(),
                |text| format!("Composing \u{201c}{text}\u{201d}"),
            ),
            on_screen: if status.visible_lines.is_empty() {
                format!("No lines of {}", grouped(status.line_count))
            } else {
                format!(
                    "Lines {}–{} of {}",
                    grouped(status.visible_lines.start + 1),
                    grouped(status.visible_lines.end),
                    grouped(status.line_count)
                )
            },
            size: format!("{} bytes", grouped(status.text_len)),
        }
    }
}

/// `value` with thousands separated by commas.
fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// Where editing stands: the caret, the selection, the input method's
/// composition, the lines on screen, and the document's size.
struct Inspector {
    status: Signal<TextSurfaceStatus>,
    readout: Rc<RefCell<Readout>>,
    rows: SingleChild,
}

impl Inspector {
    fn new(theme_reader: &DevThemeReader, status: Signal<TextSurfaceStatus>) -> Self {
        let readout = Rc::new(RefCell::new(Readout::default()));
        let row = |label: &'static str, value: fn(&Readout) -> String| {
            let readout = Rc::clone(&readout);
            DetailRow::new(label, "")
                .theme_when(clone_dev_theme_reader(theme_reader))
                .value_when(move || value(&readout.borrow()))
        };
        let rows = Grid::new([
            GridTrack::Fraction(1.0),
            GridTrack::Fraction(1.2),
            GridTrack::Fraction(1.0),
            GridTrack::Fraction(1.0),
            GridTrack::Fraction(0.8),
        ])
        .rows([GridTrack::Auto])
        .column_gap(16.0)
        .with_child(row(CARET_NAME, |readout| readout.caret.clone()))
        .with_child(row(SELECTION_NAME, |readout| readout.selection.clone()))
        .with_child(row(COMPOSITION_NAME, |readout| readout.composition.clone()))
        .with_child(row(ON_SCREEN_NAME, |readout| readout.on_screen.clone()))
        .with_child(row(SIZE_NAME, |readout| readout.size.clone()));
        Self {
            status,
            readout,
            rows: SingleChild::new(rows),
        }
    }
}

impl Widget for Inspector {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        *self.readout.borrow_mut() = Readout::of(&ctx.observe(&self.status));
        self.rows.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.rows.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.rows.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.rows.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.rows.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.rows.visit_children_mut(visitor);
    }
}
