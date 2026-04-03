use crate::{Rect, WidgetId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticsRole {
    Window,
    Root,
    GenericContainer,
    Separator,
    TabBar,
    Tabs,
    Button,
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
            bounds,
        }
    }
}
