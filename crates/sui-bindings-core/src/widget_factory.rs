use crate::actions::{
    BindingAction, BindingBoolAction, BindingColorAction, BindingColorSelectAction,
    BindingIdAction, BindingNumberAction, BindingReorderAction, BindingSelectAction,
    BindingStringAction, BindingStringsAction,
};
use crate::collections::{BindingNotificationCenter, BindingVirtualListModel};
use crate::docking::{
    BindingDockPanel, BindingDockState, BindingFloatingView, BindingFloatingWorkspaceState,
};
use crate::documents::BindingRichDocument;
use crate::drag::{BindingDragScope, binding_drop_effect};
use crate::foreign_widget::ForeignWidgetCallbacks;
use crate::graphics::{
    BindingBrushPreviewSpec, BindingCanvasShape, BindingCanvasStroke, BindingCanvasViewport,
    BindingFloatingStackWindow, BindingImageFit, BindingPixelCanvasState, BindingScrollAxes,
};
use crate::handles::BindingImageHandle;
use crate::interop::{
    ExternalTextureDescriptor, ExternalTextureValidationError, validate_external_size,
};
use crate::layout::{
    BindingConstraintCase, BindingMasterDetailState, BindingResponsiveSidebarState,
};
use crate::state::BindingState;
use crate::values::{
    BindingBool, BindingColorPaletteSwatch, BindingLayerListItem, BindingMenuItem, BindingNumber,
    BindingSegmentedControlItem, BindingStatusBarSegment, BindingTableColumn, BindingTableRow,
    BindingText, BindingTextSpan, BindingToolPaletteItem, BindingTreeItem,
};
use crate::widget_descriptor::{BindingWidget, BindingWidgetKind};
use std::sync::Arc;
use sui::Alignment;
use sui::AspectRatioFit;
use sui::Axis;
use sui::CanvasRulerAxis;
use sui::Color;
use sui::Easing;
use sui::FloatingViewConfig;
use sui::IconGlyph;
use sui::Insets;
use sui::SafeAreaEdges;
use sui::SafeAreaInsets;
use sui::SemanticTone;
use sui::SemanticsRole;
use sui::SideSheetPlacement;
use sui::SimpleColorPickerMode;
use sui::Size;
use sui::SurfaceBorder;
use sui::SurfaceElevation;
use sui::SurfaceRole;
use sui::TooltipPlacement;

impl BindingWidget {
    pub fn label(text: impl Into<BindingText>) -> Self {
        Self::from_kind(BindingWidgetKind::Label { text: text.into() })
    }

    pub fn label_state(state: BindingState) -> Self {
        Self::label(BindingText::State(state))
    }

    pub fn button(label: impl Into<BindingText>, action: Option<BindingAction>) -> Self {
        Self::from_kind(BindingWidgetKind::Button {
            label: label.into(),
            action,
        })
    }

    pub fn icon(
        glyph: IconGlyph,
        label: Option<String>,
        size: Option<f32>,
        color: Option<Color>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Icon {
            glyph,
            label,
            size,
            color,
        })
    }

    pub fn icon_button(
        glyph: IconGlyph,
        label: impl Into<BindingText>,
        selected: impl Into<BindingBool>,
        enabled: impl Into<BindingBool>,
        size: Option<f32>,
        icon_size: Option<f32>,
        description: Option<String>,
        action: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::IconButton {
            glyph,
            label: label.into(),
            selected: selected.into(),
            enabled: enabled.into(),
            size,
            icon_size,
            description,
            action,
        })
    }

    pub fn link(
        label: impl Into<BindingText>,
        url: impl Into<BindingText>,
        semantic_name: Option<String>,
        enabled: impl Into<BindingBool>,
        action: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Link {
            label: label.into(),
            url: url.into(),
            semantic_name,
            enabled: enabled.into(),
            action,
        })
    }

    pub fn checkbox(
        label: impl Into<BindingText>,
        checked: impl Into<BindingBool>,
        action: Option<BindingBoolAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Checkbox {
            label: label.into(),
            checked: checked.into(),
            action,
        })
    }

    pub fn switch(
        label: impl Into<BindingText>,
        on: impl Into<BindingBool>,
        action: Option<BindingBoolAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Switch {
            label: label.into(),
            on: on.into(),
            action,
        })
    }

    pub fn radio_button(
        label: impl Into<BindingText>,
        selected: impl Into<BindingBool>,
        action: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::RadioButton {
            label: label.into(),
            selected: selected.into(),
            action,
        })
    }

    pub fn radio_group(
        name: impl Into<BindingText>,
        options: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::RadioGroup {
            name: name.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
            action,
        })
    }

    pub fn segmented_control(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = BindingSegmentedControlItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SegmentedControl {
            name: name.into(),
            items: items.into_iter().collect(),
            selected,
            action,
        })
    }

    pub fn breadcrumb(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = impl Into<String>>,
        current: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Breadcrumb {
            name: name.into(),
            items: items.into_iter().map(Into::into).collect(),
            current,
            action,
        })
    }

    pub fn list_view(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ListView {
            name: name.into(),
            items: items.into_iter().map(Into::into).collect(),
            selected,
            action,
        })
    }

    pub fn table(
        name: impl Into<BindingText>,
        columns: impl IntoIterator<Item = BindingTableColumn>,
        rows: impl IntoIterator<Item = BindingTableRow>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Table {
            name: name.into(),
            columns: columns.into_iter().collect(),
            rows: rows.into_iter().collect(),
            selected,
            action,
        })
    }

    pub fn tree_view(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = BindingTreeItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::TreeView {
            name: name.into(),
            items: items.into_iter().collect(),
            selected,
            action,
        })
    }

    pub fn layer_list(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = BindingLayerListItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::LayerList {
            name: name.into(),
            items: items.into_iter().collect(),
            selected,
            action,
        })
    }

    pub fn menu(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = BindingMenuItem>,
        highlighted: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Menu {
            name: name.into(),
            items: items.into_iter().collect(),
            highlighted,
            action,
        })
    }

    pub fn context_menu(
        name: impl Into<String>,
        trigger: BindingWidget,
        items: impl IntoIterator<Item = BindingMenuItem>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ContextMenu {
            name: name.into(),
            trigger,
            items: items.into_iter().collect(),
            action,
        })
    }

    pub fn tab_bar(
        name: impl Into<BindingText>,
        tabs: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::TabBar {
            name: name.into(),
            tabs: tabs.into_iter().map(Into::into).collect(),
            selected,
            action,
        })
    }

    pub fn tabs(
        name: impl Into<BindingText>,
        tabs: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Tabs {
            name: name.into(),
            tabs: tabs.into_iter().map(Into::into).collect(),
            selected,
        })
    }

    pub fn dialog(
        title: impl Into<BindingText>,
        content: BindingWidget,
        shown: impl Into<BindingBool>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Dialog {
            title: title.into(),
            content,
            shown: shown.into(),
        })
    }

    pub fn signal_meter(
        name: impl Into<BindingText>,
        active: impl Into<BindingBool>,
        description: Option<String>,
        bars: usize,
        size: Option<Size>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SignalMeter {
            name: name.into(),
            active: active.into(),
            description,
            bars,
            size,
        })
    }

    pub fn status_badge(
        label: impl Into<BindingText>,
        tone: SemanticTone,
        icon: Option<IconGlyph>,
        min_width: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::StatusBadge {
            label: label.into(),
            tone,
            icon,
            min_width,
        })
    }

    pub fn status_bar(
        segments: impl IntoIterator<Item = BindingStatusBarSegment>,
        name: Option<String>,
        description: Option<BindingText>,
        height: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::StatusBar {
            segments: segments.into_iter().collect(),
            name,
            description,
            height,
        })
    }

    pub fn detail_row(
        label: impl Into<BindingText>,
        value: impl Into<BindingText>,
        max_value_lines: Option<usize>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::DetailRow {
            label: label.into(),
            value: value.into(),
            max_value_lines,
        })
    }

    pub fn slider(
        name: impl Into<BindingText>,
        value: impl Into<BindingNumber>,
        min: f64,
        max: f64,
        step: f64,
        action: Option<BindingNumberAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Slider {
            name: name.into(),
            value: value.into(),
            min,
            max,
            step,
            action,
        })
    }

    pub fn number_input(
        name: impl Into<BindingText>,
        value: impl Into<BindingNumber>,
        min: f64,
        max: f64,
        step: f64,
        precision: usize,
        action: Option<BindingNumberAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::NumberInput {
            name: name.into(),
            value: value.into(),
            min,
            max,
            step,
            precision,
            action,
        })
    }

    pub fn progress_bar(
        name: impl Into<BindingText>,
        value: impl Into<BindingNumber>,
        min: f64,
        max: f64,
        show_value: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ProgressBar {
            name: name.into(),
            value: value.into(),
            min,
            max,
            show_value,
        })
    }

    pub fn select(
        name: impl Into<BindingText>,
        options: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        placeholder: Option<String>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Select {
            name: name.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
            placeholder,
            action,
        })
    }

    pub fn busy_indicator(
        name: impl Into<BindingText>,
        label: Option<BindingText>,
        size: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::BusyIndicator {
            name: name.into(),
            label,
            size,
        })
    }

    pub fn text_input(
        name: impl Into<BindingText>,
        value: impl Into<BindingText>,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::TextInput {
            name: name.into(),
            value: value.into(),
            placeholder,
            action,
        })
    }

    pub fn password_input(
        name: impl Into<BindingText>,
        value: impl Into<BindingText>,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::PasswordInput {
            name: name.into(),
            value: value.into(),
            placeholder,
            action,
        })
    }

    pub fn datetime_input(
        name: impl Into<BindingText>,
        value: impl Into<BindingText>,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::DateTimeInput {
            name: name.into(),
            value: value.into(),
            placeholder,
            action,
        })
    }

    pub fn text_area(
        name: impl Into<BindingText>,
        value: impl Into<BindingText>,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::TextArea {
            name: name.into(),
            value: value.into(),
            placeholder,
            action,
        })
    }

    pub fn rich_text(
        spans: impl IntoIterator<Item = BindingTextSpan>,
        semantic_name: Option<String>,
        min_width: f32,
        min_height: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::RichText {
            spans: spans.into_iter().collect(),
            semantic_name,
            min_width: min_width.max(0.0),
            min_height: min_height.max(0.0),
        })
    }

    pub fn rich_document(
        document: BindingRichDocument,
        on_link: Option<BindingStringAction>,
        on_image: Option<BindingStringAction>,
        on_attachment: Option<BindingIdAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::RichDocument {
            document,
            on_link,
            on_image,
            on_attachment,
        })
    }

    pub fn image(
        image: BindingImageHandle,
        label: Option<String>,
        fit: BindingImageFit,
        size: Option<Size>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Image {
            image,
            label,
            fit,
            size,
        })
    }

    pub fn color_swatch(
        name: impl Into<String>,
        color: Color,
        size: Option<Size>,
        read_only: bool,
        action: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ColorSwatch {
            name: name.into(),
            color,
            size,
            read_only,
            action,
        })
    }

    pub fn color_palette(
        name: impl Into<String>,
        swatches: impl IntoIterator<Item = BindingColorPaletteSwatch>,
        selected: Option<BindingNumber>,
        action: Option<BindingColorSelectAction>,
        columns: Option<usize>,
        swatch_size: Option<f32>,
        gap: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ColorPalette {
            name: name.into(),
            swatches: swatches.into_iter().collect(),
            selected,
            action,
            columns,
            swatch_size,
            gap,
        })
    }

    pub fn color_picker(
        name: impl Into<String>,
        color: Option<Color>,
        action: Option<BindingColorAction>,
        show_alpha: bool,
        compact: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ColorPicker {
            name: name.into(),
            color,
            action,
            show_alpha,
            compact,
        })
    }

    pub fn simple_color_picker(
        name: impl Into<String>,
        color: Option<Color>,
        mode: SimpleColorPickerMode,
        action: Option<BindingColorAction>,
        show_alpha: bool,
        compact: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SimpleColorPicker {
            name: name.into(),
            color,
            mode,
            action,
            show_alpha,
            compact,
        })
    }

    pub fn separator(
        axis: Axis,
        name: Option<String>,
        inset: f32,
        thickness: Option<f32>,
        length: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Separator {
            axis,
            name,
            inset,
            thickness,
            length,
        })
    }

    pub fn empty_state(
        title: impl Into<String>,
        description: impl Into<String>,
        name: Option<String>,
        detail: Option<String>,
        icon: Option<IconGlyph>,
        action: Option<BindingWidget>,
        background: Option<Color>,
        transparent: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::EmptyState {
            title: title.into(),
            description: description.into(),
            name,
            detail,
            icon,
            action,
            background,
            transparent,
        })
    }

    pub fn action_card(
        title: impl Into<String>,
        description: impl Into<String>,
        icon: Option<IconGlyph>,
        tone: SemanticTone,
        enabled: impl Into<BindingBool>,
        action: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ActionCard {
            title: title.into(),
            description: description.into(),
            icon,
            tone,
            enabled: enabled.into(),
            action,
        })
    }

    pub fn brush_preview(
        name: impl Into<String>,
        kind: impl Into<String>,
        spec: BindingBrushPreviewSpec,
        size: Option<Size>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::BrushPreview {
            name: name.into(),
            kind: kind.into(),
            spec,
            size,
        })
    }

    pub fn command_group(
        name: impl Into<String>,
        children: impl IntoIterator<Item = BindingWidget>,
        axis: Axis,
        padding: Option<Insets>,
        spacing: Option<f32>,
        corner_radius: Option<f32>,
        background: Option<Color>,
        border: Option<Color>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::CommandGroup {
            name: name.into(),
            children: children.into_iter().collect(),
            axis,
            padding,
            spacing,
            corner_radius,
            background,
            border,
        })
    }

    pub fn coverage_dots(
        name: impl Into<String>,
        current: usize,
        target: usize,
        tone: SemanticTone,
        max_dots: usize,
        show_label: bool,
        min_width: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::CoverageDots {
            name: name.into(),
            current,
            target,
            tone,
            max_dots,
            show_label,
            min_width,
        })
    }

    pub fn dock(
        body: BindingWidget,
        top: Option<(f32, BindingWidget)>,
        bottom: Option<(f32, BindingWidget)>,
        fallback_width: f32,
        fallback_body_height: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Dock {
            body,
            top,
            bottom,
            fallback_width,
            fallback_body_height,
        })
    }

    pub fn fixed_pane_split(
        axis: Axis,
        first: BindingWidget,
        divider: BindingWidget,
        second: BindingWidget,
        fixed_second: bool,
        fixed_extent: f32,
        divider_extent: f32,
        fallback_flexible_extent: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FixedPaneSplit {
            axis,
            first,
            divider,
            second,
            fixed_second,
            fixed_extent,
            divider_extent,
            fallback_flexible_extent,
        })
    }

    pub fn framed_field(
        child: BindingWidget,
        name: Option<String>,
        description: Option<String>,
        padding: Option<Insets>,
        min_height: Option<f32>,
        fill_width: bool,
        focused: impl Into<BindingBool>,
        invalid: impl Into<BindingBool>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FramedField {
            child,
            name,
            description,
            padding,
            min_height,
            fill_width,
            focused: focused.into(),
            invalid: invalid.into(),
        })
    }

    pub fn measured_bottom_dock(
        body: BindingWidget,
        bottom: BindingWidget,
        fallback_size: Size,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::MeasuredBottomDock {
            body,
            bottom,
            fallback_size,
        })
    }

    pub fn placement_badge(
        label: impl Into<BindingText>,
        icon: Option<IconGlyph>,
        tone: SemanticTone,
        current: Option<usize>,
        target: Option<usize>,
        min_width: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::PlacementBadge {
            label: label.into(),
            icon,
            tone,
            current,
            target,
            min_width,
        })
    }

    pub fn property_row(
        label: impl Into<String>,
        control: BindingWidget,
        stacked: bool,
        label_width: Option<f32>,
        control_width: Option<f32>,
        gap: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::PropertyRow {
            label: label.into(),
            control,
            stacked,
            label_width,
            control_width,
            gap,
        })
    }

    pub fn section_label(
        label: impl Into<String>,
        semantic_name: Option<String>,
        color: Option<Color>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SectionLabel {
            label: label.into(),
            semantic_name,
            color,
        })
    }

    pub fn side_sheet(
        title: impl Into<String>,
        body: BindingWidget,
        description: Option<String>,
        shown: impl Into<BindingBool>,
        modal: bool,
        dismiss_on_scrim: bool,
        placement: SideSheetPlacement,
        width: Option<f32>,
        header_action: Option<BindingWidget>,
        actions: impl IntoIterator<Item = BindingWidget>,
        on_dismiss: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SideSheet {
            title: title.into(),
            body,
            description,
            shown: shown.into(),
            modal,
            dismiss_on_scrim,
            placement,
            width,
            header_action,
            actions: actions.into_iter().collect(),
            on_dismiss,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn bottom_sheet(
        title: impl Into<String>,
        body: BindingWidget,
        description: Option<String>,
        shown: impl Into<BindingBool>,
        modal: bool,
        dismiss_on_scrim: bool,
        height: Option<f32>,
        header_action: Option<BindingWidget>,
        actions: impl IntoIterator<Item = BindingWidget>,
        on_dismiss: Option<BindingAction>,
    ) -> Self {
        Self::side_sheet(
            title,
            body,
            description,
            shown,
            modal,
            dismiss_on_scrim,
            SideSheetPlacement::Bottom,
            height,
            header_action,
            actions,
            on_dismiss,
        )
    }

    pub fn split_view(
        name: Option<String>,
        axis: Axis,
        first: BindingWidget,
        second: BindingWidget,
        ratio: impl Into<BindingNumber>,
        min_first: f32,
        min_second: f32,
        divider_thickness: Option<f32>,
        on_change: Option<BindingNumberAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SplitView {
            name,
            axis,
            first,
            second,
            ratio: ratio.into(),
            min_first,
            min_second,
            divider_thickness,
            on_change,
        })
    }

    pub fn switch_view(
        children: impl IntoIterator<Item = BindingWidget>,
        selected: impl Into<BindingNumber>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SwitchView {
            children: children.into_iter().collect(),
            selected: selected.into(),
        })
    }

    pub fn trailing_slot_row(
        body: BindingWidget,
        trailing: BindingWidget,
        trailing_width: f32,
        trailing_height: f32,
        gap: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::TrailingSlotRow {
            body,
            trailing,
            trailing_width,
            trailing_height,
            gap,
        })
    }

    pub fn virtual_scroll_view(
        children: impl IntoIterator<Item = BindingWidget>,
        name: Option<String>,
        padding: Option<Insets>,
        spacing: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::VirtualScrollView {
            children: children.into_iter().collect(),
            name,
            padding,
            spacing,
        })
    }

    pub fn floating_stack(
        windows: impl IntoIterator<Item = BindingFloatingStackWindow>,
        name: Option<String>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FloatingStack {
            windows: windows.into_iter().collect(),
            name,
        })
    }

    pub fn reorderable_list(
        name: impl Into<String>,
        children: impl IntoIterator<Item = BindingWidget>,
        spacing: f32,
        drag_threshold: f32,
        preview_label: Option<String>,
        on_reorder: Option<BindingReorderAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ReorderableList {
            name: name.into(),
            children: children.into_iter().collect(),
            spacing,
            drag_threshold,
            preview_label,
            on_reorder,
        })
    }

    pub fn surface(
        child: BindingWidget,
        role: SurfaceRole,
        name: Option<String>,
        border: Option<SurfaceBorder>,
        elevation: Option<SurfaceElevation>,
        radius: Option<f32>,
        padding: Option<f32>,
        fill_width: bool,
        fill_height: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Surface {
            child,
            role,
            name,
            border,
            elevation,
            radius,
            padding,
            fill_width,
            fill_height,
        })
    }

    pub fn external_surface(
        descriptor: ExternalTextureDescriptor,
        desired_size: Option<Size>,
        name: Option<String>,
    ) -> Result<Self, ExternalTextureValidationError> {
        descriptor.validate()?;
        let desired_size = desired_size.unwrap_or_else(|| descriptor.size());
        validate_external_size(desired_size)?;
        Ok(Self::from_kind(BindingWidgetKind::ExternalSurface {
            tier: descriptor.tier(),
            descriptor,
            desired_size,
            name,
        }))
    }

    pub fn toolbar(
        children: impl IntoIterator<Item = BindingWidget>,
        axis: Axis,
        name: Option<String>,
        extent: Option<f32>,
        padding: Option<f32>,
        spacing: Option<f32>,
        background: Option<Color>,
        divider: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Toolbar {
            children: children.into_iter().collect(),
            axis,
            name,
            extent,
            padding,
            spacing,
            background,
            divider,
        })
    }

    pub fn grid(
        columns: usize,
        children: impl IntoIterator<Item = BindingWidget>,
        name: Option<String>,
        column_gap: f32,
        row_gap: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Grid {
            columns: columns.max(1),
            children: children.into_iter().collect(),
            name,
            column_gap: column_gap.max(0.0),
            row_gap: row_gap.max(0.0),
        })
    }

    pub fn aspect_ratio(
        child: BindingWidget,
        ratio: f32,
        fit: AspectRatioFit,
        horizontal: Alignment,
        vertical: Alignment,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::AspectRatio {
            child,
            ratio,
            fit,
            horizontal,
            vertical,
        })
    }

    pub fn safe_area(child: BindingWidget, edges: SafeAreaEdges, minimum: SafeAreaInsets) -> Self {
        Self::from_kind(BindingWidgetKind::SafeArea {
            child,
            edges,
            minimum,
        })
    }

    pub fn layout_transition(child: BindingWidget, duration: f64, easing: Easing) -> Self {
        Self::from_kind(BindingWidgetKind::LayoutTransition {
            child,
            duration: duration.max(0.0),
            easing,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn adaptive_view(
        compact: BindingWidget,
        medium: BindingWidget,
        expanded: BindingWidget,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        on_class_change: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::AdaptiveView {
            compact,
            medium,
            expanded,
            medium_breakpoint,
            expanded_breakpoint,
            on_class_change,
        })
    }

    pub fn constraint_view(
        cases: impl IntoIterator<Item = BindingConstraintCase>,
        fallback: BindingWidget,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ConstraintView {
            cases: cases.into_iter().collect(),
            fallback,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn responsive_sidebar(
        state: BindingResponsiveSidebarState,
        sidebar: BindingWidget,
        content: BindingWidget,
        name: Option<String>,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        rail_width: f32,
        overlay_width: f32,
        dismiss_on_scrim: bool,
        on_mode_change: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ResponsiveSidebar {
            state,
            sidebar,
            content,
            name,
            medium_breakpoint,
            expanded_breakpoint,
            rail_width,
            overlay_width,
            dismiss_on_scrim,
            on_mode_change,
        })
    }

    pub fn master_detail(
        state: BindingMasterDetailState,
        master: BindingWidget,
        detail: BindingWidget,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        master_width: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::MasterDetail {
            state,
            master,
            detail,
            medium_breakpoint,
            expanded_breakpoint,
            master_width,
        })
    }

    pub fn overlay_host(child: BindingWidget) -> Self {
        Self::from_kind(BindingWidgetKind::OverlayHost { child })
    }

    pub fn notification_host(center: BindingNotificationCenter, width: f32) -> Self {
        Self::from_kind(BindingWidgetKind::NotificationHost {
            center,
            width: width.max(120.0),
        })
    }

    pub fn command_palette(
        name: impl Into<String>,
        content: BindingWidget,
        description: Option<String>,
        shown: impl Into<BindingBool>,
        max_width: Option<f32>,
        on_dismiss: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::CommandPalette {
            name: name.into(),
            content,
            description,
            shown: shown.into(),
            max_width,
            on_dismiss,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn virtual_list(
        name: impl Into<String>,
        model: BindingVirtualListModel,
        estimated_row_height: f32,
        spacing: f32,
        padding: Option<Insets>,
        row_padding: Option<Insets>,
        overscan_viewports: f32,
        cache_capacity: usize,
        selectable: bool,
        transparent: bool,
        stick_to_end: bool,
        overlay_scroll_bars: bool,
        on_change: Option<BindingIdAction>,
        on_near_start: Option<BindingAction>,
        on_near_end: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::VirtualList {
            name: name.into(),
            model,
            estimated_row_height,
            spacing,
            padding,
            row_padding,
            overscan_viewports,
            cache_capacity,
            selectable,
            transparent,
            stick_to_end,
            overlay_scroll_bars,
            on_change,
            on_near_start,
            on_near_end,
        })
    }

    pub fn canvas(
        name: impl Into<String>,
        viewport: BindingCanvasViewport,
        shapes: impl IntoIterator<Item = BindingCanvasShape>,
        draw_stroke: BindingCanvasStroke,
        desired_size: Size,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Canvas {
            name: name.into(),
            viewport,
            shapes: shapes.into_iter().collect(),
            draw_stroke,
            desired_size,
        })
    }

    pub fn canvas_ruler(
        axis: Axis,
        name: impl Into<String>,
        document_size: Size,
        viewport: BindingCanvasViewport,
        viewport_size: Size,
        extent: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::CanvasRuler {
            axis: match axis {
                Axis::Horizontal => CanvasRulerAxis::Horizontal,
                Axis::Vertical => CanvasRulerAxis::Vertical,
            },
            name: name.into(),
            document_size,
            viewport,
            viewport_size,
            extent,
        })
    }

    pub fn drag_drop_host(
        scope: BindingDragScope,
        child: BindingWidget,
        on_external_hover: Option<BindingStringsAction>,
        on_external_drop: Option<BindingStringAction>,
        on_external_cancel: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::DragDropHost {
            scope,
            child,
            on_external_hover,
            on_external_drop,
            on_external_cancel,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draggable(
        scope: BindingDragScope,
        child: BindingWidget,
        payload: impl Into<String>,
        effect: &str,
        preview_label: Option<String>,
        threshold: f32,
        on_start: Option<BindingStringAction>,
        on_end: Option<BindingStringAction>,
    ) -> Result<Self, String> {
        Ok(Self::from_kind(BindingWidgetKind::Draggable {
            scope,
            child,
            payload: payload.into(),
            effect: binding_drop_effect(effect)?,
            preview_label,
            threshold: threshold.max(0.0),
            on_start,
            on_end,
        }))
    }

    pub fn drop_target(
        scope: BindingDragScope,
        child: BindingWidget,
        effect: &str,
        on_drop: Option<BindingStringAction>,
        on_hover_change: Option<BindingBoolAction>,
    ) -> Result<Self, String> {
        Ok(Self::from_kind(BindingWidgetKind::DropTarget {
            scope,
            child,
            effect: binding_drop_effect(effect)?,
            on_drop,
            on_hover_change,
        }))
    }

    pub fn floating_workspace(
        state: BindingFloatingWorkspaceState,
        views: impl IntoIterator<Item = BindingFloatingView>,
        name: Option<String>,
    ) -> Self {
        let views = views
            .into_iter()
            .map(|mut view| {
                let config = FloatingViewConfig::new(view.title.clone(), view.bounds)
                    .min_size(view.min_size)
                    .visible(view.visible);
                view.id = Some(state.inner.add_view(config));
                view
            })
            .collect();
        Self::from_kind(BindingWidgetKind::FloatingWorkspace { state, views, name })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pixel_canvas(
        state: BindingPixelCanvasState,
        name: impl Into<String>,
        width: usize,
        height: usize,
        paper_color: Option<Color>,
        desired_size: Size,
        viewport: BindingCanvasViewport,
        fit_on_first_layout: bool,
        pixels: Vec<Color>,
    ) -> Result<Self, String> {
        let expected = width.max(1).saturating_mul(height.max(1));
        if !pixels.is_empty() && pixels.len() != expected {
            return Err(format!(
                "pixel canvas expected {expected} colors for {}x{}, got {}",
                width.max(1),
                height.max(1),
                pixels.len()
            ));
        }
        Ok(Self::from_kind(BindingWidgetKind::PixelCanvas {
            state,
            name: name.into(),
            width: width.max(1),
            height: height.max(1),
            paper_color,
            desired_size,
            viewport,
            fit_on_first_layout,
            pixels,
        }))
    }

    pub fn padding(
        child: BindingWidget,
        insets: Insets,
        fill_child_width: bool,
        fill_child_height: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Padding {
            child,
            insets,
            fill_child_width,
            fill_child_height,
        })
    }

    pub fn align(child: BindingWidget, horizontal: Alignment, vertical: Alignment) -> Self {
        Self::from_kind(BindingWidgetKind::Align {
            child,
            horizontal,
            vertical,
        })
    }

    pub fn background(child: BindingWidget, color: Color) -> Self {
        Self::from_kind(BindingWidgetKind::Background { child, color })
    }

    pub fn sized_box(
        child: Option<BindingWidget>,
        width: Option<f32>,
        height: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SizedBox {
            child,
            width,
            height,
        })
    }

    pub fn stack(
        children: impl IntoIterator<Item = BindingWidget>,
        axis: Axis,
        spacing: f32,
        alignment: Alignment,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Stack {
            children: children.into_iter().collect(),
            axis,
            spacing,
            alignment,
        })
    }

    pub fn semantic_region(
        name: impl Into<BindingText>,
        child: BindingWidget,
        description: Option<BindingText>,
        role: SemanticsRole,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SemanticRegion {
            name: name.into(),
            child,
            description,
            role,
        })
    }

    pub fn form_row(
        label: impl Into<String>,
        control: BindingWidget,
        stacked: bool,
        label_width: Option<f32>,
        control_width: Option<f32>,
        gap: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FormRow {
            label: label.into(),
            control,
            stacked,
            label_width,
            control_width,
            gap,
        })
    }

    pub fn field_group(
        children: impl IntoIterator<Item = BindingWidget>,
        spacing: Option<f32>,
        padding: Option<f32>,
        max_width: Option<f32>,
        fill_width: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FieldGroup {
            children: children.into_iter().collect(),
            spacing,
            padding,
            max_width,
            fill_width,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn form_section(
        title: impl Into<String>,
        child: BindingWidget,
        description: Option<String>,
        header_action: Option<BindingWidget>,
        padding: Option<f32>,
        body_gap: Option<f32>,
        header_gap: Option<f32>,
        max_width: Option<f32>,
        fill_width: bool,
        radius: Option<f32>,
        elevation: Option<SurfaceElevation>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FormSection {
            title: title.into(),
            child,
            description,
            header_action,
            padding,
            body_gap,
            header_gap,
            max_width,
            fill_width,
            radius,
            elevation,
        })
    }

    pub fn panel_section(
        title: impl Into<String>,
        child: BindingWidget,
        header_action: Option<BindingWidget>,
        gap: Option<f32>,
        action_gap: Option<f32>,
        collapsible: bool,
        expanded: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::PanelSection {
            title: title.into(),
            child,
            header_action,
            gap,
            action_gap,
            collapsible,
            expanded,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dock_panel(
        title: impl Into<String>,
        child: BindingWidget,
        name: Option<String>,
        header_height: Option<f32>,
        padding: Option<f32>,
        background: Option<Color>,
        header_background: Option<Color>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::DockPanel {
            title: title.into(),
            child,
            name,
            header_height,
            padding,
            background,
            header_background,
        })
    }

    pub fn dock_workspace(
        state: BindingDockState,
        panels: impl IntoIterator<Item = BindingDockPanel>,
        name: impl Into<String>,
    ) -> Result<Self, String> {
        let panels = panels.into_iter().collect::<Vec<_>>();
        let mut ids = std::collections::BTreeSet::new();
        for panel in &panels {
            if !ids.insert(panel.id) {
                return Err(format!(
                    "dock panel {} is registered more than once",
                    panel.id
                ));
            }
        }
        Ok(Self::from_kind(BindingWidgetKind::DockWorkspace {
            state,
            panels,
            name: name.into(),
        }))
    }

    pub fn status_bar_host(content: BindingWidget, status_bar: BindingWidget) -> Self {
        Self::from_kind(BindingWidgetKind::StatusBarHost {
            content,
            status_bar,
        })
    }

    pub fn tooltip(
        text: impl Into<String>,
        child: BindingWidget,
        placement: TooltipPlacement,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Tooltip {
            text: text.into(),
            child,
            placement,
        })
    }

    pub fn popover(
        name: impl Into<String>,
        trigger: BindingWidget,
        content: BindingWidget,
        open: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Popover {
            name: name.into(),
            trigger,
            content,
            open,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tool_palette(
        name: impl Into<String>,
        items: impl IntoIterator<Item = BindingToolPaletteItem>,
        selected: Option<BindingNumber>,
        axis: Axis,
        action: Option<BindingSelectAction>,
        extent: Option<f32>,
        padding: Option<f32>,
        spacing: Option<f32>,
        item_size: Option<f32>,
        icon_size: Option<f32>,
        background: Option<Color>,
        divider: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ToolPalette {
            name: name.into(),
            items: items.into_iter().collect(),
            selected,
            axis,
            action,
            extent,
            padding,
            spacing,
            item_size,
            icon_size,
            background,
            divider,
        })
    }

    pub fn preset_strip(
        name: impl Into<String>,
        presets: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
        item_width: Option<f32>,
        item_height: Option<f32>,
        gap: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::PresetStrip {
            name: name.into(),
            presets: presets.into_iter().map(Into::into).collect(),
            selected,
            action,
            item_width,
            item_height,
            gap,
        })
    }

    pub fn browser_tab_bar(
        name: impl Into<String>,
        tabs: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        on_change: Option<BindingSelectAction>,
        on_close: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::BrowserTabBar {
            name: name.into(),
            tabs: tabs.into_iter().map(Into::into).collect(),
            selected,
            on_change,
            on_close,
        })
    }

    pub fn scroll_view(
        child: BindingWidget,
        axes: BindingScrollAxes,
        name: Option<String>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ScrollView { child, axes, name })
    }

    pub fn column(children: impl IntoIterator<Item = BindingWidget>, gap: f32) -> Self {
        Self::flex(Axis::Vertical, children, gap)
    }

    pub fn row(children: impl IntoIterator<Item = BindingWidget>, gap: f32) -> Self {
        Self::flex(Axis::Horizontal, children, gap)
    }

    pub fn flex(axis: Axis, children: impl IntoIterator<Item = BindingWidget>, gap: f32) -> Self {
        Self::from_kind(BindingWidgetKind::Flex {
            axis,
            gap: gap.max(0.0),
            children: children.into_iter().collect(),
        })
    }

    pub fn foreign(callbacks: impl ForeignWidgetCallbacks) -> Self {
        Self::foreign_arc(Arc::new(callbacks))
    }

    pub fn foreign_arc(callbacks: Arc<dyn ForeignWidgetCallbacks>) -> Self {
        Self::foreign_arc_with_children(callbacks, [])
    }

    pub fn foreign_arc_with_children(
        callbacks: Arc<dyn ForeignWidgetCallbacks>,
        children: impl IntoIterator<Item = BindingWidget>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Foreign {
            callbacks,
            children: children.into_iter().collect(),
        })
    }

    pub(crate) fn from_kind(kind: BindingWidgetKind) -> Self {
        Self {
            inner: Arc::new(kind),
        }
    }
}
