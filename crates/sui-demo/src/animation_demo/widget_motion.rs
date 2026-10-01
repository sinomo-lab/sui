//! Built-in widget motion: real widgets, each card naming the theme token
//! that times it. They follow the page's motion controls like every widget
//! in the app.

use std::rc::Rc;

use sui::prelude::*;

use super::millis;
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style_when};

const CARD_WIDTH: f32 = 280.0;

/// What a card says about its motion, from the live theme.
pub(super) type TokenLine = fn(&DefaultTheme) -> String;

pub(super) fn gallery(theme_reader: DevThemeReader) -> impl Widget {
    let theme = || clone_dev_theme_reader(&theme_reader);
    let tile = FlexItem::new()
        .basis_gap_aware_fraction(1.0 / 3.0)
        .min_width(CARD_WIDTH);
    Flex::horizontal()
        .gap(14.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Stretch)
        .with_item(
            card(
                &theme_reader,
                "Buttons",
                |theme| {
                    format!(
                        "Hover and press · {}, standard",
                        millis(theme.motion.hover_duration())
                    )
                },
                Flex::horizontal()
                    .gap(8.0)
                    .with_child(Button::new("Secondary").theme_when(theme()))
                    .with_child(Button::primary("Primary").theme_when(theme())),
            ),
            tile,
        )
        .with_item(
            card(
                &theme_reader,
                "Choices",
                |theme| {
                    format!(
                        "Toggle · {}, emphasized",
                        millis(theme.motion.toggle_duration())
                    )
                },
                Stack::vertical()
                    .spacing(8.0)
                    .alignment(Alignment::Start)
                    .with_child(Switch::new("Wi-Fi").checked(true).theme_when(theme()))
                    .with_child(
                        Checkbox::new("Remember me")
                            .checked(true)
                            .theme_when(theme()),
                    ),
            ),
            tile,
        )
        .with_item(
            card(
                &theme_reader,
                "Segmented control",
                |theme| {
                    format!(
                        "Indicator slides · {}; jumps with reduced motion",
                        millis(theme.motion.tab_switch_duration())
                    )
                },
                SegmentedControl::new("Range")
                    .segments(["Day", "Week", "Month"])
                    .theme_when(theme()),
            ),
            tile,
        )
        .with_item(
            card(
                &theme_reader,
                "Tabs",
                |theme| {
                    format!(
                        "Indicator slides · {}; jumps with reduced motion",
                        millis(theme.motion.tab_switch_duration())
                    )
                },
                TabBar::new("Sections")
                    .tabs(["Overview", "Activity", "Settings"])
                    .theme_when(theme()),
            ),
            tile,
        )
        .with_item(
            card(
                &theme_reader,
                "Tooltip",
                |theme| {
                    format!(
                        "Fades and rises {:.0} px · {}; fades in place with reduced motion",
                        theme.metrics.tooltip_reveal_offset,
                        millis(theme.motion.entrance_duration())
                    )
                },
                themed(&theme_reader, |theme| {
                    WidgetPod::new(
                        Tooltip::new(
                            "Tooltips fade in and rise into place",
                            Button::new("Hover for a tooltip").theme(theme),
                        )
                        .theme(theme),
                    )
                }),
            ),
            tile,
        )
        .with_item(
            card(
                &theme_reader,
                "Popover",
                |theme| {
                    format!(
                        "Fades, drops {:.0} px, and grows from 96% · {}",
                        theme.metrics.popover_reveal_offset,
                        millis(theme.motion.entrance_duration())
                    )
                },
                themed(&theme_reader, |theme| {
                    WidgetPod::new(
                        Popover::new(
                            "Details",
                            Button::new("Open popover").theme(theme),
                            Label::new("Popovers use the entrance token."),
                        )
                        .theme(theme),
                    )
                }),
            ),
            tile,
        )
        .with_item(
            card(
                &theme_reader,
                "Select",
                |theme| {
                    format!(
                        "Menu opens in {}; options highlight in {}",
                        millis(theme.motion.entrance_duration()),
                        millis(theme.motion.hover_duration())
                    )
                },
                SizedBox::new().width(200.0).with_child(
                    Select::new("Fruit")
                        .options(["Apple", "Banana", "Cherry"])
                        .selected(0)
                        .theme_when(theme()),
                ),
            ),
            tile,
        )
        .with_item(
            card(
                &theme_reader,
                "Text field",
                |theme| {
                    format!(
                        "Focus ring · {}, decelerate",
                        millis(theme.motion.focus_duration())
                    )
                },
                SizedBox::new().width(220.0).with_child(
                    TextInput::new("Name")
                        .value("Ada Lovelace")
                        .theme_when(theme()),
                ),
            ),
            tile,
        )
        .with_item(
            card(
                &theme_reader,
                "Slider",
                |theme| {
                    format!(
                        "Thumb hover and press · {}",
                        millis(theme.motion.hover_duration())
                    )
                },
                SizedBox::new().width(220.0).with_child(
                    Slider::new("Volume")
                        .range(0.0, 100.0)
                        .value(40.0)
                        .theme_when(theme()),
                ),
            ),
            tile,
        )
}

/// Rebuild `build` when the theme changes, for widgets that take a fixed
/// theme rather than a theme reader.
fn themed<F>(theme_reader: &DevThemeReader, build: F) -> impl Widget + use<F>
where
    F: Fn(DefaultTheme) -> WidgetPod + 'static,
{
    let key = Rc::clone(theme_reader);
    RebuildOnChange::new(move || key(), move |theme| build(*theme))
}

pub(super) fn card<W>(
    theme_reader: &DevThemeReader,
    title: &str,
    token_line: TokenLine,
    widget: W,
) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    let token_theme = Rc::clone(theme_reader);
    Surface::panel(
        Stack::vertical()
            .spacing(6.0)
            .alignment(Alignment::Stretch)
            .with_child(Label::new(title).text_style_when(demo_text_style_when(
                theme_reader,
                DemoTextRole::CardTitle,
                |theme| theme.palette.text,
            )))
            .with_child(
                Label::dynamic(token_line(&theme_reader()), move || {
                    token_line(&token_theme())
                })
                .text_style_when(demo_text_style_when(
                    theme_reader,
                    DemoTextRole::Metadata,
                    |theme| theme.palette.text_muted,
                )),
            )
            .with_child(SizedBox::new().height(8.0))
            .with_child(Align::new(Alignment::Start, Alignment::Start, widget)),
    )
    .theme_when(clone_dev_theme_reader(theme_reader))
    .padding(Insets::all(16.0))
}
