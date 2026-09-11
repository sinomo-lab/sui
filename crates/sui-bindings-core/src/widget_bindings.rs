use crate::tasks::BindingUiHandle;
use crate::widget_descriptor::{BindingWidget, BindingWidgetKind};

impl BindingWidget {
    pub(crate) fn bind_ui_handle(&self, handle: &BindingUiHandle) {
        match self.inner.as_ref() {
            BindingWidgetKind::Label { text } => text.bind_ui_handle(handle),
            BindingWidgetKind::Button { label, .. } => label.bind_ui_handle(handle),
            BindingWidgetKind::Icon { .. } => {}
            BindingWidgetKind::IconButton {
                label,
                selected,
                enabled,
                ..
            } => {
                label.bind_ui_handle(handle);
                selected.bind_ui_handle(handle);
                enabled.bind_ui_handle(handle);
            }
            BindingWidgetKind::Link {
                label,
                url,
                enabled,
                ..
            } => {
                label.bind_ui_handle(handle);
                url.bind_ui_handle(handle);
                enabled.bind_ui_handle(handle);
            }
            BindingWidgetKind::Checkbox { label, checked, .. } => {
                label.bind_ui_handle(handle);
                checked.bind_ui_handle(handle);
            }
            BindingWidgetKind::Switch { label, on, .. } => {
                label.bind_ui_handle(handle);
                on.bind_ui_handle(handle);
            }
            BindingWidgetKind::RadioButton {
                label, selected, ..
            } => {
                label.bind_ui_handle(handle);
                selected.bind_ui_handle(handle);
            }
            BindingWidgetKind::RadioGroup { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::SegmentedControl { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Breadcrumb { name, current, .. } => {
                name.bind_ui_handle(handle);
                if let Some(current) = current {
                    current.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ListView { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Table { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::TreeView { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::LayerList { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Menu {
                name, highlighted, ..
            } => {
                name.bind_ui_handle(handle);
                if let Some(highlighted) = highlighted {
                    highlighted.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ContextMenu { trigger, .. } => {
                trigger.bind_ui_handle(handle);
            }
            BindingWidgetKind::TabBar { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Tabs { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Dialog {
                title,
                content,
                shown,
            } => {
                title.bind_ui_handle(handle);
                content.bind_ui_handle(handle);
                shown.bind_ui_handle(handle);
            }
            BindingWidgetKind::SignalMeter { name, active, .. } => {
                name.bind_ui_handle(handle);
                active.bind_ui_handle(handle);
            }
            BindingWidgetKind::StatusBadge { label, .. } => label.bind_ui_handle(handle),
            BindingWidgetKind::StatusBar {
                segments,
                description,
                ..
            } => {
                for segment in segments {
                    segment.bind_ui_handle(handle);
                }
                if let Some(description) = description {
                    description.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::DetailRow { label, value, .. } => {
                label.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::Slider { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::NumberInput { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::Select { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ProgressBar { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::BusyIndicator { name, label, .. } => {
                name.bind_ui_handle(handle);
                if let Some(label) = label {
                    label.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::TextInput { name, value, .. }
            | BindingWidgetKind::PasswordInput { name, value, .. }
            | BindingWidgetKind::DateTimeInput { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::TextArea { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::RichText { .. } | BindingWidgetKind::RichDocument { .. } => {}
            BindingWidgetKind::Image { .. } => {}
            BindingWidgetKind::ColorSwatch { .. } => {}
            BindingWidgetKind::ColorPalette { selected, .. } => {
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ColorPicker { .. } | BindingWidgetKind::SimpleColorPicker { .. } => {
            }
            BindingWidgetKind::Separator { .. } => {}
            BindingWidgetKind::EmptyState { action, .. } => {
                if let Some(action) = action {
                    action.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ActionCard { enabled, .. } => enabled.bind_ui_handle(handle),
            BindingWidgetKind::BrushPreview { .. }
            | BindingWidgetKind::CoverageDots { .. }
            | BindingWidgetKind::SectionLabel { .. } => {}
            BindingWidgetKind::CommandGroup { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::SwitchView { children, selected } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
                selected.bind_ui_handle(handle);
            }
            BindingWidgetKind::Dock {
                body, top, bottom, ..
            } => {
                body.bind_ui_handle(handle);
                if let Some((_, top)) = top {
                    top.bind_ui_handle(handle);
                }
                if let Some((_, bottom)) = bottom {
                    bottom.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::FixedPaneSplit {
                first,
                divider,
                second,
                ..
            } => {
                first.bind_ui_handle(handle);
                divider.bind_ui_handle(handle);
                second.bind_ui_handle(handle);
            }
            BindingWidgetKind::FramedField {
                child,
                focused,
                invalid,
                ..
            } => {
                child.bind_ui_handle(handle);
                focused.bind_ui_handle(handle);
                invalid.bind_ui_handle(handle);
            }
            BindingWidgetKind::MeasuredBottomDock { body, bottom, .. } => {
                body.bind_ui_handle(handle);
                bottom.bind_ui_handle(handle);
            }
            BindingWidgetKind::PlacementBadge { label, .. } => label.bind_ui_handle(handle),
            BindingWidgetKind::PropertyRow { control, .. } => control.bind_ui_handle(handle),
            BindingWidgetKind::SideSheet {
                body,
                shown,
                header_action,
                actions,
                ..
            } => {
                body.bind_ui_handle(handle);
                shown.bind_ui_handle(handle);
                if let Some(header_action) = header_action {
                    header_action.bind_ui_handle(handle);
                }
                for action in actions {
                    action.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::SplitView {
                first,
                second,
                ratio,
                ..
            } => {
                first.bind_ui_handle(handle);
                second.bind_ui_handle(handle);
                ratio.bind_ui_handle(handle);
            }
            BindingWidgetKind::TrailingSlotRow { body, trailing, .. } => {
                body.bind_ui_handle(handle);
                trailing.bind_ui_handle(handle);
            }
            BindingWidgetKind::VirtualScrollView { children, .. }
            | BindingWidgetKind::ReorderableList { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::FloatingStack { windows, .. } => {
                for window in windows {
                    window.child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Surface { child, .. } => child.bind_ui_handle(handle),
            BindingWidgetKind::ExternalSurface { .. } => {}
            BindingWidgetKind::Toolbar { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Grid { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::AdaptiveView {
                compact,
                medium,
                expanded,
                ..
            } => {
                compact.bind_ui_handle(handle);
                medium.bind_ui_handle(handle);
                expanded.bind_ui_handle(handle);
            }
            BindingWidgetKind::ConstraintView { cases, fallback } => {
                for case in cases {
                    case.child.bind_ui_handle(handle);
                }
                fallback.bind_ui_handle(handle);
            }
            BindingWidgetKind::ResponsiveSidebar {
                sidebar, content, ..
            } => {
                sidebar.bind_ui_handle(handle);
                content.bind_ui_handle(handle);
            }
            BindingWidgetKind::MasterDetail { master, detail, .. } => {
                master.bind_ui_handle(handle);
                detail.bind_ui_handle(handle);
            }
            BindingWidgetKind::OverlayHost { child } => child.bind_ui_handle(handle),
            BindingWidgetKind::NotificationHost { .. } => {}
            BindingWidgetKind::CommandPalette { content, shown, .. } => {
                content.bind_ui_handle(handle);
                shown.bind_ui_handle(handle);
            }
            BindingWidgetKind::VirtualList { .. } => {}
            BindingWidgetKind::Canvas { .. } | BindingWidgetKind::CanvasRuler { .. } => {}
            BindingWidgetKind::DragDropHost { child, .. }
            | BindingWidgetKind::Draggable { child, .. }
            | BindingWidgetKind::DropTarget { child, .. } => child.bind_ui_handle(handle),
            BindingWidgetKind::FloatingWorkspace { views, .. } => {
                for view in views {
                    view.child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::PixelCanvas { .. } => {}
            BindingWidgetKind::Padding { child, .. }
            | BindingWidgetKind::Align { child, .. }
            | BindingWidgetKind::Background { child, .. }
            | BindingWidgetKind::AspectRatio { child, .. }
            | BindingWidgetKind::SafeArea { child, .. }
            | BindingWidgetKind::LayoutTransition { child, .. }
            | BindingWidgetKind::DockPanel { child, .. }
            | BindingWidgetKind::Tooltip { child, .. } => {
                child.bind_ui_handle(handle);
            }
            BindingWidgetKind::DockWorkspace { panels, .. } => {
                for panel in panels {
                    panel.child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::SizedBox { child, .. } => {
                if let Some(child) = child {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Stack { children, .. }
            | BindingWidgetKind::FieldGroup { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::SemanticRegion {
                name,
                child,
                description,
                ..
            } => {
                name.bind_ui_handle(handle);
                child.bind_ui_handle(handle);
                if let Some(description) = description {
                    description.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::FormRow { control, .. } => control.bind_ui_handle(handle),
            BindingWidgetKind::FormSection {
                child,
                header_action,
                ..
            }
            | BindingWidgetKind::PanelSection {
                child,
                header_action,
                ..
            } => {
                child.bind_ui_handle(handle);
                if let Some(header_action) = header_action {
                    header_action.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::StatusBarHost {
                content,
                status_bar,
            } => {
                content.bind_ui_handle(handle);
                status_bar.bind_ui_handle(handle);
            }
            BindingWidgetKind::Popover {
                trigger, content, ..
            } => {
                trigger.bind_ui_handle(handle);
                content.bind_ui_handle(handle);
            }
            BindingWidgetKind::ToolPalette { selected, .. }
            | BindingWidgetKind::PresetStrip { selected, .. }
            | BindingWidgetKind::BrowserTabBar { selected, .. } => {
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ScrollView { child, .. } => child.bind_ui_handle(handle),
            BindingWidgetKind::Flex { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Foreign { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
        }
    }
}
