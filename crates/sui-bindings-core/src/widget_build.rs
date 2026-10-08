use crate::drag::binding_drag_payload_text;
use crate::errors::{
    ForeignCallbackError, ForeignCallbackPhase, ForeignWidgetId, enabled_widget, named_widget,
    text_field_options, themed_widget,
};
use crate::foreign_widget::ForeignWidget;
use crate::graphics::BindingScrollAxes;
use crate::scrolling::BindingScrollController;
use crate::support::recover_lock;
use crate::values::BindingListItem;
use crate::values::{
    BindingBool, BindingColorPaletteSwatch, BindingLayerListItem, BindingMenuItem, BindingNumber,
    BindingTableColumn, BindingTableRow, BindingText, BindingTextSpan, BindingToolPaletteItem,
    BindingTreeItem, binding_number_to_index,
};
use crate::widget_adapters::BindingScrollControlledWidget;
use crate::widget_adapters::{
    BindingBusyIndicatorWidget, BindingCheckboxWidget, BindingCommandPaletteWidget,
    BindingDateTimeInputWidget, BindingExternalSurfaceWidget, BindingPasswordInputWidget,
    BindingProgressBarWidget, BindingRadioButtonWidget, BindingRuntimeWidget,
    BindingSideSheetWidget, BindingSplitViewWidget, BindingSwitchWidget, BindingTextAreaWidget,
    BindingTextInputWidget,
};
use crate::widget_descriptor::{BindingBuildContext, BindingWidget, BindingWidgetKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use sui::ActionCard;
use sui::AdaptiveBreakpoints;
use sui::AdaptiveClass;
use sui::AdaptiveView;
use sui::Align;
use sui::AspectRatio;
use sui::Background;
use sui::Breadcrumb;
use sui::BreadcrumbItem;
use sui::BrowserTabBar;
use sui::BrushPreview;
use sui::Button;
use sui::Canvas;
use sui::CanvasRuler;
use sui::Checkbox;
use sui::ColorPalette;
use sui::ColorPicker;
use sui::ColorSwatch;
use sui::CommandGroup;
use sui::CommandPalette;
use sui::ConstraintView;
use sui::ContextMenu;
use sui::CoverageDots;
use sui::DateTimeInput;
use sui::DefaultTheme;
use sui::DetailRow;
use sui::Dialog;
use sui::Dock;
use sui::DockPanel;
use sui::DockPanelId;
use sui::DockWorkspace;
use sui::DragDropHost;
use sui::DragPayload;
use sui::Draggable;
use sui::DropTarget;
use sui::EmptyState;
use sui::FieldGroup;
use sui::FixedPaneSplit;
use sui::Flex;
use sui::FlexWrap;
use sui::FloatingStack;
use sui::FloatingWorkspace;
use sui::FormRow;
use sui::FormSection;
use sui::FramedField;
use sui::Grid;
use sui::Icon;
use sui::IconButton;
use sui::Image;
use sui::Insets;
use sui::Label;
use sui::LayerList;
use sui::LayoutTransition;
use sui::Link;
use sui::ListView;
use sui::MasterDetail;
use sui::MeasuredBottomDock;
use sui::Menu;
use sui::NotificationHost;
use sui::NumberInput;
use sui::OverlayHost;
use sui::PanelSection;
use sui::PasswordInput;
use sui::PixelCanvas;
use sui::PlacementBadge;
use sui::Popover;
use sui::Presence;
use sui::PresetStrip;
use sui::PropertyRow;
use sui::RadioButton;
use sui::RadioGroup;
use sui::RebuildOnChange;
use sui::ReorderableList;
use sui::ResponsiveSidebar;
use sui::ResponsiveSidebarMode;
use sui::RichDocumentView;
use sui::RichText;
use sui::SafeArea;
use sui::ScrollState;
use sui::ScrollView;
use sui::SectionLabel;
use sui::SegmentedControl;
use sui::Select;
use sui::SelectionScope;
use sui::SemanticRegion;
use sui::Separator;
use sui::SideSheet;
use sui::SideSheetPlacement;
use sui::SignalMeter;
use sui::SimpleColorPicker;
use sui::SizedBox;
use sui::Slider;
use sui::SplitState;
use sui::SplitView;
use sui::Stack;
use sui::StatusBadge;
use sui::StatusBar;
use sui::StatusBarHost;
use sui::Surface;
use sui::SurfaceRole;
use sui::Switch;
use sui::SwitchView;
use sui::TabBar;
use sui::Table;
use sui::Tabs;
use sui::TextArea;
use sui::TextInput;
use sui::ToolPalette;
use sui::Toolbar;
use sui::Tooltip;
use sui::TrailingSlotRow;
use sui::TreeView;
use sui::VirtualList;
use sui::VirtualListChrome;
use sui::VirtualListSelectionMode;
use sui::VirtualScrollView;
use sui::WidgetPod;
use sui::containers::Padding as PaddingWidget;

/// Attach a scroll controller to a built scroll view, if there is one.
fn scroll_controlled(
    view: BindingRuntimeWidget,
    state: ScrollState,
    controller: &Option<BindingScrollController>,
) -> BindingRuntimeWidget {
    match controller {
        Some(controller) => BindingRuntimeWidget::new(BindingScrollControlledWidget {
            inner: view,
            state,
            controller: controller.clone(),
        }),
        None => view,
    }
}

/// A reader for a state-bound label; static labels need none.
fn live_label(label: &BindingText) -> Option<impl Fn() -> String + 'static> {
    matches!(label, BindingText::State(_)).then(|| {
        let label = label.clone();
        move || label.resolve()
    })
}

impl BindingWidget {
    pub(crate) fn into_runtime_widget(&self, context: BindingBuildContext) -> BindingRuntimeWidget {
        let errors = context;
        match self.inner.as_ref() {
            BindingWidgetKind::Label { text, options } => {
                let mut label = Label::new(text.resolve()).text_when({
                    let text = text.clone();
                    move || text.resolve()
                });
                // Weight belongs to the style itself; color and sizes layer
                // over whichever style the label uses.
                let weight = options.weight;
                if let Some(theme) = errors.theme.clone() {
                    label = label.text_style_when(move || {
                        let mut style = theme.snapshot().body_text_style();
                        if let Some(weight) = weight {
                            style.weight = weight;
                        }
                        style
                    });
                } else if let Some(weight) = weight {
                    let mut style = DefaultTheme::default().body_text_style();
                    style.weight = weight;
                    label = label.text_style(style);
                }
                if let Some(color) = options.color {
                    label = label.color(color);
                }
                if let Some(font_size) = options.font_size {
                    label = label.font_size(font_size);
                }
                if let Some(line_height) = options.line_height {
                    label = label.line_height(line_height);
                }
                if options.single_line {
                    label = label.single_line(true);
                }
                if options.selectable {
                    label = label.selectable(SelectionScope::new());
                }
                let label = named_widget!(label, self.semantic_name.as_ref());
                BindingRuntimeWidget::new(label)
            }
            BindingWidgetKind::Button {
                label,
                action,
                options,
            } => {
                let mut button = Button::new(label.resolve());
                if let Some(appearance) = options.appearance {
                    button = button.appearance(appearance);
                }
                if let Some(tone) = options.tone {
                    button = button.tone(tone);
                }
                if let Some(icon) = options.icon {
                    button = button.icon(icon);
                }
                if let Some(min_width) = options.min_width {
                    button = button.min_width(min_width);
                }
                if let Some(description) = &options.description {
                    button = button.description(description.clone());
                }
                if let Some(reader) = live_label(label) {
                    button = button.label_when(reader);
                }
                if let Some(action) = action.clone() {
                    button = button.on_press({
                        let errors = errors.clone();
                        move || {
                            if let Err(error) = action.run() {
                                errors.push(ForeignCallbackError::new(
                                    ForeignWidgetId::new(0),
                                    ForeignCallbackPhase::Event,
                                    error.message,
                                ));
                            }
                        }
                    });
                }
                let button = named_widget!(button, self.semantic_name.as_ref());
                let button = enabled_widget!(button, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(button, errors))
            }
            BindingWidgetKind::Icon {
                glyph,
                label,
                size,
                color,
            } => {
                let mut icon = Icon::new(*glyph);
                if let Some(label) = label {
                    icon = icon.semantic_name(label.clone());
                }
                if let Some(size) = size {
                    icon = icon.size(*size);
                }
                if let Some(color) = color {
                    icon = icon.color(*color);
                } else if let Some(theme) = errors.theme.clone() {
                    icon = icon.color_when(move || theme.snapshot().palette.text);
                }
                BindingRuntimeWidget::new(icon)
            }
            BindingWidgetKind::IconButton {
                glyph,
                label,
                selected,
                enabled,
                size,
                icon_size,
                description,
                action,
            } => {
                let mut button = IconButton::new(*glyph, label.resolve())
                    .selected(selected.resolve())
                    .enabled(enabled.resolve());
                if let Some(size) = size {
                    button = button.size(*size);
                }
                if let Some(icon_size) = icon_size {
                    button = button.icon_size(*icon_size);
                }
                if let Some(description) = description {
                    button = button.description(description.clone());
                }
                if matches!(selected, BindingBool::State(_)) {
                    let selected = selected.clone();
                    button = button.selected_when(move || selected.resolve());
                }
                if matches!(enabled, BindingBool::State(_)) {
                    let enabled = enabled.clone();
                    button = button.enabled_when(move || enabled.resolve());
                }
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    button = button.on_press(move || {
                        if let Err(error) = action.run() {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(button, errors))
            }
            BindingWidgetKind::Link {
                label,
                url,
                semantic_name,
                enabled,
                action,
            } => {
                let mut link = Link::new(label.resolve(), url.resolve()).enabled(enabled.resolve());
                if matches!(label, BindingText::State(_)) {
                    let label = label.clone();
                    link = link.label_when(move || label.resolve());
                }
                if matches!(url, BindingText::State(_)) {
                    let url = url.clone();
                    link = link.url_when(move || url.resolve());
                }
                if matches!(enabled, BindingBool::State(_)) {
                    let enabled = enabled.clone();
                    link = link.enabled_when(move || enabled.resolve());
                }
                if let Some(semantic_name) = semantic_name {
                    link = link.semantic_name(semantic_name.clone());
                }
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    link = link.on_open(move |url| {
                        if let Err(error) = action.run(url.to_string()) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(link, errors))
            }
            BindingWidgetKind::Checkbox {
                label,
                checked,
                action,
            } => {
                let mut checkbox = Checkbox::new(label.resolve()).checked(checked.resolve());
                if let Some(reader) = live_label(label) {
                    checkbox = checkbox.label_when(reader);
                }
                let state = checked.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    checkbox = checkbox.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let checkbox = named_widget!(checkbox, self.semantic_name.as_ref());
                let checkbox = enabled_widget!(checkbox, self.enabled.as_ref());
                BindingRuntimeWidget::new(BindingCheckboxWidget {
                    inner: themed_widget!(checkbox, errors),
                    checked: checked.clone(),
                })
            }
            BindingWidgetKind::Switch { label, on, action } => {
                let mut switch = Switch::new(label.resolve()).checked(on.resolve());
                if let Some(reader) = live_label(label) {
                    switch = switch.label_when(reader);
                }
                let state = on.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    switch = switch.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let switch = named_widget!(switch, self.semantic_name.as_ref());
                let switch = enabled_widget!(switch, self.enabled.as_ref());
                BindingRuntimeWidget::new(BindingSwitchWidget {
                    inner: themed_widget!(switch, errors),
                    on: on.clone(),
                })
            }
            BindingWidgetKind::RadioButton {
                label,
                selected,
                action,
            } => {
                let mut radio = RadioButton::new(label.resolve()).checked(selected.resolve());
                if let Some(reader) = live_label(label) {
                    radio = radio.label_when(reader);
                }
                let state = selected.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    radio = radio.on_change(move |_| {
                        if let Some(state) = &state {
                            state.set(true);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run()
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let radio = named_widget!(radio, self.semantic_name.as_ref());
                let radio = enabled_widget!(radio, self.enabled.as_ref());
                BindingRuntimeWidget::new(BindingRadioButtonWidget {
                    inner: themed_widget!(radio, errors),
                    selected: selected.clone(),
                })
            }
            BindingWidgetKind::RadioGroup {
                name,
                options,
                selected,
                action,
            } => {
                let mut radio_group = RadioGroup::new(name.resolve()).options(options.clone());
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        radio_group = radio_group.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        radio_group = radio_group
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    radio_group = radio_group.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let radio_group = enabled_widget!(radio_group, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(radio_group, errors))
            }
            BindingWidgetKind::SegmentedControl {
                name,
                items,
                selected,
                action,
            } => {
                let mut control = SegmentedControl::new(name.resolve())
                    .items(items.iter().map(|item| item.into_sui()));
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        control = control.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        control = control
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    control = control.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let control = enabled_widget!(control, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(control, errors))
            }
            BindingWidgetKind::Breadcrumb {
                name,
                items,
                current,
                action,
            } => {
                let mut breadcrumb = Breadcrumb::new(name.resolve())
                    .items(items.iter().cloned().map(BreadcrumbItem::new));
                if matches!(name, BindingText::State(_)) {
                    let name = name.clone();
                    breadcrumb = breadcrumb.name_when(move || name.resolve());
                }
                if let Some(current) = current {
                    if let Some(index) = binding_number_to_index(current.resolve()) {
                        breadcrumb = breadcrumb.current(index);
                    }
                    if matches!(current, BindingNumber::State(_)) {
                        let current = current.clone();
                        breadcrumb = breadcrumb
                            .current_when(move || binding_number_to_index(current.resolve()));
                    }
                }
                let state = current.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    breadcrumb = breadcrumb.on_activate(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(breadcrumb, errors))
            }
            BindingWidgetKind::ListView {
                name,
                items,
                selected,
                action,
            } => {
                let mut list_view = ListView::new(name.resolve())
                    .items(items.iter().map(BindingListItem::into_sui));
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        list_view = list_view.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        list_view = list_view
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    list_view = list_view.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let list_view = enabled_widget!(list_view, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(list_view, errors))
            }
            BindingWidgetKind::Table {
                name,
                columns,
                rows,
                selected,
                action,
            } => {
                let row_values: Vec<String> = rows
                    .iter()
                    .map(|row| row.cells.first().cloned().unwrap_or_default())
                    .collect();
                let mut table = Table::new(name.resolve())
                    .columns(columns.iter().map(BindingTableColumn::into_sui))
                    .rows(rows.iter().map(BindingTableRow::into_sui));
                if matches!(name, BindingText::State(_)) {
                    let name = name.clone();
                    table = table.name_when(move || name.resolve());
                }
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        table = table.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        table = table
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    table = table.on_change(move |index| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        let value = row_values.get(index).cloned().unwrap_or_default();
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let table = enabled_widget!(table, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(table, errors))
            }
            BindingWidgetKind::TreeView {
                name,
                items,
                selected,
                action,
            } => {
                let mut tree_view = TreeView::new(name.resolve())
                    .items(items.iter().map(BindingTreeItem::into_sui));
                if let Some(selected) = selected
                    && let Some(index) = binding_number_to_index(selected.resolve())
                {
                    tree_view = tree_view.selected(index);
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    tree_view = tree_view.on_change(move |path, value| {
                        let index = path.first().copied().unwrap_or(0);
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let tree_view = enabled_widget!(tree_view, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(tree_view, errors))
            }
            BindingWidgetKind::LayerList {
                name,
                items,
                selected,
                action,
            } => {
                let mut layer_list = LayerList::new(name.resolve())
                    .layers(items.iter().map(BindingLayerListItem::into_sui));
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        layer_list = layer_list.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        layer_list = layer_list
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    layer_list = layer_list.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let layer_list = enabled_widget!(layer_list, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(layer_list, errors))
            }
            BindingWidgetKind::Menu {
                name,
                items,
                highlighted,
                action,
            } => {
                let mut menu =
                    Menu::new(name.resolve()).items(items.iter().map(BindingMenuItem::into_sui));
                if let Some(highlighted) = highlighted
                    && let Some(index) = binding_number_to_index(highlighted.resolve())
                {
                    menu = menu.highlighted(index);
                }
                let state = highlighted.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    menu = menu.on_activate(move |index, item| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, item.label().to_string())
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let menu = enabled_widget!(menu, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(menu, errors))
            }
            BindingWidgetKind::ContextMenu {
                name,
                trigger,
                items,
                action,
            } => {
                let mut menu =
                    ContextMenu::new(name.clone(), trigger.into_runtime_widget(errors.clone()))
                        .items(items.iter().map(BindingMenuItem::into_sui));
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    menu = menu.on_activate(move |index, item| {
                        if let Err(error) = action.run(index, item.label().to_string()) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(menu, errors))
            }
            BindingWidgetKind::TabBar {
                name,
                tabs,
                selected,
                action,
            } => {
                let mut tab_bar = TabBar::new(name.resolve()).tabs(tabs.clone());
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        tab_bar = tab_bar.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        tab_bar = tab_bar
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    tab_bar = tab_bar.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let tab_bar = enabled_widget!(tab_bar, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(tab_bar, errors))
            }
            BindingWidgetKind::Tabs {
                name,
                tabs,
                selected,
                panels,
                action,
            } => {
                let mut tab_widget = Tabs::new(name.resolve());
                for (index, label) in tabs.iter().enumerate() {
                    tab_widget = match panels.get(index) {
                        Some(panel) => {
                            tab_widget.tab(label.clone(), panel.into_runtime_widget(errors.clone()))
                        }
                        None => tab_widget.tab(label.clone(), Label::new(label.clone())),
                    };
                }
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        tab_widget = tab_widget.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        tab_widget = tab_widget
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    tab_widget = tab_widget.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let tab_widget = enabled_widget!(tab_widget, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(tab_widget, errors))
            }
            BindingWidgetKind::Dialog {
                title,
                content,
                shown,
                options,
            } => {
                let mut dialog =
                    Dialog::new(title.resolve(), content.into_runtime_widget(errors.clone()))
                        .open(shown.resolve())
                        .modal(options.modal)
                        .dismiss_on_scrim(options.dismiss_on_scrim);
                if matches!(shown, BindingBool::State(_)) {
                    let shown = shown.clone();
                    dialog = dialog.open_when(move || shown.resolve());
                }
                if let Some(description) = &options.description {
                    dialog = dialog.description(description.clone());
                }
                if let Some(max_width) = options.max_width {
                    dialog = dialog.max_width(max_width);
                }
                for action in &options.actions {
                    dialog = dialog.action(action.into_runtime_widget(errors.clone()));
                }
                let shown_state = shown.state();
                if shown_state.is_some() || options.on_dismiss.is_some() {
                    let on_dismiss = options.on_dismiss.clone();
                    let callback_errors = errors.clone();
                    dialog = dialog.on_dismiss(move || {
                        if let Some(state) = &shown_state {
                            state.set(false);
                        }
                        if let Some(action) = &on_dismiss
                            && let Err(error) = action.run()
                        {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(dialog, errors))
            }
            BindingWidgetKind::SignalMeter {
                name,
                active,
                description,
                bars,
                size,
            } => {
                let mut signal_meter = SignalMeter::new(name.resolve())
                    .active(active.resolve())
                    .bars(*bars);
                if let Some(description) = description {
                    signal_meter = signal_meter.description(description.clone());
                }
                if let Some(size) = size {
                    signal_meter = signal_meter.size(*size);
                }
                if matches!(active, BindingBool::State(_)) {
                    let active = active.clone();
                    signal_meter = signal_meter.active_when(move || active.resolve());
                }
                BindingRuntimeWidget::new(themed_widget!(signal_meter, errors))
            }
            BindingWidgetKind::StatusBadge {
                label,
                tone,
                icon,
                min_width,
            } => {
                let mut badge = if matches!(label, BindingText::State(_)) {
                    StatusBadge::new(label.resolve()).label_when({
                        let label = label.clone();
                        move || label.resolve()
                    })
                } else {
                    StatusBadge::new(label.resolve())
                }
                .tone(*tone);
                if let Some(icon) = icon {
                    badge = badge.icon(*icon);
                }
                if let Some(min_width) = min_width {
                    badge = badge.min_width(*min_width);
                }
                BindingRuntimeWidget::new(themed_widget!(badge, errors))
            }
            BindingWidgetKind::StatusBar {
                segments,
                name,
                description,
                height,
            } => {
                let mut status_bar = StatusBar::new();
                if let Some(name) = name {
                    status_bar = status_bar.name(name.clone());
                }
                if let Some(description) = description {
                    status_bar = status_bar.description(description.resolve());
                    if matches!(description, BindingText::State(_)) {
                        let description = description.clone();
                        status_bar = status_bar.description_when(move || description.resolve());
                    }
                }
                if let Some(height) = height {
                    status_bar = status_bar.height(*height);
                }
                for segment in segments {
                    status_bar = status_bar.segment(segment.into_sui());
                }
                BindingRuntimeWidget::new(themed_widget!(status_bar, errors))
            }
            BindingWidgetKind::DetailRow {
                label,
                value,
                max_value_lines,
            } => {
                let mut detail_row = DetailRow::new(label.resolve(), value.resolve());
                if matches!(label, BindingText::State(_)) {
                    let label = label.clone();
                    detail_row = detail_row.label_when(move || label.resolve());
                }
                if matches!(value, BindingText::State(_)) {
                    let value = value.clone();
                    detail_row = detail_row.value_when(move || value.resolve());
                }
                if let Some(max_value_lines) = max_value_lines {
                    detail_row = detail_row.max_value_lines(*max_value_lines);
                }
                BindingRuntimeWidget::new(themed_widget!(detail_row, errors))
            }
            BindingWidgetKind::Slider {
                name,
                value,
                min,
                max,
                step,
                action,
            } => {
                let mut slider = Slider::new(name.resolve())
                    .range(*min, *max)
                    .step(*step)
                    .value(value.resolve());
                if matches!(value, BindingNumber::State(_)) {
                    let value = value.clone();
                    slider = slider.value_when(move || value.resolve());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    slider = slider.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let slider = enabled_widget!(slider, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(slider, errors))
            }
            BindingWidgetKind::NumberInput {
                name,
                value,
                min,
                max,
                step,
                precision,
                action,
            } => {
                let mut number_input = NumberInput::new(name.resolve())
                    .range(*min, *max)
                    .step(*step)
                    .precision(*precision)
                    .value(value.resolve());
                if matches!(value, BindingNumber::State(_)) {
                    let value = value.clone();
                    number_input = number_input.value_when(move || value.resolve());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    number_input = number_input.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let number_input = enabled_widget!(number_input, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(number_input, errors))
            }
            BindingWidgetKind::Select {
                name,
                options,
                selected,
                placeholder,
                action,
            } => {
                let mut select = Select::new(name.resolve()).options(options.clone());
                if let Some(placeholder) = placeholder {
                    select = select.placeholder(placeholder.clone());
                }
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        select = select.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        select = select
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    select = select.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let select = enabled_widget!(select, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(select, errors))
            }
            BindingWidgetKind::ProgressBar {
                name,
                value,
                min,
                max,
                show_value,
            } => BindingRuntimeWidget::new(BindingProgressBarWidget {
                name: name.clone(),
                value: value.clone(),
                min: *min,
                max: *max,
                show_value: *show_value,
                theme: errors.theme.clone(),
            }),
            BindingWidgetKind::BusyIndicator { name, label, size } => {
                BindingRuntimeWidget::new(BindingBusyIndicatorWidget {
                    name: name.clone(),
                    label: label.clone(),
                    size: *size,
                    theme: errors.theme.clone(),
                })
            }
            BindingWidgetKind::TextInput {
                name,
                value,
                placeholder,
                action,
                field,
            } => {
                let mut text_input = TextInput::new(name.resolve()).value(value.resolve());
                if let Some(placeholder) = placeholder {
                    text_input = text_input.placeholder(placeholder.clone());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    text_input = text_input.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value.clone());
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let text_input = text_field_options!(text_input, field, errors);
                let text_input = enabled_widget!(text_input, self.enabled.as_ref());
                BindingRuntimeWidget::new(BindingTextInputWidget {
                    inner: themed_widget!(text_input, errors),
                    value: value.clone(),
                })
            }
            BindingWidgetKind::PasswordInput {
                name,
                value,
                placeholder,
                action,
                field,
            } => {
                let mut password_input = PasswordInput::new(name.resolve()).value(value.resolve());
                if let Some(placeholder) = placeholder {
                    password_input = password_input.placeholder(placeholder.clone());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    password_input = password_input.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value.clone());
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let password_input = text_field_options!(password_input, field, errors);
                let password_input = enabled_widget!(password_input, self.enabled.as_ref());
                BindingRuntimeWidget::new(BindingPasswordInputWidget {
                    inner: themed_widget!(password_input, errors),
                    value: value.clone(),
                })
            }
            BindingWidgetKind::DateTimeInput {
                name,
                value,
                placeholder,
                action,
            } => {
                let mut datetime_input = DateTimeInput::new(name.resolve()).value(value.resolve());
                if let Some(placeholder) = placeholder {
                    datetime_input = datetime_input.placeholder(placeholder.clone());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    datetime_input = datetime_input.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value.clone());
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let datetime_input = enabled_widget!(datetime_input, self.enabled.as_ref());
                BindingRuntimeWidget::new(BindingDateTimeInputWidget {
                    inner: themed_widget!(datetime_input, errors),
                    value: value.clone(),
                })
            }
            BindingWidgetKind::TextArea {
                name,
                value,
                placeholder,
                action,
                field,
            } => {
                let mut text_area = TextArea::new(name.resolve()).value(value.resolve());
                if let Some(placeholder) = placeholder {
                    text_area = text_area.placeholder(placeholder.clone());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    text_area = text_area.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value.clone());
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let text_area = text_field_options!(text_area, field, errors);
                let text_area = enabled_widget!(text_area, self.enabled.as_ref());
                BindingRuntimeWidget::new(BindingTextAreaWidget {
                    inner: themed_widget!(text_area, errors),
                    value: value.clone(),
                })
            }
            BindingWidgetKind::RichText {
                spans,
                semantic_name,
                min_width,
                min_height,
            } => {
                let mut rich_text =
                    RichText::from_spans(spans.iter().map(BindingTextSpan::into_sui).collect());
                if let Some(semantic_name) = semantic_name {
                    rich_text = rich_text.semantic_name(semantic_name.clone());
                }
                if *min_width > 0.0 {
                    rich_text = rich_text.min_width(*min_width);
                }
                if *min_height > 0.0 {
                    rich_text = rich_text.min_height(*min_height);
                }
                BindingRuntimeWidget::new(rich_text)
            }
            BindingWidgetKind::RichDocument {
                document,
                on_link,
                on_image,
                on_attachment,
            } => {
                let mut view = RichDocumentView::new(document.inner.clone());
                if let Some(action) = on_link.clone() {
                    let errors = errors.clone();
                    view = view.on_link(move |destination| {
                        if let Err(error) = action.run(destination.to_owned()) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_image.clone() {
                    let errors = errors.clone();
                    view = view.on_image(move |source| {
                        if let Err(error) = action.run(source.to_owned()) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_attachment.clone() {
                    let errors = errors.clone();
                    view = view.on_attachment(move |id| {
                        if let Err(error) = action.run(id.get()) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(view, errors))
            }
            BindingWidgetKind::Image {
                image,
                label,
                fit,
                size,
            } => {
                let mut image = Image::new(image.into_sui()).fit((*fit).into());
                if let Some(label) = label {
                    image = image.semantic_name(label.clone());
                }
                if let Some(size) = size {
                    image = image.size(*size);
                }
                BindingRuntimeWidget::new(themed_widget!(image, errors))
            }
            BindingWidgetKind::ColorSwatch {
                name,
                color,
                size,
                read_only,
                action,
            } => {
                let mut swatch = ColorSwatch::new(name.clone(), *color);
                if let Some(size) = size {
                    swatch = swatch.size(*size);
                }
                swatch = swatch.read_only(*read_only);
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    swatch = swatch.on_press(move |_| {
                        if let Err(error) = action.run() {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(swatch, errors))
            }
            BindingWidgetKind::ColorPalette {
                name,
                swatches,
                selected,
                action,
                columns,
                swatch_size,
                gap,
            } => {
                let mut palette = ColorPalette::new(name.clone())
                    .swatches(swatches.iter().map(BindingColorPaletteSwatch::into_sui));
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        palette = palette.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        palette = palette
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                if let Some(columns) = columns {
                    palette = palette.columns(*columns);
                }
                if let Some(swatch_size) = swatch_size {
                    palette = palette.swatch_size(*swatch_size);
                }
                if let Some(gap) = gap {
                    palette = palette.gap(*gap);
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    palette = palette.on_change(move |index, name, color| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, name, color)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let palette = enabled_widget!(palette, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(palette, errors))
            }
            BindingWidgetKind::ColorPicker {
                name,
                color,
                action,
                show_alpha,
                compact,
            } => {
                let mut picker = if let Some(color) = color {
                    ColorPicker::from_color(name.clone(), *color)
                } else {
                    ColorPicker::new(name.clone())
                }
                .show_alpha(*show_alpha)
                .compact(*compact);
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    picker = picker.on_change(move |color| {
                        if let Err(error) = action.run(color) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let picker = enabled_widget!(picker, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(picker, errors))
            }
            BindingWidgetKind::SimpleColorPicker {
                name,
                color,
                mode,
                action,
                show_alpha,
                compact,
            } => {
                let mut picker = if let Some(color) = color {
                    SimpleColorPicker::from_color(name.clone(), *color)
                } else {
                    SimpleColorPicker::new(name.clone())
                }
                .mode(*mode)
                .show_alpha(*show_alpha)
                .compact(*compact);
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    picker = picker.on_change(move |color| {
                        if let Err(error) = action.run(color) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(picker, errors))
            }
            BindingWidgetKind::Separator {
                axis,
                name,
                inset,
                thickness,
                length,
            } => {
                let mut separator = Separator::new(*axis).inset(*inset);
                if let Some(name) = name {
                    separator = separator.name(name.clone());
                }
                if let Some(thickness) = thickness {
                    separator = separator.thickness(*thickness);
                }
                if let Some(length) = length {
                    separator = separator.length(*length);
                }
                BindingRuntimeWidget::new(themed_widget!(separator, errors))
            }
            BindingWidgetKind::EmptyState {
                title,
                description,
                name,
                detail,
                icon,
                action,
                background,
                transparent,
            } => {
                let mut empty_state = EmptyState::new(title.clone(), description.clone());
                if let Some(name) = name {
                    empty_state = empty_state.name(name.clone());
                }
                if let Some(detail) = detail {
                    empty_state = empty_state.detail(detail.clone());
                }
                if let Some(icon) = icon {
                    empty_state = empty_state.icon(*icon);
                }
                if *transparent {
                    empty_state = empty_state.transparent();
                } else if let Some(background) = background {
                    empty_state = empty_state.background(*background);
                }
                if let Some(action) = action {
                    empty_state = empty_state.action(action.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(themed_widget!(empty_state, errors))
            }
            BindingWidgetKind::ActionCard {
                title,
                description,
                icon,
                tone,
                enabled,
                action,
            } => {
                let mut card = ActionCard::new(title.clone(), description.clone())
                    .tone(*tone)
                    .enabled(enabled.resolve());
                if let Some(icon) = icon {
                    card = card.icon(*icon);
                }
                if matches!(enabled, BindingBool::State(_)) {
                    let enabled = enabled.clone();
                    card = card.enabled_when(move || enabled.resolve());
                }
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    card = card.on_press(move || {
                        if let Err(error) = action.run() {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(card, errors))
            }
            BindingWidgetKind::BrushPreview {
                name,
                kind,
                spec,
                size,
            } => {
                let mut preview = BrushPreview::new(name.clone())
                    .kind(kind.clone())
                    .spec(spec.into_sui());
                if let Some(size) = size {
                    preview = preview.size(*size);
                }
                BindingRuntimeWidget::new(themed_widget!(preview, errors))
            }
            BindingWidgetKind::CommandGroup {
                name,
                children,
                axis,
                padding,
                spacing,
                corner_radius,
                background,
                border,
            } => {
                let mut group = CommandGroup::new(*axis, name.clone());
                if let Some(padding) = padding {
                    group = group.padding(*padding);
                }
                if let Some(spacing) = spacing {
                    group = group.gap(*spacing);
                }
                if let Some(corner_radius) = corner_radius {
                    group = group.corner_radius(*corner_radius);
                }
                if let Some(background) = background {
                    group = group.background(*background);
                }
                if let Some(border) = border {
                    group = group.border(*border);
                }
                for child in children {
                    group = group.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(themed_widget!(group, errors))
            }
            BindingWidgetKind::CoverageDots {
                name,
                current,
                target,
                tone,
                max_dots,
                show_label,
                min_width,
            } => {
                let mut dots = CoverageDots::new(name.clone(), *current, *target)
                    .tone(*tone)
                    .max_dots(*max_dots)
                    .show_label(*show_label);
                if let Some(min_width) = min_width {
                    dots = dots.min_width(*min_width);
                }
                BindingRuntimeWidget::new(themed_widget!(dots, errors))
            }
            BindingWidgetKind::Dock {
                body,
                top,
                bottom,
                fallback_width,
                fallback_body_height,
            } => {
                let mut dock = Dock::new(body.into_runtime_widget(errors.clone()))
                    .fallback_width(*fallback_width)
                    .fallback_body_height(*fallback_body_height);
                if let Some((height, top)) = top {
                    dock = dock.top(*height, top.into_runtime_widget(errors.clone()));
                }
                if let Some((height, bottom)) = bottom {
                    dock = dock.bottom(*height, bottom.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(dock)
            }
            BindingWidgetKind::FixedPaneSplit {
                axis,
                first,
                divider,
                second,
                fixed_second,
                fixed_extent,
                divider_extent,
                fallback_flexible_extent,
            } => {
                let mut split = FixedPaneSplit::new(
                    *axis,
                    first.into_runtime_widget(errors.clone()),
                    divider.into_runtime_widget(errors.clone()),
                    second.into_runtime_widget(errors.clone()),
                );
                split = if *fixed_second {
                    split.fixed_second(*fixed_extent)
                } else {
                    split.fixed_first(*fixed_extent)
                };
                BindingRuntimeWidget::new(
                    split
                        .divider_extent(*divider_extent)
                        .fallback_flexible_extent(*fallback_flexible_extent),
                )
            }
            BindingWidgetKind::FramedField {
                child,
                name,
                description,
                padding,
                min_height,
                fill_width,
                focused,
                invalid,
            } => {
                let mut field = FramedField::new(child.into_runtime_widget(errors.clone()))
                    .focused(focused.resolve())
                    .invalid(invalid.resolve());
                if let Some(name) = name {
                    field = field.name(name.clone());
                }
                if let Some(description) = description {
                    field = field.description(description.clone());
                }
                if let Some(padding) = padding {
                    field = field.padding(*padding);
                }
                if let Some(min_height) = min_height {
                    field = field.min_height(*min_height);
                }
                field = field.fill_width(*fill_width);
                if matches!(focused, BindingBool::State(_)) {
                    let focused = focused.clone();
                    field = field.focused_when(move || focused.resolve());
                }
                if matches!(invalid, BindingBool::State(_)) {
                    let invalid = invalid.clone();
                    field = field.invalid_when(move || invalid.resolve());
                }
                BindingRuntimeWidget::new(themed_widget!(field, errors))
            }
            BindingWidgetKind::MeasuredBottomDock {
                body,
                bottom,
                fallback_size,
            } => BindingRuntimeWidget::new(
                MeasuredBottomDock::new(
                    body.into_runtime_widget(errors.clone()),
                    bottom.into_runtime_widget(errors.clone()),
                )
                .fallback_size(*fallback_size),
            ),
            BindingWidgetKind::PlacementBadge {
                label,
                icon,
                tone,
                current,
                target,
                min_width,
            } => {
                let mut badge = if matches!(label, BindingText::State(_)) {
                    PlacementBadge::new(label.resolve()).label_when({
                        let label = label.clone();
                        move || label.resolve()
                    })
                } else {
                    PlacementBadge::new(label.resolve())
                }
                .tone(*tone);
                if let Some(icon) = icon {
                    badge = badge.icon(*icon);
                }
                if let (Some(current), Some(target)) = (current, target) {
                    badge = badge.coverage(*current, *target);
                }
                if let Some(min_width) = min_width {
                    badge = badge.min_width(*min_width);
                }
                BindingRuntimeWidget::new(themed_widget!(badge, errors))
            }
            BindingWidgetKind::PropertyRow {
                label,
                control,
                stacked,
                label_width,
                control_width,
                gap,
            } => {
                let mut row =
                    PropertyRow::new(label.clone(), control.into_runtime_widget(errors.clone()));
                if !stacked {
                    row = row.inline();
                }
                if let Some(label_width) = label_width {
                    row = row.label_width(*label_width);
                }
                if let Some(control_width) = control_width {
                    row = row.control_width(*control_width);
                }
                if let Some(gap) = gap {
                    row = row.gap(*gap);
                }
                BindingRuntimeWidget::new(themed_widget!(row, errors))
            }
            BindingWidgetKind::SectionLabel {
                label,
                semantic_name,
                color,
            } => {
                let mut section = SectionLabel::new(label.clone());
                if let Some(semantic_name) = semantic_name {
                    section = section.semantic_name(semantic_name.clone());
                }
                if let Some(color) = color {
                    section = section.color(*color);
                }
                BindingRuntimeWidget::new(themed_widget!(section, errors))
            }
            BindingWidgetKind::SideSheet {
                title,
                body,
                description,
                shown,
                modal,
                dismiss_on_scrim,
                placement,
                width,
                header_action,
                actions,
                on_dismiss,
            } => {
                let title = title.clone();
                let body = body.clone();
                let description = description.clone();
                let shown_state = shown.state();
                let placement = *placement;
                let width = *width;
                let header_action = header_action.clone();
                let actions = actions.clone();
                let on_dismiss = on_dismiss.clone();
                let modal = *modal;
                let dismiss_on_scrim = *dismiss_on_scrim;
                let build_errors = errors.clone();
                let build = move |is_shown: bool| {
                    let mut sheet = SideSheet::new(
                        title.clone(),
                        body.into_runtime_widget(build_errors.clone()),
                    )
                    .open(is_shown)
                    .modal(modal)
                    .dismiss_on_scrim(dismiss_on_scrim)
                    .placement(placement);
                    if let Some(description) = &description {
                        sheet = sheet.description(description.clone());
                    }
                    if let Some(width) = width {
                        sheet = if placement == SideSheetPlacement::Bottom {
                            sheet.height(width)
                        } else {
                            sheet.width(width)
                        };
                    }
                    if let Some(header_action) = &header_action {
                        sheet = sheet
                            .header_action(header_action.into_runtime_widget(build_errors.clone()));
                    }
                    for action in &actions {
                        sheet = sheet.action(action.into_runtime_widget(build_errors.clone()));
                    }
                    if shown_state.is_some() || on_dismiss.is_some() {
                        let shown_state = shown_state.clone();
                        let on_dismiss = on_dismiss.clone();
                        let callback_errors = build_errors.clone();
                        sheet = sheet.on_dismiss(move || {
                            if let Some(state) = &shown_state {
                                state.set(false);
                            }
                            if let Some(action) = &on_dismiss
                                && let Err(error) = action.run()
                            {
                                callback_errors.push(ForeignCallbackError::new(
                                    ForeignWidgetId::new(0),
                                    ForeignCallbackPhase::Event,
                                    error.message,
                                ));
                            }
                        });
                    }
                    themed_widget!(sheet, build_errors)
                };
                if matches!(shown, BindingBool::State(_)) {
                    BindingRuntimeWidget::new(BindingSideSheetWidget::new(shown.clone(), build))
                } else {
                    BindingRuntimeWidget::new(build(shown.resolve()))
                }
            }
            BindingWidgetKind::SplitView {
                name,
                axis,
                first,
                second,
                ratio,
                min_first,
                min_second,
                divider_thickness,
                on_change,
            } => {
                let name = name.clone();
                let axis = *axis;
                let first = first.clone();
                let second = second.clone();
                let min_first = *min_first;
                let min_second = *min_second;
                let divider_thickness = *divider_thickness;
                let ratio_state = ratio.state();
                let on_change = on_change.clone();
                let build_errors = errors.clone();
                let build = move |resolved_ratio: f32| {
                    let mut split = SplitView::new(
                        axis,
                        first.into_runtime_widget(build_errors.clone()),
                        second.into_runtime_widget(build_errors.clone()),
                    )
                    .ratio(resolved_ratio)
                    .min_first(min_first)
                    .min_second(min_second);
                    if let Some(name) = &name {
                        split = split.name(name.clone());
                    }
                    if let Some(divider_thickness) = divider_thickness {
                        split = split.divider_thickness(divider_thickness);
                    }
                    if ratio_state.is_some() || on_change.is_some() {
                        let ratio_state = ratio_state.clone();
                        let on_change = on_change.clone();
                        let callback_errors = build_errors.clone();
                        split = split.on_change(move |value| {
                            if let Some(state) = &ratio_state {
                                state.set(f64::from(value));
                            }
                            if let Some(action) = &on_change
                                && let Err(error) = action.run(f64::from(value))
                            {
                                callback_errors.push(ForeignCallbackError::new(
                                    ForeignWidgetId::new(0),
                                    ForeignCallbackPhase::Event,
                                    error.message,
                                ));
                            }
                        });
                    }
                    themed_widget!(split, build_errors)
                };
                if matches!(ratio, BindingNumber::State(_)) {
                    BindingRuntimeWidget::new(BindingSplitViewWidget::new(ratio.clone(), build))
                } else {
                    BindingRuntimeWidget::new(build(ratio.resolve() as f32))
                }
            }
            BindingWidgetKind::SwitchView { children, selected } => {
                let mut view = SwitchView::new()
                    .selected(binding_number_to_index(selected.resolve()).unwrap_or(usize::MAX));
                if matches!(selected, BindingNumber::State(_)) {
                    let selected = selected.clone();
                    view = view.selected_when(move || {
                        binding_number_to_index(selected.resolve()).unwrap_or(usize::MAX)
                    });
                }
                for child in children {
                    view = view.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(view)
            }
            BindingWidgetKind::TrailingSlotRow {
                body,
                trailing,
                trailing_width,
                trailing_height,
                gap,
            } => BindingRuntimeWidget::new(
                TrailingSlotRow::new(
                    body.into_runtime_widget(errors.clone()),
                    trailing.into_runtime_widget(errors.clone()),
                )
                .trailing_width(*trailing_width)
                .trailing_height(*trailing_height)
                .gap(*gap),
            ),
            BindingWidgetKind::VirtualScrollView {
                children,
                name,
                padding,
                spacing,
                controller,
            } => {
                let state = ScrollState::new();
                let mut view = VirtualScrollView::new().state(state.clone());
                if let Some(name) = name {
                    view = view.name(name.clone());
                }
                if let Some(padding) = padding {
                    view = view.padding(*padding);
                }
                if let Some(spacing) = spacing {
                    view = view.gap(*spacing);
                }
                for child in children {
                    view = view.with_child(child.into_runtime_widget(errors.clone()));
                }
                scroll_controlled(BindingRuntimeWidget::new(view), state, controller)
            }
            BindingWidgetKind::FloatingStack { windows, name } => {
                let mut stack = FloatingStack::new();
                if let Some(name) = name {
                    stack = stack.name(name.clone());
                }
                for window in windows {
                    stack = stack.with_window(
                        window.bounds,
                        window.child.into_runtime_widget(errors.clone()),
                    );
                }
                BindingRuntimeWidget::new(stack)
            }
            BindingWidgetKind::ReorderableList {
                name,
                children,
                spacing,
                drag_threshold,
                preview_label,
                on_reorder,
            } => {
                let mut list = ReorderableList::new(name.clone())
                    .gap(*spacing)
                    .drag_threshold(*drag_threshold);
                if let Some(preview_label) = preview_label {
                    list = list.preview_label(preview_label.clone());
                }
                for child in children {
                    list = list.item(child.into_runtime_widget(errors.clone()));
                }
                if let Some(action) = on_reorder.clone() {
                    let errors = errors.clone();
                    list = list.on_reorder(move |change| {
                        if let Err(error) = action.run(change.item, change.from, change.to) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(list)
            }
            BindingWidgetKind::Surface {
                child,
                role,
                name,
                border,
                elevation,
                radius,
                padding,
                fill_width,
                fill_height,
            } => {
                let child = child.into_runtime_widget(errors.clone());
                let mut surface = match role {
                    SurfaceRole::Window => Surface::window(child),
                    SurfaceRole::Sidebar => Surface::sidebar(child),
                    SurfaceRole::Panel => Surface::panel(child),
                    SurfaceRole::Titlebar => Surface::titlebar(child),
                    SurfaceRole::Field => Surface::field(child),
                };
                if let Some(name) = name {
                    surface = surface.name(name.clone());
                }
                if let Some(border) = border {
                    surface = surface.border(*border);
                }
                if let Some(elevation) = elevation {
                    surface = surface.elevation(*elevation);
                }
                if let Some(radius) = radius {
                    surface = surface.corner_radius(*radius);
                }
                if let Some(padding) = padding {
                    surface = surface.padding(Insets::all(padding.max(0.0)));
                }
                surface = surface.fill_width(*fill_width).fill_height(*fill_height);
                BindingRuntimeWidget::new(themed_widget!(surface, errors))
            }
            BindingWidgetKind::ExternalSurface {
                descriptor,
                desired_size,
                name,
                ..
            } => BindingRuntimeWidget::new(BindingExternalSurfaceWidget {
                descriptor: descriptor.clone(),
                desired_size: *desired_size,
                name: name.clone(),
            }),
            BindingWidgetKind::Toolbar {
                children,
                axis,
                name,
                extent,
                padding,
                spacing,
                background,
                divider,
            } => {
                let mut toolbar = Toolbar::new(*axis).divider(*divider);
                if let Some(name) = name {
                    toolbar = toolbar.name(name.clone());
                }
                if let Some(extent) = extent {
                    toolbar = toolbar.extent(*extent);
                }
                if let Some(padding) = padding {
                    toolbar = toolbar.padding(Insets::all(padding.max(0.0)));
                }
                if let Some(spacing) = spacing {
                    toolbar = toolbar.gap(*spacing);
                }
                if let Some(background) = background {
                    toolbar = toolbar.background(*background);
                }
                for child in children {
                    toolbar = toolbar.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(themed_widget!(toolbar, errors))
            }
            BindingWidgetKind::Grid {
                columns,
                rows,
                children,
                name,
                column_gap,
                row_gap,
            } => {
                let mut grid = Grid::new(columns.iter().copied())
                    .column_gap(*column_gap)
                    .row_gap(*row_gap);
                if !rows.is_empty() {
                    grid = grid.rows(rows.iter().copied());
                }
                if let Some(name) = name {
                    grid = grid.name(name.clone());
                }
                for child in children {
                    match child.inner.as_ref() {
                        BindingWidgetKind::GridCell { child, cell } => {
                            grid = grid.with_cell(*cell, child.into_runtime_widget(errors.clone()));
                        }
                        _ => grid = grid.with_child(child.into_runtime_widget(errors.clone())),
                    }
                }
                BindingRuntimeWidget::new(grid)
            }
            BindingWidgetKind::GridCell { child, .. } => child.into_runtime_widget(errors),
            BindingWidgetKind::AspectRatio {
                child,
                ratio,
                fit,
                horizontal,
                vertical,
            } => BindingRuntimeWidget::new(
                AspectRatio::new(*ratio, child.into_runtime_widget(errors.clone()))
                    .fit(*fit)
                    .align(*horizontal, *vertical),
            ),
            BindingWidgetKind::SafeArea {
                child,
                edges,
                minimum,
            } => BindingRuntimeWidget::new(
                SafeArea::new(child.into_runtime_widget(errors.clone()))
                    .edges(*edges)
                    .minimum(*minimum),
            ),
            BindingWidgetKind::LayoutTransition {
                child,
                duration,
                easing,
            } => BindingRuntimeWidget::new(
                LayoutTransition::new(child.into_runtime_widget(errors.clone()))
                    .duration(*duration)
                    .easing(*easing),
            ),
            BindingWidgetKind::Presence {
                child,
                shown,
                collapse,
                appear,
            } => {
                let presence = Presence::new(child.into_runtime_widget(errors.clone()));
                let mut presence = match shown {
                    BindingBool::State(_) => {
                        let shown = shown.clone();
                        presence.shown_when(move || shown.resolve())
                    }
                    BindingBool::Static(shown) => presence.shown(*shown),
                };
                if *collapse {
                    presence = presence.collapse(sui::Axis::Vertical);
                }
                presence = presence.appear(*appear);
                BindingRuntimeWidget::new(presence)
            }
            BindingWidgetKind::AdaptiveView {
                compact,
                medium,
                expanded,
                medium_breakpoint,
                expanded_breakpoint,
                on_class_change,
            } => {
                let mut view = AdaptiveView::new(
                    compact.into_runtime_widget(errors.clone()),
                    medium.into_runtime_widget(errors.clone()),
                    expanded.into_runtime_widget(errors.clone()),
                )
                .breakpoints(AdaptiveBreakpoints::new(
                    *medium_breakpoint,
                    *expanded_breakpoint,
                ));
                if let Some(action) = on_class_change.clone() {
                    let callback_errors = errors.clone();
                    view = view.on_class_change(move |class| {
                        let value = match class {
                            AdaptiveClass::Compact => "compact",
                            AdaptiveClass::Medium => "medium",
                            AdaptiveClass::Expanded => "expanded",
                        };
                        if let Err(error) = action.run(value.to_owned()) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(view)
            }
            BindingWidgetKind::ConstraintView { cases, fallback } => {
                let mut view = ConstraintView::new(fallback.into_runtime_widget(errors.clone()));
                for case in cases {
                    view = view.when(case.query, case.child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(view)
            }
            BindingWidgetKind::ResponsiveSidebar {
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
            } => {
                let mut view = ResponsiveSidebar::new(
                    sidebar.into_runtime_widget(errors.clone()),
                    content.into_runtime_widget(errors.clone()),
                )
                .state(state.inner.clone())
                .breakpoints(AdaptiveBreakpoints::new(
                    *medium_breakpoint,
                    *expanded_breakpoint,
                ))
                .rail_width(*rail_width)
                .overlay_width(*overlay_width)
                .dismiss_on_scrim(*dismiss_on_scrim);
                if let Some(name) = name {
                    view = view.name(name.clone());
                }
                if let Some(theme) = errors.theme.clone() {
                    view = view.theme(theme.snapshot());
                }
                if let Some(action) = on_mode_change.clone() {
                    let callback_errors = errors.clone();
                    view = view.on_mode_change(move |mode| {
                        let value = match mode {
                            ResponsiveSidebarMode::OverlayClosed => "overlay-closed",
                            ResponsiveSidebarMode::OverlayOpen => "overlay-open",
                            ResponsiveSidebarMode::Rail => "rail",
                            ResponsiveSidebarMode::Inline => "inline",
                        };
                        if let Err(error) = action.run(value.to_owned()) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(view)
            }
            BindingWidgetKind::MasterDetail {
                state,
                master,
                detail,
                medium_breakpoint,
                expanded_breakpoint,
                master_width,
            } => BindingRuntimeWidget::new(
                MasterDetail::new(
                    master.into_runtime_widget(errors.clone()),
                    detail.into_runtime_widget(errors.clone()),
                )
                .state(state.inner.clone())
                .breakpoints(AdaptiveBreakpoints::new(
                    *medium_breakpoint,
                    *expanded_breakpoint,
                ))
                .split_state(SplitState::pixels(*master_width)),
            ),
            BindingWidgetKind::OverlayHost { child } => BindingRuntimeWidget::new(
                OverlayHost::new(child.into_runtime_widget(errors.clone())),
            ),
            BindingWidgetKind::NotificationHost { center, width } => {
                let mut host = NotificationHost::new(center.inner.clone()).width(*width);
                if let Some(theme) = errors.theme.clone() {
                    host = host.theme(theme.snapshot());
                }
                BindingRuntimeWidget::new(host)
            }
            BindingWidgetKind::CommandPalette {
                name,
                content,
                description,
                shown,
                max_width,
                on_dismiss,
            } => {
                let name = name.clone();
                let content = content.clone();
                let description = description.clone();
                let max_width = *max_width;
                let shown_state = shown.state();
                let on_dismiss = on_dismiss.clone();
                let build_errors = errors.clone();
                let build = move |is_shown: bool| {
                    let mut palette = CommandPalette::new(
                        name.clone(),
                        content.into_runtime_widget(build_errors.clone()),
                    )
                    .open(is_shown);
                    if let Some(description) = &description {
                        palette = palette.description(description.clone());
                    }
                    if let Some(max_width) = max_width {
                        palette = palette.max_width(max_width);
                    }
                    if let Some(theme) = build_errors.theme.clone() {
                        palette = palette.theme(theme.snapshot());
                    }
                    if shown_state.is_some() || on_dismiss.is_some() {
                        let shown_state = shown_state.clone();
                        let action = on_dismiss.clone();
                        let callback_errors = build_errors.clone();
                        palette = palette.on_dismiss(move || {
                            if let Some(state) = &shown_state {
                                state.set(false);
                            }
                            if let Some(action) = &action
                                && let Err(error) = action.run()
                            {
                                callback_errors.push(ForeignCallbackError::new(
                                    ForeignWidgetId::new(0),
                                    ForeignCallbackPhase::Event,
                                    error.message,
                                ));
                            }
                        });
                    }
                    palette
                };
                if matches!(shown, BindingBool::State(_)) {
                    BindingRuntimeWidget::new(BindingCommandPaletteWidget::new(
                        shown.clone(),
                        build,
                    ))
                } else {
                    BindingRuntimeWidget::new(build(shown.resolve()))
                }
            }
            BindingWidgetKind::VirtualList {
                name,
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
            } => {
                let mut list =
                    VirtualList::new(name.clone(), model.inner.clone(), |_key, value| {
                        Label::new("").text_from(value)
                    })
                    .estimated_row_height(*estimated_row_height)
                    .gap(*spacing)
                    .overscan_viewports(*overscan_viewports)
                    .cache_capacity(*cache_capacity)
                    .selection_mode(if *selectable {
                        VirtualListSelectionMode::Single
                    } else {
                        VirtualListSelectionMode::None
                    })
                    .chrome(if *transparent {
                        VirtualListChrome::Transparent
                    } else {
                        VirtualListChrome::Default
                    })
                    .stick_to_end(*stick_to_end)
                    .overlay_scroll_bars(*overlay_scroll_bars)
                    .row_name(|_, value| value.clone());
                if let Some(padding) = padding {
                    list = list.padding(*padding);
                }
                if let Some(row_padding) = row_padding {
                    list = list.row_padding(*row_padding);
                }
                if let Some(action) = on_change.clone() {
                    let callback_errors = errors.clone();
                    list = list.on_change(move |key| {
                        if let Err(error) = action.run(key) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_near_start.clone() {
                    let callback_errors = errors.clone();
                    list = list.on_near_start(move || {
                        if let Err(error) = action.run() {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_near_end.clone() {
                    let callback_errors = errors.clone();
                    list = list.on_near_end(move || {
                        if let Err(error) = action.run() {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(list, errors))
            }
            BindingWidgetKind::Canvas {
                name,
                viewport,
                shapes,
                draw_stroke,
                desired_size,
            } => {
                let canvas = Canvas::new(name.clone())
                    .viewport(viewport.into_sui())
                    .shapes(shapes.iter().map(|shape| shape.inner.clone()))
                    .draw_stroke(draw_stroke.into_sui())
                    .desired_size(*desired_size);
                BindingRuntimeWidget::new(themed_widget!(canvas, errors))
            }
            BindingWidgetKind::CanvasRuler {
                axis,
                name,
                document_size,
                viewport,
                viewport_size,
                extent,
            } => {
                let mut ruler = CanvasRuler::new(*axis, name.clone(), *document_size)
                    .viewport(viewport.into_sui(), *viewport_size);
                if let Some(extent) = extent {
                    ruler = ruler.extent(*extent);
                }
                BindingRuntimeWidget::new(themed_widget!(ruler, errors))
            }
            BindingWidgetKind::DragDropHost {
                scope,
                child,
                on_external_hover,
                on_external_drop,
                on_external_cancel,
            } => {
                let mut host = DragDropHost::new(
                    scope.inner.clone(),
                    child.into_runtime_widget(errors.clone()),
                );
                if let Some(theme) = errors.theme.clone() {
                    host = host.theme_when(move || theme.snapshot());
                }
                if let Some(action) = on_external_hover.clone() {
                    let callback_errors = errors.clone();
                    host = host.on_external_file_hover(move |_ctx, paths| {
                        let paths = paths
                            .iter()
                            .map(|path| path.to_string_lossy().into_owned())
                            .collect();
                        if let Err(error) = action.run(paths) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_external_drop.clone() {
                    let callback_errors = errors.clone();
                    host = host.on_external_file_drop(move |_ctx, path| {
                        if let Err(error) = action.run(path.to_string_lossy().into_owned()) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_external_cancel.clone() {
                    let callback_errors = errors.clone();
                    host = host.on_external_file_hover_cancelled(move |_ctx| {
                        if let Err(error) = action.run() {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(host)
            }
            BindingWidgetKind::Draggable {
                scope,
                child,
                payload,
                effect,
                preview_label,
                threshold,
                on_start,
                on_end,
            } => {
                let payload_value = payload.clone();
                let mut draggable = Draggable::new(child.into_runtime_widget(errors.clone()))
                    .scope(scope.inner.clone())
                    .payload(move || DragPayload::text(payload_value.clone()))
                    .effect(*effect)
                    .threshold(*threshold);
                if let Some(preview_label) = preview_label {
                    draggable = draggable.preview_label(preview_label.clone());
                }
                if let Some(action) = on_start.clone() {
                    let callback_errors = errors.clone();
                    draggable = draggable.on_drag_start(move |_ctx, preview| {
                        let value = match &preview.payload {
                            DragPayload::Text(text) => text.clone(),
                            DragPayload::Image { handle, .. } => format!("image:{}", handle.get()),
                            DragPayload::Custom { kind, .. } => kind.to_string(),
                        };
                        if let Err(error) = action.run(value) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_end.clone() {
                    let callback_errors = errors.clone();
                    draggable = draggable.on_drag_end(move |_ctx, event| {
                        if let Err(error) = action.run(binding_drag_payload_text(event)) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(draggable)
            }
            BindingWidgetKind::DropTarget {
                scope,
                child,
                effect,
                on_drop,
                on_hover_change,
            } => {
                let effect = *effect;
                let mut target = DropTarget::new(child.into_runtime_widget(errors.clone()))
                    .scope(scope.inner.clone())
                    .accept(move |_| effect);
                if let Some(action) = on_drop.clone() {
                    let callback_errors = errors.clone();
                    target = target.on_drop(move |_ctx, event| {
                        if let Err(error) = action.run(binding_drag_payload_text(event)) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_hover_change.clone() {
                    let callback_errors = errors.clone();
                    target = target.on_hover_change(move |hovered| {
                        if let Err(error) = action.run(hovered) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(target)
            }
            BindingWidgetKind::FloatingWorkspace { state, views, name } => {
                let mut workspace = FloatingWorkspace::new(state.inner.clone());
                if let Some(name) = name {
                    workspace = workspace.name(name.clone());
                }
                if let Some(theme) = errors.theme.clone() {
                    workspace = workspace.theme_when(move || theme.snapshot());
                }
                for view in views {
                    workspace = workspace.with_registered_view(
                        view.id.expect("binding floating view id assigned"),
                        view.child.into_runtime_widget(errors.clone()),
                    );
                }
                BindingRuntimeWidget::new(workspace)
            }
            BindingWidgetKind::PixelCanvas {
                state,
                name,
                width,
                height,
                paper_color,
                desired_size,
                viewport,
                fit_on_first_layout,
                pixels,
            } => {
                let mut canvas = PixelCanvas::new(name.clone(), *width, *height)
                    .state(state.inner.clone())
                    .desired_size(*desired_size)
                    .viewport(viewport.into_sui());
                if let Some(paper_color) = paper_color {
                    canvas = canvas.paper_color(*paper_color);
                }
                canvas = canvas.fit_on_first_layout(*fit_on_first_layout);
                if !pixels.is_empty() {
                    canvas = canvas.with_pixels(pixels.clone());
                }
                BindingRuntimeWidget::new(themed_widget!(canvas, errors))
            }
            BindingWidgetKind::Padding {
                child,
                insets,
                fill_child_width,
                fill_child_height,
            } => {
                let mut padding =
                    PaddingWidget::new(*insets, child.into_runtime_widget(errors.clone()));
                padding = padding
                    .fill_child_width(*fill_child_width)
                    .fill_child_height(*fill_child_height);
                BindingRuntimeWidget::new(padding)
            }
            BindingWidgetKind::Align {
                child,
                horizontal,
                vertical,
            } => BindingRuntimeWidget::new(Align::new(
                *horizontal,
                *vertical,
                child.into_runtime_widget(errors.clone()),
            )),
            BindingWidgetKind::Background { child, color } => BindingRuntimeWidget::new(
                Background::new(*color, child.into_runtime_widget(errors.clone())),
            ),
            BindingWidgetKind::SizedBox {
                child,
                width,
                height,
            } => {
                let mut sized_box = SizedBox::new();
                if let Some(width) = width {
                    sized_box = sized_box.width(*width);
                }
                if let Some(height) = height {
                    sized_box = sized_box.height(*height);
                }
                if let Some(child) = child {
                    sized_box = sized_box.child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(sized_box)
            }
            BindingWidgetKind::Stack {
                children,
                axis,
                spacing,
                alignment,
            } => {
                let mut stack = Stack::new(*axis).gap(*spacing).alignment(*alignment);
                for child in children {
                    stack = stack.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(stack)
            }
            BindingWidgetKind::SemanticRegion {
                name,
                child,
                description,
                role,
            } => {
                let mut region =
                    SemanticRegion::new(name.resolve(), child.into_runtime_widget(errors.clone()))
                        .role(role.clone());
                if matches!(name, BindingText::State(_)) {
                    let name = name.clone();
                    region = region.name_when(move || name.resolve());
                }
                if let Some(description) = description {
                    region = region.description(description.resolve());
                    if matches!(description, BindingText::State(_)) {
                        let description = description.clone();
                        region = region.description_when(move || description.resolve());
                    }
                }
                BindingRuntimeWidget::new(region)
            }
            BindingWidgetKind::FormRow {
                label,
                control,
                stacked,
                label_width,
                control_width,
                gap,
            } => {
                let mut row =
                    FormRow::new(label.clone(), control.into_runtime_widget(errors.clone()));
                if *stacked {
                    row = row.stacked();
                }
                if let Some(label_width) = label_width {
                    row = row.label_width(*label_width);
                }
                if let Some(control_width) = control_width {
                    row = row.control_width(*control_width);
                }
                if let Some(gap) = gap {
                    row = row.gap(*gap);
                }
                BindingRuntimeWidget::new(themed_widget!(row, errors))
            }
            BindingWidgetKind::FieldGroup {
                children,
                spacing,
                padding,
                max_width,
                fill_width,
            } => {
                let mut group = FieldGroup::new();
                if let Some(spacing) = spacing {
                    group = group.gap(*spacing);
                }
                if let Some(padding) = padding {
                    group = group.padding(Insets::all(padding.max(0.0)));
                }
                if let Some(max_width) = max_width {
                    group = group.max_width(*max_width);
                }
                group = group.fill_width(*fill_width);
                for child in children {
                    group = group.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(themed_widget!(group, errors))
            }
            BindingWidgetKind::FormSection {
                title,
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
            } => {
                let mut section =
                    FormSection::new(title.clone(), child.into_runtime_widget(errors.clone()));
                if let Some(description) = description {
                    section = section.description(description.clone());
                }
                if let Some(header_action) = header_action {
                    section =
                        section.header_action(header_action.into_runtime_widget(errors.clone()));
                }
                if let Some(padding) = padding {
                    section = section.padding(Insets::all(padding.max(0.0)));
                }
                if let Some(body_gap) = body_gap {
                    section = section.body_gap(*body_gap);
                }
                if let Some(header_gap) = header_gap {
                    section = section.header_gap(*header_gap);
                }
                if let Some(max_width) = max_width {
                    section = section.max_width(*max_width);
                }
                section = section.fill_width(*fill_width);
                if let Some(radius) = radius {
                    section = section.corner_radius(*radius);
                }
                if let Some(elevation) = elevation {
                    section = section.elevation(*elevation);
                }
                BindingRuntimeWidget::new(themed_widget!(section, errors))
            }
            BindingWidgetKind::PanelSection {
                title,
                child,
                header_action,
                gap,
                action_gap,
                collapsible,
                expanded,
            } => {
                let mut section =
                    PanelSection::new(title.clone(), child.into_runtime_widget(errors.clone()))
                        .collapsible(*collapsible)
                        .expanded(*expanded);
                if let Some(header_action) = header_action {
                    section =
                        section.header_action(header_action.into_runtime_widget(errors.clone()));
                }
                if let Some(gap) = gap {
                    section = section.gap(*gap);
                }
                if let Some(action_gap) = action_gap {
                    section = section.action_gap(*action_gap);
                }
                BindingRuntimeWidget::new(themed_widget!(section, errors))
            }
            BindingWidgetKind::DockPanel {
                title,
                child,
                name,
                header_height,
                padding,
                background,
                header_background,
            } => {
                let mut panel =
                    DockPanel::new(title.clone(), child.into_runtime_widget(errors.clone()));
                if let Some(name) = name {
                    panel = panel.name(name.clone());
                }
                if let Some(header_height) = header_height {
                    panel = panel.header_height(*header_height);
                }
                if let Some(padding) = padding {
                    panel = panel.padding(Insets::all(padding.max(0.0)));
                }
                if let Some(background) = background {
                    panel = panel.background(*background);
                }
                if let Some(header_background) = header_background {
                    panel = panel.header_background(*header_background);
                }
                BindingRuntimeWidget::new(themed_widget!(panel, errors))
            }
            BindingWidgetKind::DockWorkspace {
                state,
                panels,
                name,
            } => {
                let mut workspace = DockWorkspace::new(state.inner.clone()).name(name.clone());
                for panel in panels {
                    workspace = workspace.with_panel(
                        DockPanelId::new(panel.id),
                        panel.title.clone(),
                        panel.child.into_runtime_widget(errors.clone()),
                    );
                }
                BindingRuntimeWidget::new(themed_widget!(workspace, errors))
            }
            BindingWidgetKind::StatusBarHost {
                content,
                status_bar,
            } => BindingRuntimeWidget::new(StatusBarHost::new(
                content.into_runtime_widget(errors.clone()),
                status_bar.into_runtime_widget(errors.clone()),
            )),
            BindingWidgetKind::Tooltip {
                text,
                child,
                placement,
            } => BindingRuntimeWidget::new(
                Tooltip::new(text.clone(), child.into_runtime_widget(errors.clone()))
                    .placement(*placement),
            ),
            BindingWidgetKind::Popover {
                name,
                trigger,
                content,
                open,
                action,
            } => {
                let mut popover = Popover::new(
                    name.clone(),
                    trigger.into_runtime_widget(errors.clone()),
                    content.into_runtime_widget(errors.clone()),
                )
                .open(open.resolve());
                if matches!(open, BindingBool::State(_)) {
                    let open = open.clone();
                    popover = popover.open_when(move || open.resolve());
                }
                let state = open.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    popover = popover.on_open_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(popover)
            }
            BindingWidgetKind::ToolPalette {
                name,
                items,
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
            } => {
                let mut palette = ToolPalette::new(*axis, name.clone())
                    .items(items.iter().map(BindingToolPaletteItem::into_sui))
                    .divider(*divider);
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        palette = palette.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        palette = palette
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                if let Some(extent) = extent {
                    palette = palette.extent(*extent);
                }
                if let Some(padding) = padding {
                    palette = palette.padding(Insets::all(padding.max(0.0)));
                }
                if let Some(spacing) = spacing {
                    palette = palette.gap(*spacing);
                }
                if let Some(item_size) = item_size {
                    palette = palette.item_size(*item_size);
                }
                if let Some(icon_size) = icon_size {
                    palette = palette.icon_size(*icon_size);
                }
                if let Some(background) = background {
                    palette = palette.background(*background);
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    palette = palette.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let palette = enabled_widget!(palette, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(palette, errors))
            }
            BindingWidgetKind::PresetStrip {
                name,
                presets,
                selected,
                action,
                item_width,
                item_height,
                gap,
            } => {
                let mut strip = PresetStrip::new(name.clone()).presets(presets.clone());
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        strip = strip.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        strip = strip
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                if let Some(item_width) = item_width {
                    strip = strip.item_width(*item_width);
                }
                if let Some(item_height) = item_height {
                    strip = strip.item_height(*item_height);
                }
                if let Some(gap) = gap {
                    strip = strip.gap(*gap);
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    strip = strip.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                let strip = enabled_widget!(strip, self.enabled.as_ref());
                BindingRuntimeWidget::new(themed_widget!(strip, errors))
            }
            BindingWidgetKind::BrowserTabBar {
                name,
                tabs,
                selected,
                on_change,
                on_close,
            } => {
                let mut tab_bar = BrowserTabBar::new(name.clone()).tabs(tabs.clone());
                if let Some(selected) = selected {
                    let index = binding_number_to_index(selected.resolve());
                    tab_bar = tab_bar.selected(index);
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        tab_bar = tab_bar
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || on_change.is_some() {
                    let action = on_change.clone();
                    let errors = errors.clone();
                    tab_bar = tab_bar.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as i64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_close.clone() {
                    let errors = errors.clone();
                    tab_bar = tab_bar.on_close(move |index, value| {
                        if let Err(error) = action.run(index, value) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(tab_bar, errors))
            }
            BindingWidgetKind::ScrollView {
                child,
                axes,
                name,
                controller,
            } => {
                let child = child.into_runtime_widget(errors.clone());
                let state = ScrollState::new();
                let mut scroll_view = match axes {
                    BindingScrollAxes::Vertical => ScrollView::vertical(child),
                    BindingScrollAxes::Horizontal => ScrollView::horizontal(child),
                    BindingScrollAxes::Both => ScrollView::both(child),
                }
                .state(state.clone());
                if let Some(name) = name {
                    scroll_view = scroll_view.name(name.clone());
                }
                scroll_controlled(
                    BindingRuntimeWidget::new(themed_widget!(scroll_view, errors)),
                    state,
                    controller,
                )
            }
            BindingWidgetKind::Flex {
                axis,
                gap,
                children,
                options,
            } => {
                let mut flex = Flex::new(*axis).gap(*gap);
                if let Some(justify) = options.justify {
                    flex = flex.justify(justify);
                }
                if let Some(alignment) = options.align_items {
                    flex = flex.align_items(alignment);
                }
                if options.wrap {
                    flex = flex.wrap(FlexWrap::Wrap);
                }
                for child in children {
                    match child.inner.as_ref() {
                        BindingWidgetKind::FlexItem { child, item } => {
                            flex.push_item(child.into_runtime_widget(errors.clone()), *item);
                        }
                        BindingWidgetKind::Spacer => flex.push_spacer(),
                        _ => flex.push(child.into_runtime_widget(errors.clone())),
                    }
                }
                BindingRuntimeWidget::new(flex)
            }
            BindingWidgetKind::FlexItem { child, .. } => child.into_runtime_widget(errors),
            BindingWidgetKind::Spacer => BindingRuntimeWidget::new(SizedBox::new()),
            BindingWidgetKind::RebuildOnChange {
                states,
                build,
                ui_handle,
            } => {
                // Any watched change bumps the version, which RebuildOnChange
                // compares on its next layout.
                let version = Arc::new(AtomicU64::new(0));
                let subscriptions = states
                    .iter()
                    .map(|state| {
                        let version = Arc::clone(&version);
                        state.observe(move |_| {
                            version.fetch_add(1, Ordering::Relaxed);
                        })
                    })
                    .collect::<Vec<_>>();
                let build = build.clone();
                let ui_handle = Arc::clone(ui_handle);
                let errors = errors.clone();
                BindingRuntimeWidget::new(RebuildOnChange::key_when(
                    move || version.load(Ordering::Relaxed),
                    move |_| {
                        // The observers live as long as the widget's builder.
                        let _ = &subscriptions;
                        let child = match build.build() {
                            Ok(widget) => {
                                if let Some(handle) = recover_lock(&ui_handle).clone() {
                                    widget.bind_ui_handle(&handle);
                                }
                                widget.into_runtime_widget(errors.clone())
                            }
                            Err(error) => {
                                errors.push(ForeignCallbackError::new(
                                    ForeignWidgetId::new(0),
                                    ForeignCallbackPhase::Measure,
                                    error.message,
                                ));
                                BindingRuntimeWidget::new(SizedBox::new())
                            }
                        };
                        WidgetPod::new(child)
                    },
                ))
            }
            BindingWidgetKind::Foreign {
                callbacks,
                children,
            } => {
                let mut widget = ForeignWidget::from_arc(Arc::clone(callbacks))
                    .with_error_sink(errors.errors.clone());
                for child in children {
                    widget.push_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(widget)
            }
        }
    }
}
