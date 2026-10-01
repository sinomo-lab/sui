//! Edit commands from a toolbar. The buttons send `TEXT_COMMAND` to the
//! editor you used last, which a focus scope around the editors remembers.
//! Clicking a button leaves focus in the editor; the buttons stay in the
//! Tab order, and an editor takes focus back when a command reaches it.

use sui::{
    FocusScopeState, Rect, Selector, TEXT_COMMAND, TextCommand, WidgetId, WidgetPodMutVisitor,
    WidgetPodVisitor, prelude::*,
};

use super::{code_panel, titled_section};
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader};
use crate::demo_support::{DemoTextColor, demo_label};

pub(super) const TITLE_NAME: &str = "Title";
pub(super) const BODY_NAME: &str = "Body";
pub(super) const ACTS_ON_NAME: &str = "Toolbar target";
pub(super) const EDIT_CODE_NAME: &str = "Edit command code";
pub(super) const TOOLS: [(&str, TextCommand); 4] = [
    ("Cut", TextCommand::Cut),
    ("Copy", TextCommand::Copy),
    ("Paste", TextCommand::Paste),
    ("Select all", TextCommand::SelectAll),
];
const EDIT_CODE: &str = "Button::new(\"Copy\")\n    .focus_on_press(false)\n    .on_press_with_ctx(move |ctx| {\n        if let Some(editor) = editors.last_focused() {\n            ctx.post_command(editor, TEXT_COMMAND, TextCommand::Copy);\n        }\n    })";

pub(super) fn section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let editors = FocusScopeState::new();
    let (title, title_id) = Identified::new(
        TextInput::new(TITLE_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .value("Quarterly review"),
    );
    let (body, body_id) = Identified::new(
        TextArea::new(BODY_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .value("Revenue grew in every region.\nHiring slowed in the second half.")
            .min_height(96.0),
    );

    let mut toolbar = Flex::horizontal()
        .gap(8.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Center);
    for (label, command) in TOOLS {
        let editors = editors.clone();
        toolbar = toolbar.with_child(
            Button::new(label)
                .theme_when(clone_dev_theme_reader(theme_reader))
                .focus_on_press(false)
                .on_press_with_ctx(move |ctx| {
                    if let Some(editor) = editors.last_focused() {
                        ctx.post_command(editor, TEXT_COMMAND, command);
                    }
                }),
        );
    }
    let acts_on = Selector::new(
        ACTS_ON_NAME,
        editors.last_focused_observable(),
        move |editor: &Option<WidgetId>| {
            match *editor {
                Some(id) if id == title_id => "Acts on: Title",
                Some(id) if id == body_id => "Acts on: Body",
                _ => "Acts on: nothing yet. Click into an editor.",
            }
            .to_string()
        },
    );
    toolbar = toolbar.with_child(
        demo_label(theme_reader, "", DemoTextRole::Body, DemoTextColor::Muted)
            .semantic_name(ACTS_ON_NAME)
            .text_from(acts_on),
    );

    titled_section(
        theme_reader,
        "Edit commands",
        "The toolbar acts on the editor you used last. Clicking a button leaves focus in the editor, so a selection survives Copy. From the keyboard, Tab reaches the buttons as usual, and the editor takes focus back when the command arrives.",
        Stack::vertical()
            .gap(12.0)
            .alignment(Alignment::Stretch)
            .with_child(toolbar)
            .with_child(
                FocusScope::new(
                    Stack::vertical()
                        .gap(10.0)
                        .alignment(Alignment::Stretch)
                        .with_child(title)
                        .with_child(body),
                )
                .state(editors),
            )
            .with_child(code_panel(
                theme_reader,
                EDIT_CODE_NAME,
                sui::Signal::new(EDIT_CODE.to_string()),
            )),
    )
}

/// A widget, with the id commands reach it by.
struct Identified {
    child: SingleChild,
}

impl Identified {
    fn new<W>(widget: W) -> (Self, WidgetId)
    where
        W: Widget + 'static,
    {
        let child = SingleChild::new(widget);
        let id = child.child().id();
        (Self { child }, id)
    }
}

impl Widget for Identified {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
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
