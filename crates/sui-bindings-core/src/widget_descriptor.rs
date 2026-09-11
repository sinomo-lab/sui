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
use crate::drag::BindingDragScope;
use crate::errors::ForeignErrorSink;
use crate::foreign_widget::ForeignWidgetCallbacks;
use crate::graphics::{
    BindingBrushPreviewSpec, BindingCanvasShape, BindingCanvasStroke, BindingCanvasViewport,
    BindingFloatingStackWindow, BindingImageFit, BindingPixelCanvasState, BindingScrollAxes,
};
use crate::handles::BindingImageHandle;
use crate::interop::{ExternalTextureDescriptor, RendererInteropTier};
use crate::layout::{
    BindingConstraintCase, BindingMasterDetailState, BindingResponsiveSidebarState,
};
use crate::theme::BindingTheme;
use crate::values::{
    BindingBool, BindingColorPaletteSwatch, BindingLayerListItem, BindingMenuItem, BindingNumber,
    BindingSegmentedControlItem, BindingStatusBarSegment, BindingTableColumn, BindingTableRow,
    BindingText, BindingTextSpan, BindingToolPaletteItem, BindingTreeItem, binding_icon_glyph_name,
};
use std::fmt;
use std::sync::Arc;
use sui::Alignment;
use sui::AspectRatioFit;
use sui::Axis;
use sui::CanvasRulerAxis;
use sui::Color;
use sui::DropEffect;
use sui::Easing;
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

#[derive(Clone)]
pub struct BindingWidget {
    pub(crate) inner: Arc<BindingWidgetKind>,
}

#[derive(Clone)]
pub(crate) struct BindingBuildContext {
    pub(crate) errors: ForeignErrorSink,
    pub(crate) theme: Option<BindingTheme>,
}

impl BindingBuildContext {
    pub(crate) fn new(errors: ForeignErrorSink, theme: Option<BindingTheme>) -> Self {
        Self { errors, theme }
    }
}

impl std::ops::Deref for BindingBuildContext {
    type Target = ForeignErrorSink;

    fn deref(&self) -> &Self::Target {
        &self.errors
    }
}

impl fmt::Debug for BindingWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner.as_ref() {
            BindingWidgetKind::Label { .. } => f.debug_tuple("BindingWidget::Label").finish(),
            BindingWidgetKind::Button { .. } => f.debug_tuple("BindingWidget::Button").finish(),
            BindingWidgetKind::Icon { glyph, .. } => f
                .debug_struct("BindingWidget::Icon")
                .field("glyph", &binding_icon_glyph_name(*glyph))
                .finish(),
            BindingWidgetKind::IconButton { glyph, .. } => f
                .debug_struct("BindingWidget::IconButton")
                .field("glyph", &binding_icon_glyph_name(*glyph))
                .finish(),
            BindingWidgetKind::Link { .. } => f.debug_tuple("BindingWidget::Link").finish(),
            BindingWidgetKind::Checkbox { .. } => f.debug_tuple("BindingWidget::Checkbox").finish(),
            BindingWidgetKind::Switch { .. } => f.debug_tuple("BindingWidget::Switch").finish(),
            BindingWidgetKind::RadioButton { .. } => {
                f.debug_tuple("BindingWidget::RadioButton").finish()
            }
            BindingWidgetKind::RadioGroup { .. } => {
                f.debug_tuple("BindingWidget::RadioGroup").finish()
            }
            BindingWidgetKind::SegmentedControl { items, .. } => f
                .debug_struct("BindingWidget::SegmentedControl")
                .field("items", items)
                .finish(),
            BindingWidgetKind::Breadcrumb { items, .. } => f
                .debug_struct("BindingWidget::Breadcrumb")
                .field("items", items)
                .finish(),
            BindingWidgetKind::ListView { .. } => f.debug_tuple("BindingWidget::ListView").finish(),
            BindingWidgetKind::Table { columns, rows, .. } => f
                .debug_struct("BindingWidget::Table")
                .field("columns", columns)
                .field("rows", rows)
                .finish(),
            BindingWidgetKind::TreeView { items, .. } => f
                .debug_struct("BindingWidget::TreeView")
                .field("items", items)
                .finish(),
            BindingWidgetKind::LayerList { items, .. } => f
                .debug_struct("BindingWidget::LayerList")
                .field("items", items)
                .finish(),
            BindingWidgetKind::Menu { items, .. } => f
                .debug_struct("BindingWidget::Menu")
                .field("items", items)
                .finish(),
            BindingWidgetKind::ContextMenu { items, trigger, .. } => f
                .debug_struct("BindingWidget::ContextMenu")
                .field("items", items)
                .field("trigger", trigger)
                .finish(),
            BindingWidgetKind::TabBar { tabs, .. } => f
                .debug_struct("BindingWidget::TabBar")
                .field("tabs", tabs)
                .finish(),
            BindingWidgetKind::Tabs { tabs, .. } => f
                .debug_struct("BindingWidget::Tabs")
                .field("tabs", tabs)
                .finish(),
            BindingWidgetKind::Dialog { title, content, .. } => f
                .debug_struct("BindingWidget::Dialog")
                .field("title", title)
                .field("content", content)
                .finish(),
            BindingWidgetKind::SignalMeter { .. } => {
                f.debug_tuple("BindingWidget::SignalMeter").finish()
            }
            BindingWidgetKind::StatusBadge { tone, .. } => f
                .debug_struct("BindingWidget::StatusBadge")
                .field("tone", tone)
                .finish(),
            BindingWidgetKind::StatusBar { segments, .. } => f
                .debug_struct("BindingWidget::StatusBar")
                .field("segments", segments)
                .finish(),
            BindingWidgetKind::DetailRow { label, value, .. } => f
                .debug_struct("BindingWidget::DetailRow")
                .field("label", label)
                .field("value", value)
                .finish(),
            BindingWidgetKind::Slider { .. } => f.debug_tuple("BindingWidget::Slider").finish(),
            BindingWidgetKind::NumberInput { .. } => {
                f.debug_tuple("BindingWidget::NumberInput").finish()
            }
            BindingWidgetKind::Select { .. } => f.debug_tuple("BindingWidget::Select").finish(),
            BindingWidgetKind::ProgressBar { .. } => {
                f.debug_tuple("BindingWidget::ProgressBar").finish()
            }
            BindingWidgetKind::BusyIndicator { .. } => {
                f.debug_tuple("BindingWidget::BusyIndicator").finish()
            }
            BindingWidgetKind::TextInput { .. } => {
                f.debug_tuple("BindingWidget::TextInput").finish()
            }
            BindingWidgetKind::PasswordInput { .. } => {
                f.debug_tuple("BindingWidget::PasswordInput").finish()
            }
            BindingWidgetKind::DateTimeInput { .. } => {
                f.debug_tuple("BindingWidget::DateTimeInput").finish()
            }
            BindingWidgetKind::TextArea { .. } => f.debug_tuple("BindingWidget::TextArea").finish(),
            BindingWidgetKind::RichText { .. } => f.debug_tuple("BindingWidget::RichText").finish(),
            BindingWidgetKind::RichDocument { document, .. } => f
                .debug_struct("BindingWidget::RichDocument")
                .field("document", document)
                .finish(),
            BindingWidgetKind::Image { .. } => f.debug_tuple("BindingWidget::Image").finish(),
            BindingWidgetKind::ColorSwatch { .. } => {
                f.debug_tuple("BindingWidget::ColorSwatch").finish()
            }
            BindingWidgetKind::ColorPalette { swatches, .. } => f
                .debug_struct("BindingWidget::ColorPalette")
                .field("swatches", swatches)
                .finish(),
            BindingWidgetKind::ColorPicker { .. } => {
                f.debug_tuple("BindingWidget::ColorPicker").finish()
            }
            BindingWidgetKind::SimpleColorPicker { .. } => {
                f.debug_tuple("BindingWidget::SimpleColorPicker").finish()
            }
            BindingWidgetKind::Separator { .. } => {
                f.debug_tuple("BindingWidget::Separator").finish()
            }
            BindingWidgetKind::EmptyState { title, action, .. } => f
                .debug_struct("BindingWidget::EmptyState")
                .field("title", title)
                .field("action", action)
                .finish(),
            BindingWidgetKind::ActionCard { title, .. } => f
                .debug_struct("BindingWidget::ActionCard")
                .field("title", title)
                .finish(),
            BindingWidgetKind::BrushPreview { name, spec, .. } => f
                .debug_struct("BindingWidget::BrushPreview")
                .field("name", name)
                .field("spec", spec)
                .finish(),
            BindingWidgetKind::CommandGroup {
                name,
                axis,
                children,
                ..
            } => f
                .debug_struct("BindingWidget::CommandGroup")
                .field("name", name)
                .field("axis", axis)
                .field("children", children)
                .finish(),
            BindingWidgetKind::CoverageDots { name, .. } => f
                .debug_struct("BindingWidget::CoverageDots")
                .field("name", name)
                .finish(),
            BindingWidgetKind::Dock {
                body, top, bottom, ..
            } => f
                .debug_struct("BindingWidget::Dock")
                .field("body", body)
                .field("top", top)
                .field("bottom", bottom)
                .finish(),
            BindingWidgetKind::FixedPaneSplit {
                axis,
                first,
                divider,
                second,
                ..
            } => f
                .debug_struct("BindingWidget::FixedPaneSplit")
                .field("axis", axis)
                .field("first", first)
                .field("divider", divider)
                .field("second", second)
                .finish(),
            BindingWidgetKind::FramedField { child, name, .. } => f
                .debug_struct("BindingWidget::FramedField")
                .field("name", name)
                .field("child", child)
                .finish(),
            BindingWidgetKind::MeasuredBottomDock { body, bottom, .. } => f
                .debug_struct("BindingWidget::MeasuredBottomDock")
                .field("body", body)
                .field("bottom", bottom)
                .finish(),
            BindingWidgetKind::PlacementBadge { label, .. } => f
                .debug_struct("BindingWidget::PlacementBadge")
                .field("label", label)
                .finish(),
            BindingWidgetKind::PropertyRow { label, control, .. } => f
                .debug_struct("BindingWidget::PropertyRow")
                .field("label", label)
                .field("control", control)
                .finish(),
            BindingWidgetKind::SectionLabel { label, .. } => f
                .debug_struct("BindingWidget::SectionLabel")
                .field("label", label)
                .finish(),
            BindingWidgetKind::SideSheet { title, body, .. } => f
                .debug_struct("BindingWidget::SideSheet")
                .field("title", title)
                .field("body", body)
                .finish(),
            BindingWidgetKind::SplitView {
                name,
                axis,
                first,
                second,
                ..
            } => f
                .debug_struct("BindingWidget::SplitView")
                .field("name", name)
                .field("axis", axis)
                .field("first", first)
                .field("second", second)
                .finish(),
            BindingWidgetKind::SwitchView {
                children, selected, ..
            } => f
                .debug_struct("BindingWidget::SwitchView")
                .field("children", children)
                .field("selected", selected)
                .finish(),
            BindingWidgetKind::TrailingSlotRow { body, trailing, .. } => f
                .debug_struct("BindingWidget::TrailingSlotRow")
                .field("body", body)
                .field("trailing", trailing)
                .finish(),
            BindingWidgetKind::VirtualScrollView { children, name, .. } => f
                .debug_struct("BindingWidget::VirtualScrollView")
                .field("name", name)
                .field("children", children)
                .finish(),
            BindingWidgetKind::FloatingStack { windows, name } => f
                .debug_struct("BindingWidget::FloatingStack")
                .field("name", name)
                .field("windows", windows)
                .finish(),
            BindingWidgetKind::ReorderableList { name, children, .. } => f
                .debug_struct("BindingWidget::ReorderableList")
                .field("name", name)
                .field("children", children)
                .finish(),
            BindingWidgetKind::Surface { role, child, .. } => f
                .debug_struct("BindingWidget::Surface")
                .field("role", role)
                .field("child", child)
                .finish(),
            BindingWidgetKind::ExternalSurface { tier, .. } => f
                .debug_struct("BindingWidget::ExternalSurface")
                .field("tier", tier)
                .finish(),
            BindingWidgetKind::Toolbar { axis, children, .. } => f
                .debug_struct("BindingWidget::Toolbar")
                .field("axis", axis)
                .field("children", children)
                .finish(),
            BindingWidgetKind::Grid {
                columns, children, ..
            } => f
                .debug_struct("BindingWidget::Grid")
                .field("columns", columns)
                .field("children", children)
                .finish(),
            BindingWidgetKind::AspectRatio { child, ratio, .. } => f
                .debug_struct("BindingWidget::AspectRatio")
                .field("ratio", ratio)
                .field("child", child)
                .finish(),
            BindingWidgetKind::SafeArea { child, .. } => f
                .debug_struct("BindingWidget::SafeArea")
                .field("child", child)
                .finish(),
            BindingWidgetKind::LayoutTransition {
                child, duration, ..
            } => f
                .debug_struct("BindingWidget::LayoutTransition")
                .field("duration", duration)
                .field("child", child)
                .finish(),
            BindingWidgetKind::AdaptiveView { compact, .. } => f
                .debug_struct("BindingWidget::AdaptiveView")
                .field("compact", compact)
                .finish_non_exhaustive(),
            BindingWidgetKind::ConstraintView { cases, fallback } => f
                .debug_struct("BindingWidget::ConstraintView")
                .field("cases", cases)
                .field("fallback", fallback)
                .finish(),
            BindingWidgetKind::ResponsiveSidebar {
                sidebar, content, ..
            } => f
                .debug_struct("BindingWidget::ResponsiveSidebar")
                .field("sidebar", sidebar)
                .field("content", content)
                .finish_non_exhaustive(),
            BindingWidgetKind::MasterDetail { master, detail, .. } => f
                .debug_struct("BindingWidget::MasterDetail")
                .field("master", master)
                .field("detail", detail)
                .finish_non_exhaustive(),
            BindingWidgetKind::OverlayHost { child } => f
                .debug_struct("BindingWidget::OverlayHost")
                .field("child", child)
                .finish(),
            BindingWidgetKind::NotificationHost { center, width } => f
                .debug_struct("BindingWidget::NotificationHost")
                .field("center", center)
                .field("width", width)
                .finish(),
            BindingWidgetKind::CommandPalette { name, content, .. } => f
                .debug_struct("BindingWidget::CommandPalette")
                .field("name", name)
                .field("content", content)
                .finish_non_exhaustive(),
            BindingWidgetKind::VirtualList { name, model, .. } => f
                .debug_struct("BindingWidget::VirtualList")
                .field("name", name)
                .field("model", model)
                .finish_non_exhaustive(),
            BindingWidgetKind::Canvas { name, shapes, .. } => f
                .debug_struct("BindingWidget::Canvas")
                .field("name", name)
                .field("shapes", shapes)
                .finish_non_exhaustive(),
            BindingWidgetKind::CanvasRuler { name, axis, .. } => f
                .debug_struct("BindingWidget::CanvasRuler")
                .field("name", name)
                .field("axis", axis)
                .finish_non_exhaustive(),
            BindingWidgetKind::DragDropHost { child, .. } => f
                .debug_struct("BindingWidget::DragDropHost")
                .field("child", child)
                .finish_non_exhaustive(),
            BindingWidgetKind::Draggable { child, payload, .. } => f
                .debug_struct("BindingWidget::Draggable")
                .field("child", child)
                .field("payload", payload)
                .finish_non_exhaustive(),
            BindingWidgetKind::DropTarget { child, .. } => f
                .debug_struct("BindingWidget::DropTarget")
                .field("child", child)
                .finish_non_exhaustive(),
            BindingWidgetKind::FloatingWorkspace { name, views, .. } => f
                .debug_struct("BindingWidget::FloatingWorkspace")
                .field("name", name)
                .field("views", views)
                .finish_non_exhaustive(),
            BindingWidgetKind::PixelCanvas {
                name,
                width,
                height,
                ..
            } => f
                .debug_struct("BindingWidget::PixelCanvas")
                .field("name", name)
                .field("width", width)
                .field("height", height)
                .finish_non_exhaustive(),
            BindingWidgetKind::Padding { child, insets, .. } => f
                .debug_struct("BindingWidget::Padding")
                .field("child", child)
                .field("insets", insets)
                .finish(),
            BindingWidgetKind::Align {
                child,
                horizontal,
                vertical,
            } => f
                .debug_struct("BindingWidget::Align")
                .field("child", child)
                .field("horizontal", horizontal)
                .field("vertical", vertical)
                .finish(),
            BindingWidgetKind::Background { child, .. } => f
                .debug_struct("BindingWidget::Background")
                .field("child", child)
                .finish(),
            BindingWidgetKind::SizedBox {
                child,
                width,
                height,
            } => f
                .debug_struct("BindingWidget::SizedBox")
                .field("child", child)
                .field("width", width)
                .field("height", height)
                .finish(),
            BindingWidgetKind::Stack { axis, children, .. } => f
                .debug_struct("BindingWidget::Stack")
                .field("axis", axis)
                .field("children", children)
                .finish(),
            BindingWidgetKind::SemanticRegion { name, child, .. } => f
                .debug_struct("BindingWidget::SemanticRegion")
                .field("name", name)
                .field("child", child)
                .finish(),
            BindingWidgetKind::FormRow { label, control, .. } => f
                .debug_struct("BindingWidget::FormRow")
                .field("label", label)
                .field("control", control)
                .finish(),
            BindingWidgetKind::FieldGroup { children, .. } => f
                .debug_struct("BindingWidget::FieldGroup")
                .field("children", children)
                .finish(),
            BindingWidgetKind::FormSection { title, child, .. } => f
                .debug_struct("BindingWidget::FormSection")
                .field("title", title)
                .field("child", child)
                .finish(),
            BindingWidgetKind::PanelSection { title, child, .. } => f
                .debug_struct("BindingWidget::PanelSection")
                .field("title", title)
                .field("child", child)
                .finish(),
            BindingWidgetKind::DockPanel { title, child, .. } => f
                .debug_struct("BindingWidget::DockPanel")
                .field("title", title)
                .field("child", child)
                .finish(),
            BindingWidgetKind::DockWorkspace { name, panels, .. } => f
                .debug_struct("BindingWidget::DockWorkspace")
                .field("name", name)
                .field("panels", panels)
                .finish(),
            BindingWidgetKind::StatusBarHost {
                content,
                status_bar,
            } => f
                .debug_struct("BindingWidget::StatusBarHost")
                .field("content", content)
                .field("status_bar", status_bar)
                .finish(),
            BindingWidgetKind::Tooltip { text, child, .. } => f
                .debug_struct("BindingWidget::Tooltip")
                .field("text", text)
                .field("child", child)
                .finish(),
            BindingWidgetKind::Popover {
                name,
                trigger,
                content,
                ..
            } => f
                .debug_struct("BindingWidget::Popover")
                .field("name", name)
                .field("trigger", trigger)
                .field("content", content)
                .finish(),
            BindingWidgetKind::ToolPalette { items, .. } => f
                .debug_struct("BindingWidget::ToolPalette")
                .field("items", items)
                .finish(),
            BindingWidgetKind::PresetStrip { presets, .. } => f
                .debug_struct("BindingWidget::PresetStrip")
                .field("presets", presets)
                .finish(),
            BindingWidgetKind::BrowserTabBar { tabs, .. } => f
                .debug_struct("BindingWidget::BrowserTabBar")
                .field("tabs", tabs)
                .finish(),
            BindingWidgetKind::ScrollView { axes, child, .. } => f
                .debug_struct("BindingWidget::ScrollView")
                .field("axes", axes)
                .field("child", child)
                .finish(),
            BindingWidgetKind::Flex {
                axis,
                gap,
                children,
            } => f
                .debug_struct("BindingWidget::Flex")
                .field("axis", axis)
                .field("gap", gap)
                .field("children", children)
                .finish(),
            BindingWidgetKind::Foreign { children, .. } => f
                .debug_struct("BindingWidget::Foreign")
                .field("children", children)
                .finish(),
        }
    }
}

#[derive(Clone)]
pub(crate) enum BindingWidgetKind {
    Label {
        text: BindingText,
    },
    Button {
        label: BindingText,
        action: Option<BindingAction>,
    },
    Icon {
        glyph: IconGlyph,
        label: Option<String>,
        size: Option<f32>,
        color: Option<Color>,
    },
    IconButton {
        glyph: IconGlyph,
        label: BindingText,
        selected: BindingBool,
        enabled: BindingBool,
        size: Option<f32>,
        icon_size: Option<f32>,
        description: Option<String>,
        action: Option<BindingAction>,
    },
    Link {
        label: BindingText,
        url: BindingText,
        semantic_name: Option<String>,
        enabled: BindingBool,
        action: Option<BindingStringAction>,
    },
    Checkbox {
        label: BindingText,
        checked: BindingBool,
        action: Option<BindingBoolAction>,
    },
    Switch {
        label: BindingText,
        on: BindingBool,
        action: Option<BindingBoolAction>,
    },
    RadioButton {
        label: BindingText,
        selected: BindingBool,
        action: Option<BindingAction>,
    },
    RadioGroup {
        name: BindingText,
        options: Vec<String>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    SegmentedControl {
        name: BindingText,
        items: Vec<BindingSegmentedControlItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    Breadcrumb {
        name: BindingText,
        items: Vec<String>,
        current: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    ListView {
        name: BindingText,
        items: Vec<String>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    Table {
        name: BindingText,
        columns: Vec<BindingTableColumn>,
        rows: Vec<BindingTableRow>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    TreeView {
        name: BindingText,
        items: Vec<BindingTreeItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    LayerList {
        name: BindingText,
        items: Vec<BindingLayerListItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    Menu {
        name: BindingText,
        items: Vec<BindingMenuItem>,
        highlighted: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    ContextMenu {
        name: String,
        trigger: BindingWidget,
        items: Vec<BindingMenuItem>,
        action: Option<BindingSelectAction>,
    },
    TabBar {
        name: BindingText,
        tabs: Vec<String>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    Tabs {
        name: BindingText,
        tabs: Vec<String>,
        selected: Option<BindingNumber>,
    },
    Dialog {
        title: BindingText,
        content: BindingWidget,
        shown: BindingBool,
    },
    SignalMeter {
        name: BindingText,
        active: BindingBool,
        description: Option<String>,
        bars: usize,
        size: Option<Size>,
    },
    StatusBadge {
        label: BindingText,
        tone: SemanticTone,
        icon: Option<IconGlyph>,
        min_width: Option<f32>,
    },
    StatusBar {
        segments: Vec<BindingStatusBarSegment>,
        name: Option<String>,
        description: Option<BindingText>,
        height: Option<f32>,
    },
    DetailRow {
        label: BindingText,
        value: BindingText,
        max_value_lines: Option<usize>,
    },
    Slider {
        name: BindingText,
        value: BindingNumber,
        min: f64,
        max: f64,
        step: f64,
        action: Option<BindingNumberAction>,
    },
    NumberInput {
        name: BindingText,
        value: BindingNumber,
        min: f64,
        max: f64,
        step: f64,
        precision: usize,
        action: Option<BindingNumberAction>,
    },
    Select {
        name: BindingText,
        options: Vec<String>,
        selected: Option<BindingNumber>,
        placeholder: Option<String>,
        action: Option<BindingSelectAction>,
    },
    ProgressBar {
        name: BindingText,
        value: BindingNumber,
        min: f64,
        max: f64,
        show_value: bool,
    },
    BusyIndicator {
        name: BindingText,
        label: Option<BindingText>,
        size: f32,
    },
    TextInput {
        name: BindingText,
        value: BindingText,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    },
    PasswordInput {
        name: BindingText,
        value: BindingText,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    },
    DateTimeInput {
        name: BindingText,
        value: BindingText,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    },
    TextArea {
        name: BindingText,
        value: BindingText,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    },
    RichText {
        spans: Vec<BindingTextSpan>,
        semantic_name: Option<String>,
        min_width: f32,
        min_height: f32,
    },
    RichDocument {
        document: BindingRichDocument,
        on_link: Option<BindingStringAction>,
        on_image: Option<BindingStringAction>,
        on_attachment: Option<BindingIdAction>,
    },
    Image {
        image: BindingImageHandle,
        label: Option<String>,
        fit: BindingImageFit,
        size: Option<Size>,
    },
    ColorSwatch {
        name: String,
        color: Color,
        size: Option<Size>,
        read_only: bool,
        action: Option<BindingAction>,
    },
    ColorPalette {
        name: String,
        swatches: Vec<BindingColorPaletteSwatch>,
        selected: Option<BindingNumber>,
        action: Option<BindingColorSelectAction>,
        columns: Option<usize>,
        swatch_size: Option<f32>,
        gap: Option<f32>,
    },
    ColorPicker {
        name: String,
        color: Option<Color>,
        action: Option<BindingColorAction>,
        show_alpha: bool,
        compact: bool,
    },
    SimpleColorPicker {
        name: String,
        color: Option<Color>,
        mode: SimpleColorPickerMode,
        action: Option<BindingColorAction>,
        show_alpha: bool,
        compact: bool,
    },
    Separator {
        axis: Axis,
        name: Option<String>,
        inset: f32,
        thickness: Option<f32>,
        length: Option<f32>,
    },
    EmptyState {
        title: String,
        description: String,
        name: Option<String>,
        detail: Option<String>,
        icon: Option<IconGlyph>,
        action: Option<BindingWidget>,
        background: Option<Color>,
        transparent: bool,
    },
    ActionCard {
        title: String,
        description: String,
        icon: Option<IconGlyph>,
        tone: SemanticTone,
        enabled: BindingBool,
        action: Option<BindingAction>,
    },
    BrushPreview {
        name: String,
        kind: String,
        spec: BindingBrushPreviewSpec,
        size: Option<Size>,
    },
    CommandGroup {
        name: String,
        children: Vec<BindingWidget>,
        axis: Axis,
        padding: Option<Insets>,
        spacing: Option<f32>,
        corner_radius: Option<f32>,
        background: Option<Color>,
        border: Option<Color>,
    },
    CoverageDots {
        name: String,
        current: usize,
        target: usize,
        tone: SemanticTone,
        max_dots: usize,
        show_label: bool,
        min_width: Option<f32>,
    },
    Dock {
        body: BindingWidget,
        top: Option<(f32, BindingWidget)>,
        bottom: Option<(f32, BindingWidget)>,
        fallback_width: f32,
        fallback_body_height: f32,
    },
    FixedPaneSplit {
        axis: Axis,
        first: BindingWidget,
        divider: BindingWidget,
        second: BindingWidget,
        fixed_second: bool,
        fixed_extent: f32,
        divider_extent: f32,
        fallback_flexible_extent: f32,
    },
    FramedField {
        child: BindingWidget,
        name: Option<String>,
        description: Option<String>,
        padding: Option<Insets>,
        min_height: Option<f32>,
        fill_width: bool,
        focused: BindingBool,
        invalid: BindingBool,
    },
    MeasuredBottomDock {
        body: BindingWidget,
        bottom: BindingWidget,
        fallback_size: Size,
    },
    PlacementBadge {
        label: BindingText,
        icon: Option<IconGlyph>,
        tone: SemanticTone,
        current: Option<usize>,
        target: Option<usize>,
        min_width: Option<f32>,
    },
    PropertyRow {
        label: String,
        control: BindingWidget,
        stacked: bool,
        label_width: Option<f32>,
        control_width: Option<f32>,
        gap: Option<f32>,
    },
    SectionLabel {
        label: String,
        semantic_name: Option<String>,
        color: Option<Color>,
    },
    SideSheet {
        title: String,
        body: BindingWidget,
        description: Option<String>,
        shown: BindingBool,
        modal: bool,
        dismiss_on_scrim: bool,
        placement: SideSheetPlacement,
        width: Option<f32>,
        header_action: Option<BindingWidget>,
        actions: Vec<BindingWidget>,
        on_dismiss: Option<BindingAction>,
    },
    SplitView {
        name: Option<String>,
        axis: Axis,
        first: BindingWidget,
        second: BindingWidget,
        ratio: BindingNumber,
        min_first: f32,
        min_second: f32,
        divider_thickness: Option<f32>,
        on_change: Option<BindingNumberAction>,
    },
    SwitchView {
        children: Vec<BindingWidget>,
        selected: BindingNumber,
    },
    TrailingSlotRow {
        body: BindingWidget,
        trailing: BindingWidget,
        trailing_width: f32,
        trailing_height: f32,
        gap: f32,
    },
    VirtualScrollView {
        children: Vec<BindingWidget>,
        name: Option<String>,
        padding: Option<Insets>,
        spacing: Option<f32>,
    },
    FloatingStack {
        windows: Vec<BindingFloatingStackWindow>,
        name: Option<String>,
    },
    ReorderableList {
        name: String,
        children: Vec<BindingWidget>,
        spacing: f32,
        drag_threshold: f32,
        preview_label: Option<String>,
        on_reorder: Option<BindingReorderAction>,
    },
    Surface {
        child: BindingWidget,
        role: SurfaceRole,
        name: Option<String>,
        border: Option<SurfaceBorder>,
        elevation: Option<SurfaceElevation>,
        radius: Option<f32>,
        padding: Option<f32>,
        fill_width: bool,
        fill_height: bool,
    },
    ExternalSurface {
        descriptor: ExternalTextureDescriptor,
        desired_size: Size,
        name: Option<String>,
        tier: RendererInteropTier,
    },
    Toolbar {
        children: Vec<BindingWidget>,
        axis: Axis,
        name: Option<String>,
        extent: Option<f32>,
        padding: Option<f32>,
        spacing: Option<f32>,
        background: Option<Color>,
        divider: bool,
    },
    Grid {
        columns: usize,
        children: Vec<BindingWidget>,
        name: Option<String>,
        column_gap: f32,
        row_gap: f32,
    },
    AspectRatio {
        child: BindingWidget,
        ratio: f32,
        fit: AspectRatioFit,
        horizontal: Alignment,
        vertical: Alignment,
    },
    SafeArea {
        child: BindingWidget,
        edges: SafeAreaEdges,
        minimum: SafeAreaInsets,
    },
    LayoutTransition {
        child: BindingWidget,
        duration: f64,
        easing: Easing,
    },
    AdaptiveView {
        compact: BindingWidget,
        medium: BindingWidget,
        expanded: BindingWidget,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        on_class_change: Option<BindingStringAction>,
    },
    ConstraintView {
        cases: Vec<BindingConstraintCase>,
        fallback: BindingWidget,
    },
    ResponsiveSidebar {
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
    },
    MasterDetail {
        state: BindingMasterDetailState,
        master: BindingWidget,
        detail: BindingWidget,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        master_width: f32,
    },
    OverlayHost {
        child: BindingWidget,
    },
    NotificationHost {
        center: BindingNotificationCenter,
        width: f32,
    },
    CommandPalette {
        name: String,
        content: BindingWidget,
        description: Option<String>,
        shown: BindingBool,
        max_width: Option<f32>,
        on_dismiss: Option<BindingAction>,
    },
    VirtualList {
        name: String,
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
    },
    Canvas {
        name: String,
        viewport: BindingCanvasViewport,
        shapes: Vec<BindingCanvasShape>,
        draw_stroke: BindingCanvasStroke,
        desired_size: Size,
    },
    CanvasRuler {
        axis: CanvasRulerAxis,
        name: String,
        document_size: Size,
        viewport: BindingCanvasViewport,
        viewport_size: Size,
        extent: Option<f32>,
    },
    DragDropHost {
        scope: BindingDragScope,
        child: BindingWidget,
        on_external_hover: Option<BindingStringsAction>,
        on_external_drop: Option<BindingStringAction>,
        on_external_cancel: Option<BindingAction>,
    },
    Draggable {
        scope: BindingDragScope,
        child: BindingWidget,
        payload: String,
        effect: DropEffect,
        preview_label: Option<String>,
        threshold: f32,
        on_start: Option<BindingStringAction>,
        on_end: Option<BindingStringAction>,
    },
    DropTarget {
        scope: BindingDragScope,
        child: BindingWidget,
        effect: DropEffect,
        on_drop: Option<BindingStringAction>,
        on_hover_change: Option<BindingBoolAction>,
    },
    FloatingWorkspace {
        state: BindingFloatingWorkspaceState,
        views: Vec<BindingFloatingView>,
        name: Option<String>,
    },
    PixelCanvas {
        state: BindingPixelCanvasState,
        name: String,
        width: usize,
        height: usize,
        paper_color: Option<Color>,
        desired_size: Size,
        viewport: BindingCanvasViewport,
        fit_on_first_layout: bool,
        pixels: Vec<Color>,
    },
    Padding {
        child: BindingWidget,
        insets: Insets,
        fill_child_width: bool,
        fill_child_height: bool,
    },
    Align {
        child: BindingWidget,
        horizontal: Alignment,
        vertical: Alignment,
    },
    Background {
        child: BindingWidget,
        color: Color,
    },
    SizedBox {
        child: Option<BindingWidget>,
        width: Option<f32>,
        height: Option<f32>,
    },
    Stack {
        children: Vec<BindingWidget>,
        axis: Axis,
        spacing: f32,
        alignment: Alignment,
    },
    SemanticRegion {
        name: BindingText,
        child: BindingWidget,
        description: Option<BindingText>,
        role: SemanticsRole,
    },
    FormRow {
        label: String,
        control: BindingWidget,
        stacked: bool,
        label_width: Option<f32>,
        control_width: Option<f32>,
        gap: Option<f32>,
    },
    FieldGroup {
        children: Vec<BindingWidget>,
        spacing: Option<f32>,
        padding: Option<f32>,
        max_width: Option<f32>,
        fill_width: bool,
    },
    FormSection {
        title: String,
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
    },
    PanelSection {
        title: String,
        child: BindingWidget,
        header_action: Option<BindingWidget>,
        gap: Option<f32>,
        action_gap: Option<f32>,
        collapsible: bool,
        expanded: bool,
    },
    DockPanel {
        title: String,
        child: BindingWidget,
        name: Option<String>,
        header_height: Option<f32>,
        padding: Option<f32>,
        background: Option<Color>,
        header_background: Option<Color>,
    },
    DockWorkspace {
        state: BindingDockState,
        panels: Vec<BindingDockPanel>,
        name: String,
    },
    StatusBarHost {
        content: BindingWidget,
        status_bar: BindingWidget,
    },
    Tooltip {
        text: String,
        child: BindingWidget,
        placement: TooltipPlacement,
    },
    Popover {
        name: String,
        trigger: BindingWidget,
        content: BindingWidget,
        open: bool,
    },
    ToolPalette {
        name: String,
        items: Vec<BindingToolPaletteItem>,
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
    },
    PresetStrip {
        name: String,
        presets: Vec<String>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
        item_width: Option<f32>,
        item_height: Option<f32>,
        gap: Option<f32>,
    },
    BrowserTabBar {
        name: String,
        tabs: Vec<String>,
        selected: Option<BindingNumber>,
        on_change: Option<BindingSelectAction>,
        on_close: Option<BindingSelectAction>,
    },
    ScrollView {
        child: BindingWidget,
        axes: BindingScrollAxes,
        name: Option<String>,
    },
    Flex {
        axis: Axis,
        gap: f32,
        children: Vec<BindingWidget>,
    },
    Foreign {
        callbacks: Arc<dyn ForeignWidgetCallbacks>,
        children: Vec<BindingWidget>,
    },
}
