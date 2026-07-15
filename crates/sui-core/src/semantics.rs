use crate::{Rect, WidgetId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticsRole {
    Window,
    Root,
    GenericContainer,
    Separator,
    List,
    ListItem,
    Tree,
    Table,
    Splitter,
    Breadcrumb,
    TabBar,
    Tabs,
    Button,
    Link,
    CheckBox,
    Switch,
    RadioButton,
    RadioGroup,
    Menu,
    MenuItem,
    ContextMenu,
    Tooltip,
    Dialog,
    Popover,
    Slider,
    ProgressBar,
    BusyIndicator,
    Text,
    TextInput,
    SpinBox,
    ComboBox,
    Image,
    ColorSwatch,
    ColorPicker,
    Canvas,
    ScrollView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticsAction {
    Focus,
    Blur,
    Activate,
    Expand,
    Collapse,
    Increment,
    Decrement,
    SetValue,
    SetSelection,
    InsertText,
    DeleteBackward,
    DeleteForward,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Custom(String),
}

/// A concrete accessibility action requested by a platform assistive-technology
/// provider.
///
/// [`SemanticsAction`] describes the capabilities advertised by a semantic
/// node. This type carries the payload needed when one of those capabilities is
/// invoked. Keeping the request typed avoids platform-specific string encoding
/// while still allowing widgets to own the behavior and state changes.
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticsActionRequest {
    Focus,
    Blur,
    Activate,
    Expand,
    Collapse,
    Increment,
    Decrement,
    SetValue(SemanticsValue),
    SetSelection(SemanticsTextRange),
    InsertText(String),
    DeleteBackward,
    DeleteForward,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Custom { name: String, value: Option<String> },
}

impl SemanticsActionRequest {
    /// Return the capability a semantic node must advertise for this request.
    pub fn advertised_action(&self) -> SemanticsAction {
        match self {
            Self::Focus => SemanticsAction::Focus,
            Self::Blur => SemanticsAction::Blur,
            Self::Activate => SemanticsAction::Activate,
            Self::Expand => SemanticsAction::Expand,
            Self::Collapse => SemanticsAction::Collapse,
            Self::Increment => SemanticsAction::Increment,
            Self::Decrement => SemanticsAction::Decrement,
            Self::SetValue(_) => SemanticsAction::SetValue,
            Self::SetSelection(_) => SemanticsAction::SetSelection,
            Self::InsertText(_) => SemanticsAction::InsertText,
            Self::DeleteBackward => SemanticsAction::DeleteBackward,
            Self::DeleteForward => SemanticsAction::DeleteForward,
            Self::Copy => SemanticsAction::Copy,
            Self::Cut => SemanticsAction::Cut,
            Self::Paste => SemanticsAction::Paste,
            Self::Undo => SemanticsAction::Undo,
            Self::Redo => SemanticsAction::Redo,
            Self::Custom { name, .. } => SemanticsAction::Custom(name.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleState {
    Unchecked,
    Checked,
    Mixed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticsValue {
    Text(String),
    Number(f64),
    Range { value: f64, min: f64, max: f64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SemanticsState {
    pub disabled: bool,
    pub focused: bool,
    pub hidden: bool,
    pub hovered: bool,
    pub checked: Option<ToggleState>,
    pub selected: bool,
    pub expanded: Option<bool>,
    pub busy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticsTextRange {
    pub start: usize,
    pub end: usize,
}

impl SemanticsTextRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditableTextSemantics {
    pub caret_offset: usize,
    pub selection: SemanticsTextRange,
    pub multiline: bool,
    pub password: bool,
    pub readonly: bool,
    pub scroll_x: f32,
    pub scroll_y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticsNode {
    pub id: WidgetId,
    pub parent: Option<WidgetId>,
    pub role: SemanticsRole,
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<SemanticsValue>,
    /// Smallest meaningful numeric adjustment for range-style controls.
    /// `None` means the control does not expose a numeric step.
    pub numeric_step: Option<f64>,
    pub state: SemanticsState,
    pub actions: Vec<SemanticsAction>,
    pub editable_text: Option<EditableTextSemantics>,
    pub bounds: Rect,
}

impl SemanticsNode {
    pub fn new(id: WidgetId, role: SemanticsRole, bounds: Rect) -> Self {
        Self {
            id,
            parent: None,
            role,
            name: None,
            description: None,
            value: None,
            numeric_step: None,
            state: SemanticsState::default(),
            actions: Vec::new(),
            editable_text: None,
            bounds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SemanticsNode, SemanticsRole};
    use crate::{Rect, WidgetId};

    #[test]
    fn semantic_nodes_default_to_no_numeric_step() {
        let node = SemanticsNode::new(WidgetId::new(7), SemanticsRole::Slider, Rect::ZERO);

        assert_eq!(node.numeric_step, None);
    }
}
