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
            state: SemanticsState::default(),
            actions: Vec::new(),
            editable_text: None,
            bounds,
        }
    }
}
