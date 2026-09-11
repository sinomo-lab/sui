//! Exercise the extended public binding constructors through the JS render bridge.
use crate::render_binding_widget;
use ::sui::{Axis, BrushPreviewShape, Color, Rect, SemanticTone, SideSheetPlacement, Size, Vector};
use sui_bindings_core::{
    BindingBrushPreviewSpec, BindingCanvasStroke, BindingCanvasViewport, BindingDragScope,
    BindingFloatingStackWindow, BindingFloatingView, BindingFloatingWorkspaceState,
    BindingNotificationCenter, BindingPixelCanvasState, BindingVirtualListItem,
    BindingVirtualListModel, BindingWidget,
};

#[test]
fn high_level_app_renders_extended_compatibility_signature() {
    let child = || BindingWidget::label("content");
    let viewport = || BindingCanvasViewport::new(Vector::ZERO, 1.0, 0.0);
    let size = Size::new(240.0, 160.0);
    let scope = BindingDragScope::new();
    let widgets = [
        ("OverlayHost", BindingWidget::overlay_host(child())),
        (
            "NotificationHost",
            BindingWidget::notification_host(BindingNotificationCenter::new(), 240.0),
        ),
        (
            "Canvas",
            BindingWidget::canvas(
                "canvas",
                viewport(),
                [],
                BindingCanvasStroke::new(Color::WHITE, 1.0),
                size,
            ),
        ),
        (
            "CanvasRuler",
            BindingWidget::canvas_ruler(Axis::Horizontal, "ruler", size, viewport(), size, None),
        ),
        (
            "PixelCanvas",
            BindingWidget::pixel_canvas(
                BindingPixelCanvasState::new(),
                "pixels",
                4,
                4,
                None,
                size,
                viewport(),
                true,
                Vec::new(),
            )
            .unwrap(),
        ),
        (
            "DragDropHost",
            BindingWidget::drag_drop_host(scope.clone(), child(), None, None, None),
        ),
        (
            "Draggable",
            BindingWidget::draggable(
                scope.clone(),
                child(),
                "payload",
                "copy",
                None,
                4.0,
                None,
                None,
            )
            .unwrap(),
        ),
        (
            "DropTarget",
            BindingWidget::drop_target(scope, child(), "copy", None, None).unwrap(),
        ),
        (
            "VirtualList",
            BindingWidget::virtual_list(
                "rows",
                BindingVirtualListModel::new(
                    "rows",
                    [BindingVirtualListItem::new(1, "row").unwrap()],
                )
                .unwrap(),
                24.0,
                0.0,
                None,
                None,
                0.5,
                16,
                true,
                false,
                false,
                false,
                None,
                None,
                None,
            ),
        ),
        (
            "CommandPalette",
            BindingWidget::command_palette("commands", child(), None, true, None, None),
        ),
        (
            "PasswordInput",
            BindingWidget::password_input("password", "secret", None, None),
        ),
        (
            "DateTimeInput",
            BindingWidget::datetime_input("date", "2026-09-11", None, None),
        ),
        (
            "ActionCard",
            BindingWidget::action_card(
                "action",
                "description",
                None,
                SemanticTone::Neutral,
                true,
                None,
            ),
        ),
        (
            "BrushPreview",
            BindingWidget::brush_preview(
                "brush",
                "ink",
                BindingBrushPreviewSpec::new(Color::WHITE, 16.0, 1.0, BrushPreviewShape::Round),
                None,
            ),
        ),
        (
            "CommandGroup",
            BindingWidget::command_group(
                "group",
                [child()],
                Axis::Horizontal,
                None,
                None,
                None,
                None,
                None,
            ),
        ),
        (
            "CoverageDots",
            BindingWidget::coverage_dots("coverage", 1, 2, SemanticTone::Neutral, 4, true, None),
        ),
        (
            "Dock",
            BindingWidget::dock(child(), Some((20.0, child())), None, 240.0, 160.0),
        ),
        (
            "FixedPaneSplit",
            BindingWidget::fixed_pane_split(
                Axis::Horizontal,
                child(),
                child(),
                child(),
                false,
                80.0,
                1.0,
                160.0,
            ),
        ),
        (
            "FramedField",
            BindingWidget::framed_field(child(), None, None, None, None, true, false, false),
        ),
        (
            "MeasuredBottomDock",
            BindingWidget::measured_bottom_dock(child(), child(), size),
        ),
        (
            "PlacementBadge",
            BindingWidget::placement_badge(
                "badge",
                None,
                SemanticTone::Neutral,
                Some(1),
                Some(2),
                None,
            ),
        ),
        (
            "PropertyRow",
            BindingWidget::property_row("property", child(), false, None, None, None),
        ),
        (
            "SectionLabel",
            BindingWidget::section_label("section", None, None),
        ),
        (
            "SideSheet",
            BindingWidget::side_sheet(
                "sheet",
                child(),
                None,
                true,
                false,
                true,
                SideSheetPlacement::Right,
                None,
                None,
                [],
                None,
            ),
        ),
        (
            "BottomSheet",
            BindingWidget::bottom_sheet(
                "sheet",
                child(),
                None,
                true,
                false,
                true,
                None,
                None,
                [],
                None,
            ),
        ),
        (
            "SplitView",
            BindingWidget::split_view(
                None,
                Axis::Horizontal,
                child(),
                child(),
                0.5,
                20.0,
                20.0,
                None,
                None,
            ),
        ),
        (
            "SwitchView",
            BindingWidget::switch_view([child(), child()], 0.0),
        ),
        (
            "TrailingSlotRow",
            BindingWidget::trailing_slot_row(child(), child(), 80.0, 24.0, 4.0),
        ),
        (
            "FloatingWorkspace",
            BindingWidget::floating_workspace(
                BindingFloatingWorkspaceState::new(),
                [BindingFloatingView::new(
                    "view",
                    Rect::new(0.0, 0.0, 180.0, 100.0),
                    Size::new(40.0, 40.0),
                    true,
                    child(),
                )],
                None,
            ),
        ),
        (
            "FloatingStack",
            BindingWidget::floating_stack(
                [BindingFloatingStackWindow::new(
                    Rect::new(0.0, 0.0, 180.0, 100.0),
                    child(),
                )],
                None,
            ),
        ),
        (
            "VirtualScrollView",
            BindingWidget::virtual_scroll_view([child()], None, None, None),
        ),
        (
            "ReorderableList",
            BindingWidget::reorderable_list("list", [child()], 4.0, 4.0, None, None),
        ),
    ];
    for (name, widget) in widgets {
        let root = BindingWidget::column([BindingWidget::label(name), widget], 4.0);
        let snapshot =
            render_binding_widget(root, None).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(snapshot.command_count > 0, "{name}");
        assert!(
            snapshot.semantics_names.iter().any(|value| value == name),
            "{name}"
        );
    }
}
