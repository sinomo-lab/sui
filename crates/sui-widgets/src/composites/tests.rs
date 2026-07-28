use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
};

use super::Tabs;
use super::{
    ActionCard, ActionTilePaint, BottomSheet, BrowserTabBar, CalloutPaint, CodePanelPaint,
    CodeTextLine, CodeTextPaint, CodeTextSpan, CommandButtonPaint, CommandGroup, ContextMenu,
    CoverageDots, DetailRow, Dialog, DisclosureButtonPaint, DockPanel, EmptyState, FieldGroup,
    FormRow, FormSection, FramedField, HairlineEdge, Menu, MenuItem, PanelSection, PlacementBadge,
    PlacementBadgePaint, Popover, PopoverAlignment, PresetStrip, ProgressBar, PropertyRow,
    PropertyRowLayout, SectionLabel, SectionLabelPaint, SectionPanelPaint, SegmentedControl,
    SegmentedControlItem, SheetState, SideSheet, SideSheetPlacement, Spinner, StatusBadge,
    StatusBar, StatusBarHost, StatusBarSegment, Surface, SurfaceAppearance, TabBar, ToolPalette,
    ToolPaletteItem, Toolbar, paint_action_tile, paint_border, paint_callout, paint_code_lines,
    paint_code_panel, paint_command_button, paint_disclosure_button, paint_hairline,
    paint_placement_badge_with, paint_rounded_panel, paint_section_label,
    paint_section_label_detail, paint_section_panel, text_token_style,
};
use crate::FloatingStack;
use crate::{
    DefaultTheme, HdrThemeMode, Padding, ScrollView, SelectionScope, SemanticColorToken,
    SemanticTone, SizedBox, Stack, TEXT_COMMAND, TextArea, TextCommand, ThemeTextToken,
};
use sui_core::{
    Color, Event, KeyState, KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent,
    PointerEventKind, Rect, SemanticsAction, SemanticsActionRequest, SemanticsNode, SemanticsRole,
    SemanticsValue, Size, Vector, WidgetId, WindowEvent,
};
use sui_layout::{Alignment, Constraints};
use sui_reactive::Signal;
use sui_runtime::{
    Application, ArrangeCtx, MeasureCtx, PaintCtx, RenderOutput, Runtime, SemanticsCtx, Widget,
    WindowBuilder,
};
use sui_scene::{
    Brush, LayerCompositionMode, SceneCommand, SceneLayerDescriptor, SceneLayerUpdateKind,
};
use sui_text::{FontFeature, FontRegistry, FontWeight, TextSystem};

fn build_runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
where
    W: Widget + 'static,
{
    let runtime = Application::new()
        .window(WindowBuilder::new().title("Composites").root(root))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    (runtime, window_id)
}

fn render<W>(root: W) -> RenderOutput
where
    W: Widget + 'static,
{
    let (mut runtime, window_id) = build_runtime(root);
    runtime.render(window_id).unwrap()
}

fn render_isolated<W>(root: W) -> RenderOutput
where
    W: Widget + 'static,
{
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Unused")
                .root(crate::Label::new("Unused")),
        )
        .window(WindowBuilder::new().title("Composites").root(root))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[1];
    runtime.render(window_id).unwrap()
}

#[test]
fn filling_surface_measures_flex_text_at_its_arranged_width() {
    const TEXT: &str = "Evolution dashboard source ready";
    let output = render(
        crate::SizedBox::new().width(640.0).height(78.0).with_child(
            Surface::window(
                crate::Flex::horizontal()
                    .with_item(crate::Label::new(TEXT), sui_layout::FlexItem::flex(1.0))
                    .with_child(crate::Label::new("1 promoted")),
            )
            .padding(sui_layout::Padding::all(20.0))
            .fill(),
        ),
    );

    let label = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some(TEXT))
        .expect("flex label semantics present");
    assert!(
        label.bounds.width() > 200.0,
        "filling surface should allocate a useful text width, got {:?}",
        label.bounds
    );

    let mut line_count = None;
    output.frame.scene.visit_commands(&mut |command| {
        if let SceneCommand::DrawShapedText(run) = command
            && let Some(layout) = run.resolve(output.frame.text_layout_registry.as_ref())
            && layout.text() == TEXT
        {
            line_count = Some(layout.lines().len());
        }
    });
    assert_eq!(line_count, Some(1));
}

#[test]
fn wrapping_toolbar_retains_items_across_multiple_rows() {
    let toolbar = Toolbar::horizontal()
        .wrapping()
        .padding(sui_layout::Padding::ZERO)
        .spacing(4.0)
        .line_spacing(6.0)
        .divider(false)
        .with_child(SizedBox::new().size(Size::new(60.0, 20.0)))
        .with_child(SizedBox::new().size(Size::new(60.0, 20.0)))
        .with_child(SizedBox::new().size(Size::new(60.0, 20.0)));
    let (mut runtime, window) = build_runtime(
        SizedBox::new()
            .size(Size::new(128.0, 80.0))
            .with_child(toolbar),
    );
    runtime.render(window).unwrap();

    let graph = runtime.widget_graph(window).unwrap();
    let item_bounds = graph
        .nodes
        .iter()
        .filter(|node| node.bounds.size == Size::new(60.0, 20.0))
        .map(|node| node.bounds)
        .collect::<Vec<_>>();
    assert_eq!(item_bounds.len(), 3);
    assert_eq!(item_bounds[0].y(), item_bounds[1].y());
    assert!(item_bounds[2].y() >= item_bounds[0].max_y() + 6.0);
}

#[test]
fn density_modes_resize_menu_and_tabs() {
    let compact = DefaultTheme::compact();
    let touch = DefaultTheme::touch();

    assert!(
        render(
            Menu::new("Actions")
                .theme(compact)
                .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")])
        )
        .frame
        .viewport
        .height
            < render(
                Menu::new("Actions")
                    .theme(touch)
                    .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")])
            )
            .frame
            .viewport
            .height
    );
    assert!(
        render(
            TabBar::new("Tabs")
                .theme(compact)
                .tabs(["Canvas", "Inspector"])
        )
        .frame
        .viewport
        .height
            < render(
                TabBar::new("Tabs")
                    .theme(touch)
                    .tabs(["Canvas", "Inspector"])
            )
            .frame
            .viewport
            .height
    );
    assert!(
        render(
            Tabs::new("Tabs")
                .theme(compact)
                .tab("Canvas", crate::Label::new("Canvas"))
                .tab("Inspector", crate::Label::new("Inspector"))
        )
        .frame
        .viewport
        .height
            < render(
                Tabs::new("Tabs")
                    .theme(touch)
                    .tab("Canvas", crate::Label::new("Canvas"))
                    .tab("Inspector", crate::Label::new("Inspector"))
            )
            .frame
            .viewport
            .height
    );
    assert!(
        render(
            SegmentedControl::new("View")
                .theme(compact)
                .segments(["All", "Chats", "Channels"])
        )
        .frame
        .viewport
        .height
            < render(
                SegmentedControl::new("View")
                    .theme(touch)
                    .segments(["All", "Chats", "Channels"])
            )
            .frame
            .viewport
            .height
    );
}

#[test]
fn segmented_control_selection_thumb_is_evenly_inset() {
    let theme = DefaultTheme::default();
    let output = render(
        crate::SizedBox::new()
            .width(360.0)
            .height(theme.metrics.tab_height)
            .with_child(
                SegmentedControl::new("Conversation view")
                    .theme(theme)
                    .segments(["All", "Chats", "Channels"]),
            ),
    );
    let group = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::RadioGroup)
        .expect("segmented control semantics present");
    let mut selection_thumb = None;
    output.frame.scene.visit_commands(&mut |command| {
        if let SceneCommand::FillPath { path, brush } = command
            && *brush == Brush::Solid(theme.palette.selection)
        {
            selection_thumb = Some(path.bounds());
        }
    });
    let thumb = selection_thumb.expect("selected segment thumb painted");

    assert!(
        (thumb.y() - group.bounds.y() - 2.0).abs() < 0.001,
        "selection thumb should have a 2 px top inset: group={:?}, thumb={thumb:?}",
        group.bounds
    );
    assert!(
        (group.bounds.max_y() - thumb.max_y() - 2.0).abs() < 0.001,
        "selection thumb should have a 2 px bottom inset: group={:?}, thumb={thumb:?}",
        group.bounds
    );
    assert!(
        !solid_fill_colors(&output).contains(&theme.palette.accent),
        "segmented controls should use a filled selection thumb without a tab underline"
    );
}

#[test]
fn segmented_control_click_updates_radio_semantics() -> Result<(), String> {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let on_change = Rc::clone(&changes);
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new().width(260.0).height(28.0).with_child(
            SegmentedControl::new("Conversation view")
                .items([
                    SegmentedControlItem::new("All 2").semantic_name("Show all conversations"),
                    SegmentedControlItem::new("Chats 1").semantic_name("Show chats only"),
                    SegmentedControlItem::new("Channels 1")
                        .semantic_name("Show channels only")
                        .description("1 visible conversation(s)"),
                ])
                .on_change(move |index, label| on_change.borrow_mut().push((index, label))),
        ),
    );

    let _ = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(220.0, 14.0), true),
        )
        .map_err(|error| error.to_string())?;
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(220.0, 14.0), false),
        )
        .map_err(|error| error.to_string())?;

    assert_eq!(
        changes.borrow().as_slice(),
        &[(2, "Channels 1".to_string())]
    );
    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let group = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::RadioGroup)
        .expect("radio group semantics present");
    assert_eq!(group.name.as_deref(), Some("Conversation view"));
    assert_eq!(
        group.value,
        Some(SemanticsValue::Text("Channels 1".to_string()))
    );
    let channel = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::RadioButton
                && node.name.as_deref() == Some("Show channels only")
        })
        .expect("selected segment semantics present");
    assert_eq!(channel.parent, Some(group.id));
    assert_eq!(
        channel.description.as_deref(),
        Some("1 visible conversation(s)")
    );
    assert!(channel.state.selected);
    assert_eq!(channel.state.checked, Some(sui_core::ToggleState::Checked));
    assert!(channel.actions.contains(&SemanticsAction::Activate));
    Ok(())
}

#[test]
fn segmented_control_keyboard_changes_selection() -> Result<(), String> {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let on_change = Rc::clone(&changes);
    let (mut runtime, window_id) = build_runtime(
        SegmentedControl::new("Conversation view")
            .segments(["All", "Chats", "Channels"])
            .on_change_with_ctx(move |index, label, ctx| {
                on_change.borrow_mut().push((index, label));
                ctx.request_measure();
                ctx.request_arrange();
                ctx.request_paint();
                ctx.request_semantics();
            }),
    );

    let _ = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
        )
        .map_err(|error| error.to_string())?;
    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
        )
        .map_err(|error| error.to_string())?;

    assert_eq!(changes.borrow().as_slice(), &[(1, "Chats".to_string())]);
    Ok(())
}

#[test]
fn density_modes_resize_tool_command_widgets() {
    let compact = DefaultTheme::compact();
    let touch = DefaultTheme::touch();

    assert!(
        render(
            Toolbar::horizontal()
                .theme(compact)
                .with_child(crate::Button::new("Undo"))
                .with_child(crate::Button::new("Redo"))
        )
        .frame
        .viewport
        .height
            < render(
                Toolbar::horizontal()
                    .theme(touch)
                    .with_child(crate::Button::new("Undo"))
                    .with_child(crate::Button::new("Redo"))
            )
            .frame
            .viewport
            .height
    );
    assert!(
        render(
            CommandGroup::horizontal("History")
                .theme(compact)
                .with_child(crate::Button::new("Undo"))
                .with_child(crate::Button::new("Redo"))
        )
        .frame
        .viewport
        .height
            < render(
                CommandGroup::horizontal("History")
                    .theme(touch)
                    .with_child(crate::Button::new("Undo"))
                    .with_child(crate::Button::new("Redo"))
            )
            .frame
            .viewport
            .height
    );
    assert!(
        render(
            ToolPalette::vertical("Tools")
                .theme(compact)
                .item(ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush"))
                .item(ToolPaletteItem::new(crate::IconGlyph::Eraser, "Erase"))
        )
        .frame
        .viewport
        .width
            < render(
                ToolPalette::vertical("Tools")
                    .theme(touch)
                    .item(ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush"))
                    .item(ToolPaletteItem::new(crate::IconGlyph::Eraser, "Erase"))
            )
            .frame
            .viewport
            .width
    );
    assert!(
        render(
            PresetStrip::new("Brush presets")
                .theme(compact)
                .presets(["8 px", "18 px", "36 px"])
        )
        .frame
        .viewport
        .height
            < render(
                PresetStrip::new("Brush presets")
                    .theme(touch)
                    .presets(["8 px", "18 px", "36 px"])
            )
            .frame
            .viewport
            .height
    );
}

#[test]
fn density_modes_resize_overlay_widgets() {
    let compact = DefaultTheme::compact();
    let touch = DefaultTheme::touch();

    let compact_dialog = render(
        Dialog::new("Export", crate::Label::new("Export settings"))
            .theme(compact)
            .description("Choose file settings"),
    );
    let touch_dialog = render(
        Dialog::new("Export", crate::Label::new("Export settings"))
            .theme(touch)
            .description("Choose file settings"),
    );
    let compact_bounds = compact_dialog
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("compact dialog semantics present")
        .bounds;
    let touch_bounds = touch_dialog
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("touch dialog semantics present")
        .bounds;
    assert!(compact_bounds.width() < touch_bounds.width());
    assert!(compact_bounds.height() < touch_bounds.height());

    let (mut compact_popover, compact_window) = build_runtime(
        Popover::new(
            "Options",
            crate::Button::new("Open"),
            crate::Label::new("Popover body"),
        )
        .theme(compact),
    );
    let _ = compact_popover.render(compact_window).unwrap();
    compact_popover
        .handle_event(
            compact_window,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )
        .unwrap();
    let compact_output = compact_popover.render(compact_window).unwrap();
    let compact_offset = overlay_layer_descriptor(&compact_output)
        .expect("compact popover overlay present")
        .properties
        .translation
        .y
        .abs();

    let (mut touch_popover, touch_window) = build_runtime(
        Popover::new(
            "Options",
            crate::Button::new("Open"),
            crate::Label::new("Popover body"),
        )
        .theme(touch),
    );
    let _ = touch_popover.render(touch_window).unwrap();
    touch_popover
        .handle_event(
            touch_window,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )
        .unwrap();
    let touch_output = touch_popover.render(touch_window).unwrap();
    let touch_offset = overlay_layer_descriptor(&touch_output)
        .expect("touch popover overlay present")
        .properties
        .translation
        .y
        .abs();
    assert!(compact_offset < touch_offset);
}

#[test]
fn popover_keyboard_and_semantic_actions_share_open_contract() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let on_open_change = Rc::clone(&changes);
    let (mut runtime, window_id) = build_runtime(
        Popover::new(
            "Health center",
            crate::Button::new("Open"),
            crate::Label::new("All systems operational"),
        )
        .on_open_change(move |open| on_open_change.borrow_mut().push(open)),
    );
    let output = runtime.render(window_id).unwrap();
    let popover_id = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Popover)
        .expect("popover semantics present")
        .id;

    assert!(
        runtime
            .handle_semantics_action(window_id, popover_id, SemanticsActionRequest::Focus,)
            .unwrap()
    );
    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Enter", KeyState::Pressed)),
        )
        .unwrap();
    assert_eq!(changes.borrow().as_slice(), &[true]);
    assert_eq!(
        runtime
            .render(window_id)
            .unwrap()
            .semantics
            .iter()
            .find(|node| node.id == popover_id)
            .and_then(|node| node.state.expanded),
        Some(true)
    );

    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new(" ", KeyState::Pressed)),
        )
        .unwrap();
    assert_eq!(changes.borrow().as_slice(), &[true, false]);
    let _ = runtime.render(window_id).unwrap();
    assert!(
        runtime
            .handle_semantics_action(window_id, popover_id, SemanticsActionRequest::Expand,)
            .unwrap()
    );
    let _ = runtime.render(window_id).unwrap();
    assert!(
        runtime
            .handle_semantics_action(window_id, popover_id, SemanticsActionRequest::Collapse,)
            .unwrap()
    );
    assert_eq!(changes.borrow().as_slice(), &[true, false, true, false]);
}

#[test]
fn popover_end_alignment_keeps_a_narrow_trigger_on_the_trailing_edge() {
    let output = render(
        crate::SizedBox::new().width(360.0).with_child(
            Popover::new(
                "Trailing inspector",
                crate::Button::new("Open").min_width(92.0),
                crate::SizedBox::new()
                    .width(240.0)
                    .with_child(crate::Label::new("Inspector body")),
            )
            .alignment(PopoverAlignment::End)
            .open(true),
        ),
    );
    let popover = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Popover
                && node.name.as_deref() == Some("Trailing inspector")
        })
        .expect("popover semantics present");
    let trigger = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Open"))
        .expect("popover trigger semantics present");

    assert!(trigger.bounds.x() > popover.bounds.x());
    assert!((trigger.bounds.max_x() - popover.bounds.max_x()).abs() < 0.01);
}

#[test]
fn popover_surface_escapes_a_tight_toolbar_slot() {
    let output = render(
        crate::SizedBox::new()
            .width(800.0)
            .height(600.0)
            .with_child(crate::Align::new(
                Alignment::End,
                Alignment::Start,
                crate::SizedBox::new().width(150.0).height(24.0).with_child(
                    Popover::new(
                        "Toolbar recovery",
                        crate::Button::new("2 issues").min_width(150.0),
                        crate::SizedBox::new()
                            .width(400.0)
                            .height(300.0)
                            .with_child(crate::Label::new("Recovery panel")),
                    )
                    .alignment(PopoverAlignment::End)
                    .open(true),
                ),
            )),
    );
    let popover = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Popover && node.name.as_deref() == Some("Toolbar recovery")
        })
        .expect("popover semantics present");
    let content = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Recovery panel")
        })
        .expect("popover content present");
    let surface = overlay_layer_descriptor(&output).expect("popover overlay present");

    assert!((popover.bounds.width() - 150.0).abs() < 0.01);
    assert!((popover.bounds.height() - 24.0).abs() < 0.01);
    assert!(
        surface.bounds.width() >= 400.0,
        "surface={:?}, popover={:?}, content={:?}",
        surface.bounds,
        popover.bounds,
        content.bounds
    );
    assert!(content.bounds.y() >= popover.bounds.max_y());
}

#[test]
fn framed_field_owns_standard_and_validation_chrome() {
    let theme = DefaultTheme::default();
    let output = render(
        FramedField::new(crate::Label::new("Query"))
            .theme(theme)
            .name("Search query")
            .invalid(true),
    );

    assert!(solid_fill_colors(&output).contains(&theme.surfaces.field));
    assert!(
        solid_stroke_colors(&output).contains(&theme.semantic_tone_color(SemanticTone::Danger))
    );
    assert!(output.semantics.iter().any(|node| {
        node.role == SemanticsRole::GenericContainer && node.name.as_deref() == Some("Search query")
    }));
    assert!(
        output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Query")
        })
    );
}

#[test]
fn framed_field_tracks_focus_anywhere_in_its_child_subtree() {
    let theme = DefaultTheme::default();
    let (mut runtime, window_id) = build_runtime(
        FramedField::new(crate::TextInput::new("Query").bare())
            .theme(theme)
            .name("Search field"),
    );
    let _ = runtime.render(window_id).unwrap();
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )
        .unwrap();
    let output = runtime.render(window_id).unwrap();
    let frame = output
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some("Search field"))
        .expect("framed field semantics present");
    assert!(frame.state.focused);
    assert!(solid_stroke_colors(&output).contains(&theme.palette.focus_ring));
}

#[test]
fn framed_field_uses_stock_hover_and_focus_surface_tokens() {
    let theme = DefaultTheme::default();
    let (mut runtime, window_id) = build_runtime(
        FramedField::new(crate::TextInput::new("Query").bare())
            .theme(theme)
            .name("Search field"),
    );
    let _ = runtime.render(window_id).unwrap();

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(12.0, 12.0), false),
        )
        .unwrap();
    runtime.tick(1.0);
    handle_ready_events(&mut runtime).unwrap();
    let hovered = runtime.render(window_id).unwrap();
    assert!(solid_stroke_colors(&hovered).contains(&theme.palette.border_hover));
    assert!(
        hovered
            .semantics
            .iter()
            .any(|node| { node.name.as_deref() == Some("Search field") && node.state.hovered })
    );

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )
        .unwrap();
    let focused = runtime.render(window_id).unwrap();
    assert!(solid_fill_colors(&focused).contains(&theme.palette.surface_focus));
    assert!(solid_stroke_colors(&focused).contains(&theme.palette.focus_ring));
}

#[test]
fn surface_appearance_uses_shared_raised_and_semantic_tokens() {
    let theme = DefaultTheme::default();
    let raised = render(
        Surface::field(crate::Label::new("Ready"))
            .theme(theme)
            .appearance(SurfaceAppearance::Raised),
    );
    assert!(solid_fill_colors(&raised).contains(&theme.palette.surface_raised));

    let soft = render(
        Surface::field(crate::Label::new("Queued"))
            .theme(theme)
            .appearance(SurfaceAppearance::Soft)
            .tone(SemanticTone::Accent),
    );
    assert!(
        solid_fill_colors(&soft).contains(&theme.semantic_tone_soft_colors(SemanticTone::Accent).0)
    );
}

#[test]
fn side_sheet_anchors_to_edge_and_exposes_dialog_semantics() {
    let output = render(
        SideSheet::new("Inspector", crate::Label::new("Selection details"))
            .description("Properties for the selected item")
            .placement(SideSheetPlacement::Right)
            .width(360.0),
    );
    let sheet = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Dialog && node.name.as_deref() == Some("Inspector")
        })
        .expect("side sheet dialog semantics present");
    assert!((sheet.bounds.width() - 360.0).abs() < 0.01);
    assert!((sheet.bounds.max_x() - output.frame.viewport.width).abs() < 0.01);
    assert_eq!(sheet.state.expanded, Some(true));
    assert!(output.semantics.iter().any(|node| {
        node.role == SemanticsRole::Text && node.name.as_deref() == Some("Selection details")
    }));
}

#[test]
fn side_sheet_scrim_dismisses_without_swallowing_sheet_presses() {
    let dismissals = Rc::new(Cell::new(0_u32));
    let on_dismiss = Rc::clone(&dismissals);
    let (mut runtime, window_id) = build_runtime(
        SideSheet::new("Inspector", crate::Label::new("Body"))
            .width(320.0)
            .on_dismiss(move || on_dismiss.set(on_dismiss.get() + 1)),
    );
    let _ = runtime.render(window_id).unwrap();
    runtime.tick(1.0);
    handle_ready_events(&mut runtime).unwrap();
    let settled = runtime.render(window_id).unwrap();
    let sheet = settled
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("side sheet semantics present");
    let sheet_id = sheet.id;
    let sheet_point = Point::new(sheet.bounds.x() + 12.0, sheet.bounds.y() + 12.0);
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, sheet_point, true),
        )
        .unwrap();
    assert_eq!(dismissals.get(), 0);

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )
        .unwrap();
    assert_eq!(dismissals.get(), 1);
    assert!(
        runtime
            .handle_semantics_action(window_id, sheet_id, SemanticsActionRequest::Collapse,)
            .unwrap()
    );
    assert_eq!(dismissals.get(), 2);
}

#[test]
fn side_sheet_takes_initial_focus_and_escape_dismisses_from_a_child() {
    let dismissals = Rc::new(Cell::new(0_u32));
    let on_dismiss = Rc::clone(&dismissals);
    let (mut runtime, window_id) = build_runtime(
        SideSheet::new("Inspector", crate::Button::new("Child action"))
            .width(320.0)
            .on_dismiss(move || on_dismiss.set(on_dismiss.get() + 1)),
    );
    let initial = runtime.render(window_id).unwrap();
    let _sheet_id = initial
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("side sheet semantics present")
        .id;
    let child_id = initial
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Child action")
        })
        .expect("side sheet child semantics present")
        .id;

    runtime.tick(1.0);
    handle_ready_events(&mut runtime).unwrap();
    assert_eq!(runtime.focused_widget(window_id).unwrap(), Some(child_id));

    assert!(
        runtime
            .handle_semantics_action(window_id, child_id, SemanticsActionRequest::Focus)
            .unwrap()
    );
    assert_eq!(runtime.focused_widget(window_id).unwrap(), Some(child_id));
    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Escape", KeyState::Pressed)),
        )
        .unwrap();
    assert_eq!(dismissals.get(), 1);
}

#[test]
fn dialog_title_and_description_visual_centers_match_header_slots() {
    let theme = DefaultTheme::default();
    let output = render(
        Dialog::new("Export", crate::Label::new("Export settings"))
            .theme(theme)
            .description("Choose file settings"),
    );
    let dialog = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("dialog semantics present")
        .bounds;

    let title = text_run_for(&output, "Export");
    assert_text_run_uses_token(&title, theme.text.lg);
    let title_layout = text_run_layout(&title);
    let title_line = title_layout
        .lines()
        .first()
        .expect("dialog title should contain one line");
    let title_visual_center =
        title.rect.y() + title_line.baseline + optical_visual_center(title_layout.measurement());

    let description = text_run_for(&output, "Choose file settings");
    let description_layout = text_run_layout(&description);
    let description_line = description_layout
        .lines()
        .first()
        .expect("dialog description should contain one line");
    let description_visual_center = description.rect.y()
        + description_line.baseline
        + optical_visual_center(description_layout.measurement());

    let metrics = theme.metrics;
    let padding = metrics.dialog_padding;
    let text_width = (dialog.width() - padding.left - padding.right).max(0.0);
    let title_height = title
        .style
        .line_height
        .max(title_layout.measurement().height);
    let description_height = description
        .style
        .line_height
        .max(description_layout.measurement().height);
    let title_slot = Rect::new(
        dialog.x() + padding.left,
        dialog.y() + padding.top,
        text_width,
        title_height,
    );
    let description_slot = Rect::new(
        dialog.x() + padding.left,
        title_slot.max_y() + metrics.dialog_description_gap,
        text_width,
        description_height,
    );

    assert!((title_visual_center - super::rect_center(title_slot).y).abs() < 0.75);
    assert!((description_visual_center - super::rect_center(description_slot).y).abs() < 0.75);
}

#[test]
fn dialog_and_side_sheet_titles_follow_the_lg_theme_token() {
    let mut theme = DefaultTheme::default();
    theme.text.lg = ThemeTextToken {
        size: 23.0,
        line_height: 31.0,
    };
    theme.metrics.dialog_title_font_size = 9.0;
    theme.metrics.dialog_title_line_height = 11.0;

    let dialog = render(Dialog::new("Token dialog", crate::Label::new("Dialog body")).theme(theme));
    assert_text_run_uses_token(&text_run_for(&dialog, "Token dialog"), theme.text.lg);

    let side_sheet = render(
        SideSheet::new("Token sheet", crate::Label::new("Sheet body"))
            .theme(theme)
            .width(360.0),
    );
    assert_text_run_uses_token(&text_run_for(&side_sheet, "Token sheet"), theme.text.lg);
}

#[test]
fn dialog_header_text_preserves_tall_measurements_in_compact_line_boxes() {
    let mut theme = DefaultTheme::default();
    theme.text.lg = ThemeTextToken {
        size: 32.0,
        line_height: 12.0,
    };
    theme.typography.body_font_size = 28.0;
    theme.typography.body_line_height = 10.0;

    let output = render(
        Dialog::new("Export", crate::Label::new("Export settings"))
            .theme(theme)
            .description("Choose file settings"),
    );
    let dialog = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("dialog semantics present")
        .bounds;
    let title = text_run_for(&output, "Export");
    let title_layout = TextSystem::new()
        .shape_text_run(&title, &FontRegistry::new())
        .expect("dialog title should shape");
    let description = text_run_for(&output, "Choose file settings");
    let description_layout = TextSystem::new()
        .shape_text_run(&description, &FontRegistry::new())
        .expect("dialog description should shape");
    let metrics = theme.metrics;
    let padding = metrics.dialog_padding;
    let text_width = (dialog.width() - padding.left - padding.right).max(0.0);
    let title_height = title
        .style
        .line_height
        .max(title_layout.measurement().height);
    let description_height = description
        .style
        .line_height
        .max(description_layout.measurement().height);
    let title_slot = Rect::new(
        dialog.x() + padding.left,
        dialog.y() + padding.top,
        text_width,
        title_height,
    );
    let description_slot = Rect::new(
        dialog.x() + padding.left,
        title_slot.max_y() + metrics.dialog_description_gap,
        text_width,
        description_height,
    );

    assert!(title.rect.height() >= title_layout.measurement().height - 0.01);
    assert!(title.rect.height() > title.style.line_height);
    assert!(description.rect.height() >= description_layout.measurement().height - 0.01);
    assert!(description.rect.height() > description.style.line_height);
    assert_eq!(description.style.color, theme.palette.placeholder);
    assert!((text_run_visual_center(&title) - super::rect_center(title_slot).y).abs() < 0.75);
    assert!(
        (text_run_visual_center(&description) - super::rect_center(description_slot).y).abs()
            < 0.75
    );
}

#[test]
fn density_modes_resize_composite_status_widgets() {
    let compact = DefaultTheme::compact();
    let touch = DefaultTheme::touch();

    assert!(
        render(
            ActionCard::new("Paint", "Pixel canvas workspace")
                .theme(compact)
                .icon(crate::IconGlyph::Brush)
        )
        .frame
        .viewport
        .height
            < render(
                ActionCard::new("Paint", "Pixel canvas workspace")
                    .theme(touch)
                    .icon(crate::IconGlyph::Brush)
            )
            .frame
            .viewport
            .height
    );
    assert!(
        render(
            StatusBar::new()
                .theme(compact)
                .text_segment("Ready")
                .text_segment("100%")
        )
        .frame
        .viewport
        .height
            < render(
                StatusBar::new()
                    .theme(touch)
                    .text_segment("Ready")
                    .text_segment("100%")
            )
            .frame
            .viewport
            .height
    );
    assert!(
        render(
            StatusBadge::new("Synced")
                .theme(compact)
                .tone(SemanticTone::Success)
                .icon(crate::IconGlyph::Storage)
        )
        .frame
        .viewport
        .height
            < render(
                StatusBadge::new("Synced")
                    .theme(touch)
                    .tone(SemanticTone::Success)
                    .icon(crate::IconGlyph::Storage)
            )
            .frame
            .viewport
            .height
    );
    assert!(
        render(
            CoverageDots::new("Replicas", 2, 3)
                .theme(compact)
                .tone(SemanticTone::Success)
        )
        .frame
        .viewport
        .height
            < render(
                CoverageDots::new("Replicas", 2, 3)
                    .theme(touch)
                    .tone(SemanticTone::Success)
            )
            .frame
            .viewport
            .height
    );
    assert!(
        render(
            ProgressBar::new("Export progress")
                .theme(compact)
                .value(0.42)
        )
        .frame
        .viewport
        .height
            < render(ProgressBar::new("Export progress").theme(touch).value(0.42))
                .frame
                .viewport
                .height
    );
}

#[test]
fn composite_focus_rings_use_theme_motion() -> Result<(), String> {
    assert_focus_ring_uses_theme_motion(
        crate::SizedBox::new()
            .size(Size::new(112.0, 44.0))
            .with_child(
                ToolPalette::horizontal("Tools")
                    .items([
                        ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush"),
                        ToolPaletteItem::new(crate::IconGlyph::Eraser, "Erase"),
                    ])
                    .selected(0),
            ),
        Point::new(18.0, 18.0),
    )?;

    assert_focus_ring_uses_theme_motion(
        crate::SizedBox::new()
            .size(Size::new(260.0, 92.0))
            .with_child(ActionCard::new("Paint", "Pixel canvas workspace")),
        Point::new(18.0, 18.0),
    )?;

    assert_focus_ring_uses_theme_motion(
        crate::SizedBox::new()
            .size(Size::new(240.0, 40.0))
            .with_child(PresetStrip::new("Brush presets").presets(["8 px", "18 px"])),
        Point::new(24.0, 18.0),
    )?;

    assert_focus_ring_uses_theme_motion(
        crate::SizedBox::new()
            .size(Size::new(260.0, 92.0))
            .with_child(
                PanelSection::new("Advanced color", crate::Label::new("RGB sliders"))
                    .collapsible(true)
                    .collapsed(),
            ),
        Point::new(24.0, 18.0),
    )?;

    assert_focus_ring_uses_theme_motion(
        Menu::new("App menu").items([MenuItem::new("New File"), MenuItem::new("Open...")]),
        Point::new(24.0, 24.0),
    )?;

    assert_focus_ring_uses_theme_motion(
        ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
            .activation_button(PointerButton::Primary)
            .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")]),
        Point::new(24.0, 24.0),
    )?;

    assert_focus_ring_uses_theme_motion(
        crate::SizedBox::new()
            .size(Size::new(640.0, 420.0))
            .with_child(Dialog::new(
                "Confirm",
                crate::Label::new("Apply the change?"),
            )),
        Point::new(320.0, 210.0),
    )
}

#[test]
fn density_modes_resize_form_and_panel_widgets() {
    let compact = DefaultTheme::compact();
    let touch = DefaultTheme::touch();

    assert!(
        render(PropertyRow::new("Opacity", crate::Slider::new("Opacity")).theme(compact))
            .frame
            .viewport
            .height
            < render(PropertyRow::new("Opacity", crate::Slider::new("Opacity")).theme(touch))
                .frame
                .viewport
                .height
    );
    assert!(
        render(
            FieldGroup::new()
                .theme(compact)
                .with_child(crate::Label::new("First"))
                .with_child(crate::Label::new("Second"))
        )
        .frame
        .viewport
        .height
            < render(
                FieldGroup::new()
                    .theme(touch)
                    .with_child(crate::Label::new("First"))
                    .with_child(crate::Label::new("Second"))
            )
            .frame
            .viewport
            .height
    );
    assert!(
        render(FormSection::new("Providers", crate::Label::new("Configured")).theme(compact))
            .frame
            .viewport
            .height
            < render(FormSection::new("Providers", crate::Label::new("Configured")).theme(touch))
                .frame
                .viewport
                .height
    );
    assert!(
        render(PanelSection::new("Brush", crate::Label::new("Opacity")).theme(compact))
            .frame
            .viewport
            .height
            < render(PanelSection::new("Brush", crate::Label::new("Opacity")).theme(touch))
                .frame
                .viewport
                .height
    );
    assert!(
        render(DockPanel::new("Tool properties", crate::Label::new("Brush size")).theme(compact))
            .frame
            .viewport
            .height
            < render(
                DockPanel::new("Tool properties", crate::Label::new("Brush size")).theme(touch)
            )
            .frame
            .viewport
            .height
    );
}

#[test]
fn detail_row_wraps_metadata_and_exposes_value_semantics() {
    let theme = DefaultTheme::default();
    let value = "replicated across atlas, keep, and wren with one pending repair";
    let narrow = render(
        crate::SizedBox::new()
            .width(140.0)
            .with_child(DetailRow::new("Placement", value).theme(theme)),
    );
    let wide = render(
        crate::SizedBox::new()
            .width(360.0)
            .with_child(DetailRow::new("Placement", value).theme(theme)),
    );

    assert!(
        narrow.frame.viewport.height > wide.frame.viewport.height,
        "narrow detail rows should wrap their metadata value"
    );
    let label = text_run_for(&wide, "PLACEMENT");
    assert_eq!(label.style.color, theme.palette.text_muted);
    let node = wide
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some("Placement"))
        .expect("detail row semantics should exist");
    assert_eq!(node.value, Some(SemanticsValue::Text(value.to_string())));
}

struct DetailRowHeightProbe {
    theme: DefaultTheme,
    measured: Rc<Cell<f32>>,
    painted: Rc<Cell<f32>>,
}

impl Widget for DetailRowHeightProbe {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(140.0, 96.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let value = "replicated across atlas, keep, and wren with one pending repair";
        self.measured.set(super::detail_row_height_for_value(
            ctx,
            &self.theme,
            ctx.bounds().width(),
            value,
            Some(2),
        ));
        self.painted.set(super::paint_detail_row_at(
            ctx,
            &self.theme,
            Point::new(ctx.bounds().x(), ctx.bounds().y()),
            ctx.bounds().width(),
            "Placement",
            value,
            Some(2),
        ));
    }
}

#[test]
fn detail_row_height_helper_matches_painter() {
    let measured = Rc::new(Cell::new(0.0));
    let painted = Rc::new(Cell::new(0.0));
    let _ = render(DetailRowHeightProbe {
        theme: DefaultTheme::default(),
        measured: Rc::clone(&measured),
        painted: Rc::clone(&painted),
    });

    assert!(measured.get() > 0.0);
    assert!((measured.get() - painted.get()).abs() < 0.01);
}

#[test]
fn section_and_detail_row_styles_preserve_compact_token_line_heights() {
    let mut theme = DefaultTheme::default();
    theme.text.xs = ThemeTextToken {
        size: 9.0,
        line_height: 11.0,
    };
    theme.text.sm = ThemeTextToken {
        size: 10.0,
        line_height: 13.0,
    };

    let section = super::section_label_text_style(&theme, None);
    assert_eq!(section.font_size, theme.text.xs.size);
    assert_eq!(section.line_height, theme.text.xs.line_height);

    let label = super::detail_row_label_style(&theme);
    assert_eq!(label.font_size, theme.text.xs.size);
    assert_eq!(label.line_height, theme.text.xs.line_height);

    let value = super::detail_row_value_style(&theme);
    assert_eq!(value.font_size, theme.text.sm.size);
    assert_eq!(value.line_height, theme.text.sm.line_height);
}

#[test]
fn section_label_uses_micro_label_token_and_text_semantics() {
    let theme = DefaultTheme::default();
    let output = render(
        crate::SizedBox::new().width(120.0).height(18.0).with_child(
            SectionLabel::new("file tasks")
                .semantic_name("File tasks")
                .theme(theme),
        ),
    );

    let label = text_run_for(&output, "FILE TASKS");
    assert_text_run_uses_token(&label, theme.text.xs);
    assert_eq!(label.style.color, theme.surfaces.text_faint);
    assert_eq!(label.style.weight, FontWeight::SEMIBOLD);

    let node = output
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some("File tasks"))
        .expect("section label semantics should exist");
    assert_eq!(node.role, SemanticsRole::Text);
    assert_eq!(
        node.value,
        Some(SemanticsValue::Text("File tasks".to_string()))
    );
}

struct SectionLabelPaintFixture {
    theme: DefaultTheme,
    color: Color,
}

impl Widget for SectionLabelPaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 18.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_section_label(
            ctx,
            &self.theme,
            ctx.bounds(),
            "placement",
            SectionLabelPaint::new().color(self.color),
        );
    }
}

#[test]
fn section_label_paint_matches_widget_style_with_color_override() {
    let theme = DefaultTheme::default();
    let color = theme.palette.warning;
    let output = render(SectionLabelPaintFixture { theme, color });

    let label = text_run_for(&output, "PLACEMENT");
    assert_text_run_uses_token(&label, theme.text.xs);
    assert_eq!(label.style.color, color);
    assert_eq!(label.style.weight, FontWeight::SEMIBOLD);
}

struct SectionLabelDetailPaintFixture {
    theme: DefaultTheme,
    color: Color,
}

impl Widget for SectionLabelDetailPaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(180.0, 18.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_section_label_detail(
            ctx,
            &self.theme,
            ctx.bounds(),
            "input",
            "command payload",
            SectionLabelPaint::new().color(self.color),
        );
    }
}

#[test]
fn section_label_detail_paint_preserves_detail_text() {
    let theme = DefaultTheme::default();
    let color = theme.palette.warning;
    let output = render(SectionLabelDetailPaintFixture { theme, color });

    let label = text_run_for(&output, "INPUT · command payload");
    assert_text_run_uses_token(&label, theme.text.xs);
    assert_eq!(label.style.color, color);
    assert_eq!(label.style.weight, FontWeight::SEMIBOLD);
}

struct GenericPaintPrimitiveFixture {
    fill: Color,
    panel_border: Color,
    right_hairline: Color,
    full_border: Color,
}

impl Widget for GenericPaintPrimitiveFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(96.0, 64.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_rounded_panel(
            ctx,
            Rect::new(4.0, 4.0, 36.0, 20.0),
            self.fill,
            self.panel_border,
            0.0,
        );
        paint_hairline(
            ctx,
            Rect::new(50.0, 6.0, 24.0, 18.0),
            HairlineEdge::Right,
            self.right_hairline,
        );
        paint_border(ctx, Rect::new(10.0, 34.0, 18.0, 12.0), self.full_border);
    }
}

#[test]
fn generic_paint_primitives_emit_expected_rects() {
    let fill = Color::rgba(0.1, 0.2, 0.3, 1.0);
    let panel_border = Color::rgba(0.4, 0.5, 0.6, 1.0);
    let right_hairline = Color::rgba(0.7, 0.2, 0.1, 1.0);
    let full_border = Color::rgba(0.2, 0.8, 0.4, 1.0);
    let output = render(GenericPaintPrimitiveFixture {
        fill,
        panel_border,
        right_hairline,
        full_border,
    });

    assert_eq!(
        solid_fill_rects_for_color(&output, fill),
        vec![Rect::new(5.0, 5.0, 34.0, 18.0)]
    );
    assert_eq!(
        solid_fill_rects_for_color(&output, panel_border),
        vec![Rect::new(4.0, 4.0, 36.0, 20.0)]
    );
    assert_eq!(
        solid_fill_rects_for_color(&output, right_hairline),
        vec![Rect::new(73.0, 6.0, 1.0, 18.0)]
    );
    assert_eq!(
        solid_fill_rects_for_color(&output, full_border),
        vec![
            Rect::new(10.0, 34.0, 18.0, 1.0),
            Rect::new(27.0, 34.0, 1.0, 12.0),
            Rect::new(10.0, 45.0, 18.0, 1.0),
            Rect::new(10.0, 34.0, 1.0, 12.0),
        ]
    );
}

#[test]
fn semantic_tones_drive_composite_status_colors() {
    let theme = DefaultTheme::default();

    let action_card = render(
        ActionCard::new("Deploy", "Publish release artifacts")
            .theme(theme)
            .tone(SemanticTone::Success),
    );
    assert!(solid_fill_colors(&action_card).contains(&theme.palette.success.with_alpha(0.78)));

    let status_bar = render(
        StatusBar::new()
            .theme(theme)
            .segment(StatusBarSegment::new("Offline").tone(SemanticTone::Warning)),
    );
    assert!(solid_fill_colors(&status_bar).contains(&theme.palette.warning.with_alpha(0.12)));

    let badge = render(
        StatusBadge::new("Replicated")
            .theme(theme)
            .tone(SemanticTone::Success)
            .icon(crate::IconGlyph::Storage),
    );
    // Mesh badges fill with the soft status wash and draw no border.
    assert!(solid_fill_colors(&badge).contains(&theme.palette.success_soft));
    assert!(!solid_stroke_colors(&badge).contains(&theme.palette.success.with_alpha(0.52)));

    let progress_bar = render(
        ProgressBar::new("Delete progress")
            .theme(theme)
            .tone(SemanticTone::Danger)
            .value(0.5),
    );
    assert!(solid_fill_colors(&progress_bar).contains(&theme.palette.danger));
}

struct CommandButtonPaintFixture {
    theme: DefaultTheme,
    style: CommandButtonPaint,
}

impl Widget for CommandButtonPaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(150.0, 28.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_command_button(
            ctx,
            &self.theme,
            ctx.bounds(),
            "Repair",
            Some(crate::IconGlyph::Storage),
            self.style,
        );
    }
}

#[test]
fn command_button_paint_uses_theme_surfaces_and_tones() {
    let theme = DefaultTheme::default();

    let neutral = render(CommandButtonPaintFixture {
        theme,
        style: CommandButtonPaint::neutral().icon_tone(SemanticTone::Accent),
    });
    assert!(solid_fill_colors(&neutral).contains(&theme.surfaces.field));
    assert!(solid_stroke_colors(&neutral).contains(&theme.palette.border));

    let hovered = render(CommandButtonPaintFixture {
        theme,
        style: CommandButtonPaint::neutral()
            .icon_tone(SemanticTone::Accent)
            .hovered(true),
    });
    assert!(solid_fill_colors(&hovered).contains(&theme.palette.control_hover));

    let accent = render(CommandButtonPaintFixture {
        theme,
        style: CommandButtonPaint::filled(SemanticTone::Accent),
    });
    assert!(solid_fill_colors(&accent).contains(&theme.palette.accent));
    assert!(solid_stroke_colors(&accent).contains(&theme.palette.accent));

    let pressed = render(CommandButtonPaintFixture {
        theme,
        style: CommandButtonPaint::filled(SemanticTone::Accent).pressed(true),
    });
    assert!(solid_fill_colors(&pressed).contains(&theme.palette.accent_pressed));

    let danger = render(CommandButtonPaintFixture {
        theme,
        style: CommandButtonPaint::tonal(SemanticTone::Danger),
    });
    assert!(solid_fill_colors(&danger).contains(&theme.surfaces.field));
    assert!(solid_stroke_colors(&danger).contains(&theme.palette.danger.with_alpha(0.72)));
}

struct DisclosureButtonPaintFixture {
    theme: DefaultTheme,
    expanded: bool,
}

impl Widget for DisclosureButtonPaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(126.0, 24.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_disclosure_button(
            ctx,
            &self.theme,
            ctx.bounds(),
            if self.expanded {
                "Show less"
            } else {
                "Show more"
            },
            self.expanded,
            DisclosureButtonPaint::new(),
        );
    }
}

#[test]
fn disclosure_button_paint_uses_accent_command_button_style() {
    let theme = DefaultTheme::default();
    let collapsed = render(DisclosureButtonPaintFixture {
        theme,
        expanded: false,
    });
    let expanded = render(DisclosureButtonPaintFixture {
        theme,
        expanded: true,
    });

    assert!(solid_fill_colors(&collapsed).contains(&theme.surfaces.field));
    assert!(solid_stroke_colors(&collapsed).contains(&theme.palette.accent.with_alpha(0.72)));
    assert_eq!(
        text_run_for(&collapsed, "Show more").style.color,
        theme.palette.accent
    );
    assert_eq!(
        text_run_for(&expanded, "Show less").style.color,
        theme.palette.accent
    );
}

struct CompactCommandButtonPaintFixture {
    theme: DefaultTheme,
}

impl Widget for CompactCommandButtonPaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(200.0, 24.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let mut natural_style =
            text_token_style(&self.theme, self.theme.text.sm, self.theme.palette.accent);
        natural_style.weight = FontWeight::SEMIBOLD;
        let natural_width = ctx
            .measure_text("Hide details", natural_style)
            .expect("command button label should be measurable")
            .width;
        let target_label_width = natural_width * 0.95;
        // Compact command buttons reserve 8px leading padding, a 12px icon,
        // a 6px gap, and 6px trailing padding around the label slot.
        let button_rect = Rect::new(
            ctx.bounds().x(),
            ctx.bounds().y(),
            target_label_width + 32.0,
            ctx.bounds().height(),
        );
        paint_command_button(
            ctx,
            &self.theme,
            button_rect,
            "Hide details",
            Some(crate::IconGlyph::ChevronUp),
            CommandButtonPaint::tonal(SemanticTone::Accent).icon_tone(SemanticTone::Accent),
        );
    }
}

#[test]
fn command_button_paint_keeps_label_token_and_clips_compact_slots() {
    let theme = DefaultTheme::default();
    let output = render(CompactCommandButtonPaintFixture { theme });
    let label = text_run_for(&output, "Hide details");
    let layout = text_layout_for(&output, "Hide details");
    let clip = clip_rect_for_text(&output, "Hide details");

    assert_text_run_uses_token(&label, theme.text.sm);
    assert_eq!(layout.lines().len(), 1, "label should not wrap");
    assert!(
        layout.measurement().width > layout.box_size().width,
        "compact command button should preserve and clip the natural label width: measurement={:?} box={:?}",
        layout.measurement(),
        layout.box_size()
    );
    assert!(
        (clip.width() - layout.box_size().width).abs() < 0.75,
        "label clip should match the allocated slot: clip={clip:?} box={:?}",
        layout.box_size()
    );
}

struct ActionTilePaintFixture {
    theme: DefaultTheme,
    style: ActionTilePaint,
}

impl Widget for ActionTilePaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(178.0, 58.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_action_tile(
            ctx,
            &self.theme,
            ctx.bounds(),
            "Check cluster",
            Some("background-agent"),
            Some(crate::IconGlyph::Storage),
            self.style,
        );
    }
}

struct ActionTileReservedSlotFixture {
    theme: DefaultTheme,
    style: ActionTilePaint,
}

impl Widget for ActionTileReservedSlotFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(178.0, 58.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_action_tile(
            ctx,
            &self.theme,
            ctx.bounds(),
            "Check cluster",
            Some("background-agent"),
            None,
            self.style,
        );
    }
}

#[test]
fn action_tile_paint_supports_highlight_hover_and_press_states() {
    let theme = DefaultTheme::default();

    let neutral = render(ActionTilePaintFixture {
        theme,
        style: ActionTilePaint::neutral(),
    });
    assert!(solid_fill_colors(&neutral).contains(&theme.palette.control));
    assert!(solid_stroke_colors(&neutral).contains(&theme.palette.border));

    let highlighted = render(ActionTilePaintFixture {
        theme,
        style: ActionTilePaint::tonal(SemanticTone::Accent),
    });
    assert!(solid_stroke_colors(&highlighted).contains(&theme.palette.accent.with_alpha(0.84)));

    let hovered = render(ActionTilePaintFixture {
        theme,
        style: ActionTilePaint::neutral().hovered(true),
    });
    assert!(solid_fill_colors(&hovered).contains(&theme.palette.control_hover));

    let pressed = render(ActionTilePaintFixture {
        theme,
        style: ActionTilePaint::neutral().pressed(true),
    });
    assert!(solid_fill_colors(&pressed).contains(&theme.palette.control_active));
}

#[test]
fn action_tile_paint_supports_surface_overrides_and_reserved_slots() {
    let theme = DefaultTheme::default();
    let output = render(ActionTileReservedSlotFixture {
        theme,
        style: ActionTilePaint::neutral()
            .background(theme.surfaces.panel)
            .border(theme.palette.warning)
            .title_color(theme.palette.text)
            .subtitle_color(theme.surfaces.text_faint)
            .icon_color(theme.palette.accent)
            .radius(theme.radius.xl)
            .padding_x(12.0)
            .leading_width(18.0)
            .trailing_width(48.0),
    });

    assert!(solid_fill_colors(&output).contains(&theme.surfaces.panel));
    assert!(solid_stroke_colors(&output).contains(&theme.palette.warning));

    let title = text_run_for(&output, "Check cluster");
    let subtitle = text_run_for(&output, "background-agent");
    assert_eq!(title.style.color, theme.palette.text);
    assert_eq!(subtitle.style.color, theme.surfaces.text_faint);
    assert!(
        title.rect.x() >= 30.0,
        "reserved leading slot should move title after the status slot: {:?}",
        title.rect
    );
    let title_clip = clip_rect_for_text(&output, "Check cluster");
    assert!(
        title_clip.max_x() <= 118.0,
        "reserved trailing slot should keep title clip clear of action area: {:?}",
        title_clip
    );
}

#[test]
fn action_tile_paint_supports_leading_semantic_status_dot() {
    let theme = DefaultTheme::default();
    let output = render(ActionTileReservedSlotFixture {
        theme,
        style: ActionTilePaint::neutral()
            .padding_x(12.0)
            .leading_tone_dot(SemanticTone::Success)
            .leading_width(18.0),
    });

    assert!(
        solid_fill_colors(&output).contains(&theme.semantic_tone_color(SemanticTone::Success)),
        "leading status dot should use semantic tone color"
    );
    let title = text_run_for(&output, "Check cluster");
    assert!(
        title.rect.x() >= 30.0,
        "leading status dot should reserve the standard leading slot: {:?}",
        title.rect
    );
}

struct CalloutPaintFixture {
    theme: DefaultTheme,
}

impl Widget for CalloutPaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(260.0, 104.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_callout(
            ctx,
            &self.theme,
            ctx.bounds(),
            Some(crate::IconGlyph::Alert),
            Some("Conflict"),
            "notes.md changed on two devices while offline. Resolve before syncing.",
            CalloutPaint::new(SemanticTone::Warning).reserved_bottom(24.0),
        );
    }
}

#[test]
fn callout_paint_uses_tone_rail_wrapped_text_and_reserved_bottom() {
    let theme = DefaultTheme::default();
    let (tone, _) = theme.semantic_tone_colors(SemanticTone::Warning);
    let (tone_soft, _) = theme.semantic_tone_soft_colors(SemanticTone::Warning);
    let output = render(CalloutPaintFixture { theme });

    assert!(solid_fill_colors(&output).contains(&tone_soft));
    assert!(solid_fill_colors(&output).contains(&tone));
    assert!(solid_stroke_colors(&output).contains(&theme.palette.border));

    let title = text_run_for(&output, "Conflict");
    assert_text_run_uses_token(&title, theme.text.sm);
    assert_eq!(title.style.color, theme.palette.text);
    assert_eq!(title.style.weight, FontWeight::SEMIBOLD);

    let body_text = "notes.md changed on two devices while offline. Resolve before syncing.";
    let body = text_run_for(&output, body_text);
    assert_text_run_uses_token(&body, theme.text.sm);
    assert_eq!(body.style.color, theme.palette.text_muted);
    let body_clip = clip_rect_for_text(&output, body_text);
    assert!(body_clip.height() <= 104.0 - 20.0 - 24.0 + 0.01);
}

struct CodePanelPaintFixture {
    theme: DefaultTheme,
    style: CodePanelPaint,
    content: Rc<Cell<Rect>>,
}

impl Widget for CodePanelPaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(180.0, 84.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let content = paint_code_panel(ctx, &self.theme, ctx.bounds(), "rust", self.style);
        self.content.set(content);
        ctx.fill_rect(content, self.theme.palette.accent.with_alpha(0.08));
    }
}

#[test]
fn code_panel_paint_uses_compact_header_and_returns_content_rect() {
    let theme = DefaultTheme::default();
    let content = Rc::new(Cell::new(Rect::ZERO));
    let output = render(CodePanelPaintFixture {
        theme,
        style: CodePanelPaint::new(),
        content: Rc::clone(&content),
    });

    assert!(solid_fill_colors(&output).contains(&theme.surfaces.field));
    assert!(solid_fill_colors(&output).contains(&theme.surfaces.titlebar));
    assert!(solid_stroke_colors(&output).contains(&theme.surfaces.border));

    let label = text_run_for(&output, "rust");
    assert_text_run_uses_token(&label, theme.text.xs);
    assert_eq!(label.style.color, theme.surfaces.text_faint);
    assert_eq!(label.style.weight, FontWeight::SEMIBOLD);

    let content = content.get();
    assert!((content.x() - 8.0).abs() < 0.01);
    assert!((content.y() - 30.0).abs() < 0.01);
    assert!((content.width() - 164.0).abs() < 0.01);
    assert!(content.height() > 0.0);
}

struct CodeLinesPaintFixture {
    theme: DefaultTheme,
    style: CodeTextPaint,
}

impl Widget for CodeLinesPaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(180.0, 46.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let first_spans = [
            CodeTextSpan::new("let").color(self.theme.palette.accent),
            CodeTextSpan::new(" value"),
        ];
        let second_spans = [CodeTextSpan::new("+ added").color(self.theme.palette.success)];
        let lines = [
            CodeTextLine::new(&first_spans),
            CodeTextLine::new(&second_spans)
                .background(self.theme.palette.success.with_alpha(0.12)),
        ];
        paint_code_lines(ctx, &self.theme, ctx.bounds(), &lines, self.style);
    }
}

#[test]
fn code_lines_paint_supports_span_colors_and_line_backgrounds() {
    let theme = DefaultTheme::default();
    let output = render(CodeLinesPaintFixture {
        theme,
        style: CodeTextPaint::new()
            .color(theme.palette.text)
            .font_size(16.0)
            .line_height(18.0),
    });

    assert!(
        solid_fill_colors(&output).contains(&theme.palette.success.with_alpha(0.12)),
        "line background should be drawn before text"
    );
    let keyword = text_run_for(&output, "let");
    assert_eq!(keyword.style.color, theme.palette.accent);
    assert_eq!(keyword.style.font_families, Some(theme.fonts.mono.into()));
    assert_eq!(keyword.style.font_size, 16.0);
    assert_eq!(keyword.style.line_height, 18.0);
    let fallback = text_run_for(&output, " value");
    assert_eq!(fallback.style.color, theme.palette.text);
    let added = text_run_for(&output, "+ added");
    assert_eq!(added.style.color, theme.palette.success);
}

#[test]
fn code_lines_default_paint_follows_the_theme_xs_token() {
    let mut theme = DefaultTheme::default();
    theme.text.xs = ThemeTextToken {
        size: 14.0,
        line_height: 21.0,
    };
    let output = render(CodeLinesPaintFixture {
        theme,
        style: CodeTextPaint::new().color(theme.palette.text),
    });

    assert_text_run_uses_token(&text_run_for(&output, "let"), theme.text.xs);
    assert_text_run_uses_token(&text_run_for(&output, " value"), theme.text.xs);
}

struct SectionPanelPaintFixture {
    theme: DefaultTheme,
    content: Rc<Cell<Rect>>,
    title: Rc<Cell<Rect>>,
}

impl Widget for SectionPanelPaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(220.0, 90.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let geometry = paint_section_panel(
            ctx,
            &self.theme,
            ctx.bounds(),
            "Metadata",
            SectionPanelPaint::new()
                .header_height(38.0)
                .title_token(self.theme.text.base)
                .trailing_width(42.0),
        );
        self.content.set(geometry.content_rect);
        self.title.set(geometry.title_rect);
        ctx.fill_rect(
            geometry.content_rect,
            self.theme.palette.accent.with_alpha(0.08),
        );
    }
}

#[test]
fn section_panel_paint_reserves_header_action_space_and_returns_content_rect() {
    let theme = DefaultTheme::default();
    let content = Rc::new(Cell::new(Rect::ZERO));
    let title_rect = Rc::new(Cell::new(Rect::ZERO));
    let output = render(SectionPanelPaintFixture {
        theme,
        content: Rc::clone(&content),
        title: Rc::clone(&title_rect),
    });

    assert!(solid_fill_colors(&output).contains(&theme.surfaces.panel));
    assert!(solid_stroke_colors(&output).contains(&theme.surfaces.border));

    let title = text_run_for(&output, "Metadata");
    assert_text_run_uses_token(&title, theme.text.base);
    assert_eq!(title.style.color, theme.surfaces.text);
    assert_eq!(title.style.weight, FontWeight::SEMIBOLD);

    let title_rect = title_rect.get();
    assert!((title_rect.x() - 12.0).abs() < 0.01);
    assert!((title_rect.width() - 154.0).abs() < 0.01);

    let content = content.get();
    assert!((content.x() - 12.0).abs() < 0.01);
    assert!((content.y() - 38.0).abs() < 0.01);
    assert!((content.width() - 196.0).abs() < 0.01);
    assert!(content.height() > 0.0);
}

#[test]
fn placement_badge_combines_status_and_replica_coverage() {
    let theme = DefaultTheme::default();
    let output = render(
        PlacementBadge::new("synced")
            .theme(theme)
            .icon(crate::IconGlyph::Storage)
            .tone(SemanticTone::Success)
            .coverage(2, 3)
            .min_width(136.0),
    );

    assert!(solid_fill_colors(&output).contains(&theme.palette.success_soft));
    assert!(
        solid_fill_colors(&output)
            .iter()
            .filter(|color| **color == theme.palette.success)
            .count()
            >= 2,
        "coverage dots should paint filled replica dots"
    );
    let node = output
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some("synced"))
        .expect("placement badge semantics should exist");
    assert_eq!(
        node.description.as_deref(),
        Some("2 of 3 replicas available")
    );
}

struct PlacementBadgePaintFixture {
    theme: DefaultTheme,
}

impl Widget for PlacementBadgePaintFixture {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(156.0, 34.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_placement_badge_with(
            ctx,
            &self.theme,
            ctx.bounds(),
            "synced",
            Some(crate::IconGlyph::Storage),
            SemanticTone::Success,
            Some((2, 3)),
            PlacementBadgePaint::new().padding(8.0, 5.0, 8.0, 5.0),
        );
    }
}

#[test]
fn placement_badge_paint_applies_cell_padding() {
    let theme = DefaultTheme::default();
    let output = render(PlacementBadgePaintFixture { theme });

    let status = text_run_for(&output, "synced");
    assert!(
        status.rect.x() >= 8.0,
        "status label should stay inside left padding: {:?}",
        status.rect
    );
    assert!(
        status.rect.y() >= 5.0,
        "status label should stay inside top padding: {:?}",
        status.rect
    );
    assert!(
        status.rect.max_x() <= 148.0,
        "status label should stay inside right padding: {:?}",
        status.rect
    );

    let coverage = text_run_for(&output, "2/3");
    assert!(
        coverage.rect.max_x() <= 148.0,
        "coverage label should stay inside right padding: {:?}",
        coverage.rect
    );
    assert!(
        solid_fill_colors(&output).contains(&theme.palette.success_soft),
        "padded placement badge should still paint the status fill"
    );
}

#[test]
fn action_card_exposes_accessible_description() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 104.0))
            .with_child(
                ActionCard::new(
                    "Paint",
                    "Pixel canvas painting workspace with editor-style panels.",
                )
                .icon(crate::IconGlyph::Brush)
                .accent(Color::rgba(0.80, 0.22, 0.44, 1.0)),
            ),
    );

    let card = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button)
        .expect("action card should expose button semantics");
    assert_eq!(card.name.as_deref(), Some("Paint"));
    assert_eq!(
        card.description.as_deref(),
        Some("Pixel canvas painting workspace with editor-style panels.")
    );
    assert_eq!(
        card.value,
        Some(SemanticsValue::Text(
            "Pixel canvas painting workspace with editor-style panels.".to_string()
        ))
    );
    assert!(card.actions.contains(&SemanticsAction::Focus));
    assert!(card.actions.contains(&SemanticsAction::Activate));
}

#[test]
fn action_card_text_visual_centers_match_title_and_description_slots() {
    let theme = DefaultTheme::default();
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 104.0))
            .with_child(
                ActionCard::new("Paint", "Pixel canvas workspace")
                    .theme(theme)
                    .icon(crate::IconGlyph::Brush),
            ),
    );
    let card = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Paint"))
        .expect("action card should expose button semantics");

    let title = text_run_for(&output, "Paint");
    let title_layout = text_run_layout(&title);
    let title_line = title_layout
        .lines()
        .first()
        .expect("action card title should contain one line");
    let title_visual_center =
        title.rect.y() + title_line.baseline + optical_visual_center(title_layout.measurement());

    let description = text_run_for(&output, "Pixel canvas workspace");
    let description_layout = text_run_layout(&description);
    let description_line = description_layout
        .lines()
        .first()
        .expect("action card description should contain one line");
    let description_visual_center = description.rect.y()
        + description_line.baseline
        + optical_visual_center(description_layout.measurement());

    let metrics = theme.metrics;
    let content = super::inset_rect(card.bounds, metrics.action_card_padding);
    let icon_extent = metrics.action_card_icon_box_size + metrics.action_card_icon_gap;
    let text_bounds = Rect::new(
        content.x() + icon_extent,
        content.y(),
        (content.width() - icon_extent - metrics.action_card_trailing_gap).max(0.0),
        content.height(),
    );
    let title_height = title
        .style
        .line_height
        .max(title_layout.measurement().height);
    let description_height = (text_bounds.height() - title_height - metrics.action_card_text_gap)
        .max(description.style.line_height)
        .min(description.style.line_height * 2.0);
    let text_block_height = title_height + metrics.action_card_text_gap + description_height;
    let text_y = text_bounds.y() + ((text_bounds.height() - text_block_height) * 0.5).max(0.0);
    let title_slot = Rect::new(text_bounds.x(), text_y, text_bounds.width(), title_height);
    let description_slot = Rect::new(
        text_bounds.x(),
        title_slot.max_y() + metrics.action_card_text_gap,
        text_bounds.width(),
        description_height,
    );

    assert!((title_visual_center - super::rect_center(title_slot).y).abs() < 0.75);
    assert!((description_visual_center - super::rect_center(description_slot).y).abs() < 0.75);
}

#[test]
fn action_card_multiline_description_stays_inside_clip_slot() {
    let description = "Catalog of controls, containers, media, and text surfaces.";
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(360.0, 104.0))
            .with_child(
                ActionCard::new("Widget book", description).icon(crate::IconGlyph::MoreHorizontal),
            ),
    );
    let run = text_run_for(&output, description);
    let clip = clip_rect_for_text(&output, description);
    let layout = TextSystem::new()
        .shape_text_run(&run, &FontRegistry::new())
        .expect("action card description should shape");

    assert!(
        layout.lines().len() > 1,
        "test description should wrap in the dev picker-sized card"
    );
    assert!(
        run.rect.y() >= clip.y() - 0.01,
        "description should start inside its clip: rect={:?}, clip={:?}",
        run.rect,
        clip
    );
    assert!(
        run.rect.max_y() <= clip.max_y() + 0.01,
        "description should end inside its clip: rect={:?}, clip={:?}",
        run.rect,
        clip
    );
}

#[test]
fn action_card_text_preserves_tall_measurements_in_compact_line_boxes() {
    let mut theme = DefaultTheme::default();
    theme.text.base = ThemeTextToken {
        size: 32.0,
        line_height: 12.0,
    };
    theme.text.sm = ThemeTextToken {
        size: 32.0,
        line_height: 12.0,
    };
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(360.0, 148.0))
            .with_child(
                ActionCard::new("Paint", "Glyph box")
                    .theme(theme)
                    .icon(crate::IconGlyph::Brush),
            ),
    );
    let card = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Paint"))
        .expect("action card should expose button semantics");
    let description = text_run_for(&output, "Glyph box");
    let description_layout = TextSystem::new()
        .shape_text_run(&description, &FontRegistry::new())
        .expect("action card description should shape");
    let title = text_run_for(&output, "Paint");
    let title_layout = TextSystem::new()
        .shape_text_run(&title, &FontRegistry::new())
        .expect("action card title should shape");
    let metrics = theme.metrics;
    let content = super::inset_rect(card.bounds, metrics.action_card_padding);
    let icon_extent = metrics.action_card_icon_box_size + metrics.action_card_icon_gap;
    let text_bounds = Rect::new(
        content.x() + icon_extent,
        content.y(),
        (content.width() - icon_extent - metrics.action_card_trailing_gap).max(0.0),
        content.height(),
    );
    let title_height = title
        .style
        .line_height
        .max(title_layout.measurement().height);
    let description_min_height = description
        .style
        .line_height
        .max(description_layout.measurement().height);
    let description_height = (text_bounds.height() - title_height - metrics.action_card_text_gap)
        .max(description_min_height)
        .min((description.style.line_height * 2.0).max(description_min_height));
    let text_block_height = title_height + metrics.action_card_text_gap + description_height;
    let text_y = text_bounds.y() + ((text_bounds.height() - text_block_height) * 0.5).max(0.0);
    let title_slot = Rect::new(text_bounds.x(), text_y, text_bounds.width(), title_height);
    let description_slot = Rect::new(
        text_bounds.x(),
        text_y + title_height + metrics.action_card_text_gap,
        text_bounds.width(),
        description_height,
    );

    assert_text_run_uses_token(&title, theme.text.base);
    assert!(
        title.rect.height() >= title_layout.measurement().height - 0.01,
        "action card title rect should preserve measured glyph height: rect={:?}, measurement={:?}",
        title.rect,
        title_layout.measurement()
    );
    assert!(
        title.rect.height() > title.style.line_height,
        "test theme should exercise a title measurement taller than line-height"
    );
    assert!(
        (text_run_visual_center(&title) - super::rect_center(title_slot).y).abs() < 0.75,
        "title text should remain optically centered in its slot"
    );
    assert_text_run_uses_token(&description, theme.text.sm);
    assert!(
        description.rect.height() >= description_layout.measurement().height - 0.01,
        "action card description rect should preserve measured glyph height: rect={:?}, measurement={:?}",
        description.rect,
        description_layout.measurement()
    );
    assert!(
        description.rect.height() > description.style.line_height * 2.0,
        "test theme should exercise measured-height preservation beyond the old two-line cap"
    );
    assert!(
        (text_run_visual_center(&description) - super::rect_center(description_slot).y).abs()
            < 0.75,
        "description text should remain optically centered in its slot"
    );
    assert!(
        description.rect.y() >= text_bounds.y(),
        "description should stay inside action card text bounds"
    );
    assert!(
        description.rect.max_y() <= text_bounds.max_y() + 0.75,
        "description should stay inside action card text bounds"
    );
}

#[test]
fn composite_default_text_styles_follow_theme_text_tokens() {
    let mut theme = DefaultTheme::default();
    theme.text.xs = ThemeTextToken {
        size: 11.0,
        line_height: 15.0,
    };
    theme.text.sm = ThemeTextToken {
        size: 15.0,
        line_height: 23.0,
    };
    theme.text.base = ThemeTextToken {
        size: 17.0,
        line_height: 25.0,
    };
    theme.sync_derived_fields();

    let action_card = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 112.0))
            .with_child(ActionCard::new("Token action", "Token action detail").theme(theme)),
    );
    assert_text_run_uses_token(&text_run_for(&action_card, "Token action"), theme.text.base);
    assert_text_run_uses_token(
        &text_run_for(&action_card, "Token action detail"),
        theme.text.sm,
    );

    let property_row = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 64.0))
            .with_child(
                PropertyRow::new("Token property", crate::Button::new("Edit"))
                    .theme(theme)
                    .inline(),
            ),
    );
    assert_text_run_uses_token(
        &text_run_for(&property_row, "Token property"),
        theme.text.sm,
    );

    let form_section = render(
        crate::SizedBox::new()
            .size(Size::new(360.0, 140.0))
            .with_child(
                FormSection::new("Token section", crate::Button::new("Apply"))
                    .theme(theme)
                    .description("Token section detail"),
            ),
    );
    assert_text_run_uses_token(&text_run_for(&form_section, "Token section"), theme.text.sm);
    assert_text_run_uses_token(
        &text_run_for(&form_section, "Token section detail"),
        theme.text.xs,
    );

    let preset_strip = render(
        crate::SizedBox::new()
            .size(Size::new(240.0, 44.0))
            .with_child(
                PresetStrip::new("Brush")
                    .theme(theme)
                    .preset("Token preset"),
            ),
    );
    assert_text_run_uses_token(
        &text_run_for(&preset_strip, "Token preset"),
        theme.text.base,
    );

    let tab_bar = render(TabBar::new("Token tabs").theme(theme).tab("Token tab bar"));
    assert_text_run_uses_token(&text_run_for(&tab_bar, "Token tab bar"), theme.text.base);

    let browser_tab_bar = render(
        BrowserTabBar::new("Token browser tabs")
            .theme(theme)
            .tabs(["Token browser tab"])
            .selected(Some(0)),
    );
    assert_text_run_uses_token(
        &text_run_for(&browser_tab_bar, "Token browser tab"),
        theme.text.base,
    );

    let tabs = render(
        Tabs::new("Token panel tabs")
            .theme(theme)
            .tab("Token panel tab", crate::SizedBox::new()),
    );
    assert_text_run_uses_token(&text_run_for(&tabs, "Token panel tab"), theme.text.base);

    let segmented_control = render(
        SegmentedControl::new("Token segments")
            .theme(theme)
            .segments(["Token segment"]),
    );
    let segment_label = text_run_for(&segmented_control, "Token segment");
    assert_text_run_uses_token(&segment_label, theme.text.base);
    assert_eq!(segment_label.style.weight, FontWeight::SEMIBOLD);

    let status_badge = render(StatusBadge::new("Token badge").theme(theme));
    let status_badge_label = text_run_for(&status_badge, "Token badge");
    assert_text_run_uses_token(&status_badge_label, theme.text.base);
    assert_eq!(status_badge_label.style.weight, FontWeight::SEMIBOLD);

    let placement_badge = render(PlacementBadge::new("Token placement").theme(theme));
    let placement_badge_label = text_run_for(&placement_badge, "Token placement");
    assert_text_run_uses_token(&placement_badge_label, theme.text.base);
    assert_eq!(placement_badge_label.style.weight, FontWeight::SEMIBOLD);

    let status_bar = render(
        crate::SizedBox::new()
            .size(Size::new(240.0, 32.0))
            .with_child(StatusBar::new().theme(theme).text_segment("Token status")),
    );
    assert_text_run_uses_token(&text_run_for(&status_bar, "Token status"), theme.text.xs);
}

#[test]
fn preset_strip_exposes_selected_preset_semantics() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(220.0, 32.0))
            .with_child(
                PresetStrip::new("Brush presets")
                    .presets(["8 px", "18 px", "36 px"])
                    .selected(1),
            ),
    );

    let strip = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Brush presets")
        })
        .expect("preset strip container semantics should exist");
    assert_eq!(strip.value, Some(SemanticsValue::Text("18 px".to_string())));
    assert!(strip.actions.contains(&SemanticsAction::SetValue));

    let selected = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("18 px"))
        .expect("selected preset button semantics should exist");
    assert!(selected.state.selected);
    assert_eq!(
        selected.value,
        Some(SemanticsValue::Text("18 px".to_string()))
    );
}

#[test]
fn preset_strip_label_clips_to_padded_item_slot() {
    let theme = DefaultTheme::default();
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(220.0, 40.0))
            .with_child(
                PresetStrip::new("Brush presets")
                    .item_width(180.0)
                    .preset("Soft"),
            ),
    );
    let preset = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Soft"))
        .expect("preset button semantics should exist");
    let text = text_run_for(&output, "Soft");
    let clip = clip_rect_for_text(&output, "Soft");
    let expected_clip = super::inset_rect(preset.bounds, theme.metrics.preset_strip_label_padding);

    assert!(
        clip.width() > text.rect.width(),
        "clip should cover the padded item slot rather than the measured text rect"
    );
    assert!((clip.x() - expected_clip.x()).abs() < 0.75);
    assert!((clip.y() - expected_clip.y()).abs() < 0.75);
    assert!((clip.width() - expected_clip.width()).abs() < 0.75);
    assert!((clip.height() - expected_clip.height()).abs() < 0.75);
}

#[test]
fn preset_strip_label_preserves_tall_measurements_and_item_centering() {
    let mut theme = DefaultTheme::default();
    theme.text.base = ThemeTextToken {
        size: 28.0,
        line_height: 12.0,
    };
    theme.sync_derived_fields();

    let output = render_isolated(
        crate::SizedBox::new()
            .size(Size::new(240.0, 56.0))
            .with_child(
                PresetStrip::new("Brush presets")
                    .theme(theme)
                    .item_height(56.0)
                    .item_width(180.0)
                    .preset("Soft"),
            ),
    );
    let preset = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Soft"))
        .expect("preset button semantics should exist");
    let text = text_run_for(&output, "Soft");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("preset label should shape");

    assert_text_run_uses_token(&text, theme.text.base);
    assert!(text.rect.height() >= layout.measurement().height - 0.01);
    assert!(text.rect.height() > text.style.line_height);
    assert!(
        (text_visual_center_for(&output, "Soft") - super::rect_center(preset.bounds).y).abs()
            < 0.75
    );
}

#[test]
fn preset_strip_pointer_activation_updates_selection() -> sui_core::Result<()> {
    let chosen = Rc::new(RefCell::new(None));
    let chosen_writer = Rc::clone(&chosen);
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(220.0, 32.0))
            .with_child(
                PresetStrip::new("Brush presets")
                    .presets(["8 px", "18 px", "36 px"])
                    .selected(0)
                    .on_change(move |index, label| {
                        *chosen_writer.borrow_mut() = Some((index, label));
                    }),
            ),
    );
    let output = runtime.render(window_id)?;
    let preset = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("36 px"))
        .expect("target preset button should exist");
    let position = super::rect_center(preset.bounds);

    let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
    move_event.pointer_id = 1;
    runtime.handle_event(window_id, Event::Pointer(move_event))?;

    let mut down = PointerEvent::new(PointerEventKind::Down, position);
    down.pointer_id = 1;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime.handle_event(window_id, Event::Pointer(down))?;

    let mut up = PointerEvent::new(PointerEventKind::Up, position);
    up.pointer_id = 1;
    up.button = Some(PointerButton::Primary);
    runtime.handle_event(window_id, Event::Pointer(up))?;

    assert_eq!(*chosen.borrow(), Some((2, "36 px".to_string())));
    let output = runtime.render(window_id)?;
    assert!(output.semantics.iter().any(|node| {
        node.role == SemanticsRole::Button
            && node.name.as_deref() == Some("36 px")
            && node.state.selected
    }));
    Ok(())
}

#[test]
fn preset_strip_hover_and_press_use_theme_motion() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let hover_duration = theme.motion.hover_duration();
    let press_duration = theme.motion.press_duration();
    let expected_hover = super::mix_color(
        theme.palette.surface,
        theme.palette.control_hover,
        theme.interaction.hover_blend,
    );
    let expected_press = super::mix_color(
        expected_hover,
        theme.palette.control_active,
        theme.interaction.pressed_blend,
    );
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(220.0, 32.0))
            .with_child(
                PresetStrip::new("Brush presets")
                    .theme(theme)
                    .presets(["8 px", "18 px", "36 px"]),
            ),
    );
    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let preset = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("18 px"))
        .expect("target preset button should exist");
    let position = super::rect_center(preset.bounds);

    let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
    move_event.pointer_id = 1;
    runtime
        .handle_event(window_id, Event::Pointer(move_event))
        .map_err(|error| error.to_string())?;

    runtime.tick(hover_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_hover = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !solid_fill_colors(&mid_hover).contains(&expected_hover),
        "hover fill should not snap to the settled hover color"
    );

    runtime.tick(hover_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_hover = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )
        .map_err(|error| error.to_string())?;

    runtime.tick(hover_duration + press_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_press = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !solid_fill_colors(&mid_press).contains(&expected_press),
        "press fill should not snap to the settled pressed color"
    );

    runtime.tick(hover_duration + press_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_press = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&settled_press).contains(&expected_press));

    Ok(())
}

#[test]
fn status_bar_exposes_dynamic_segment_semantics() {
    let theme = DefaultTheme::default();
    let zoom = Rc::new(RefCell::new("Zoom 35%".to_string()));
    let zoom_reader = Rc::clone(&zoom);
    let output = render(
        StatusBar::new()
            .name("Editor status")
            .segment(StatusBarSegment::new("Ready").min_width(80.0))
            .segment(
                StatusBarSegment::dynamic("Zoom --", move || zoom_reader.borrow().clone())
                    .min_width(120.0),
            ),
    );

    let status = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Editor status")
        })
        .expect("status bar container semantics should exist");
    assert_eq!(status.bounds.height(), theme.metrics.status_bar_height);
    assert!(output.semantics.iter().any(|node| {
        node.role == SemanticsRole::Text && node.name.as_deref() == Some("Zoom 35%")
    }));
}

#[test]
fn status_bar_description_is_exposed_to_semantics() {
    let summary = Rc::new(RefCell::new(
        "SIFS online | Path /services | idle".to_string(),
    ));
    let summary_reader = Rc::clone(&summary);
    let output = render(
        StatusBar::new()
            .name("Files status")
            .description_when(move || summary_reader.borrow().clone())
            .segment(StatusBarSegment::new("Path /services").expand(true)),
    );

    let status = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Files status")
        })
        .expect("status bar container semantics should exist");
    assert_eq!(
        status.value,
        Some(SemanticsValue::Text(
            "SIFS online | Path /services | idle".to_string(),
        ))
    );
    assert_eq!(
        status.description.as_deref(),
        Some("SIFS online | Path /services | idle")
    );
}

#[test]
fn status_badge_publishes_text_semantics_and_theme_token_text() {
    let theme = DefaultTheme::default();
    let output = render(
        StatusBadge::new("Primary on atlas")
            .theme(theme)
            .tone(SemanticTone::Accent)
            .icon(crate::IconGlyph::Storage),
    );
    let node = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Primary on atlas")
        })
        .expect("status badge should publish text semantics");
    assert_eq!(
        node.value,
        Some(SemanticsValue::Text("Primary on atlas".to_string()))
    );
    assert_text_run_uses_token(&text_run_for(&output, "Primary on atlas"), theme.text.base);
}

#[test]
fn coverage_dots_publish_replica_like_coverage_semantics_and_token_text() {
    let theme = DefaultTheme::default();
    let output = render(
        CoverageDots::new("Replicas", 2, 3)
            .theme(theme)
            .tone(SemanticTone::Warning),
    );
    let node = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Replicas"))
        .expect("coverage dots should publish text semantics");
    assert_eq!(node.value, Some(SemanticsValue::Text("2/3".to_string())));
    assert_eq!(node.description.as_deref(), Some("2 of 3 covered"));
    assert_text_run_uses_token(&text_run_for(&output, "2/3"), theme.text.xs);
    assert!(contains_approx_color(
        &solid_fill_colors(&output),
        theme.semantic_tone_colors(SemanticTone::Warning).0,
    ));
}

#[test]
fn status_bar_sizes_segments_from_measured_text() {
    let theme = DefaultTheme::default();
    let text = "Layer Paint / Normal / 100% / Unlocked";
    let output = render(
        StatusBar::new()
            .name("Editor status")
            .segment(StatusBarSegment::new(text))
            .segment(StatusBarSegment::new("Cursor --").expand(true)),
    );

    let layer = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("Layer Paint / Normal / 100% / Unlocked")
        })
        .expect("long status segment should expose text semantics");
    let measured_width = text_layout_for(&output, text).measurement().width;
    let expected_width = (measured_width + theme.metrics.status_bar_segment_padding * 2.0)
        .ceil()
        .max(theme.metrics.status_bar_segment_min_width);
    assert!(
        (layer.bounds.width() - expected_width).abs() < 0.01,
        "expected status segment width {expected_width} from text measurement, got {:?}",
        layer.bounds
    );
}

#[test]
fn status_bar_numeric_segments_use_tabular_figures_without_forcing_plain_labels() {
    let output = render(
        StatusBar::new()
            .name("Editor status")
            .segment(StatusBarSegment::new("Ready").min_width(80.0))
            .segment(StatusBarSegment::new("Zoom 35%").min_width(120.0)),
    );

    let ready = text_run_for(&output, "Ready");
    let zoom = text_run_for(&output, "Zoom 35%");

    assert!(
        !ready
            .style
            .features
            .iter()
            .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
    );
    assert!(
        zoom.style
            .features
            .iter()
            .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
    );
}

#[test]
fn status_bar_segment_text_visual_center_matches_segment_center() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(220.0, 40.0))
            .with_child(
                StatusBar::new()
                    .height(40.0)
                    .segment(StatusBarSegment::new("Ready").min_width(96.0)),
            ),
    );
    let text = text_run_for(&output, "Ready");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("status segment text should shape");
    let line = layout
        .lines()
        .first()
        .expect("status segment text should contain one line");
    let actual_visual_center =
        text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
    let segment = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Ready"))
        .expect("status segment semantics should exist");
    let segment_center = segment.bounds.y() + (segment.bounds.height() * 0.5);

    assert!((actual_visual_center - segment_center).abs() < 0.75);
}

#[test]
fn status_bar_segments_preserve_tall_measurements_and_numeric_features() {
    let mut theme = DefaultTheme::default();
    theme.text.xs = ThemeTextToken {
        size: 28.0,
        line_height: 10.0,
    };
    theme.metrics.status_bar_height = 52.0;
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(360.0, 52.0))
            .with_child(
                StatusBar::new()
                    .theme(theme)
                    .height(52.0)
                    .segment(StatusBarSegment::new("Ready").min_width(120.0))
                    .segment(StatusBarSegment::new("Zoom 35%").min_width(140.0)),
            ),
    );
    for label in ["Ready", "Zoom 35%"] {
        let text = text_run_for(&output, label);
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("status segment text should shape");
        let segment = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some(label))
            .expect("status segment semantics should exist");
        let segment_center = segment.bounds.y() + (segment.bounds.height() * 0.5);

        assert_text_run_uses_token(&text, theme.text.xs);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
        assert!((text_run_visual_center(&text) - segment_center).abs() < 0.75);
    }

    let ready = text_run_for(&output, "Ready");
    let zoom = text_run_for(&output, "Zoom 35%");
    assert!(
        !ready
            .style
            .features
            .iter()
            .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
    );
    assert!(
        zoom.style
            .features
            .iter()
            .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
    );
}

#[test]
fn horizontal_toolbar_centers_children_and_exposes_group_semantics() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 52.0))
            .with_child(
                Toolbar::horizontal()
                    .name("Editor toolbar")
                    .with_child(crate::Button::new("Fit").min_width(48.0).min_height(32.0))
                    .with_child(
                        crate::Button::new("Export")
                            .min_width(72.0)
                            .min_height(32.0),
                    ),
            ),
    );

    let toolbar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Editor toolbar")
        })
        .expect("toolbar semantics should exist");
    assert_eq!(toolbar.bounds, Rect::new(0.0, 0.0, 320.0, 52.0));

    let fit = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Fit"))
        .expect("toolbar child button should exist");
    assert!(fit.bounds.y() > 0.0);
    assert!(fit.bounds.max_y() < toolbar.bounds.max_y());
}

#[test]
fn command_group_keeps_natural_size_and_exposes_group_semantics() {
    let theme = DefaultTheme::default();
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 48.0))
            .with_child(
                Toolbar::horizontal()
                    .name("Editor toolbar")
                    .padding(sui_layout::Padding::all(4.0))
                    .with_child(
                        CommandGroup::horizontal("History commands")
                            .with_child(
                                crate::IconButton::new(crate::IconGlyph::Undo, "Undo").size(28.0),
                            )
                            .with_child(
                                crate::IconButton::new(crate::IconGlyph::Redo, "Redo").size(28.0),
                            ),
                    ),
            ),
    );

    let group = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("History commands")
        })
        .expect("command group semantics should exist");
    let button_size = 28.0_f32.max(theme.metrics.icon_button_size);
    let expected_width = button_size * 2.0
        + theme.metrics.command_group_spacing
        + theme.metrics.command_group_padding.left
        + theme.metrics.command_group_padding.right;
    let expected_height = button_size
        + theme.metrics.command_group_padding.top
        + theme.metrics.command_group_padding.bottom;
    assert!(
        (group.bounds.width() - expected_width).abs() < 0.01,
        "expected command group width {expected_width}, got {}",
        group.bounds.width()
    );
    assert!(
        (group.bounds.height() - expected_height).abs() < 0.01,
        "expected command group height {expected_height}, got {}",
        group.bounds.height()
    );

    for name in ["Undo", "Redo"] {
        let button = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some(name))
            .expect("command button should exist");
        assert!(button.bounds.x() >= group.bounds.x());
        assert!(button.bounds.max_x() <= group.bounds.max_x());
    }
}

#[test]
fn vertical_toolbar_uses_fixed_extent_and_centers_children() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(80.0, 180.0))
            .with_child(
                Toolbar::vertical()
                    .name("Paint tools")
                    .extent(60.0)
                    .with_child(
                        crate::IconButton::new(crate::IconGlyph::Brush, "Brush tool").size(44.0),
                    )
                    .with_child(
                        crate::IconButton::new(crate::IconGlyph::Eraser, "Eraser tool").size(44.0),
                    ),
            ),
    );

    let toolbar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Paint tools")
        })
        .expect("vertical toolbar semantics should exist");
    assert_eq!(toolbar.bounds, Rect::new(0.0, 0.0, 80.0, 180.0));

    let brush = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Brush tool")
        })
        .expect("toolbar child button should exist");
    assert_eq!(brush.bounds.width(), 44.0);
    assert!((brush.bounds.x() - 18.0).abs() < 0.001);
}

#[test]
fn tool_palette_exposes_selected_tool_semantics() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(64.0, 180.0))
            .with_child(
                ToolPalette::vertical("Paint tools")
                    .items([
                        ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush tool"),
                        ToolPaletteItem::new(crate::IconGlyph::Eraser, "Eraser tool"),
                        ToolPaletteItem::new(crate::IconGlyph::PaintBucket, "Fill tool"),
                    ])
                    .selected(1),
            ),
    );

    let palette = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Paint tools")
        })
        .expect("tool palette container semantics should exist");
    assert_eq!(
        palette.value,
        Some(SemanticsValue::Text("Eraser tool".to_string()))
    );
    assert!(palette.actions.contains(&SemanticsAction::Focus));
    assert!(palette.actions.contains(&SemanticsAction::SetValue));

    let selected = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Eraser tool")
        })
        .expect("selected tool button semantics should exist");
    assert!(selected.state.selected);
    assert!(selected.actions.contains(&SemanticsAction::Activate));
}

#[test]
fn tool_palette_pointer_activation_updates_selection() -> sui_core::Result<()> {
    let chosen = Rc::new(RefCell::new(None));
    let chosen_writer = Rc::clone(&chosen);
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(64.0, 180.0))
            .with_child(
                ToolPalette::vertical("Paint tools")
                    .items([
                        ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush tool"),
                        ToolPaletteItem::new(crate::IconGlyph::Eraser, "Eraser tool"),
                        ToolPaletteItem::new(crate::IconGlyph::PaintBucket, "Fill tool"),
                    ])
                    .selected(0)
                    .on_change(move |index, label| {
                        *chosen_writer.borrow_mut() = Some((index, label));
                    }),
            ),
    );
    let output = runtime.render(window_id)?;
    let fill = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Fill tool")
        })
        .expect("fill tool button semantics should exist");
    let position = super::rect_center(fill.bounds);

    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Down, position, true),
    )?;
    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Up, position, false),
    )?;

    assert_eq!(*chosen.borrow(), Some((2, "Fill tool".to_string())));
    let output = runtime.render(window_id)?;
    assert!(output.semantics.iter().any(|node| {
        node.role == SemanticsRole::Button
            && node.name.as_deref() == Some("Fill tool")
            && node.state.selected
    }));
    Ok(())
}

#[test]
fn tool_palette_hover_and_press_use_theme_motion() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let hover_duration = theme.motion.hover_duration();
    let press_duration = theme.motion.press_duration();
    let expected_hover = super::mix_color(
        theme.palette.surface,
        theme.palette.control_hover,
        theme.interaction.hover_blend,
    );
    let expected_press = super::mix_color(
        expected_hover,
        theme.palette.control_active,
        theme.interaction.pressed_blend,
    );
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(64.0, 180.0))
            .with_child(ToolPalette::vertical("Paint tools").theme(theme).items([
                ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush tool"),
                ToolPaletteItem::new(crate::IconGlyph::Eraser, "Eraser tool"),
                ToolPaletteItem::new(crate::IconGlyph::PaintBucket, "Fill tool"),
            ])),
    );
    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let eraser = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Eraser tool")
        })
        .expect("eraser tool button semantics should exist");
    let position = super::rect_center(eraser.bounds);

    let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
    move_event.pointer_id = 1;
    runtime
        .handle_event(window_id, Event::Pointer(move_event))
        .map_err(|error| error.to_string())?;

    runtime.tick(hover_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_hover = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !solid_fill_colors(&mid_hover).contains(&expected_hover),
        "tool hover fill should not snap to the settled hover color"
    );

    runtime.tick(hover_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_hover = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )
        .map_err(|error| error.to_string())?;

    runtime.tick(hover_duration + press_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_press = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !solid_fill_colors(&mid_press).contains(&expected_press),
        "tool press fill should not snap to the settled pressed color"
    );

    runtime.tick(hover_duration + press_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_press = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&settled_press).contains(&expected_press));

    Ok(())
}

#[test]
fn tool_palette_keyboard_moves_between_tools() -> sui_core::Result<()> {
    let chosen = Rc::new(RefCell::new(Vec::new()));
    let chosen_writer = Rc::clone(&chosen);
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(64.0, 180.0))
            .with_child(
                ToolPalette::vertical("Paint tools")
                    .items([
                        ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush tool"),
                        ToolPaletteItem::new(crate::IconGlyph::Eraser, "Eraser tool"),
                        ToolPaletteItem::new(crate::IconGlyph::PaintBucket, "Fill tool"),
                    ])
                    .selected(0)
                    .on_change(move |index, label| {
                        chosen_writer.borrow_mut().push((index, label));
                    }),
            ),
    );
    let output = runtime.render(window_id)?;
    let brush = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Brush tool")
        })
        .expect("brush tool button semantics should exist");
    let position = super::rect_center(brush.bounds);

    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Down, position, true),
    )?;
    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Up, position, false),
    )?;
    runtime.handle_event(
        window_id,
        Event::Keyboard(KeyboardEvent::new("ArrowDown", KeyState::Pressed)),
    )?;

    assert_eq!(
        chosen.borrow().last(),
        Some(&(1, "Eraser tool".to_string()))
    );
    let output = runtime.render(window_id)?;
    assert!(output.semantics.iter().any(|node| {
        node.role == SemanticsRole::Button
            && node.name.as_deref() == Some("Eraser tool")
            && node.state.selected
    }));
    Ok(())
}

#[test]
fn property_row_stacked_exposes_label_and_control_semantics() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 72.0))
            .with_child(
                PropertyRow::new("Brush size", crate::NumberInput::new("Brush size"))
                    .control_width(120.0),
            ),
    );

    let row = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Brush size")
        })
        .expect("property row semantics should exist");
    assert_eq!(row.bounds, Rect::new(0.0, 0.0, 320.0, 72.0));

    let label = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Brush size"))
        .expect("property label semantics should exist");
    let control = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::SpinBox && node.name.as_deref() == Some("Brush size")
        })
        .expect("property control semantics should exist");
    assert_eq!(control.bounds.width(), 120.0);
    assert!(control.bounds.y() > label.bounds.y());
}

#[test]
fn property_row_inline_arranges_control_after_label() {
    let theme = DefaultTheme::default();
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 36.0))
            .with_child(
                PropertyRow::new("Opacity", crate::Slider::new("Opacity"))
                    .layout(PropertyRowLayout::Inline)
                    .label_width(96.0),
            ),
    );

    let label = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Opacity"))
        .expect("inline property label semantics should exist");
    let control = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Slider && node.name.as_deref() == Some("Opacity"))
        .expect("inline property control semantics should exist");
    assert!(control.bounds.x() > label.bounds.max_x());
    let expected_width = 320.0 - 96.0 - theme.metrics.property_row_inline_gap;
    assert!((control.bounds.width() - expected_width).abs() < 0.01);
}

#[test]
fn property_row_inline_label_visual_center_matches_row_center() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 36.0))
            .with_child(
                PropertyRow::new("Opacity", crate::Slider::new("Opacity"))
                    .layout(PropertyRowLayout::Inline)
                    .label_width(96.0),
            ),
    );
    let text = text_run_for(&output, "Opacity");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("property row label should shape");
    let line = layout
        .lines()
        .first()
        .expect("property row label should contain one line");
    let actual_visual_center =
        text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
    let row_center = output.frame.viewport.height * 0.5;

    assert!((actual_visual_center - row_center).abs() < 0.75);
}

#[test]
fn property_row_numeric_control_aligns_value_to_control_edge() {
    let theme = DefaultTheme::default();
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 36.0))
            .with_child(
                PropertyRow::new(
                    "Brush size",
                    crate::NumberInput::new("Brush size")
                        .precision(0)
                        .value(128.0),
                )
                .layout(PropertyRowLayout::Inline)
                .label_width(96.0)
                .control_width(120.0),
            ),
    );
    let value = text_run_for(&output, "128");
    let label = text_run_for(&output, "Brush size");
    let control = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::SpinBox && node.name.as_deref() == Some("Brush size")
        })
        .expect("number input semantics should exist");
    let expected_right = control.bounds.max_x()
        - theme.metrics.number_input_stepper_width
        - theme.metrics.text_input_padding.right;

    assert!(
        value
            .style
            .features
            .iter()
            .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
    );
    assert!((value.rect.max_x() - expected_right).abs() < 1.0);
    assert!(
        (text_run_visual_center(&value) - (control.bounds.y() + control.bounds.height() * 0.5))
            .abs()
            < 0.75,
        "property row numeric value should remain optically centered in the control"
    );
    assert!(
        (text_run_visual_center(&value) - text_run_visual_center(&label)).abs() < 0.75,
        "property row label and numeric value should share a visual baseline"
    );
}

#[test]
fn property_row_inline_label_preserves_tall_metrics_with_numeric_control() {
    let mut theme = DefaultTheme::default();
    theme.text.sm = ThemeTextToken {
        size: 28.0,
        line_height: 10.0,
    };
    theme.sync_derived_fields();
    theme.metrics.min_height = 56.0;
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(380.0, 64.0))
            .with_child(
                PropertyRow::new(
                    "Brush size",
                    crate::NumberInput::new("Brush size")
                        .theme(theme)
                        .precision(0)
                        .value(128.0),
                )
                .theme(theme)
                .layout(PropertyRowLayout::Inline)
                .label_width(132.0)
                .control_width(150.0),
            ),
    );
    let label = text_run_for(&output, "Brush size");
    let label_layout = TextSystem::new()
        .shape_text_run(&label, &FontRegistry::new())
        .expect("property row label should shape");
    let value = text_run_for(&output, "128");
    let row_center = output.frame.viewport.height * 0.5;
    let control = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::SpinBox && node.name.as_deref() == Some("Brush size")
        })
        .expect("number input semantics should exist");
    let expected_right = control.bounds.max_x()
        - theme.metrics.number_input_stepper_width
        - theme.metrics.text_input_padding.right;

    assert_text_run_uses_token(&label, theme.text.sm);
    assert!(label.rect.height() >= label_layout.measurement().height - 0.01);
    assert!(label.rect.height() > label.style.line_height);
    assert!((text_run_visual_center(&label) - row_center).abs() < 0.75);
    assert!((value.rect.max_x() - expected_right).abs() < 1.0);
    assert!(
        (text_run_visual_center(&value) - text_run_visual_center(&label)).abs() < 0.75,
        "property row label and numeric value should share a visual baseline for tall metrics"
    );
}

#[test]
fn property_row_label_id_is_javascript_safe() {
    let id = super::property_row_label_id(WidgetId::new(402)).get();

    assert!(id < (1_u64 << 53));
}

#[test]
fn form_row_control_uses_full_available_width_by_default() {
    let theme = DefaultTheme::default();
    let width = 760.0;
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(width, 36.0))
            .with_child(FormRow::new(
                "Model",
                crate::TextInput::new("provider-model"),
            )),
    );

    let control = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::TextInput && node.name.as_deref() == Some("provider-model")
        })
        .expect("form row control semantics should exist");
    let expected_width = width - theme.metrics.form_row_label_width - theme.metrics.form_row_gap;
    assert!(
        (control.bounds.width() - expected_width).abs() < 0.01,
        "form controls should use the card's available row width"
    );
    assert!((control.bounds.max_x() - width).abs() < 0.01);
}

#[test]
fn form_section_bounds_grouped_rows_and_exposes_semantics() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(900.0, 180.0))
            .with_child(
                FormSection::new(
                    "Providers",
                    FieldGroup::new()
                        .with_child(FormRow::new("API key", crate::Label::new("Configured")))
                        .with_child(FormRow::new(
                            "Default model",
                            crate::Label::new("Provider default"),
                        )),
                )
                .description("Credentials and model defaults"),
            ),
    );

    let theme = DefaultTheme::default();
    assert!(
        solid_fill_colors(&output).contains(&theme.surfaces.panel),
        "form section card fill should use the surface panel token"
    );

    let section = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Providers")
        })
        .expect("form section semantics should exist");
    let padding = theme.metrics.form_section_padding;
    assert!(
        section.bounds.width()
            <= theme.metrics.form_section_max_width + padding.left + padding.right
    );
    assert!(
        section.bounds.x() > 100.0,
        "wide parent should center a max-width form section"
    );
    assert_eq!(
        section.description.as_deref(),
        Some("Credentials and model defaults")
    );
    assert!(
        output
            .semantics
            .iter()
            .any(|node| node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("Default model"))
    );
}

#[test]
fn form_section_header_text_block_centers_against_tall_header_action() {
    let theme = DefaultTheme::default();
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(420.0, 140.0))
            .with_child(
                FormSection::new("Providers", crate::Label::new("Configured"))
                    .theme(theme)
                    .description("Credentials and defaults")
                    .header_action(
                        crate::SizedBox::new()
                            .size(Size::new(76.0, 52.0))
                            .with_child(crate::Label::new("Sync")),
                    ),
            ),
    );
    let section = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Providers")
        })
        .expect("form section semantics should exist");

    let title = text_run_for(&output, "Providers");
    let title_layout = text_run_layout(&title);
    let title_line = title_layout
        .lines()
        .first()
        .expect("form section title should contain one line");
    let title_visual_center =
        title.rect.y() + title_line.baseline + optical_visual_center(title_layout.measurement());

    let description = text_run_for(&output, "Credentials and defaults");
    let description_layout = text_run_layout(&description);
    let description_line = description_layout
        .lines()
        .first()
        .expect("form section description should contain one line");
    let description_visual_center = description.rect.y()
        + description_line.baseline
        + optical_visual_center(description_layout.measurement());

    let metrics = theme.metrics;
    let content = super::inset_rect(section.bounds, metrics.form_section_padding);
    let header_gap = metrics.form_section_header_gap;
    let action_width = (76.0 + header_gap).min(content.width());
    let text_width = (content.width() - action_width).max(0.0);
    let title_height = title
        .style
        .line_height
        .max(title_layout.measurement().height);
    let description_height = description
        .style
        .line_height
        .max(description_layout.measurement().height);
    let text_block_height =
        title_height + metrics.form_section_description_gap + description_height;
    let header_height = text_block_height.max(52.0);
    let text_y = content.y() + ((header_height - text_block_height) * 0.5).max(0.0);
    let title_slot = Rect::new(content.x(), text_y, text_width, title_height);
    let description_slot = Rect::new(
        content.x(),
        title_slot.max_y() + metrics.form_section_description_gap,
        text_width,
        description_height,
    );

    assert!((title_visual_center - super::rect_center(title_slot).y).abs() < 0.75);
    assert!((description_visual_center - super::rect_center(description_slot).y).abs() < 0.75);
}

#[test]
fn form_section_header_text_preserves_tall_measurements_in_compact_line_boxes() {
    let mut theme = DefaultTheme::default();
    theme.text.sm = ThemeTextToken {
        size: 30.0,
        line_height: 10.0,
    };
    theme.text.xs = ThemeTextToken {
        size: 28.0,
        line_height: 10.0,
    };
    theme.sync_derived_fields();

    let output = render(
        crate::SizedBox::new()
            .size(Size::new(460.0, 190.0))
            .with_child(
                FormSection::new("Providers", crate::Label::new("Configured"))
                    .theme(theme)
                    .description("Credentials and defaults")
                    .header_action(
                        crate::SizedBox::new()
                            .size(Size::new(76.0, 52.0))
                            .with_child(crate::Label::new("Sync")),
                    ),
            ),
    );
    let section = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Providers")
        })
        .expect("form section semantics should exist");
    let title = text_run_for(&output, "Providers");
    let title_layout = TextSystem::new()
        .shape_text_run(&title, &FontRegistry::new())
        .expect("form section title should shape");
    let description = text_run_for(&output, "Credentials and defaults");
    let description_layout = TextSystem::new()
        .shape_text_run(&description, &FontRegistry::new())
        .expect("form section description should shape");
    let metrics = theme.metrics;
    let content = super::inset_rect(section.bounds, metrics.form_section_padding);
    let action_width = (76.0 + metrics.form_section_header_gap).min(content.width());
    let text_width = (content.width() - action_width).max(0.0);
    let title_height = title
        .style
        .line_height
        .max(title_layout.measurement().height);
    let description_height = description
        .style
        .line_height
        .max(description_layout.measurement().height);
    let text_block_height =
        title_height + metrics.form_section_description_gap + description_height;
    let header_height = text_block_height.max(52.0);
    let text_y = content.y() + ((header_height - text_block_height) * 0.5).max(0.0);
    let title_slot = Rect::new(content.x(), text_y, text_width, title_height);
    let description_slot = Rect::new(
        content.x(),
        title_slot.max_y() + metrics.form_section_description_gap,
        text_width,
        description_height,
    );

    assert_text_run_uses_token(&title, theme.text.sm);
    assert_text_run_uses_token(&description, theme.text.xs);
    assert!(title.rect.height() >= title_layout.measurement().height - 0.01);
    assert!(title.rect.height() > title.style.line_height);
    assert!(description.rect.height() >= description_layout.measurement().height - 0.01);
    assert!(description.rect.height() > description.style.line_height);
    assert!((text_run_visual_center(&title) - super::rect_center(title_slot).y).abs() < 0.75);
    assert!(
        (text_run_visual_center(&description) - super::rect_center(description_slot).y).abs()
            < 0.75
    );
}

#[test]
fn panel_section_exposes_group_title_and_child_semantics() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(240.0, 92.0))
            .with_child(PanelSection::new("Brush", crate::Label::new("Opacity"))),
    );

    let section = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer && node.name.as_deref() == Some("Brush")
        })
        .expect("panel section group semantics should exist");
    let title = output
        .semantics
        .iter()
        .find(|node| {
            node.parent == Some(section.id)
                && node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("Brush")
        })
        .expect("panel section title semantics should exist");
    let child = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Opacity"))
        .expect("panel section child semantics should exist");

    assert!(child.bounds.y() > title.bounds.max_y());
}

#[test]
fn panel_section_header_action_is_arranged_after_title() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(240.0, 92.0))
            .with_child(
                PanelSection::new("Layers", crate::Label::new("Paint"))
                    .header_action(crate::IconButton::new(crate::IconGlyph::Add, "Add layer")),
            ),
    );

    let section = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer && node.name.as_deref() == Some("Layers")
        })
        .expect("panel section group semantics should exist");
    let title = output
        .semantics
        .iter()
        .find(|node| {
            node.parent == Some(section.id)
                && node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("Layers")
        })
        .expect("panel section title semantics should exist");
    let action = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Add layer")
        })
        .expect("panel section header action semantics should exist");
    let child = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Paint"))
        .expect("panel section child semantics should exist");

    assert!(action.bounds.x() > title.bounds.x());
    assert!(child.bounds.y() > action.bounds.max_y());
}

#[test]
fn panel_section_title_visual_center_matches_title_slot_center() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(240.0, 92.0))
            .with_child(
                PanelSection::new("Layers", crate::Label::new("Paint"))
                    .header_action(crate::IconButton::new(crate::IconGlyph::Add, "Add layer")),
            ),
    );
    let section = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer && node.name.as_deref() == Some("Layers")
        })
        .expect("panel section group semantics should exist");
    let title_slot = output
        .semantics
        .iter()
        .find(|node| {
            node.parent == Some(section.id)
                && node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("Layers")
        })
        .expect("panel section title semantics should exist")
        .bounds;
    let text = text_run_for(&output, "Layers");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("panel section title should shape");
    let line = layout
        .lines()
        .first()
        .expect("panel section title should contain one line");
    let actual_visual_center =
        text.rect.y() + line.baseline + optical_visual_center(layout.measurement());

    assert!((actual_visual_center - super::rect_center(title_slot).y).abs() < 0.75);
}

#[test]
fn panel_section_title_preserves_tall_measurement_and_header_centering() {
    let mut theme = DefaultTheme::default();
    theme.text.xs = ThemeTextToken {
        size: 30.0,
        line_height: 10.0,
    };
    theme.sync_derived_fields();

    let output = render(
        crate::SizedBox::new()
            .size(Size::new(280.0, 120.0))
            .with_child(
                PanelSection::new("Layers", crate::Label::new("Paint"))
                    .theme(theme)
                    .header_action(crate::IconButton::new(crate::IconGlyph::Add, "Add layer")),
            ),
    );
    let section = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer && node.name.as_deref() == Some("Layers")
        })
        .expect("panel section group semantics should exist");
    let title_slot = output
        .semantics
        .iter()
        .find(|node| {
            node.parent == Some(section.id)
                && node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("Layers")
        })
        .expect("panel section title semantics should exist")
        .bounds;
    let title = text_run_for(&output, "Layers");
    let layout = TextSystem::new()
        .shape_text_run(&title, &FontRegistry::new())
        .expect("panel section title should shape");

    assert_text_run_uses_token(&title, theme.text.xs);
    assert!(title.rect.height() >= layout.measurement().height - 0.01);
    assert!(title.rect.height() > title.style.line_height);
    assert!((text_run_visual_center(&title) - super::rect_center(title_slot).y).abs() < 0.75);
}

#[test]
fn collapsible_panel_section_hides_collapsed_child_semantics() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(240.0, 92.0))
            .with_child(
                PanelSection::new("Advanced color", crate::Label::new("RGB sliders"))
                    .collapsible(true)
                    .collapsed(),
            ),
    );

    let section = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Advanced color")
        })
        .expect("collapsible panel section semantics should exist");
    assert_eq!(section.state.expanded, Some(false));
    assert!(section.actions.contains(&SemanticsAction::Expand));
    assert!(
        !output
            .semantics
            .iter()
            .any(|node| node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("RGB sliders")),
        "collapsed section should not expose hidden child semantics"
    );
}

#[test]
fn collapsible_panel_section_pointer_toggle_exposes_child() -> sui_core::Result<()> {
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(240.0, 120.0))
            .with_child(
                PanelSection::new("Advanced color", crate::Label::new("RGB sliders"))
                    .collapsible(true)
                    .collapsed(),
            ),
    );
    let output = runtime.render(window_id)?;
    let section = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Advanced color")
        })
        .expect("collapsible panel section semantics should exist");
    let position = Point::new(section.bounds.x() + 20.0, section.bounds.y() + 8.0);

    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Move, position, false),
    )?;
    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Down, position, true),
    )?;
    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Up, position, false),
    )?;

    let output = runtime.render(window_id)?;
    let section = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Advanced color")
        })
        .expect("collapsible panel section semantics should still exist");
    assert_eq!(section.state.expanded, Some(true));
    assert!(
        output
            .semantics
            .iter()
            .any(|node| node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("RGB sliders")),
        "expanded section should expose child semantics"
    );

    Ok(())
}

#[test]
fn collapsible_panel_section_header_motion_uses_theme_motion() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let hover_duration = theme.motion.hover_duration();
    let press_duration = theme.motion.press_duration();
    let expected_hover = theme
        .palette
        .accent
        .with_alpha((theme.interaction.hover_blend * 0.07).min(0.08));
    let expected_press = theme
        .palette
        .accent
        .with_alpha((theme.interaction.selected_blend * 0.48).min(0.14));
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(240.0, 120.0))
            .with_child(
                PanelSection::new("Advanced color", crate::Label::new("RGB sliders"))
                    .theme(theme)
                    .collapsible(true)
                    .collapsed(),
            ),
    );
    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let title = text_run_for(&output, "Advanced color");
    let position = super::rect_center(title.rect);

    let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
    move_event.pointer_id = 1;
    runtime
        .handle_event(window_id, Event::Pointer(move_event))
        .map_err(|error| error.to_string())?;

    runtime.tick(hover_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_hover = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !solid_fill_colors(&mid_hover).contains(&expected_hover),
        "panel header hover fill should not snap to the settled hover color"
    );

    runtime.tick(hover_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_hover = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )
        .map_err(|error| error.to_string())?;

    runtime.tick(hover_duration + press_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_press = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !solid_fill_colors(&mid_press).contains(&expected_press),
        "panel header press fill should not snap to the settled pressed color"
    );

    runtime.tick(hover_duration + press_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_press = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&settled_press).contains(&expected_press));

    Ok(())
}

#[test]
fn panel_section_title_id_is_javascript_safe() {
    let id = super::panel_section_title_id(WidgetId::new(402)).get();

    assert!(id < (1_u64 << 53));
}

#[test]
fn dock_panel_exposes_title_and_arranges_child_below_header() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(280.0, 160.0))
            .with_child(
                DockPanel::new("Tool properties", crate::Label::new("Brush size"))
                    .name("Inspector")
                    .padding(sui_layout::Padding::all(8.0)),
            ),
    );

    let panel = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Inspector")
        })
        .expect("dock panel semantics should exist");
    assert_eq!(panel.bounds, Rect::new(0.0, 0.0, 280.0, 160.0));

    let title = output
        .semantics
        .iter()
        .find(|node| {
            node.parent == Some(panel.id)
                && node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("Tool properties")
        })
        .expect("dock panel title semantics should exist");
    let child = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Brush size"))
        .expect("dock panel child semantics should exist");

    let theme = DefaultTheme::default();
    assert!(title.bounds.max_y() <= theme.metrics.dock_panel_header_height);
    assert!(
        child.bounds.y()
            >= theme.metrics.dock_panel_header_height + theme.metrics.dock_panel_padding.top
    );
}

#[test]
fn empty_state_exposes_content_and_action_semantics() {
    let theme = DefaultTheme::default();
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 220.0))
            .with_child(
                EmptyState::new("Empty directory", "Nothing here yet.")
                    .theme(theme)
                    .icon(crate::IconGlyph::Folder)
                    .detail("No files matched the active filter.")
                    .action(crate::Button::new("Create file").theme(theme)),
            ),
    );

    let state = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Empty directory")
        })
        .expect("empty-state semantics should exist");
    assert_eq!(
        state.description.as_deref(),
        Some("Nothing here yet. No files matched the active filter.")
    );
    let action = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Create file")
        })
        .expect("empty-state action semantics should exist");
    assert!(action.bounds.x() >= state.bounds.x());
    assert!(action.bounds.max_x() <= state.bounds.max_x());
    assert!(action.bounds.max_y() <= state.bounds.max_y());
    assert!(
        action.bounds.width() < state.bounds.width() * 0.75,
        "empty-state action should keep its natural width instead of stretching: state={:?}, action={:?}",
        state.bounds,
        action.bounds
    );
    assert_eq!(
        text_run_for(&output, "Empty directory").style.color,
        theme.surfaces.text_muted
    );
    assert_eq!(
        text_run_for(&output, "Nothing here yet.").style.color,
        theme.surfaces.text_faint
    );
    assert_eq!(
        text_run_for(&output, "No files matched the active filter.")
            .style
            .color,
        theme.surfaces.text_muted
    );
    // The action reserves 18px above the geometric center. Each line is
    // then centered on its authored slot through the same optical-baseline
    // path used by buttons, inputs, tables, and other SUI controls.
    assert!((text_visual_center_for(&output, "Empty directory") - 96.0).abs() < 0.75);
    assert!((text_visual_center_for(&output, "Nothing here yet.") - 122.0).abs() < 0.75);
    assert!(
        (text_visual_center_for(&output, "No files matched the active filter.") - 140.0).abs()
            < 0.75
    );
}

#[test]
fn dock_panel_title_visual_center_matches_header_title_slot_center() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(280.0, 160.0))
            .with_child(
                DockPanel::new("Tool properties", crate::Label::new("Brush size"))
                    .name("Inspector")
                    .padding(sui_layout::Padding::all(8.0)),
            ),
    );
    let panel = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Inspector")
        })
        .expect("dock panel semantics should exist");
    let title_slot = output
        .semantics
        .iter()
        .find(|node| {
            node.parent == Some(panel.id)
                && node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("Tool properties")
        })
        .expect("dock panel title semantics should exist")
        .bounds;
    let text = text_run_for(&output, "Tool properties");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("dock panel title should shape");
    let line = layout
        .lines()
        .first()
        .expect("dock panel title should contain one line");
    let actual_visual_center =
        text.rect.y() + line.baseline + optical_visual_center(layout.measurement());

    assert!((actual_visual_center - super::rect_center(title_slot).y).abs() < 0.75);
}

#[test]
fn dock_panel_title_preserves_tall_measurement_and_header_centering() {
    let mut theme = DefaultTheme::default();
    theme.text.sm = ThemeTextToken {
        size: 30.0,
        line_height: 10.0,
    };
    theme.sync_derived_fields();
    theme.metrics.dock_panel_header_height = 52.0;

    let output = render(
        crate::SizedBox::new()
            .size(Size::new(300.0, 180.0))
            .with_child(
                DockPanel::new("Tool properties", crate::Label::new("Brush size"))
                    .theme(theme)
                    .name("Inspector")
                    .padding(sui_layout::Padding::all(8.0)),
            ),
    );
    let panel = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Inspector")
        })
        .expect("dock panel semantics should exist");
    let title_slot = output
        .semantics
        .iter()
        .find(|node| {
            node.parent == Some(panel.id)
                && node.role == SemanticsRole::Text
                && node.name.as_deref() == Some("Tool properties")
        })
        .expect("dock panel title semantics should exist")
        .bounds;
    let title = text_run_for(&output, "Tool properties");
    let layout = TextSystem::new()
        .shape_text_run(&title, &FontRegistry::new())
        .expect("dock panel title should shape");

    assert_text_run_uses_token(&title, theme.text.sm);
    assert!(title.rect.height() >= layout.measurement().height - 0.01);
    assert!(title.rect.height() > title.style.line_height);
    assert!((text_run_visual_center(&title) - super::rect_center(title_slot).y).abs() < 0.75);
}

#[test]
fn dock_panel_title_id_is_javascript_safe() {
    let id = super::dock_panel_title_id(WidgetId::new(402)).get();

    assert!(id < (1_u64 << 53));
}

#[test]
fn status_bar_host_reserves_footer_height() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(320.0, 160.0))
            .with_child(StatusBarHost::new(
                crate::Label::new("Canvas content"),
                StatusBar::new()
                    .name("Editor status")
                    .segment(StatusBarSegment::new("Ready").min_width(80.0)),
            )),
    );

    let status = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Editor status")
        })
        .expect("status bar container semantics should exist");

    let theme = DefaultTheme::default();
    assert_eq!(
        status.bounds,
        Rect::new(
            0.0,
            160.0 - theme.metrics.status_bar_height,
            320.0,
            theme.metrics.status_bar_height,
        )
    );
}

#[test]
fn status_bar_segment_ids_are_javascript_safe_and_distinct() {
    let parent = WidgetId::new(402);
    let ids = (0..6)
        .map(|index| super::status_bar_segment_id(parent, index).get())
        .collect::<Vec<_>>();

    for id in &ids {
        assert!(*id < (1_u64 << 53));
    }
    for (left_index, left) in ids.iter().enumerate() {
        for right in ids.iter().skip(left_index + 1) {
            assert_ne!(left, right);
        }
    }
}

fn text_run_from_shaped(
    output: &RenderOutput,
    run: &sui_text::ShapedText,
) -> Option<sui_text::TextRun> {
    run.resolve(output.frame.text_layout_registry.as_ref())
        .map(|layout| {
            let mut style = layout.style().clone();
            if let Some(color) = run.color_override {
                style.color = color;
            }
            sui_text::TextRun {
                rect: shaped_text_run_rect(run.origin, layout),
                text: layout.text().to_string(),
                style,
            }
        })
}

fn shaped_text_run_rect(origin: Point, layout: &sui_text::TextLayout) -> Rect {
    let measurement = layout.measurement();
    let bounds = measurement.bounds;
    let width = if bounds.width().is_finite() && bounds.width() > 0.0 {
        bounds.width()
    } else {
        measurement.width
    };
    Rect::new(
        origin.x + bounds.x(),
        origin.y + ((layout.box_size().height - measurement.height).max(0.0) * 0.5),
        width,
        layout.style().line_height.max(measurement.height),
    )
}

fn first_text_run(output: &RenderOutput) -> sui_text::TextRun {
    output
        .frame
        .scene
        .commands()
        .iter()
        .find_map(|command| match command {
            sui_scene::SceneCommand::DrawText(text) => Some(text.clone()),
            sui_scene::SceneCommand::DrawShapedText(text) => text_run_from_shaped(output, text),
            _ => None,
        })
        .expect("text draw command present")
}

fn text_run_for(output: &RenderOutput, text: &str) -> sui_text::TextRun {
    let mut found = None;
    output.frame.scene.visit_commands(&mut |command| {
        if found.is_some() {
            return;
        }
        found = match command {
            sui_scene::SceneCommand::DrawText(run) if run.text == text => Some(run.clone()),
            sui_scene::SceneCommand::DrawShapedText(run) => {
                text_run_from_shaped(output, run).filter(|resolved| resolved.text == text)
            }
            _ => None,
        };
    });
    found.expect("text draw command present")
}

fn text_layout_for(output: &RenderOutput, text: &str) -> sui_text::TextLayout {
    let mut found = None;
    output.frame.scene.visit_commands(&mut |command| {
        if found.is_some() {
            return;
        }
        if let sui_scene::SceneCommand::DrawShapedText(run) = command
            && let Some(layout) = run.resolve(output.frame.text_layout_registry.as_ref())
            && layout.text() == text
        {
            found = Some(layout.clone());
        }
    });
    found.expect("shaped text layout present")
}

fn slow_normal_motion_theme() -> DefaultTheme {
    let mut theme = DefaultTheme::default();
    theme.motion.duration_fast = 0.0;
    theme.motion.duration_normal = 0.6;
    theme
}

fn text_transform_dx_for(output: &RenderOutput, text: &str) -> Option<f32> {
    fn find_in_commands(
        output: &RenderOutput,
        text: &str,
        commands: &[SceneCommand],
        inherited_dx: f32,
        stack: &mut Vec<f32>,
    ) -> Option<f32> {
        let mut current_dx = inherited_dx;
        for command in commands {
            match command {
                SceneCommand::PushTransform { transform } => {
                    stack.push(current_dx);
                    current_dx += transform.dx;
                }
                SceneCommand::PopTransform => {
                    current_dx = stack.pop().unwrap_or(inherited_dx);
                }
                SceneCommand::DrawText(run) if run.text == text => {
                    return Some(current_dx);
                }
                SceneCommand::DrawShapedText(run) => {
                    if run
                        .resolve(output.frame.text_layout_registry.as_ref())
                        .is_some_and(|layout| layout.text() == text)
                    {
                        return Some(current_dx);
                    }
                }
                SceneCommand::Layer(layer) => {
                    if let Some(dx) = find_in_commands(
                        output,
                        text,
                        layer.scene.commands(),
                        current_dx + layer.descriptor.properties.translation.x,
                        stack,
                    ) {
                        return Some(dx);
                    }
                }
                _ => {}
            }
        }

        None
    }

    find_in_commands(
        output,
        text,
        output.frame.scene.commands(),
        0.0,
        &mut Vec::new(),
    )
}

fn clip_rect_for_text(output: &RenderOutput, text: &str) -> Rect {
    let mut stack = Vec::new();
    let mut found = None;
    output.frame.scene.visit_commands(&mut |command| {
        if found.is_some() {
            return;
        }
        match command {
            sui_scene::SceneCommand::PushClip { rect } => stack.push(*rect),
            sui_scene::SceneCommand::PopClip => {
                stack.pop();
            }
            sui_scene::SceneCommand::DrawText(run) if run.text == text => {
                found = stack.last().copied();
            }
            sui_scene::SceneCommand::DrawShapedText(run)
                if run
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .is_some_and(|layout| layout.text() == text) =>
            {
                found = stack.last().copied();
            }
            _ => {}
        }
    });
    found.expect("text draw command should have an active clip")
}

fn optical_visual_center(measurement: sui_text::TextMeasurement) -> f32 {
    let top = -measurement.cap_height.unwrap_or(measurement.ascent);
    let bottom = measurement.descent * 0.5;
    (top + bottom) * 0.5
}

fn text_run_layout(run: &sui_text::TextRun) -> sui_text::TextLayout {
    TextSystem::new()
        .shape_text(
            run.text.clone(),
            Size::new(f32::INFINITY, run.rect.height().max(1.0)),
            run.style.clone(),
            &FontRegistry::new(),
        )
        .expect("text run should shape")
}

fn text_run_visual_center(run: &sui_text::TextRun) -> f32 {
    let layout = text_run_layout(run);
    let line = layout.lines().first().expect("text run should have a line");
    run.rect.y() + line.baseline + optical_visual_center(layout.measurement())
}

fn text_visual_center_for(output: &RenderOutput, text: &str) -> f32 {
    output
        .frame
        .scene
        .commands()
        .iter()
        .find_map(|command| match command {
            sui_scene::SceneCommand::DrawText(run) if run.text == text => {
                Some(text_run_visual_center(run))
            }
            sui_scene::SceneCommand::DrawShapedText(run) => {
                let layout = run.resolve(output.frame.text_layout_registry.as_ref())?;
                if layout.text() != text {
                    return None;
                }
                let line = layout.lines().first().expect("text run should have a line");
                Some(run.origin.y + line.baseline + optical_visual_center(layout.measurement()))
            }
            _ => None,
        })
        .expect("text draw command present")
}

fn assert_text_run_uses_token(run: &sui_text::TextRun, token: ThemeTextToken) {
    assert!(
        (run.style.font_size - token.size).abs() < 0.001,
        "text '{}' used font size {}, expected token size {}",
        run.text,
        run.style.font_size,
        token.size
    );
    assert!(
        (run.style.line_height - token.line_height).abs() < 0.001,
        "text '{}' used line height {}, expected token line height {}",
        run.text,
        run.style.line_height,
        token.line_height
    );
}

fn assert_focus_ring_uses_theme_motion<W>(root: W, position: Point) -> Result<(), String>
where
    W: Widget + 'static,
{
    let theme = DefaultTheme::default();
    let focus_duration = theme.motion.focus_duration();
    let (mut runtime, window_id) = build_runtime(root);
    let _ = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )
        .map_err(|error| error.to_string())?;
    let _ = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;

    runtime.tick(focus_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !contains_approx_color(&solid_stroke_colors(&mid), theme.palette.focus_ring),
        "focus ring should not snap to the settled focus color"
    );

    runtime.tick(focus_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        contains_approx_color(&solid_stroke_colors(&settled), theme.palette.focus_ring),
        "focus ring should settle to the theme focus color"
    );

    Ok(())
}

fn layer_descriptor_for(output: &RenderOutput, owner: WidgetId) -> Option<SceneLayerDescriptor> {
    let mut descriptor = None;
    output.frame.scene.visit_layers(&mut |layer| {
        if layer.widget_id() == owner {
            descriptor = Some(layer.descriptor.clone());
        }
    });
    descriptor
}

fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
    let mut event = PointerEvent::new(kind, position);
    event.pointer_id = 1;
    event.button = Some(PointerButton::Primary);
    event.buttons = if pressed {
        PointerButtons::new(1)
    } else {
        PointerButtons::NONE
    };
    Event::Pointer(event)
}

fn handle_ready_events(runtime: &mut Runtime) -> Result<usize, String> {
    let ready = runtime.drain_ready_events();
    let count = ready.len();
    for (ready_window, event) in ready {
        runtime
            .handle_event(ready_window, event)
            .map_err(|error| error.to_string())?;
    }
    Ok(count)
}

fn overlay_layer_descriptor(output: &RenderOutput) -> Option<SceneLayerDescriptor> {
    let mut descriptor = None;
    output.frame.scene.visit_layers(&mut |layer| {
        if layer.descriptor.composition_mode == LayerCompositionMode::Overlay {
            descriptor = Some(layer.descriptor.clone());
        }
    });
    descriptor
}

fn overlay_layer_owner(output: &RenderOutput) -> Option<WidgetId> {
    let mut owner = None;
    output.frame.scene.visit_layers(&mut |layer| {
        if layer.descriptor.composition_mode == LayerCompositionMode::Overlay {
            owner = Some(layer.widget_id());
        }
    });
    owner
}

fn solid_fill_colors(output: &RenderOutput) -> Vec<Color> {
    let mut colors = Vec::new();
    output
        .frame
        .scene
        .visit_commands(&mut |command| match command {
            SceneCommand::FillRect {
                brush: Brush::Solid(color),
                ..
            }
            | SceneCommand::FillPath {
                brush: Brush::Solid(color),
                ..
            } => colors.push(*color),
            _ => {}
        });
    colors
}

fn solid_fill_rects_for_color(output: &RenderOutput, expected: Color) -> Vec<Rect> {
    let mut rects = Vec::new();
    output
        .frame
        .scene
        .visit_commands(&mut |command| match command {
            SceneCommand::FillRect {
                rect,
                brush: Brush::Solid(color),
            } if *color == expected => rects.push(*rect),
            _ => {}
        });
    rects
}

fn solid_stroke_colors(output: &RenderOutput) -> Vec<Color> {
    let mut colors = Vec::new();
    output
        .frame
        .scene
        .visit_commands(&mut |command| match command {
            SceneCommand::StrokeRect {
                brush: Brush::Solid(color),
                ..
            }
            | SceneCommand::StrokePath {
                brush: Brush::Solid(color),
                ..
            } => colors.push(*color),
            _ => {}
        });
    colors
}

fn contains_approx_color(colors: &[Color], expected: Color) -> bool {
    const CHANNEL_TOLERANCE: f32 = 1.0 / 255.0;

    colors.iter().any(|color| {
        color.space == expected.space
            && (color.red - expected.red).abs() <= CHANNEL_TOLERANCE
            && (color.green - expected.green).abs() <= CHANNEL_TOLERANCE
            && (color.blue - expected.blue).abs() <= CHANNEL_TOLERANCE
            && (color.alpha - expected.alpha).abs() <= CHANNEL_TOLERANCE
    })
}

fn non_hit_test_layer_descriptors(output: &RenderOutput) -> Vec<SceneLayerDescriptor> {
    let mut descriptors = Vec::new();
    output.frame.scene.visit_layers(&mut |layer| {
        if !layer.descriptor.hit_test {
            descriptors.push(layer.descriptor.clone());
        }
    });
    descriptors
}

fn non_hit_test_layer_owners(output: &RenderOutput) -> Vec<WidgetId> {
    let mut owners = Vec::new();
    output.frame.scene.visit_layers(&mut |layer| {
        if !layer.descriptor.hit_test {
            owners.push(layer.widget_id());
        }
    });
    owners
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
struct PanelCounters {
    measure: usize,
    arrange: usize,
    paint: usize,
    semantics: usize,
}

struct SpyPanel {
    name: &'static str,
    counters: Rc<RefCell<PanelCounters>>,
}

impl SpyPanel {
    fn new(name: &'static str, counters: Rc<RefCell<PanelCounters>>) -> Self {
        Self { name, counters }
    }
}

impl Widget for SpyPanel {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.counters.borrow_mut().measure += 1;
        constraints.clamp(Size::new(180.0, 72.0))
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: sui_core::Rect) {
        self.counters.borrow_mut().arrange += 1;
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.counters.borrow_mut().paint += 1;
        ctx.fill_bounds(Color::rgba(0.20, 0.28, 0.38, 1.0));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.counters.borrow_mut().semantics += 1;
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.to_string());
        ctx.push(node);
    }
}

#[test]
fn tab_bar_exposes_selected_value() {
    let output = render(
        TabBar::new("Main tabs")
            .tabs(["Design", "Inspect", "Export"])
            .selected(1),
    );

    let tabs = output
        .semantics
        .into_iter()
        .find(|node| node.role == SemanticsRole::TabBar)
        .expect("tab bar semantics node present");
    assert_eq!(
        tabs.value,
        Some(SemanticsValue::Text("Inspect".to_string()))
    );
}

#[test]
fn navigation_tab_bar_uses_flat_strip_and_accent_underline() {
    let mut theme = DefaultTheme::default();
    theme.interaction.tab_selected_blend = 0.31;
    let selected_fill = super::mix_color(
        theme.palette.surface_raised,
        theme.palette.accent,
        theme.interaction.tab_selected_blend,
    );

    let tab_bar = render_isolated(
        TabBar::new("Main tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"])
            .selected(1),
    );
    let tab_bar_fills = solid_fill_colors(&tab_bar);
    assert!(tab_bar_fills.contains(&theme.palette.control));
    assert!(tab_bar_fills.contains(&theme.palette.accent));
    assert!(
        !tab_bar_fills.contains(&selected_fill),
        "selected navigation tabs should not paint a raised selection tile"
    );
    let mut accent_bounds = Vec::new();
    tab_bar
        .frame
        .scene
        .visit_commands(&mut |command| match command {
            SceneCommand::FillRect {
                rect,
                brush: Brush::Solid(color),
            } if *color == theme.palette.accent => accent_bounds.push(*rect),
            SceneCommand::FillPath {
                path,
                brush: Brush::Solid(color),
            } if *color == theme.palette.accent => accent_bounds.push(path.bounds()),
            _ => {}
        });
    assert_eq!(
        accent_bounds.len(),
        1,
        "navigation tab bars should paint exactly one accent underline"
    );
    let underline = accent_bounds[0];
    assert!((underline.height() - theme.interaction.active_indicator_thickness).abs() < 0.01);
    assert!((underline.max_y() - tab_bar.frame.viewport.height).abs() < 0.01);
    assert!(
        !solid_stroke_colors(&tab_bar).contains(&theme.palette.border_focus),
        "selected tab bar chrome should not use the focus border color"
    );
    assert!(
        !solid_stroke_colors(&tab_bar).contains(&theme.palette.focus_ring),
        "unfocused selected tab bar chrome should not paint a focus ring"
    );

    // Content-bearing `Tabs` keeps its existing selected-panel treatment;
    // this test scopes the flat navigation grammar specifically to TabBar.
    let tabs = render_isolated(
        Tabs::new("Main tabs")
            .theme(theme)
            .selected(1)
            .tab("Design", crate::Label::new("Design"))
            .tab("Inspect", crate::Label::new("Inspect")),
    );
    assert!(solid_fill_colors(&tabs).contains(&selected_fill));
    assert!(
        !solid_stroke_colors(&tabs).contains(&theme.palette.border_focus),
        "selected tabs chrome should not use the focus border color"
    );
    assert!(
        !solid_stroke_colors(&tabs).contains(&theme.palette.focus_ring),
        "unfocused selected tabs chrome should not paint a focus ring"
    );
}

#[test]
fn tab_widgets_focus_highlights_selected_tab_button() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let focus_duration = theme.motion.focus_duration();
    let tab_bar_point = Point::new(
        theme.metrics.tab_min_width + theme.metrics.tab_gap + 8.0,
        18.0,
    );
    let (mut tab_bar_runtime, tab_bar_window) = build_runtime(
        TabBar::new("Main tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"])
            .selected(1),
    );
    let _ = tab_bar_runtime
        .render(tab_bar_window)
        .map_err(|error| error.to_string())?;
    tab_bar_runtime
        .handle_event(
            tab_bar_window,
            primary_pointer(PointerEventKind::Down, tab_bar_point, true),
        )
        .map_err(|error| error.to_string())?;
    tab_bar_runtime.tick(focus_duration + 0.01);
    assert!(handle_ready_events(&mut tab_bar_runtime)? >= 1);
    let tab_bar_focused = tab_bar_runtime
        .render(tab_bar_window)
        .map_err(|error| error.to_string())?;
    let tab_bar_strokes = solid_stroke_colors(&tab_bar_focused);
    assert!(
        contains_approx_color(&tab_bar_strokes, theme.palette.focus_ring),
        "focused selected tab button should paint a focus ring; strokes={tab_bar_strokes:?}"
    );
    assert!(
        !contains_approx_color(&tab_bar_strokes, theme.palette.border_focus),
        "focused tab bar should keep neutral selected strokes; strokes={tab_bar_strokes:?}"
    );

    let tabs_point = Point::new(
        theme.metrics.tab_min_width + theme.metrics.tab_gap + 8.0,
        18.0,
    );
    let (mut tabs_runtime, tabs_window) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(260.0, 120.0))
            .with_child(
                Tabs::new("Main tabs")
                    .theme(theme)
                    .selected(1)
                    .tab("Design", crate::Label::new("Design"))
                    .tab("Inspect", crate::Label::new("Inspect")),
            ),
    );
    let _ = tabs_runtime
        .render(tabs_window)
        .map_err(|error| error.to_string())?;
    tabs_runtime
        .handle_event(
            tabs_window,
            primary_pointer(PointerEventKind::Down, tabs_point, true),
        )
        .map_err(|error| error.to_string())?;
    tabs_runtime.tick(focus_duration + 0.01);
    assert!(handle_ready_events(&mut tabs_runtime)? >= 1);
    let tabs_focused = tabs_runtime
        .render(tabs_window)
        .map_err(|error| error.to_string())?;
    let tabs_strokes = solid_stroke_colors(&tabs_focused);
    assert!(
        contains_approx_color(&tabs_strokes, theme.palette.focus_ring),
        "focused selected tab button should paint a focus ring; strokes={tabs_strokes:?}"
    );
    assert!(
        !contains_approx_color(&tabs_strokes, theme.palette.border_focus),
        "focused tabs should keep neutral selected strokes; strokes={tabs_strokes:?}"
    );

    Ok(())
}

#[test]
fn selected_tab_labels_preserve_body_text_metrics() {
    let mut theme = DefaultTheme::default();
    theme.text.base = ThemeTextToken {
        size: 15.5,
        line_height: 22.0,
    };
    theme.sync_derived_fields();

    let tab_bar = render_isolated(
        TabBar::new("Main tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"])
            .selected(1),
    );
    let tab_bar_label = text_run_for(&tab_bar, "Inspect");
    assert_text_run_uses_token(&tab_bar_label, theme.text.base);
    assert_eq!(tab_bar_label.style.color, theme.palette.text);
    assert!(
        (text_run_visual_center(&tab_bar_label) - (tab_bar.frame.viewport.height * 0.5)).abs()
            < 0.75
    );

    let tabs = render(
        Tabs::new("Main tabs")
            .theme(theme)
            .selected(1)
            .tab("Design", crate::Label::new("Design"))
            .tab("Inspect", crate::Label::new("Inspect")),
    );
    let tabs_label = text_run_for(&tabs, "Inspect");
    assert_text_run_uses_token(&tabs_label, theme.text.base);
    assert_eq!(tabs_label.style.color, theme.palette.text);
    assert!((text_run_visual_center(&tabs_label) - (theme.metrics.tab_height * 0.5)).abs() < 0.75);
}

#[test]
fn selected_tab_labels_preserve_tall_measurements_and_exact_centering() {
    let mut theme = DefaultTheme::default();
    theme.text.base = ThemeTextToken {
        size: 28.0,
        line_height: 12.0,
    };
    theme.sync_derived_fields();
    theme.metrics.tab_height = 48.0;

    let tab_bar = render_isolated(
        TabBar::new("Main tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"])
            .selected(1),
    );
    let tab_bar_label = text_run_for(&tab_bar, "Inspect");
    let tab_bar_layout = TextSystem::new()
        .shape_text_run(&tab_bar_label, &FontRegistry::new())
        .expect("selected tab bar label should shape");

    assert_text_run_uses_token(&tab_bar_label, theme.text.base);
    assert!(tab_bar_label.rect.height() >= tab_bar_layout.measurement().height - 0.01);
    assert!(tab_bar_label.rect.height() > tab_bar_label.style.line_height);
    assert!(
        (text_visual_center_for(&tab_bar, "Inspect") - (tab_bar.frame.viewport.height * 0.5)).abs()
            < 0.75
    );

    let tabs = render_isolated(
        Tabs::new("Main tabs")
            .theme(theme)
            .selected(1)
            .tab("Design", crate::Label::new("Design panel"))
            .tab("Inspect", crate::Label::new("Selected panel")),
    );
    let tabs_label = text_run_for(&tabs, "Inspect");
    let tabs_layout = TextSystem::new()
        .shape_text_run(&tabs_label, &FontRegistry::new())
        .expect("selected tabs label should shape");

    assert_text_run_uses_token(&tabs_label, theme.text.base);
    assert!(tabs_label.rect.height() >= tabs_layout.measurement().height - 0.01);
    assert!(tabs_label.rect.height() > tabs_label.style.line_height);
    assert!(
        (text_visual_center_for(&tabs, "Inspect") - (theme.metrics.tab_height * 0.5)).abs() < 0.75
    );
}

#[test]
fn tab_widgets_share_pressed_tab_border() -> Result<(), String> {
    let theme = DefaultTheme::default();

    let (mut runtime, window_id) = build_runtime(
        TabBar::new("Main tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"]),
    );
    let initial_tab_bar = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let press_point = super::rect_center(text_run_for(&initial_tab_bar, "Inspect").rect);
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, press_point, true),
        )
        .map_err(|error| error.to_string())?;
    let tab_bar = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(contains_approx_color(
        &solid_stroke_colors(&tab_bar),
        theme.palette.border_hover,
    ));

    let (mut runtime, window_id) = build_runtime(
        Tabs::new("Main tabs")
            .theme(theme)
            .tab("Design", crate::Label::new("Design"))
            .tab("Inspect", crate::Label::new("Inspect")),
    );
    let initial_tabs = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let press_point = super::rect_center(text_run_for(&initial_tabs, "Inspect").rect);
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, press_point, true),
        )
        .map_err(|error| error.to_string())?;
    let tabs = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(contains_approx_color(
        &solid_stroke_colors(&tabs),
        theme.palette.border_hover,
    ));

    Ok(())
}

#[test]
fn tab_hover_and_press_chrome_use_theme_motion() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let hover_duration = theme.motion.hover_duration();
    let press_duration = theme.motion.press_duration();
    let expected_hover = super::mix_color(
        theme.palette.control,
        theme.palette.control_hover,
        theme.interaction.hover_blend,
    );
    let expected_press = super::mix_color(
        expected_hover,
        theme.palette.control_active,
        theme.interaction.pressed_blend,
    );

    fn assert_tab_header_motion<W>(
        root: W,
        hover_duration: f64,
        press_duration: f64,
        expected_hover: Color,
        expected_press: Color,
    ) -> Result<(), String>
    where
        W: Widget + 'static,
    {
        let (mut runtime, window_id) = build_runtime(root);
        let initial = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let second_tab_point = super::rect_center(text_run_for(&initial, "Inspect").rect);

        let mut move_event = PointerEvent::new(PointerEventKind::Move, second_tab_point);
        move_event.pointer_id = 1;
        runtime
            .handle_event(window_id, Event::Pointer(move_event))
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_hover).contains(&expected_hover),
            "tab hover fill should not snap to the settled hover color"
        );

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, second_tab_point, true),
            )
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration + press_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_press).contains(&expected_press),
            "tab press fill should not snap to the settled pressed color"
        );

        runtime.tick(hover_duration + press_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_press).contains(&expected_press));

        Ok(())
    }

    assert_tab_header_motion(
        TabBar::new("Main tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"]),
        hover_duration,
        press_duration,
        expected_hover,
        expected_press,
    )?;
    assert_tab_header_motion(
        Tabs::new("Main tabs")
            .theme(theme)
            .tab("Design", crate::Label::new("Design"))
            .tab("Inspect", crate::Label::new("Inspect")),
        hover_duration,
        press_duration,
        expected_hover,
        expected_press,
    )?;

    Ok(())
}

#[test]
fn tab_switch_animation_uses_theme_motion() -> Result<(), String> {
    let mut theme = DefaultTheme::default();
    theme.motion.duration_fast = 0.6;
    theme.motion.duration_normal = 0.0;
    let switch_duration = theme.motion.tab_switch_duration();

    assert_keyboard_tab_switch_uses_duration(
        TabBar::new("Main tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"]),
        SemanticsRole::TabBar,
        switch_duration,
    )?;
    let selected = Rc::new(RefCell::new(0_usize));
    let selected_reader = Rc::clone(&selected);
    let selected_writer = Rc::clone(&selected);
    assert_keyboard_tab_switch_uses_duration(
        TabBar::new("Main tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"])
            .selected_when(move || Some(*selected_reader.borrow()))
            .on_change(move |index, _| *selected_writer.borrow_mut() = index),
        SemanticsRole::TabBar,
        switch_duration,
    )?;
    assert_keyboard_tab_switch_uses_duration(
        BrowserTabBar::new("Open tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"]),
        SemanticsRole::TabBar,
        switch_duration,
    )?;
    assert_keyboard_tab_switch_uses_duration(
        Tabs::new("Main tabs")
            .theme(theme)
            .tab("Design", crate::Label::new("Design panel"))
            .tab("Inspect", crate::Label::new("Inspect panel")),
        SemanticsRole::Tabs,
        switch_duration,
    )?;

    Ok(())
}

#[test]
fn tab_bar_external_selected_state_animates_without_rebuild() -> Result<(), String> {
    let mut theme = DefaultTheme::default();
    theme.motion.duration_fast = 0.6;
    theme.motion.duration_normal = 0.0;
    let switch_duration = theme.motion.tab_switch_duration();
    let selected = Rc::new(RefCell::new(0_usize));
    let selected_reader = Rc::clone(&selected);
    let (mut runtime, window_id) = build_runtime(
        TabBar::new("Main tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"])
            .selected_when(move || Some(*selected_reader.borrow())),
    );

    runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    *selected.borrow_mut() = 1;
    runtime
        .handle_event(window_id, Event::Window(WindowEvent::RedrawRequested))
        .map_err(|error| error.to_string())?;

    runtime.tick(switch_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    assert!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?
            .is_some(),
        "externally selected tab switch should still be animating at half duration"
    );

    runtime.tick(switch_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let tabs = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::TabBar)
        .expect("tab semantics present");
    assert_eq!(
        tabs.value,
        Some(SemanticsValue::Text("Inspect".to_string()))
    );

    Ok(())
}

#[test]
fn tab_bar_observable_selection_animates_without_redraw_polling() -> Result<(), String> {
    let mut theme = DefaultTheme::default();
    theme.motion.duration_fast = 0.6;
    theme.motion.duration_normal = 0.0;
    let switch_duration = theme.motion.tab_switch_duration();
    let selected = Signal::named("active_tab", Some(0_usize));
    let (mut runtime, window_id) = build_runtime(
        TabBar::new("Main tabs")
            .theme(theme)
            .tabs(["Design", "Inspect"])
            .selected_from(selected.clone()),
    );

    runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(selected.set(Some(1)));
    let changed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        changed
            .diagnostics
            .reactive_invalidations
            .iter()
            .any(|sample| sample.source_name == "active_tab")
    );

    runtime.tick(switch_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    assert!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?
            .is_some(),
        "observable tab selection should retain its in-flight animation"
    );

    runtime.tick(switch_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let tabs = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::TabBar)
        .expect("tab semantics present");
    assert_eq!(
        tabs.value,
        Some(SemanticsValue::Text("Inspect".to_string()))
    );
    Ok(())
}

#[test]
fn browser_tab_bar_semantics_ids_are_javascript_safe_and_distinct() {
    let parent = WidgetId::new(17);
    let mut ids = BTreeSet::new();
    for tab_index in 0..13 {
        for id in [
            super::browser_tab_semantics_id(parent, tab_index).get(),
            super::browser_tab_close_semantics_id(parent, tab_index).get(),
        ] {
            assert!(id < (1_u64 << 53), "{id} should be JS-safe");
            assert!(ids.insert(id), "{id} should be unique");
        }
    }
}

fn assert_keyboard_tab_switch_uses_duration<W>(
    widget: W,
    role: SemanticsRole,
    switch_duration: f64,
) -> Result<(), String>
where
    W: Widget + 'static,
{
    let (mut runtime, window_id) = build_runtime(widget);

    runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
        )
        .map_err(|error| error.to_string())?;
    runtime.tick(0.0);
    let _ = handle_ready_events(&mut runtime)?;
    assert!(
        runtime
            .focused_widget(window_id)
            .map_err(|error| error.to_string())?
            .is_some(),
        "tab widget should receive keyboard focus before arrow-key switching"
    );

    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
        )
        .map_err(|error| error.to_string())?;

    runtime.tick(switch_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    assert!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?
            .is_some(),
        "tab switch should still be animating at half of the custom theme duration"
    );

    runtime.tick(switch_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let tabs = output
        .semantics
        .iter()
        .find(|node| node.role == role)
        .expect("tab semantics present");
    assert_eq!(
        tabs.value,
        Some(SemanticsValue::Text("Inspect".to_string()))
    );
    assert_eq!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?,
        None
    );

    Ok(())
}

#[test]
fn tabs_panel_body_slides_during_theme_switch_motion() -> Result<(), String> {
    let mut theme = DefaultTheme::default();
    theme.motion.duration_fast = 0.6;
    theme.motion.duration_normal = 0.0;
    let switch_duration = theme.motion.tab_switch_duration();
    let (mut runtime, window_id) = build_runtime(
        Tabs::new("Main tabs")
            .theme(theme)
            .tab("Design", crate::Label::new("Design panel"))
            .tab("Inspect", crate::Label::new("Inspect panel")),
    );

    runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
        )
        .map_err(|error| error.to_string())?;
    runtime.tick(0.0);
    let _ = handle_ready_events(&mut runtime)?;
    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
        )
        .map_err(|error| error.to_string())?;

    runtime.tick(switch_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let mid_dx = text_transform_dx_for(&mid, "Inspect panel")
        .expect("incoming panel text should be painted");
    assert!(
        mid_dx > 0.0,
        "forward tab switch should slide the incoming panel in from the right; dx={mid_dx}"
    );

    runtime.tick(switch_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let settled_dx = text_transform_dx_for(&settled, "Inspect panel")
        .expect("settled panel text should be painted");
    assert_eq!(settled_dx, 0.0);

    Ok(())
}

#[test]
fn tabs_render_only_the_active_panel_after_switching() {
    let first = Rc::new(RefCell::new(PanelCounters::default()));
    let second = Rc::new(RefCell::new(PanelCounters::default()));
    let (mut runtime, window_id) = build_runtime(
        Tabs::new("Main tabs")
            .tab("First", SpyPanel::new("first-panel", Rc::clone(&first)))
            .tab("Second", SpyPanel::new("second-panel", Rc::clone(&second))),
    );

    let initial = runtime.render(window_id).unwrap();
    assert_eq!(
        *first.borrow(),
        PanelCounters {
            measure: 1,
            arrange: 1,
            paint: 1,
            semantics: 1
        }
    );
    assert_eq!(*second.borrow(), PanelCounters::default());
    assert!(
        initial
            .semantics
            .iter()
            .any(|node| node.name.as_deref() == Some("first-panel"))
    );
    assert!(
        !initial
            .semantics
            .iter()
            .any(|node| node.name.as_deref() == Some("second-panel"))
    );

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 20.0));
    down.pointer_id = 1;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    let mut up = PointerEvent::new(PointerEventKind::Up, Point::new(48.0, 20.0));
    up.pointer_id = 1;
    up.button = Some(PointerButton::Primary);
    runtime.handle_event(window_id, Event::Pointer(up)).unwrap();

    let first_before_switch = *first.borrow();
    let second_before_switch = *second.borrow();

    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
        )
        .unwrap();

    let after_switch = runtime.render(window_id).unwrap();
    assert_eq!(first.borrow().paint, first_before_switch.paint);
    assert_eq!(first.borrow().semantics, first_before_switch.semantics);
    assert_eq!(second.borrow().paint, second_before_switch.paint + 1);
    assert_eq!(
        second.borrow().semantics,
        second_before_switch.semantics + 1
    );
    assert!(
        !after_switch
            .semantics
            .iter()
            .any(|node| node.name.as_deref() == Some("first-panel"))
    );
    assert!(
        after_switch
            .semantics
            .iter()
            .any(|node| node.name.as_deref() == Some("second-panel"))
    );
}

#[test]
fn tab_bar_header_label_visual_center_matches_control_center() {
    let output = render(TabBar::new("Main tabs").tabs(["A", "B"]));
    assert_eq!(
        output.frame.viewport.height,
        DefaultTheme::default().metrics.tab_height
    );

    let text = first_text_run(&output);
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("tab header label should shape");
    let line = layout
        .lines()
        .first()
        .expect("tab header label should contain one line");
    let actual_visual_center =
        text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
    let control_center = output.frame.viewport.height * 0.5;

    assert!((actual_visual_center - control_center).abs() < 0.75);
}

#[test]
fn menu_row_label_visual_center_matches_row_center() {
    let output =
        render(Menu::new("App menu").items([MenuItem::new("New File"), MenuItem::new("Open...")]));
    assert!(output.semantics.iter().any(|node| {
        node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("New File")
    }));
    let text = text_run_for(&output, "New File");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("menu item text should shape");
    let line = layout
        .lines()
        .first()
        .expect("menu item text should contain one line");
    let actual_visual_center =
        text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
    let theme = DefaultTheme::default();
    let padding = theme.metrics.menu_padding;
    let row_height = (output.frame.viewport.height - padding.top - padding.bottom) / 2.0;
    let row_center = padding.top + (row_height * 0.5);

    assert!((actual_visual_center - row_center).abs() < 0.75);
}

#[test]
fn menu_row_hover_and_press_use_theme_motion() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let hover_duration = theme.motion.hover_duration();
    let press_duration = theme.motion.press_duration();
    let expected_hover = super::mix_color(
        theme.palette.control,
        theme.palette.accent,
        theme.interaction.selected_blend,
    );
    let expected_press = super::mix_color(
        expected_hover,
        theme.palette.control_active,
        theme.interaction.pressed_blend,
    );
    let (mut runtime, window_id) = build_runtime(
        Menu::new("App menu")
            .theme(theme)
            .items([MenuItem::new("New File"), MenuItem::new("Open...")]),
    );
    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let item = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("New File")
        })
        .expect("menu item semantics should exist");
    let position = super::rect_center(item.bounds);

    let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
    move_event.pointer_id = 1;
    runtime
        .handle_event(window_id, Event::Pointer(move_event))
        .map_err(|error| error.to_string())?;

    runtime.tick(hover_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_hover = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !solid_fill_colors(&mid_hover).contains(&expected_hover),
        "menu hover fill should not snap to the settled highlighted color"
    );

    runtime.tick(hover_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_hover = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )
        .map_err(|error| error.to_string())?;

    runtime.tick(hover_duration + press_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_press = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !solid_fill_colors(&mid_press).contains(&expected_press),
        "menu press fill should not snap to the settled pressed color"
    );

    runtime.tick(hover_duration + press_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_press = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&settled_press).contains(&expected_press));

    Ok(())
}

#[test]
fn menu_shortcuts_align_to_trailing_edge_and_row_center() {
    let theme = DefaultTheme::default();
    let output = render(Menu::new("App menu").items([
        MenuItem::new("New File").shortcut("Ctrl+N"),
        MenuItem::new("Open...").shortcut("Ctrl+Shift+O"),
    ]));
    let first_row = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("New File")
        })
        .expect("first menu item semantics present")
        .bounds;
    let second_row = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Open...")
        })
        .expect("second menu item semantics present")
        .bounds;
    let first_shortcut = text_run_for(&output, "Ctrl+N");
    let second_shortcut = text_run_for(&output, "Ctrl+Shift+O");
    let first_label_clip = clip_rect_for_text(&output, "New File");
    let second_label_clip = clip_rect_for_text(&output, "Open...");
    let first_edge = first_row.max_x() - theme.metrics.menu_item_padding.right;
    let second_edge = second_row.max_x() - theme.metrics.menu_item_padding.right;
    let first_label_edge = first_row.max_x()
        - theme.metrics.menu_item_padding.right
        - theme.metrics.menu_shortcut_width;
    let second_label_edge = second_row.max_x()
        - theme.metrics.menu_item_padding.right
        - theme.metrics.menu_shortcut_width;

    assert_eq!(
        first_shortcut.style.color,
        theme.placeholder_text_style().color
    );
    assert!((first_label_clip.max_x() - first_label_edge).abs() < 0.75);
    assert!((second_label_clip.max_x() - second_label_edge).abs() < 0.75);
    assert!((first_shortcut.rect.max_x() - first_edge).abs() < 0.75);
    assert!((second_shortcut.rect.max_x() - second_edge).abs() < 0.75);
    assert!((first_shortcut.rect.max_x() - second_shortcut.rect.max_x()).abs() < 0.75);
    assert!(
        (text_run_visual_center(&first_shortcut) - (first_row.y() + first_row.height() * 0.5))
            .abs()
            < 0.75
    );
}

#[test]
fn menu_shortcuts_preserve_tall_measurements_and_row_center() {
    let mut theme = DefaultTheme::default();
    theme.typography.body_font_size = 28.0;
    theme.typography.body_line_height = 12.0;
    theme.metrics.menu_row_height = 64.0;
    let metrics = theme.metrics;
    let output = render_isolated(
        Menu::new("App menu")
            .theme(theme)
            .items([MenuItem::new("New File").shortcut("Ctrl+N")]),
    );
    let row = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("New File")
        })
        .expect("menu item semantics present")
        .bounds;
    let label = text_run_for(&output, "New File");
    let shortcut = text_run_for(&output, "Ctrl+N");
    let label_layout = TextSystem::new()
        .shape_text_run(&label, &FontRegistry::new())
        .expect("menu item text should shape");
    let shortcut_layout = TextSystem::new()
        .shape_text_run(&shortcut, &FontRegistry::new())
        .expect("menu shortcut text should shape");
    let shortcut_edge = row.max_x() - metrics.menu_item_padding.right;
    let row_center = row.y() + (row.height() * 0.5);

    assert_eq!(label.style.font_size, 28.0);
    assert_eq!(label.style.line_height, 12.0);
    assert_eq!(shortcut.style.font_size, 28.0);
    assert_eq!(shortcut.style.line_height, 12.0);
    assert!(label.rect.height() >= label_layout.measurement().height - 0.01);
    assert!(shortcut.rect.height() >= shortcut_layout.measurement().height - 0.01);
    assert!(label.rect.height() > label.style.line_height);
    assert!(shortcut.rect.height() > shortcut.style.line_height);
    assert!((shortcut.rect.max_x() - shortcut_edge).abs() < 0.75);
    assert!((text_run_visual_center(&label) - row_center).abs() < 0.75);
    assert!((text_run_visual_center(&shortcut) - row_center).abs() < 0.75);
}

#[test]
fn context_menu_row_label_visual_center_matches_row_center() -> Result<(), String> {
    // Dropdown anchoring keeps the row geometry derivable from the
    // trigger bounds; pointer anchoring is covered separately.
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .width(320.0)
            .height(180.0)
            .with_child(
                ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                    .anchor_to_pointer(false)
                    .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")]),
            ),
    );

    let closed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = closed
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button)
        .expect("context menu trigger present")
        .bounds;
    let trigger_center = Point::new(
        trigger.x() + (trigger.width() * 0.5),
        trigger.y() + (trigger.height() * 0.5),
    );

    let mut down = PointerEvent::new(PointerEventKind::Down, trigger_center);
    down.pointer_id = 1;
    down.button = Some(PointerButton::Secondary);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .map_err(|error| error.to_string())?;

    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let _context = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::ContextMenu)
        .expect("context menu semantics present");
    let row = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Rename"))
        .expect("rename menu item semantics present")
        .bounds;
    let text = text_run_for(&output, "Rename");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("context menu item text should shape");
    let line = layout
        .lines()
        .first()
        .expect("context menu item text should contain one line");
    let actual_visual_center =
        text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
    let row_center = row.y() + row.height() * 0.5;

    assert!((actual_visual_center - row_center).abs() < 0.75);
    Ok(())
}

#[test]
fn context_menu_primary_activation_owns_interactive_trigger_click() {
    let trigger_activations = Rc::new(Cell::new(0));
    let recorded_trigger_activations = Rc::clone(&trigger_activations);
    let menu_activations = Rc::new(Cell::new(0));
    let recorded_menu_activations = Rc::clone(&menu_activations);
    let (mut runtime, window_id) = build_runtime(
        SizedBox::new().width(320.0).height(180.0).with_child(
            ContextMenu::new(
                "Actions menu",
                crate::Button::new("Actions").on_press(move || {
                    trigger_activations.set(trigger_activations.get() + 1);
                }),
            )
            .activation_button(PointerButton::Primary)
            .anchor_to_pointer(false)
            .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")])
            .on_activate(move |_, _| {
                menu_activations.set(menu_activations.get() + 1);
            }),
        ),
    );

    let closed = runtime.render(window_id).unwrap();
    let trigger = closed
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Actions"))
        .expect("interactive context-menu trigger present")
        .bounds;
    let trigger_center = super::rect_center(trigger);

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, trigger_center, true),
        )
        .unwrap();
    let pressed = runtime.render(window_id).unwrap();
    assert!(
        !pressed.semantics.iter().any(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Rename")
        }),
        "primary menus should register after release so the opening press is not dismissed"
    );
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, trigger_center, false),
        )
        .unwrap();

    let opened = runtime.render(window_id).unwrap();
    let rename = opened
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Rename"))
        .expect("primary trigger should open the context menu");
    assert_eq!(
        recorded_trigger_activations.get(),
        0,
        "the menu trigger click must not also invoke the wrapped button"
    );
    assert_eq!(
        opened
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ContextMenu
                    && node.name.as_deref() == Some("Actions menu")
            })
            .and_then(|node| node.state.expanded),
        Some(true)
    );

    let rename_center = super::rect_center(rename.bounds);
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, rename_center, true),
        )
        .unwrap();
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, rename_center, false),
        )
        .unwrap();
    assert_eq!(recorded_menu_activations.get(), 1);
}

#[test]
fn context_menu_primary_keyboard_and_semantics_share_open_contract() {
    let (mut runtime, window_id) = build_runtime(
        SizedBox::new().width(320.0).height(180.0).with_child(
            ContextMenu::new("Actions menu", crate::Button::new("Actions"))
                .activation_button(PointerButton::Primary)
                .anchor_to_pointer(false)
                .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")]),
        ),
    );
    let closed = runtime.render(window_id).unwrap();
    let menu_id = closed
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ContextMenu && node.name.as_deref() == Some("Actions menu")
        })
        .expect("context menu semantics present")
        .id;

    assert!(
        runtime
            .handle_semantics_action(window_id, menu_id, SemanticsActionRequest::Focus)
            .unwrap()
    );
    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Enter", KeyState::Pressed)),
        )
        .unwrap();
    assert_eq!(
        runtime
            .render(window_id)
            .unwrap()
            .semantics
            .iter()
            .find(|node| node.id == menu_id)
            .and_then(|node| node.state.expanded),
        Some(true)
    );

    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Escape", KeyState::Pressed)),
        )
        .unwrap();
    let _ = runtime.render(window_id).unwrap();
    assert!(
        runtime
            .handle_semantics_action(window_id, menu_id, SemanticsActionRequest::Expand)
            .unwrap()
    );
    assert_eq!(
        runtime
            .render(window_id)
            .unwrap()
            .semantics
            .iter()
            .find(|node| node.id == menu_id)
            .and_then(|node| node.state.expanded),
        Some(true)
    );
    assert!(
        runtime
            .handle_semantics_action(window_id, menu_id, SemanticsActionRequest::Collapse)
            .unwrap()
    );
    let _ = runtime.render(window_id).unwrap();
    assert!(
        runtime
            .handle_semantics_action(window_id, menu_id, SemanticsActionRequest::Activate)
            .unwrap()
    );
    assert_eq!(
        runtime
            .render(window_id)
            .unwrap()
            .semantics
            .iter()
            .find(|node| node.id == menu_id)
            .and_then(|node| node.state.expanded),
        Some(true)
    );
}

#[test]
fn context_menu_pointer_opens_submenu_and_activates_leaf_path() {
    let activations = Rc::new(RefCell::new(Vec::new()));
    let recorded_activations = Rc::clone(&activations);
    let (mut runtime, window_id) = build_runtime(
        SizedBox::new().width(360.0).height(240.0).with_child(
            ContextMenu::new("Actions menu", crate::Button::new("Actions"))
                .activation_button(PointerButton::Primary)
                .anchor_to_pointer(false)
                .items([
                    MenuItem::new("Open"),
                    MenuItem::new("Move to")
                        .submenu([MenuItem::new("Archive"), MenuItem::new("Shared")]),
                ])
                .on_activate_path(move |path, item| {
                    recorded_activations
                        .borrow_mut()
                        .push((path, item.label().to_string()));
                }),
        ),
    );

    let closed = runtime.render(window_id).unwrap();
    let trigger = closed
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Actions"))
        .expect("submenu context-menu trigger present")
        .bounds;
    let trigger_center = super::rect_center(trigger);
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, trigger_center, true),
        )
        .unwrap();
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, trigger_center, false),
        )
        .unwrap();

    let opened = runtime.render(window_id).unwrap();
    let owner = opened
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Move to")
        })
        .expect("submenu owner present");
    assert!(owner.popup.is_some());
    assert_eq!(owner.state.expanded, Some(false));
    let owner_id = owner.id;
    assert!(
        runtime
            .handle_semantics_action(window_id, owner_id, SemanticsActionRequest::Expand,)
            .unwrap()
    );
    let semantically_expanded = runtime.render(window_id).unwrap();
    assert!(semantically_expanded.semantics.iter().any(|node| {
        node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Archive")
    }));
    assert!(
        runtime
            .handle_semantics_action(window_id, owner_id, SemanticsActionRequest::Collapse,)
            .unwrap()
    );
    let semantically_collapsed = runtime.render(window_id).unwrap();
    let owner = semantically_collapsed
        .semantics
        .iter()
        .find(|node| node.id == owner_id)
        .expect("collapsed submenu owner present");
    assert_eq!(owner.state.expanded, Some(false));

    runtime
        .handle_event(
            window_id,
            Event::Pointer(PointerEvent::new(
                PointerEventKind::Move,
                super::rect_center(owner.bounds),
            )),
        )
        .unwrap();
    let nested = runtime.render(window_id).unwrap();
    let owner = nested
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Move to")
        })
        .expect("expanded submenu owner present");
    let archive = nested
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Archive")
        })
        .expect("submenu leaf present");
    assert_eq!(owner.state.expanded, Some(true));
    assert_eq!(archive.parent, Some(owner.id));

    let archive_center = super::rect_center(archive.bounds);
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, archive_center, true),
        )
        .unwrap();
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, archive_center, false),
        )
        .unwrap();

    assert_eq!(
        activations.borrow().as_slice(),
        &[(vec![1, 0], "Archive".to_string())]
    );
    assert_eq!(
        runtime
            .render(window_id)
            .unwrap()
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ContextMenu
                    && node.name.as_deref() == Some("Actions menu")
            })
            .and_then(|node| node.state.expanded),
        Some(false)
    );
}

#[test]
fn context_menu_keyboard_enters_and_leaves_submenus() {
    let activated_path = Rc::new(RefCell::new(None));
    let recorded_path = Rc::clone(&activated_path);
    let (mut runtime, window_id) = build_runtime(
        SizedBox::new().width(360.0).height(240.0).with_child(
            ContextMenu::new("Actions menu", crate::Button::new("Actions"))
                .activation_button(PointerButton::Primary)
                .anchor_to_pointer(false)
                .items([
                    MenuItem::new("Move to").submenu([
                        MenuItem::new("Archive"),
                        MenuItem::new("Shared").submenu([MenuItem::new("Team workspace")]),
                    ]),
                    MenuItem::new("Rename"),
                ])
                .on_activate_path(move |path, _| {
                    recorded_path.replace(Some(path));
                }),
        ),
    );
    let closed = runtime.render(window_id).unwrap();
    let menu_id = closed
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ContextMenu && node.name.as_deref() == Some("Actions menu")
        })
        .expect("context menu semantics present")
        .id;
    assert!(
        runtime
            .handle_semantics_action(window_id, menu_id, SemanticsActionRequest::Focus)
            .unwrap()
    );
    for key in ["Enter", "ArrowRight", "ArrowDown"] {
        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new(key, KeyState::Pressed)),
            )
            .unwrap();
    }
    let nested = runtime.render(window_id).unwrap();
    assert!(
        nested.semantics.iter().any(|node| {
            node.role == SemanticsRole::MenuItem
                && node.name.as_deref() == Some("Shared")
                && node.state.selected
        }),
        "ArrowRight should enter the submenu and ArrowDown should move within it"
    );

    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
        )
        .unwrap();
    let deeply_nested = runtime.render(window_id).unwrap();
    assert!(deeply_nested.semantics.iter().any(|node| {
        node.role == SemanticsRole::MenuItem
            && node.name.as_deref() == Some("Team workspace")
            && node.state.selected
    }));

    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowLeft", KeyState::Pressed)),
        )
        .unwrap();
    let one_level_nested = runtime.render(window_id).unwrap();
    assert!(!one_level_nested.semantics.iter().any(|node| {
        node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Team workspace")
    }));
    assert!(one_level_nested.semantics.iter().any(|node| {
        node.role == SemanticsRole::MenuItem
            && node.name.as_deref() == Some("Shared")
            && node.state.selected
    }));

    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowLeft", KeyState::Pressed)),
        )
        .unwrap();
    let closed_submenu = runtime.render(window_id).unwrap();
    assert!(!closed_submenu.semantics.iter().any(|node| {
        node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Archive")
    }));
    assert_eq!(
        closed_submenu
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Move to")
            })
            .and_then(|node| node.state.expanded),
        Some(false)
    );

    for key in ["ArrowRight", "Enter"] {
        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new(key, KeyState::Pressed)),
            )
            .unwrap();
    }
    assert_eq!(activated_path.borrow().as_deref(), Some(&[0, 0][..]));
}

#[test]
fn context_menu_submenu_falls_back_inside_right_viewport_edge() {
    let (mut runtime, window_id) = build_runtime(
        ContextMenu::new("Surface menu", SizedBox::new().width(800.0).height(300.0)).items([
            MenuItem::new("Export").submenu([
                MenuItem::new("Archive").submenu([MenuItem::new("Zip")]),
                MenuItem::new("Plain text"),
            ]),
        ]),
    );
    runtime.render(window_id).unwrap();
    let press = Point::new(790.0, 32.0);
    let mut down = PointerEvent::new(PointerEventKind::Down, press);
    down.pointer_id = 1;
    down.button = Some(PointerButton::Secondary);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    let root = runtime.render(window_id).unwrap();
    let owner = root
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Export"))
        .expect("submenu owner present")
        .bounds;
    runtime
        .handle_event(
            window_id,
            Event::Pointer(PointerEvent::new(
                PointerEventKind::Move,
                super::rect_center(owner),
            )),
        )
        .unwrap();
    let nested = runtime.render(window_id).unwrap();
    let child = nested
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Archive")
        })
        .expect("submenu leaf present")
        .bounds;
    assert!(
        child.x() < owner.x(),
        "the submenu should fall back to the owner's left near the viewport edge"
    );
    assert!(child.x() >= 4.0);
    assert!(child.max_x() <= 796.0);

    runtime
        .handle_event(
            window_id,
            Event::Pointer(PointerEvent::new(
                PointerEventKind::Move,
                super::rect_center(child),
            )),
        )
        .unwrap();
    let deeply_nested = runtime.render(window_id).unwrap();
    let grandchild = deeply_nested
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Zip"))
        .expect("nested submenu leaf present")
        .bounds;
    assert!(
        grandchild.x() < child.x(),
        "nested submenus should continue toward the side selected by their parent"
    );
    assert!(grandchild.x() >= 4.0);
}

#[test]
fn context_menu_preflights_nested_cascade_before_right_viewport_edge() {
    let (mut runtime, window_id) = build_runtime(
        ContextMenu::new("Surface menu", SizedBox::new().width(800.0).height(300.0)).items([
            MenuItem::new("Export").submenu([
                MenuItem::new("Archive").submenu([MenuItem::new("Zip")]),
                MenuItem::new("Plain text"),
            ]),
        ]),
    );
    runtime.render(window_id).unwrap();
    let press = Point::new(500.0, 32.0);
    let mut down = PointerEvent::new(PointerEventKind::Down, press);
    down.pointer_id = 1;
    down.button = Some(PointerButton::Secondary);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    let root = runtime.render(window_id).unwrap();
    let owner = root
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Export"))
        .expect("submenu owner present")
        .bounds;
    runtime
        .handle_event(
            window_id,
            Event::Pointer(PointerEvent::new(
                PointerEventKind::Move,
                super::rect_center(owner),
            )),
        )
        .unwrap();

    let nested = runtime.render(window_id).unwrap();
    let child = nested
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Archive")
        })
        .expect("submenu leaf present")
        .bounds;
    assert!(
        child.max_x() <= owner.x(),
        "the first submenu should reserve room for its nested cascade"
    );

    runtime
        .handle_event(
            window_id,
            Event::Pointer(PointerEvent::new(
                PointerEventKind::Move,
                super::rect_center(child),
            )),
        )
        .unwrap();
    let deeply_nested = runtime.render(window_id).unwrap();
    let grandchild = deeply_nested
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Zip"))
        .expect("nested submenu leaf present")
        .bounds;
    assert!(
        grandchild.max_x() <= child.x(),
        "the planned cascade should keep nested panels from covering ancestors"
    );
    assert!(grandchild.x() >= 4.0);
}

#[test]
fn context_menu_opens_at_the_right_click_position() -> Result<(), String> {
    let (mut runtime, window_id) = build_runtime(
        ContextMenu::new(
            "Surface menu",
            crate::containers::SizedBox::new()
                .width(600.0)
                .height(400.0),
        )
        .items([MenuItem::new("Copy"), MenuItem::new("Select All")]),
    );

    runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let press = Point::new(220.0, 140.0);
    let mut down = PointerEvent::new(PointerEventKind::Down, press);
    down.pointer_id = 1;
    down.button = Some(PointerButton::Secondary);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .map_err(|error| error.to_string())?;

    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let first_item = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Copy"))
        .expect("menu item semantics present");
    let theme = DefaultTheme::default();
    let padding = theme.metrics.menu_padding;
    assert!(
        (first_item.bounds.x() - (press.x + padding.left)).abs() < 1.0,
        "menu should open at the press x, got item at {:?}",
        first_item.bounds
    );
    assert!(
        (first_item.bounds.y() - (press.y + padding.top)).abs() < 1.0,
        "menu should open at the press y, got item at {:?}",
        first_item.bounds
    );

    // The menu must also actually paint at the anchored position.
    let label = text_run_for(&output, "Copy");
    assert!(
        (label.rect.y() - press.y).abs() < theme.metrics.menu_padding.top + 16.0,
        "menu label should paint near the press position, got {:?}",
        label.rect
    );
    Ok(())
}

#[test]
fn context_menu_routes_copy_to_read_only_text_area() -> Result<(), String> {
    let value = "node = local\naddress = 127.0.0.1:21353";
    let selection = SelectionScope::new();
    let menu = ContextMenu::new(
        "Connection details menu",
        TextArea::new("Connection details")
            .value(value)
            .read_only()
            .selectable(selection.clone()),
    );
    let text_area_id = menu.trigger_id();
    let menu = menu
        .items([MenuItem::new("Copy")])
        .on_activate_with_ctx(move |ctx, _, _| {
            ctx.post_command(text_area_id, TEXT_COMMAND, TextCommand::Copy);
        });
    let (mut runtime, window_id) = build_runtime(menu);

    runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        runtime
            .handle_semantics_action(
                window_id,
                text_area_id,
                SemanticsActionRequest::SetSelection(sui_core::SemanticsTextRange::new(
                    0,
                    value.len(),
                )),
            )
            .map_err(|error| error.to_string())?
    );
    assert_eq!(selection.selected_text().as_deref(), Some(value));

    let press = Point::new(20.0, 20.0);
    let mut right_click = PointerEvent::new(PointerEventKind::Down, press);
    right_click.pointer_id = 1;
    right_click.button = Some(PointerButton::Secondary);
    runtime
        .handle_event(window_id, Event::Pointer(right_click))
        .map_err(|error| error.to_string())?;

    let open = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let copy = open
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Copy"))
        .expect("copy context-menu item should be presented")
        .bounds;
    let copy_center = Point::new(
        copy.x() + copy.width() * 0.5,
        copy.y() + copy.height() * 0.5,
    );
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, copy_center, true),
        )
        .map_err(|error| error.to_string())?;
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, copy_center, false),
        )
        .map_err(|error| error.to_string())?;

    assert_eq!(runtime.clipboard().text().as_deref(), Some(value));
    assert_eq!(selection.selected_text().as_deref(), Some(value));
    Ok(())
}

#[test]
fn context_menu_pointer_activation_escapes_tight_trigger_inside_scroll_view() -> Result<(), String>
{
    let activations = Rc::new(Cell::new(0));
    let recorded_activations = Rc::clone(&activations);
    let menu = ContextMenu::new(
        "Connection details menu",
        TextArea::new("Connection details")
            .value("node = local\naddress = 127.0.0.1:21353")
            .read_only(),
    )
    .items([MenuItem::new("Select all"), MenuItem::new("Copy")])
    .on_activate(move |index, _| {
        if index == 1 {
            activations.set(activations.get() + 1);
        }
    });
    let root = SizedBox::new()
        .size(Size::new(600.0, 360.0))
        .with_child(ScrollView::vertical(Padding::all(
            24.0,
            Stack::vertical()
                .spacing(16.0)
                .with_child(SizedBox::new().height(96.0))
                .with_child(SizedBox::new().height(68.0).width(480.0).with_child(menu))
                .with_child(SizedBox::new().height(420.0)),
        )));
    let (mut runtime, window_id) = build_runtime(root);

    let closed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = closed
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::TextInput
                && node.name.as_deref() == Some("Connection details")
        })
        .expect("nested context-menu trigger should be present")
        .bounds;
    assert!(
        trigger.height() <= 68.0,
        "trigger should remain constrained by the tight settings row"
    );
    let trigger_center = Point::new(
        trigger.x() + trigger.width() * 0.5,
        trigger.y() + trigger.height() * 0.5,
    );
    let mut right_click = PointerEvent::new(PointerEventKind::Down, trigger_center);
    right_click.pointer_id = 1;
    right_click.button = Some(PointerButton::Secondary);
    runtime
        .handle_event(window_id, Event::Pointer(right_click))
        .map_err(|error| error.to_string())?;

    let open = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let copy = open
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Copy"))
        .expect("copy item should be presented outside the tight trigger")
        .bounds;
    assert!(
        copy.max_y() > trigger.max_y(),
        "dropdown row should exercise out-of-trigger hit testing: trigger={trigger:?}, copy={copy:?}"
    );
    let copy_center = Point::new(
        copy.x() + copy.width() * 0.5,
        copy.y() + copy.height() * 0.5,
    );
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, copy_center, true),
        )
        .map_err(|error| error.to_string())?;
    let pressed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let context_menu = pressed
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ContextMenu
                && node.name.as_deref() == Some("Connection details menu")
        })
        .expect("context menu should remain present while its row is pressed");
    assert!(
        context_menu.state.focused,
        "ancestor scrollers must not steal focus during pointer capture"
    );
    assert_eq!(context_menu.state.expanded, Some(true));
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, copy_center, false),
        )
        .map_err(|error| error.to_string())?;

    assert_eq!(recorded_activations.get(), 1);
    Ok(())
}

#[test]
fn context_menu_shortcut_aligns_to_trailing_edge() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .width(320.0)
            .height(180.0)
            .with_child(
                ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                    .anchor_to_pointer(false)
                    .items([MenuItem::new("Rename").shortcut("F2")]),
            ),
    );

    let closed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = closed
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button)
        .expect("context menu trigger present")
        .bounds;
    let trigger_center = Point::new(
        trigger.x() + (trigger.width() * 0.5),
        trigger.y() + (trigger.height() * 0.5),
    );

    let mut down = PointerEvent::new(PointerEventKind::Down, trigger_center);
    down.pointer_id = 1;
    down.button = Some(PointerButton::Secondary);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .map_err(|error| error.to_string())?;

    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let row = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Rename"))
        .expect("context menu item semantics present")
        .bounds;
    let label_clip = clip_rect_for_text(&output, "Rename");
    let shortcut = text_run_for(&output, "F2");
    let label_edge =
        row.max_x() - theme.metrics.menu_item_padding.right - theme.metrics.menu_shortcut_width;
    let shortcut_edge = row.max_x() - theme.metrics.menu_item_padding.right;

    assert_eq!(shortcut.style.color, theme.placeholder_text_style().color);
    assert!(
        (label_clip.max_x() - label_edge).abs() < 0.75,
        "label clip {:?} should end at {label_edge}; row={row:?}",
        label_clip
    );
    assert!((shortcut.rect.max_x() - shortcut_edge).abs() < 0.75);
    assert!((text_run_visual_center(&shortcut) - (row.y() + row.height() * 0.5)).abs() < 0.75);
    Ok(())
}

#[test]
fn context_menu_shortcuts_preserve_tall_measurements_and_row_center() -> Result<(), String> {
    let mut theme = DefaultTheme::default();
    theme.typography.body_font_size = 28.0;
    theme.typography.body_line_height = 12.0;
    theme.metrics.menu_row_height = 64.0;
    let metrics = theme.metrics;
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .width(320.0)
            .height(180.0)
            .with_child(
                ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                    .theme(theme)
                    .items([MenuItem::new("Rename").shortcut("F2")]),
            ),
    );

    let closed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = closed
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button)
        .expect("context menu trigger present")
        .bounds;
    let trigger_center = Point::new(
        trigger.x() + (trigger.width() * 0.5),
        trigger.y() + (trigger.height() * 0.5),
    );

    let mut down = PointerEvent::new(PointerEventKind::Down, trigger_center);
    down.pointer_id = 1;
    down.button = Some(PointerButton::Secondary);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .map_err(|error| error.to_string())?;

    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let row = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Rename"))
        .expect("context menu item semantics present")
        .bounds;
    let label = text_run_for(&output, "Rename");
    let shortcut = text_run_for(&output, "F2");
    let label_layout = TextSystem::new()
        .shape_text_run(&label, &FontRegistry::new())
        .expect("context menu item text should shape");
    let shortcut_layout = TextSystem::new()
        .shape_text_run(&shortcut, &FontRegistry::new())
        .expect("context menu shortcut text should shape");
    let shortcut_edge = row.max_x() - metrics.menu_item_padding.right;
    let row_center = row.y() + (row.height() * 0.5);

    assert_eq!(label.style.font_size, 28.0);
    assert_eq!(label.style.line_height, 12.0);
    assert_eq!(shortcut.style.font_size, 28.0);
    assert_eq!(shortcut.style.line_height, 12.0);
    assert!(label.rect.height() >= label_layout.measurement().height - 0.01);
    assert!(shortcut.rect.height() >= shortcut_layout.measurement().height - 0.01);
    assert!(label.rect.height() > label.style.line_height);
    assert!(shortcut.rect.height() > shortcut.style.line_height);
    assert!((shortcut.rect.max_x() - shortcut_edge).abs() < 0.75);
    assert!((text_run_visual_center(&label) - row_center).abs() < 0.75);
    assert!((text_run_visual_center(&shortcut) - row_center).abs() < 0.75);
    Ok(())
}

#[test]
fn context_menu_entrance_uses_theme_motion_layer_properties() -> Result<(), String> {
    let theme = slow_normal_motion_theme();
    let duration = theme.motion.entrance_duration();
    let (mut runtime, window_id) = build_runtime(
        ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
            .theme(theme)
            .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")]),
    );

    let closed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(overlay_layer_descriptor(&closed).is_none());
    let trigger = closed
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button)
        .expect("context menu trigger present")
        .bounds;
    let trigger_center = Point::new(
        trigger.x() + (trigger.width() * 0.5),
        trigger.y() + (trigger.height() * 0.5),
    );

    let mut down = PointerEvent::new(PointerEventKind::Down, trigger_center);
    down.pointer_id = 1;
    down.button = Some(PointerButton::Secondary);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .map_err(|error| error.to_string())?;

    let start = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let context = start
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::ContextMenu)
        .expect("context menu semantics present");
    let start_descriptor =
        overlay_layer_descriptor(&start).expect("context menu overlay layer should appear");
    let menu_owner = overlay_layer_owner(&start).expect("context menu overlay owner present");
    assert_eq!(start_descriptor.properties.opacity, 0.0);
    assert!(start_descriptor.properties.translation.y < 0.0);
    assert!(
        layer_descriptor_for(&start, context.id).is_none(),
        "the context menu owner should not fade or translate the trigger"
    );

    runtime.tick(duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let mid_descriptor =
        overlay_layer_descriptor(&mid).expect("context menu overlay layer should stay active");
    assert!(mid_descriptor.properties.opacity > 0.0);
    assert!(mid_descriptor.properties.opacity < 1.0);
    assert!(mid_descriptor.properties.translation.y < 0.0);
    assert!(
        mid_descriptor.properties.translation.y.abs()
            < start_descriptor.properties.translation.y.abs()
    );
    assert!(
        mid.frame.layer_updates.iter().any(|update| {
            update.owner == menu_owner
                && matches!(
                    update.kind,
                    SceneLayerUpdateKind::Transform | SceneLayerUpdateKind::Effect
                )
        }),
        "context menu entrance should update retained layer properties"
    );
    assert!(
        !mid.frame.layer_updates.iter().any(|update| {
            update.owner == menu_owner && update.kind == SceneLayerUpdateKind::Content
        }),
        "context menu entrance should not repaint menu content"
    );
    assert!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?
            .is_some()
    );

    runtime.tick(duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let settled_descriptor =
        overlay_layer_descriptor(&settled).expect("context menu overlay layer should remain");
    assert_eq!(settled_descriptor.properties.opacity, 1.0);
    assert_eq!(settled_descriptor.properties.translation.y, 0.0);
    assert_eq!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?,
        None
    );
    Ok(())
}

#[test]
fn context_menu_focus_ring_uses_non_hit_test_retained_layer() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let focus_duration = theme.motion.focus_duration();
    let (mut runtime, window_id) = build_runtime(
        ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
            .theme(theme)
            .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")]),
    );

    let closed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = closed
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button)
        .expect("context menu trigger present")
        .bounds;
    let trigger_center = Point::new(
        trigger.x() + (trigger.width() * 0.5),
        trigger.y() + (trigger.height() * 0.5),
    );

    let mut down = PointerEvent::new(PointerEventKind::Down, trigger_center);
    down.pointer_id = 1;
    down.button = Some(PointerButton::Secondary);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .map_err(|error| error.to_string())?;

    let opened = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let menu_owner = overlay_layer_owner(&opened).expect("context menu overlay owner present");
    let overlay =
        overlay_layer_descriptor(&opened).expect("context menu overlay layer should appear");
    assert!(overlay.hit_test);
    let focus_layers = non_hit_test_layer_descriptors(&opened);
    assert_eq!(
        focus_layers.len(),
        1,
        "context menu focus chrome should be the only non-hit-test layer"
    );
    assert_eq!(
        focus_layers[0].composition_mode,
        LayerCompositionMode::Normal
    );
    let focus_owner = non_hit_test_layer_owners(&opened)
        .into_iter()
        .next()
        .expect("context menu focus layer owner present");

    runtime.tick(focus_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_focus = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !contains_approx_color(&solid_stroke_colors(&mid_focus), theme.palette.focus_ring),
        "context menu focus ring should not snap to the settled focus color"
    );
    assert!(
        !mid_focus.frame.layer_updates.iter().any(|update| {
            update.owner == menu_owner && update.kind == SceneLayerUpdateKind::Content
        }),
        "context menu rows should stay retained during focus chrome animation"
    );
    assert!(
        mid_focus
            .frame
            .layer_updates
            .iter()
            .any(|update| update.owner == focus_owner),
        "context menu focus layer should receive the animation update"
    );

    runtime.tick(focus_duration + 0.01);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_focus = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let settled_strokes = solid_stroke_colors(&settled_focus);
    assert!(
        contains_approx_color(&settled_strokes, theme.palette.focus_ring),
        "context menu focus ring should settle to the theme focus color; strokes={settled_strokes:?}"
    );

    Ok(())
}

#[test]
fn context_menu_row_hover_and_press_use_theme_motion() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let hover_duration = theme.motion.hover_duration();
    let press_duration = theme.motion.press_duration();
    let expected_hover = super::mix_color(
        theme.palette.control,
        theme.palette.accent,
        theme.interaction.selected_blend,
    );
    let expected_press = super::mix_color(
        expected_hover,
        theme.palette.control_active,
        theme.interaction.pressed_blend,
    );
    let (mut runtime, window_id) = build_runtime(
        ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
            .theme(theme)
            .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")]),
    );

    let closed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = closed
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button)
        .expect("context menu trigger present")
        .bounds;
    let trigger_center = super::rect_center(trigger);
    let mut secondary_down = PointerEvent::new(PointerEventKind::Down, trigger_center);
    secondary_down.pointer_id = 1;
    secondary_down.button = Some(PointerButton::Secondary);
    secondary_down.buttons = PointerButtons::new(2);
    runtime
        .handle_event(window_id, Event::Pointer(secondary_down))
        .map_err(|error| error.to_string())?;

    let opened = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let duplicate = opened
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Duplicate")
        })
        .expect("duplicate menu item semantics should exist");
    let position = super::rect_center(duplicate.bounds);

    let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
    move_event.pointer_id = 1;
    runtime
        .handle_event(window_id, Event::Pointer(move_event))
        .map_err(|error| error.to_string())?;

    runtime.tick(hover_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_hover = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !solid_fill_colors(&mid_hover).contains(&expected_hover),
        "context menu hover fill should not snap to the settled highlighted color"
    );

    runtime.tick(hover_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_hover = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )
        .map_err(|error| error.to_string())?;

    runtime.tick(hover_duration + press_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_press = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !solid_fill_colors(&mid_press).contains(&expected_press),
        "context menu press fill should not snap to the settled pressed color"
    );

    runtime.tick(hover_duration + press_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_press = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&settled_press).contains(&expected_press));

    Ok(())
}

#[test]
fn progress_bar_value_text_visual_center_matches_control_center() {
    let output = render_isolated(
        ProgressBar::new("Export progress")
            .range(0.0, 100.0)
            .value(42.0)
            .show_value(true),
    );
    let text = text_run_for(&output, "42%");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("progress bar label should shape");
    let line = layout
        .lines()
        .first()
        .expect("progress bar label should contain one line");
    let actual_visual_center =
        text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
    let control_center = output.frame.viewport.height * 0.5;

    assert!(
        text.style
            .features
            .iter()
            .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
    );
    assert!((actual_visual_center - control_center).abs() < 0.75);
}

#[test]
fn progress_bar_value_text_preserves_tall_measurements_and_exact_centering() {
    let mut theme = DefaultTheme::default();
    theme.text.sm = ThemeTextToken {
        size: 28.0,
        line_height: 12.0,
    };
    theme.sync_derived_fields();

    let output = render_isolated(
        ProgressBar::new("Export progress")
            .theme(theme)
            .range(0.0, 100.0)
            .value(42.0)
            .height(48.0)
            .show_value(true),
    );
    let text = text_run_for(&output, "42%");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("progress bar label should shape");

    assert_text_run_uses_token(&text, theme.text.sm);
    assert!(text.rect.height() >= layout.measurement().height - 0.01);
    assert!(text.rect.height() > text.style.line_height);
    assert!(
        (text_visual_center_for(&output, "42%") - (output.frame.viewport.height * 0.5)).abs()
            < 0.75
    );
}

#[test]
fn spinner_label_visual_center_matches_indicator_center() {
    let output = render(Spinner::new("Background work").label("Uploading textures"));
    let text = text_run_for(&output, "Uploading textures");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("spinner label should shape");
    let line = layout
        .lines()
        .first()
        .expect("spinner label should contain one line");
    let actual_visual_center =
        text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
    let indicator_center = output.frame.viewport.height * 0.5;

    assert!((actual_visual_center - indicator_center).abs() < 0.75);
}

#[test]
fn spinner_label_preserves_tall_measurement_and_indicator_centering() {
    let mut theme = DefaultTheme::default();
    theme.text.sm = ThemeTextToken {
        size: 28.0,
        line_height: 10.0,
    };
    theme.sync_derived_fields();
    let output = render(
        Spinner::new("Background work")
            .theme(theme)
            .label("Uploading"),
    );
    let text = text_run_for(&output, "Uploading");
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("spinner label should shape");
    let busy = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::BusyIndicator)
        .expect("spinner semantics should exist");
    let center_y = busy.bounds.y() + busy.bounds.height() * 0.5;

    assert_text_run_uses_token(&text, theme.text.sm);
    assert!(busy.bounds.height() > 20.0);
    assert!(text.rect.height() >= layout.measurement().height - 0.01);
    assert!(text.rect.height() > text.style.line_height);
    assert!((text_run_visual_center(&text) - center_y).abs() < 0.75);
}

#[test]
fn progress_bar_and_spinner_publish_semantics() {
    let output = render(sui_widgets_fixture(
        ProgressBar::new("Export progress")
            .range(0.0, 100.0)
            .value(42.0),
        Spinner::new("Background work").label("Uploading textures"),
    ));

    let progress = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::ProgressBar)
        .expect("progress bar node present");
    assert_eq!(
        progress.value,
        Some(SemanticsValue::Range {
            value: 42.0,
            min: 0.0,
            max: 100.0,
        })
    );
    let spinner = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::BusyIndicator)
        .expect("spinner node present");
    assert!(spinner.state.busy);
}

#[test]
fn open_popover_uses_direct_overlay_layer_metadata() {
    let output = render(crate::Padding::all(
        16.0,
        Popover::new(
            "Inline inspector",
            crate::Button::new("Open inspector"),
            crate::Label::new("popover body"),
        )
        .open(true),
    ));

    let descriptor = overlay_layer_descriptor(&output).expect("popover layer descriptor present");

    assert!(descriptor.is_stack_surface);
    assert_eq!(descriptor.composition_mode, LayerCompositionMode::Overlay);
}

#[test]
fn tooltip_paints_with_surface_tokens() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let (mut runtime, window_id) = build_runtime(crate::Padding::all(
        16.0,
        crate::Tooltip::new(
            "Quick access to common commands",
            crate::Button::new("Hover for shortcuts").min_width(180.0),
        )
        .theme(theme),
    ));

    let initial = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = initial
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some("Hover for shortcuts")
        })
        .expect("tooltip trigger semantics present")
        .bounds;
    let hover_point = Point::new(trigger.x() + 12.0, trigger.y() + (trigger.height() * 0.5));
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, hover_point, false),
        )
        .map_err(|error| error.to_string())?;

    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(solid_fill_colors(&output).contains(&theme.surfaces.tooltip));
    let mut painted_tooltip_border = false;
    output
        .frame
        .scene
        .visit_commands(&mut |command| match command {
            SceneCommand::StrokeRect {
                brush: Brush::Solid(color),
                ..
            }
            | SceneCommand::StrokePath {
                brush: Brush::Solid(color),
                ..
            } if *color == theme.surfaces.tooltip_border => {
                painted_tooltip_border = true;
            }
            _ => {}
        });
    assert!(painted_tooltip_border);
    Ok(())
}

#[test]
fn tooltip_end_alignment_keeps_bubble_inside_trailing_trigger_edge() -> Result<(), String> {
    let tooltip_text = "Enter sends · Shift+Enter newline";
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(360.0, 160.0))
            .with_child(crate::Padding::all(
                16.0,
                crate::Tooltip::new(
                    tooltip_text,
                    crate::Button::new("Send message").min_width(32.0),
                )
                .alignment(super::TooltipAlignment::End),
            )),
    );

    let initial = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = initial
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Send message")
        })
        .expect("tooltip trigger semantics present")
        .bounds;
    runtime
        .handle_event(
            window_id,
            primary_pointer(
                PointerEventKind::Move,
                Point::new(trigger.x() + 8.0, trigger.y() + 8.0),
                false,
            ),
        )
        .map_err(|error| error.to_string())?;

    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let tooltip = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Tooltip && node.name.as_deref() == Some(tooltip_text)
        })
        .expect("tooltip semantics present");
    assert!(
        (tooltip.bounds.max_x() - trigger.max_x()).abs() < 0.01,
        "end-aligned tooltip should share the trigger's trailing edge: trigger={trigger:?}, tooltip={:?}",
        tooltip.bounds
    );
    Ok(())
}

#[test]
fn tooltip_text_visual_center_matches_padded_bubble_center() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let tooltip_text = "Quick access to common commands";
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(360.0, 160.0))
            .with_child(crate::Padding::all(
                16.0,
                crate::Tooltip::new(
                    tooltip_text,
                    crate::Button::new("Hover for shortcuts").min_width(180.0),
                )
                .theme(theme),
            )),
    );

    let initial = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = initial
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some("Hover for shortcuts")
        })
        .expect("tooltip trigger semantics present")
        .bounds;
    let hover_point = Point::new(trigger.x() + 12.0, trigger.y() + (trigger.height() * 0.5));
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, hover_point, false),
        )
        .map_err(|error| error.to_string())?;

    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let tooltip = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Tooltip && node.name.as_deref() == Some(tooltip_text)
        })
        .expect("tooltip semantics present");
    let text = text_run_for(&output, tooltip_text);
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("tooltip text should shape");
    let line = layout
        .lines()
        .first()
        .expect("tooltip text should contain one line");
    let actual_visual_center =
        text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
    let text_slot = super::inset_rect(tooltip.bounds, theme.metrics.tooltip_padding);

    assert!((actual_visual_center - super::rect_center(text_slot).y).abs() < 0.75);
    Ok(())
}

#[test]
fn tooltip_text_preserves_tall_measurement_in_padded_bubble() -> Result<(), String> {
    let mut theme = DefaultTheme::default();
    theme.text.sm = ThemeTextToken {
        size: 28.0,
        line_height: 10.0,
    };
    theme.sync_derived_fields();
    let tooltip_text = "Quick commands";
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(360.0, 160.0))
            .with_child(crate::Padding::all(
                16.0,
                crate::Tooltip::new(
                    tooltip_text,
                    crate::Button::new("Hover for shortcuts").min_width(180.0),
                )
                .theme(theme),
            )),
    );

    let initial = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = initial
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some("Hover for shortcuts")
        })
        .expect("tooltip trigger semantics present")
        .bounds;
    let hover_point = Point::new(trigger.x() + 12.0, trigger.y() + (trigger.height() * 0.5));
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, hover_point, false),
        )
        .map_err(|error| error.to_string())?;

    let output = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let tooltip = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Tooltip && node.name.as_deref() == Some(tooltip_text)
        })
        .expect("tooltip semantics present");
    let text = text_run_for(&output, tooltip_text);
    let layout = TextSystem::new()
        .shape_text_run(&text, &FontRegistry::new())
        .expect("tooltip text should shape");
    let text_slot = super::inset_rect(tooltip.bounds, theme.metrics.tooltip_padding);

    assert_text_run_uses_token(&text, theme.text.sm);
    assert!(text.rect.height() >= layout.measurement().height - 0.01);
    assert!(text.rect.height() > text.style.line_height);
    assert!(
        (text_run_visual_center(&text) - super::rect_center(text_slot).y).abs() < 0.75,
        "tooltip text should remain visually centered in the padded bubble; rect={:?}, slot={:?}, measurement={:?}",
        text.rect,
        text_slot,
        layout.measurement()
    );
    Ok(())
}

#[test]
fn tooltip_reveal_animation_updates_layer_properties_until_complete() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let entrance_duration = theme.motion.entrance_duration();

    let (mut runtime, window_id) = build_runtime(crate::Padding::all(
        16.0,
        crate::Tooltip::new(
            "Quick access to common commands",
            crate::Button::new("Hover for shortcuts").min_width(180.0),
        )
        .theme(theme),
    ));

    let initial = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(overlay_layer_descriptor(&initial).is_none());

    let trigger = initial
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some("Hover for shortcuts")
        })
        .expect("tooltip trigger semantics present")
        .bounds;
    let hover_point = Point::new(trigger.x() + 12.0, trigger.y() + (trigger.height() * 0.5));
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, hover_point, false),
        )
        .map_err(|error| error.to_string())?;

    let start = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let start_descriptor =
        overlay_layer_descriptor(&start).expect("tooltip overlay layer should appear");
    assert!(
        !start_descriptor.hit_test,
        "tooltip overlay should not intercept pointer hit testing"
    );
    assert_eq!(
        start_descriptor.properties.translation.y.signum(),
        -1.0,
        "tooltip reveal should start offset upward"
    );
    assert_eq!(
        start_descriptor.properties.translation.y.abs(),
        theme.metrics.tooltip_reveal_offset
    );
    assert_eq!(start_descriptor.properties.opacity, 0.0);

    runtime.tick(entrance_duration * 0.5);
    assert!(handle_ready_events(&mut runtime)? >= 1);
    let mid = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let mid_descriptor =
        overlay_layer_descriptor(&mid).expect("tooltip overlay layer should stay active");
    assert!(mid_descriptor.properties.opacity > 0.0);
    assert!(mid_descriptor.properties.opacity < 1.0);
    assert!(mid_descriptor.properties.translation.y < 0.0);
    assert!(
        mid_descriptor.properties.translation.y.abs()
            < start_descriptor.properties.translation.y.abs()
    );
    assert!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?
            .is_some()
    );

    runtime.tick(entrance_duration);
    assert!(handle_ready_events(&mut runtime)? >= 1);
    let settled = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let settled_descriptor =
        overlay_layer_descriptor(&settled).expect("tooltip overlay layer should still exist");
    assert_eq!(settled_descriptor.properties.opacity, 1.0);
    assert_eq!(settled_descriptor.properties.translation.y, 0.0);
    assert_eq!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?,
        None
    );

    Ok(())
}

#[test]
fn popover_open_animation_stops_requesting_frames_after_completion() -> Result<(), String> {
    let theme = slow_normal_motion_theme();
    let entrance_duration = theme.motion.entrance_duration();

    let content = Rc::new(RefCell::new(PanelCounters::default()));
    let (mut runtime, window_id) = build_runtime(crate::Padding::all(
        16.0,
        Popover::new(
            "Inline inspector",
            crate::Button::new("Open inspector").min_width(180.0),
            SpyPanel::new("popover-content", Rc::clone(&content)),
        )
        .theme(theme),
    ));

    let closed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = closed
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Open inspector")
        })
        .expect("popover trigger semantics present")
        .bounds;
    assert_eq!(content.borrow().paint, 0);

    let press_point = Point::new(trigger.x() + 12.0, trigger.y() + (trigger.height() * 0.5));
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, press_point, true),
        )
        .map_err(|error| error.to_string())?;

    let opened = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let open_descriptor =
        overlay_layer_descriptor(&opened).expect("popover overlay layer should appear");
    assert_eq!(content.borrow().paint, 1);
    assert_eq!(open_descriptor.properties.opacity, 0.0);
    assert!(open_descriptor.properties.translation.y < 0.0);

    runtime.tick(entrance_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let mid_descriptor =
        overlay_layer_descriptor(&mid).expect("popover overlay layer should stay active");
    assert!(mid_descriptor.properties.opacity > 0.0);
    assert!(mid_descriptor.properties.opacity < 1.0);
    assert!(mid_descriptor.properties.translation.y < 0.0);
    assert_eq!(
        content.borrow().paint,
        1,
        "popover content should stay retained while only layer properties change"
    );
    assert!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?
            .is_some()
    );

    runtime.tick(entrance_duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let settled_descriptor =
        overlay_layer_descriptor(&settled).expect("popover overlay layer should remain open");
    assert_eq!(settled_descriptor.properties.opacity, 1.0);
    assert_eq!(settled_descriptor.properties.translation.y, 0.0);
    assert_eq!(
        content.borrow().paint,
        1,
        "popover content should not repaint on retained-only animation frames"
    );
    assert_eq!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?,
        None
    );

    Ok(())
}

#[test]
fn popover_focus_ring_animates_without_repainting_retained_content() -> Result<(), String> {
    let theme = DefaultTheme::default();
    let entrance_duration = theme.motion.entrance_duration();
    let focus_duration = theme.motion.focus_duration();

    let content = Rc::new(RefCell::new(PanelCounters::default()));
    let (mut runtime, window_id) = build_runtime(crate::Padding::all(
        16.0,
        Popover::new(
            "Inline inspector",
            crate::Button::new("Open inspector").min_width(180.0),
            SpyPanel::new("popover-content", Rc::clone(&content)),
        )
        .theme(theme),
    ));

    let closed = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let trigger = closed
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Open inspector")
        })
        .expect("popover trigger semantics present")
        .bounds;
    assert_eq!(content.borrow().paint, 0);

    let press_point = Point::new(trigger.x() + 12.0, trigger.y() + (trigger.height() * 0.5));
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, press_point, true),
        )
        .map_err(|error| error.to_string())?;

    let opened = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let open_descriptor =
        overlay_layer_descriptor(&opened).expect("popover overlay layer should appear");
    assert!(open_descriptor.hit_test);
    let open_focus_layers = non_hit_test_layer_descriptors(&opened);
    assert_eq!(
        open_focus_layers.len(),
        1,
        "popover focus chrome should be the only non-hit-test layer"
    );
    assert_eq!(
        open_focus_layers[0].composition_mode,
        LayerCompositionMode::Normal
    );
    assert_eq!(content.borrow().paint, 1);

    runtime.tick(focus_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid_focus = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    assert!(
        !contains_approx_color(&solid_stroke_colors(&mid_focus), theme.palette.focus_ring),
        "popover focus ring should not snap to the settled focus color"
    );
    assert_eq!(
        content.borrow().paint,
        1,
        "popover content should stay retained while focus chrome repaints"
    );

    runtime.tick(entrance_duration.max(focus_duration) + 0.01);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled_focus = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let settled_strokes = solid_stroke_colors(&settled_focus);
    assert!(
        contains_approx_color(&settled_strokes, theme.palette.focus_ring),
        "popover focus ring should settle to the theme focus color; strokes={settled_strokes:?}"
    );
    assert_eq!(
        content.borrow().paint,
        1,
        "popover content should not repaint on focus-only animation frames"
    );
    assert_eq!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?,
        None
    );

    Ok(())
}

#[test]
fn popover_arrival_effect_obeys_hdr_theme_mode() {
    let mut disabled_theme = DefaultTheme::default();
    disabled_theme.hdr.mode = HdrThemeMode::Disabled;
    disabled_theme.hdr.policy.max_large_area_lift = 1.12;
    disabled_theme.hdr.color_roles.surface_elevated =
        SemanticColorToken::from_sdr(disabled_theme.palette.surface_raised)
            .with_hdr(Color::linear_display_p3(1.30, 1.08, 1.05, 1.0));

    let mut disabled = Popover::new(
        "Options",
        crate::Button::new("Open"),
        crate::Label::new("Popover body"),
    )
    .theme(disabled_theme);
    disabled.open = true;
    {
        let mut state = disabled.state.borrow_mut();
        state.reveal = super::AnimatedScalar::new(1.0);
        state.arrival_active = true;
    }
    let disabled_visuals = disabled.state.borrow().resolved_visuals();

    assert_eq!(
        disabled_visuals.background,
        disabled_theme.palette.surface_raised
    );
    assert!(disabled_visuals.surface_style.is_none());
    assert!(disabled_visuals.arrival_effect.is_none());

    let (mut disabled_runtime, disabled_window) = build_runtime(
        Popover::new(
            "Options",
            crate::Button::new("Open"),
            crate::Label::new("Popover body"),
        )
        .theme(disabled_theme),
    );
    let _ = disabled_runtime.render(disabled_window).unwrap();
    disabled_runtime
        .handle_event(
            disabled_window,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )
        .unwrap();
    let disabled_output = disabled_runtime.render(disabled_window).unwrap();
    assert!(
        !solid_fill_colors(&disabled_output)
            .iter()
            .any(|color| color.alpha < 1.0)
    );

    let mut hdr_theme = disabled_theme;
    hdr_theme.hdr.mode = HdrThemeMode::ConstrainedHdr;
    hdr_theme.hdr.color_roles.surface_elevated =
        SemanticColorToken::from_sdr(hdr_theme.palette.surface_raised)
            .with_hdr(Color::linear_display_p3(1.30, 1.08, 1.05, 1.0));

    let mut hdr = Popover::new(
        "Options",
        crate::Button::new("Open"),
        crate::Label::new("Popover body"),
    )
    .theme(hdr_theme);
    hdr.open = true;
    {
        let mut state = hdr.state.borrow_mut();
        state.reveal = super::AnimatedScalar::new(1.0);
        state.arrival_active = true;
    }
    let hdr_visuals = hdr.state.borrow().resolved_visuals();
    let surface_style = hdr_visuals
        .surface_style
        .expect("hdr surface style present");
    let arrival_effect = hdr_visuals
        .arrival_effect
        .expect("pulse arrival effect present");

    assert_eq!(hdr_visuals.background, surface_style.color);
    assert!(surface_style.color.red <= hdr_theme.hdr.policy.max_large_area_lift);
    assert_ne!(hdr_visuals.background, hdr_theme.palette.surface_raised);
    assert!(arrival_effect.intensity > 0.0);
    assert!(arrival_effect.speed > 0.0);

    let (mut runtime, window_id) = build_runtime(
        Popover::new(
            "Options",
            crate::Button::new("Open"),
            crate::Label::new("Popover body"),
        )
        .theme(hdr_theme),
    );
    let _ = runtime.render(window_id).unwrap();
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )
        .unwrap();
    let arrival_output = runtime.render(window_id).unwrap();
    assert!(
        solid_fill_colors(&arrival_output)
            .iter()
            .any(|color| color.alpha < 1.0)
    );

    runtime.tick(1.0);
    for (ready_window, event) in runtime.drain_ready_events() {
        runtime.handle_event(ready_window, event).unwrap();
    }
    let settled_output = runtime.render(window_id).unwrap();
    assert!(
        !solid_fill_colors(&settled_output)
            .iter()
            .any(|color| color.alpha < 1.0)
    );
}

#[test]
fn open_popover_resolves_to_nearest_stack_host_and_tracks_owner_surface() {
    let (mut runtime, window_id) = build_runtime(
        FloatingStack::new().with_window(
            sui_core::Rect::new(24.0, 24.0, 240.0, 160.0),
            crate::Padding::all(
                16.0,
                Popover::new(
                    "Options",
                    crate::Button::new("Open"),
                    crate::Label::new("Popover body"),
                )
                .open(true),
            ),
        ),
    );

    let output = runtime.render(window_id).unwrap();
    let graph = runtime.widget_graph(window_id).unwrap();
    let owner = overlay_layer_owner(&output).expect("popover layer owner present");
    let descriptor = overlay_layer_descriptor(&output).expect("popover layer descriptor present");
    let node = graph
        .nodes
        .iter()
        .find(|node| node.id == owner)
        .expect("popover graph node present");
    let host = graph
        .stack_hosts
        .iter()
        .find(|host| host.host == graph.root)
        .expect("root stack host present");

    assert_eq!(node.stack_host, graph.root);
    assert_eq!(node.stack_surface, owner);
    assert_eq!(node.transient_owner_surface, Some(host.surfaces[0]));
    assert_eq!(host.surfaces.last().copied(), Some(owner));
    assert_eq!(descriptor.stack_host, graph.root);
    assert_eq!(descriptor.transient_owner_surface, Some(host.surfaces[0]));
    assert!(descriptor.is_stack_surface);
}

#[test]
fn modal_dialog_uses_direct_effect_layer_metadata() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(640.0, 420.0))
            .with_child(Dialog::new(
                "Confirm",
                crate::Label::new("Apply the change?"),
            )),
    );

    let dialog = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("dialog semantics present");
    let descriptor =
        layer_descriptor_for(&output, dialog.id).expect("dialog layer descriptor present");

    assert_eq!(descriptor.composition_mode, LayerCompositionMode::Effect);
    assert!(solid_fill_colors(&output).contains(&DefaultTheme::default().surfaces.overlay_scrim));
}

#[test]
fn modal_dialog_first_pointer_click_reaches_scrolled_body_control() {
    let activated = Rc::new(Cell::new(false));
    let action = Rc::clone(&activated);
    let body = SizedBox::new()
        .height(200.0)
        .with_child(ScrollView::vertical(Stack::vertical().with_child(
            crate::Button::new("Done").on_press(move || action.set(true)),
        )));
    let (mut runtime, window_id) = build_runtime(
        SizedBox::new()
            .size(Size::new(640.0, 420.0))
            .with_child(Dialog::new("Confirm", body)),
    );

    let output = runtime.render(window_id).unwrap();
    let done = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Done"))
        .expect("dialog action semantics");
    let point = Point::new(
        done.bounds.x() + done.bounds.width() * 0.5,
        done.bounds.y() + done.bounds.height() * 0.5,
    );

    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, point, false),
        )
        .unwrap();
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, point, true),
        )
        .unwrap();
    runtime
        .handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, point, false),
        )
        .unwrap();

    assert!(
        activated.get(),
        "the dialog focus surface must not intercept the first click"
    );
}

#[test]
fn modal_dialog_entrance_uses_theme_motion_effect_layer_properties() -> Result<(), String> {
    let theme = slow_normal_motion_theme();
    let duration = theme.motion.entrance_duration();
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(640.0, 420.0))
            .with_child(
                Dialog::new("Confirm", crate::Label::new("Apply the change?")).theme(theme),
            ),
    );

    let start = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let dialog = start
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("dialog semantics present");
    let start_descriptor =
        layer_descriptor_for(&start, dialog.id).expect("dialog layer descriptor present");
    assert_eq!(
        start_descriptor.composition_mode,
        LayerCompositionMode::Effect
    );
    assert_eq!(start_descriptor.properties.opacity, 0.0);
    assert_eq!(start_descriptor.properties.translation, Vector::ZERO);
    assert!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?
            .is_some()
    );

    runtime.tick(duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let mid_descriptor =
        layer_descriptor_for(&mid, dialog.id).expect("dialog layer descriptor still present");
    assert!(mid_descriptor.properties.opacity > 0.0);
    assert!(mid_descriptor.properties.opacity < 1.0);
    assert_eq!(mid_descriptor.properties.translation, Vector::ZERO);

    runtime.tick(duration);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let settled_descriptor = layer_descriptor_for(&settled, dialog.id)
        .expect("dialog layer descriptor still present after settling");
    assert_eq!(settled_descriptor.properties.opacity, 1.0);
    assert_eq!(settled_descriptor.properties.translation, Vector::ZERO);
    assert_eq!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?,
        None
    );
    Ok(())
}

#[test]
fn dialog_entrance_animates_without_repainting_retained_body() -> Result<(), String> {
    let theme = slow_normal_motion_theme();
    let entrance_duration = theme.motion.entrance_duration();
    let body = Rc::new(RefCell::new(PanelCounters::default()));
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(640.0, 420.0))
            .with_child(
                Dialog::new("Confirm", SpyPanel::new("dialog-body", Rc::clone(&body))).theme(theme),
            ),
    );

    let start = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let dialog = start
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("dialog semantics present");
    let start_descriptor =
        layer_descriptor_for(&start, dialog.id).expect("dialog layer descriptor present");
    assert_eq!(start_descriptor.properties.opacity, 0.0);
    assert_eq!(body.borrow().paint, 1);

    runtime.tick(entrance_duration * 0.5);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let mid = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let mid_descriptor =
        layer_descriptor_for(&mid, dialog.id).expect("dialog layer descriptor still present");
    assert!(mid_descriptor.properties.opacity > 0.0);
    assert!(mid_descriptor.properties.opacity < 1.0);
    assert_eq!(
        body.borrow().paint,
        1,
        "dialog body should stay retained while entrance only changes layer properties"
    );

    runtime.tick(entrance_duration + 0.01);
    assert_eq!(handle_ready_events(&mut runtime)?, 1);
    let settled = runtime
        .render(window_id)
        .map_err(|error| error.to_string())?;
    let settled_descriptor = layer_descriptor_for(&settled, dialog.id)
        .expect("dialog layer descriptor still present after settling");
    assert_eq!(
        settled_descriptor.properties.opacity, 1.0,
        "dialog entrance should settle to full layer opacity"
    );
    assert_eq!(
        body.borrow().paint,
        1,
        "dialog body should not repaint on retained-only entrance frames"
    );
    assert_eq!(
        runtime
            .next_wakeup_time(window_id)
            .map_err(|error| error.to_string())?,
        None
    );

    Ok(())
}

#[test]
fn non_modal_dialog_entrance_uses_overlay_translation() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(640.0, 420.0))
            .with_child(Dialog::new("Inspector", crate::Label::new("Layer settings")).modal(false)),
    );

    let dialog = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("dialog semantics present");
    let descriptor =
        layer_descriptor_for(&output, dialog.id).expect("dialog layer descriptor present");

    assert_eq!(descriptor.composition_mode, LayerCompositionMode::Overlay);
    assert_eq!(descriptor.properties.opacity, 0.0);
    assert!(descriptor.properties.translation.y > 0.0);
}

#[test]
fn bottom_sheet_uses_requested_height_and_bottom_edge() {
    let output = render(
        crate::SizedBox::new()
            .size(Size::new(640.0, 480.0))
            .with_child(
                BottomSheet::new("Filters", crate::Label::new("Filter options")).height(240.0),
            ),
    );

    let sheet = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Dialog)
        .expect("bottom sheet semantics present");
    assert_eq!(sheet.name.as_deref(), Some("Filters"));
    assert_eq!(sheet.bounds, Rect::new(0.0, 240.0, 640.0, 240.0));
}

#[test]
fn sheet_state_presents_a_retained_bottom_sheet() {
    let state = SheetState::default();
    let (mut runtime, window_id) = build_runtime(
        crate::SizedBox::new()
            .size(Size::new(640.0, 480.0))
            .with_child(
                BottomSheet::new("Filters", crate::Label::new("Filter options"))
                    .state(state.clone()),
            ),
    );

    assert!(
        !runtime
            .render(window_id)
            .unwrap()
            .semantics
            .iter()
            .any(|node| node.role == SemanticsRole::Dialog)
    );
    state.show();
    assert!(
        runtime
            .render(window_id)
            .unwrap()
            .semantics
            .iter()
            .any(|node| node.name.as_deref() == Some("Filters"))
    );
}

fn sui_widgets_fixture<A, B>(top: A, bottom: B) -> impl Widget
where
    A: Widget + 'static,
    B: Widget + 'static,
{
    crate::Stack::vertical()
        .spacing(12.0)
        .with_child(top)
        .with_child(bottom)
}
