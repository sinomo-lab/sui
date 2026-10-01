//! The token panel: preset and actions, source colors grouped by job, role
//! overrides, and shape and type. Pressing a token opens its editor right
//! beneath it.

use std::rc::Rc;

use sui::SimpleColorPicker;
use sui::prelude::*;

use super::export::{hex, parse_hex, rust_source};
use super::state::{
    EditOrigin, MOTION_RANGE, PickerMode, Preset, RADIUS_RANGE, SPACING_RANGE, TEXT_RANGE,
    ThemeEditorState,
};
use super::token_row::TokenRow;
use super::tokens::{Token, TokenGroup};
use super::*;
use crate::app::{DemoTextRole, DevAppTheme, DevThemeReader, clone_dev_theme_reader};
use crate::demo_support::{DemoTextColor, demo_label};

pub(super) fn build_panel(
    state: ThemeEditorState,
    shell: DevThemeReader,
    app_theme: Option<DevAppTheme>,
) -> impl Widget {
    let content_state = state.clone();
    let content_shell = Rc::clone(&shell);
    Surface::sidebar(
        ScrollView::vertical(RebuildOnChange::new_observable(
            state.structure_signal(),
            move |_| {
                WidgetPod::new(panel_content(
                    content_state.clone(),
                    Rc::clone(&content_shell),
                    app_theme.clone(),
                ))
            },
        ))
        .name(THEME_EDITOR_CONTROLS_SCROLL_NAME)
        .theme_when(clone_dev_theme_reader(&shell)),
    )
    .theme_when(clone_dev_theme_reader(&shell))
    .fill()
}

fn panel_content(
    state: ThemeEditorState,
    shell: DevThemeReader,
    app_theme: Option<DevAppTheme>,
) -> impl Widget {
    let mut content = Stack::vertical()
        .spacing(26.0)
        .alignment(Alignment::Stretch)
        .with_child(header(state.clone(), Rc::clone(&shell), app_theme))
        .with_child(foundation(state.clone(), Rc::clone(&shell)));
    for group in TokenGroup::ALL {
        if group != TokenGroup::Decorative {
            content = content.with_child(token_group(group, state.clone(), Rc::clone(&shell)));
        }
    }
    Padding::all(
        16.0,
        content
            .with_child(decorative_group(state.clone(), Rc::clone(&shell)))
            .with_child(role_overrides(state.clone(), Rc::clone(&shell)))
            .with_child(shape_and_type(state, shell)),
    )
}

fn header(
    state: ThemeEditorState,
    shell: DevThemeReader,
    app_theme: Option<DevAppTheme>,
) -> impl Widget {
    let summary_shell = Rc::clone(&shell);
    let reset_state = state.clone();
    let export_state = state.clone();
    let mut actions = Flex::horizontal()
        .gap(8.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Center)
        .with_child(
            Button::new(THEME_RESET_ALL_NAME)
                .icon(IconGlyph::Restore)
                .theme_when(clone_dev_theme_reader(&shell))
                .on_press(move || reset_state.reset_all()),
        )
        .with_child(
            Button::new(THEME_COPY_RUST_NAME)
                .icon(IconGlyph::FileText)
                .theme_when(clone_dev_theme_reader(&shell))
                .on_press_with_ctx(move |ctx| {
                    ctx.set_clipboard_text(rust_source(&export_state.recipe()));
                    export_state.set_status("copied Rust source");
                }),
        );
    if let Some(app_theme) = app_theme {
        let apply_state = state.clone();
        actions = actions.with_child(
            Button::primary(THEME_USE_AS_APP_THEME_NAME)
                .icon(IconGlyph::PaintBucket)
                .theme_when(clone_dev_theme_reader(&shell))
                .on_press_with_ctx(move |ctx| {
                    app_theme.apply(ctx, apply_state.theme());
                    apply_state.set_status("applied to the app");
                }),
        );
    }
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(demo_label(
            &shell,
            "Theme tokens",
            DemoTextRole::SectionTitle,
            DemoTextColor::Text,
        ))
        .with_child(demo_label(
            &shell,
            "Edit the source colors and every widget role follows. Press a token to open its \
             editor; the preview shows each change.",
            DemoTextRole::Supporting,
            DemoTextColor::Muted,
        ))
        .with_child(RebuildOnChange::new_observable(
            state.summary_signal(),
            move |summary| {
                WidgetPod::new(demo_label(
                    &summary_shell,
                    summary.clone(),
                    DemoTextRole::Supporting,
                    DemoTextColor::Text,
                ))
            },
        ))
        .with_child(SizedBox::new().height(2.0))
        .with_child(actions)
}

fn foundation(state: ThemeEditorState, shell: DevThemeReader) -> impl Widget {
    let preset_state = state.clone();
    let size_state = state.clone();
    let size_index = match state.control_size() {
        ControlSize::Small => 0,
        ControlSize::Medium => 1,
        ControlSize::Large => 2,
    };
    group_frame(
        &shell,
        "Foundation",
        "Start from a preset, then pick a control size.",
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                PropertyRow::new(
                    "Preset",
                    Select::new(THEME_PRESET_NAME)
                        .options(Preset::ALL.map(Preset::label))
                        .selected(state.preset().index())
                        .theme_when(clone_dev_theme_reader(&shell))
                        .on_change_with_ctx(move |_, index, _| {
                            preset_state.set_preset(Preset::from_index(index));
                        }),
                )
                .theme_when(clone_dev_theme_reader(&shell))
                .stacked(),
            )
            .with_child(
                PropertyRow::new(
                    "Control size",
                    SegmentedControl::new(THEME_CONTROL_SIZE_NAME)
                        .segments(["Small", "Medium", "Large"])
                        .selected(size_index)
                        .theme_when(clone_dev_theme_reader(&shell))
                        .on_change_with_ctx(move |_, index, _| {
                            size_state.set_control_size(match index {
                                0 => ControlSize::Small,
                                2 => ControlSize::Large,
                                _ => ControlSize::Medium,
                            });
                        }),
                )
                .theme_when(clone_dev_theme_reader(&shell))
                .stacked(),
            ),
    )
}

fn group_frame<W>(
    shell: &DevThemeReader,
    title: &'static str,
    summary: &'static str,
    body: W,
) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(4.0)
        .alignment(Alignment::Stretch)
        .with_child(demo_label(
            shell,
            title,
            DemoTextRole::CardTitle,
            DemoTextColor::Text,
        ))
        .with_child(demo_label(
            shell,
            summary,
            DemoTextRole::Metadata,
            DemoTextColor::Muted,
        ))
        .with_child(SizedBox::new().height(6.0))
        .with_child(body)
}

fn token_group(group: TokenGroup, state: ThemeEditorState, shell: DevThemeReader) -> impl Widget {
    let rows = group.tokens().map(Token::Source).fold(
        Stack::vertical().spacing(2.0).alignment(Alignment::Stretch),
        |rows, token| rows.with_child(token_with_editor(token, state.clone(), Rc::clone(&shell))),
    );
    group_frame(&shell, group.title(), group.summary(), rows)
}

fn token_with_editor(token: Token, state: ThemeEditorState, shell: DevThemeReader) -> impl Widget {
    Stack::vertical()
        .spacing(0.0)
        .alignment(Alignment::Stretch)
        .with_child(TokenRow::new(token, state.clone(), Rc::clone(&shell)))
        .with_child(editor_slot(vec![token], state, shell))
}

fn decorative_group(state: ThemeEditorState, shell: DevThemeReader) -> impl Widget {
    let tokens = TokenGroup::Decorative
        .tokens()
        .map(Token::Source)
        .collect::<Vec<_>>();
    let chips = tokens.iter().fold(
        Flex::horizontal()
            .gap(6.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Center),
        |chips, token| chips.with_child(TokenRow::chip(*token, state.clone(), Rc::clone(&shell))),
    );
    group_frame(
        &shell,
        TokenGroup::Decorative.title(),
        TokenGroup::Decorative.summary(),
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(chips)
            .with_child(editor_slot(tokens, state, Rc::clone(&shell))),
    )
}

fn role_overrides(state: ThemeEditorState, shell: DevThemeReader) -> impl Widget {
    let available = state.available_roles();
    let add_state = state.clone();
    let add_roles = available.clone();
    let rows = state.overrides().into_iter().map(Token::Role).fold(
        Stack::vertical().spacing(2.0).alignment(Alignment::Stretch),
        |rows, token| rows.with_child(token_with_editor(token, state.clone(), Rc::clone(&shell))),
    );
    group_frame(
        &shell,
        "Role overrides",
        "Roles follow the source colors. Override one to set it directly.",
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(rows)
            .with_child(
                Select::new(THEME_OVERRIDE_ROLE_NAME)
                    .placeholder("Override a role")
                    .options(available.iter().map(|role| role.label()))
                    .theme_when(clone_dev_theme_reader(&shell))
                    .on_change_with_ctx(move |_, index, _| {
                        if let Some(role) = add_roles.get(index) {
                            add_state.add_override(*role);
                        }
                    }),
            ),
    )
}

/// Holds `tokens`' editor while one of them is selected.
fn editor_slot(tokens: Vec<Token>, state: ThemeEditorState, shell: DevThemeReader) -> impl Widget {
    let editor_state = state.clone();
    RebuildOnChange::new_observable(state.selected_signal(), move |selected| match selected {
        Some(token) if tokens.contains(token) => WidgetPod::new(color_editor(
            *token,
            editor_state.clone(),
            Rc::clone(&shell),
        )),
        _ => WidgetPod::new(SizedBox::new()),
    })
}

fn color_editor(token: Token, state: ThemeEditorState, shell: DevThemeReader) -> impl Widget {
    let mode_state = state.clone();
    let reset_state = state.clone();
    let picker_state = state.clone();
    let picker_shell = Rc::clone(&shell);
    let hex_state = state.clone();
    let hex_shell = Rc::clone(&shell);
    let mode = state.picker_signal().get().mode;
    let reset_label = match token {
        Token::Source(_) => "Reset to preset",
        Token::Role(_) => "Remove override",
    };
    let controls = Flex::horizontal()
        .gap(8.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Center)
        .with_child(
            SegmentedControl::new(THEME_COLOR_MODEL_NAME)
                .segments(PickerMode::ALL.map(PickerMode::label))
                .selected(mode.index())
                .theme_when(clone_dev_theme_reader(&shell))
                .on_change_with_ctx(move |_, index, _| {
                    mode_state.set_picker_mode(PickerMode::ALL[index.min(2)]);
                }),
        )
        .with_item(SizedBox::new(), FlexItem::flex(1.0))
        .with_child(
            Button::new(reset_label)
                .appearance(ButtonAppearance::Ghost)
                .icon(IconGlyph::Restore)
                .theme_when(clone_dev_theme_reader(&shell))
                .on_press(move || reset_state.reset(token)),
        );
    let picker = RebuildOnChange::new_observable(state.picker_signal(), move |key| {
        let color_state = picker_state.clone();
        let change_state = picker_state.clone();
        WidgetPod::new(
            SimpleColorPicker::from_color(THEME_COLOR_PICKER_NAME, picker_state.color(token))
                .mode(key.mode.picker_mode())
                .show_alpha(true)
                .color_when(move || color_state.color(token))
                .theme_when(clone_dev_theme_reader(&picker_shell))
                .on_change(move |color| change_state.set_color(token, color, EditOrigin::Picker)),
        )
    });
    let hex_field = RebuildOnChange::new_observable(state.hex_signal(), move |_| {
        let change_state = hex_state.clone();
        WidgetPod::new(
            TextInput::new(THEME_HEX_NAME)
                .value(hex(hex_state.color(token)))
                .theme_when(clone_dev_theme_reader(&hex_shell))
                .on_change(move |value| {
                    if let Some(color) = parse_hex(&value) {
                        change_state.set_color(token, color, EditOrigin::Hex);
                    }
                }),
        )
    });
    Padding::new(
        Insets {
            left: 8.0,
            top: 6.0,
            right: 0.0,
            bottom: 10.0,
        },
        Surface::panel(
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(controls)
                .with_child(picker)
                .with_child(
                    PropertyRow::new("Hex", hex_field)
                        .theme_when(clone_dev_theme_reader(&shell))
                        .inline()
                        .label_width(44.0),
                ),
        )
        .theme_when(clone_dev_theme_reader(&shell))
        .padding(Insets::all(12.0))
        .fill_width(),
    )
}

fn shape_and_type(state: ThemeEditorState, shell: DevThemeReader) -> impl Widget {
    let rows = [
        ScaleRow {
            label: "Spacing",
            name: THEME_SPACING_NAME,
            range: SPACING_RANGE,
            step: 0.5,
            read: |state| f64::from(state.spacing()),
            write: |state, value| state.set_spacing(value as f32),
            format: |value| format!("{value:.1} px"),
        },
        ScaleRow {
            label: "Corner radius",
            name: THEME_RADIUS_SCALE_NAME,
            range: RADIUS_RANGE,
            step: 0.05,
            read: |state| f64::from(state.radius_scale()),
            write: |state, value| state.set_radius_scale(value as f32),
            format: |value| format!("{:.0}%", value * 100.0),
        },
        ScaleRow {
            label: "Type size",
            name: THEME_TEXT_SCALE_NAME,
            range: TEXT_RANGE,
            step: 0.05,
            read: |state| f64::from(state.text_scale()),
            write: |state, value| state.set_text_scale(value as f32),
            format: |value| format!("{:.0}%", value * 100.0),
        },
        ScaleRow {
            label: "Motion duration",
            name: THEME_MOTION_SCALE_NAME,
            range: MOTION_RANGE,
            step: 0.05,
            read: |state| f64::from(state.motion_scale()),
            write: |state, value| state.set_motion_scale(value as f32),
            format: |value| format!("{:.0}%", value * 100.0),
        },
    ];
    let body = rows.into_iter().fold(
        Stack::vertical()
            .spacing(12.0)
            .alignment(Alignment::Stretch),
        |body, row| body.with_child(row.build(state.clone(), Rc::clone(&shell))),
    );
    group_frame(
        &shell,
        "Shape and type",
        "Spacing, corner rounding, text size, and animation length.",
        body,
    )
}

struct ScaleRow {
    label: &'static str,
    name: &'static str,
    range: (f64, f64),
    step: f64,
    read: fn(&ThemeEditorState) -> f64,
    write: fn(&ThemeEditorState, f64),
    format: fn(f64) -> String,
}

impl ScaleRow {
    fn build(self, state: ThemeEditorState, shell: DevThemeReader) -> impl Widget {
        let read_state = state.clone();
        let write_state = state.clone();
        let value_state = state.clone();
        let value_shell = Rc::clone(&shell);
        let Self {
            read,
            write,
            format,
            ..
        } = self;
        let value = RebuildOnChange::new_observable(state.theme_signal(), move |_| {
            WidgetPod::new(
                demo_label(
                    &value_shell,
                    format(read(&value_state)),
                    DemoTextRole::Supporting,
                    DemoTextColor::Muted,
                )
                .single_line(),
            )
        });
        let slider = Slider::new(self.name)
            .range(self.range.0, self.range.1)
            .step(self.step)
            .value_when(move || read(&read_state))
            .theme_when(clone_dev_theme_reader(&shell))
            .on_change(move |value| write(&write_state, value));
        Stack::vertical()
            .spacing(4.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Flex::horizontal()
                    .align_items(Alignment::Center)
                    .with_item(
                        demo_label(
                            &shell,
                            self.label,
                            DemoTextRole::Supporting,
                            DemoTextColor::Text,
                        ),
                        FlexItem::flex(1.0),
                    )
                    .with_child(value),
            )
            .with_child(slider)
    }
}
