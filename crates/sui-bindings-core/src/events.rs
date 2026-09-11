use sui::CustomEvent;
use sui::Event;
use sui::ImeEvent;
use sui::KeyState;
use sui::KeyboardEvent;
use sui::Modifiers;
use sui::Point;
use sui::PointerButton;
use sui::PointerButtons;
use sui::PointerEvent;
use sui::PointerEventKind;
use sui::PointerKind;
use sui::RawMouseMotionEvent;
use sui::ScrollDelta;
use sui::Size;
use sui::Vector;
use sui::WindowEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BindingModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

impl From<Modifiers> for BindingModifiers {
    fn from(value: Modifiers) -> Self {
        Self {
            shift: value.shift,
            control: value.control,
            alt: value.alt,
            meta: value.meta,
        }
    }
}

impl From<BindingModifiers> for Modifiers {
    fn from(value: BindingModifiers) -> Self {
        Self {
            shift: value.shift,
            control: value.control,
            alt: value.alt,
            meta: value.meta,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingPointerButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
    Other(u16),
}

impl From<PointerButton> for BindingPointerButton {
    fn from(value: PointerButton) -> Self {
        match value {
            PointerButton::Primary => Self::Primary,
            PointerButton::Secondary => Self::Secondary,
            PointerButton::Middle => Self::Middle,
            PointerButton::Back => Self::Back,
            PointerButton::Forward => Self::Forward,
            PointerButton::Other(button) => Self::Other(button),
        }
    }
}

impl From<BindingPointerButton> for PointerButton {
    fn from(value: BindingPointerButton) -> Self {
        match value {
            BindingPointerButton::Primary => Self::Primary,
            BindingPointerButton::Secondary => Self::Secondary,
            BindingPointerButton::Middle => Self::Middle,
            BindingPointerButton::Back => Self::Back,
            BindingPointerButton::Forward => Self::Forward,
            BindingPointerButton::Other(button) => Self::Other(button),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindingPointerKind {
    #[default]
    Mouse,
    Touch,
    Pen,
    Unknown,
}

impl From<PointerKind> for BindingPointerKind {
    fn from(value: PointerKind) -> Self {
        match value {
            PointerKind::Mouse => Self::Mouse,
            PointerKind::Touch => Self::Touch,
            PointerKind::Pen => Self::Pen,
            PointerKind::Unknown => Self::Unknown,
        }
    }
}

impl From<BindingPointerKind> for PointerKind {
    fn from(value: BindingPointerKind) -> Self {
        match value {
            BindingPointerKind::Mouse => Self::Mouse,
            BindingPointerKind::Touch => Self::Touch,
            BindingPointerKind::Pen => Self::Pen,
            BindingPointerKind::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingPointerEventKind {
    Down,
    Up,
    Move,
    Scroll,
    Enter,
    Leave,
    Cancel,
}

impl From<PointerEventKind> for BindingPointerEventKind {
    fn from(value: PointerEventKind) -> Self {
        match value {
            PointerEventKind::Down => Self::Down,
            PointerEventKind::Up => Self::Up,
            PointerEventKind::Move => Self::Move,
            PointerEventKind::Scroll => Self::Scroll,
            PointerEventKind::Enter => Self::Enter,
            PointerEventKind::Leave => Self::Leave,
            PointerEventKind::Cancel => Self::Cancel,
        }
    }
}

impl From<BindingPointerEventKind> for PointerEventKind {
    fn from(value: BindingPointerEventKind) -> Self {
        match value {
            BindingPointerEventKind::Down => Self::Down,
            BindingPointerEventKind::Up => Self::Up,
            BindingPointerEventKind::Move => Self::Move,
            BindingPointerEventKind::Scroll => Self::Scroll,
            BindingPointerEventKind::Enter => Self::Enter,
            BindingPointerEventKind::Leave => Self::Leave,
            BindingPointerEventKind::Cancel => Self::Cancel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BindingScrollDelta {
    Lines(Vector),
    Pixels(Vector),
}

impl From<ScrollDelta> for BindingScrollDelta {
    fn from(value: ScrollDelta) -> Self {
        match value {
            ScrollDelta::Lines(delta) => Self::Lines(delta),
            ScrollDelta::Pixels(delta) => Self::Pixels(delta),
        }
    }
}

impl From<BindingScrollDelta> for ScrollDelta {
    fn from(value: BindingScrollDelta) -> Self {
        match value {
            BindingScrollDelta::Lines(delta) => Self::Lines(delta),
            BindingScrollDelta::Pixels(delta) => Self::Pixels(delta),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingPointerEvent {
    pub pointer_id: u64,
    pub kind: BindingPointerEventKind,
    pub position: Point,
    pub delta: Vector,
    pub scroll_delta: Option<BindingScrollDelta>,
    pub button: Option<BindingPointerButton>,
    pub buttons: u8,
    pub modifiers: BindingModifiers,
    pub pointer_kind: BindingPointerKind,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingRawMouseMotionEvent {
    pub delta: Vector,
    pub modifiers: BindingModifiers,
}

impl From<&RawMouseMotionEvent> for BindingRawMouseMotionEvent {
    fn from(value: &RawMouseMotionEvent) -> Self {
        Self {
            delta: value.delta,
            modifiers: value.modifiers.into(),
        }
    }
}

impl From<BindingRawMouseMotionEvent> for RawMouseMotionEvent {
    fn from(value: BindingRawMouseMotionEvent) -> Self {
        Self {
            delta: value.delta,
            modifiers: value.modifiers.into(),
        }
    }
}

impl BindingPointerEvent {
    pub fn new(kind: BindingPointerEventKind, position: Point) -> Self {
        let event = PointerEvent::new(kind.into(), position);
        Self::from(&event)
    }
}

impl From<&PointerEvent> for BindingPointerEvent {
    fn from(value: &PointerEvent) -> Self {
        Self {
            pointer_id: value.pointer_id,
            kind: value.kind.into(),
            position: value.position,
            delta: value.delta,
            scroll_delta: value.scroll_delta.map(Into::into),
            button: value.button.map(Into::into),
            buttons: value.buttons.bits(),
            modifiers: value.modifiers.into(),
            pointer_kind: value.pointer_kind.into(),
            is_primary: value.is_primary,
        }
    }
}

impl From<BindingPointerEvent> for PointerEvent {
    fn from(value: BindingPointerEvent) -> Self {
        Self {
            pointer_id: value.pointer_id,
            kind: value.kind.into(),
            position: value.position,
            delta: value.delta,
            scroll_delta: value.scroll_delta.map(Into::into),
            button: value.button.map(Into::into),
            buttons: PointerButtons::new(value.buttons),
            modifiers: value.modifiers.into(),
            pointer_kind: value.pointer_kind.into(),
            is_primary: value.is_primary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKeyState {
    Pressed,
    Released,
}

impl From<KeyState> for BindingKeyState {
    fn from(value: KeyState) -> Self {
        match value {
            KeyState::Pressed => Self::Pressed,
            KeyState::Released => Self::Released,
        }
    }
}

impl From<BindingKeyState> for KeyState {
    fn from(value: BindingKeyState) -> Self {
        match value {
            BindingKeyState::Pressed => Self::Pressed,
            BindingKeyState::Released => Self::Released,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingKeyboardEvent {
    pub key: String,
    pub code: String,
    pub text: Option<String>,
    pub state: BindingKeyState,
    pub modifiers: BindingModifiers,
    pub repeat: bool,
    pub is_composing: bool,
}

impl BindingKeyboardEvent {
    pub fn new(key: impl Into<String>, state: BindingKeyState) -> Self {
        let event = KeyboardEvent::new(key, state.into());
        Self::from(&event)
    }
}

impl From<&KeyboardEvent> for BindingKeyboardEvent {
    fn from(value: &KeyboardEvent) -> Self {
        Self {
            key: value.key.clone(),
            code: value.code.clone(),
            text: value.text.clone(),
            state: value.state.into(),
            modifiers: value.modifiers.into(),
            repeat: value.repeat,
            is_composing: value.is_composing,
        }
    }
}

impl From<BindingKeyboardEvent> for KeyboardEvent {
    fn from(value: BindingKeyboardEvent) -> Self {
        Self {
            key: value.key,
            code: value.code,
            text: value.text,
            state: value.state.into(),
            modifiers: value.modifiers.into(),
            repeat: value.repeat,
            is_composing: value.is_composing,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingImeEvent {
    CompositionStart,
    CompositionUpdate {
        text: String,
        cursor_start: Option<usize>,
        cursor_end: Option<usize>,
    },
    CompositionCommit {
        text: String,
    },
    CompositionEnd,
}

impl From<&ImeEvent> for BindingImeEvent {
    fn from(value: &ImeEvent) -> Self {
        match value {
            ImeEvent::CompositionStart => Self::CompositionStart,
            ImeEvent::CompositionUpdate { text, cursor_range } => Self::CompositionUpdate {
                text: text.clone(),
                cursor_start: cursor_range.as_ref().map(|range| range.start),
                cursor_end: cursor_range.as_ref().map(|range| range.end),
            },
            ImeEvent::CompositionCommit { text } => Self::CompositionCommit { text: text.clone() },
            ImeEvent::CompositionEnd => Self::CompositionEnd,
        }
    }
}

impl From<BindingImeEvent> for ImeEvent {
    fn from(value: BindingImeEvent) -> Self {
        match value {
            BindingImeEvent::CompositionStart => Self::CompositionStart,
            BindingImeEvent::CompositionUpdate {
                text,
                cursor_start,
                cursor_end,
            } => Self::CompositionUpdate {
                text,
                cursor_range: match (cursor_start, cursor_end) {
                    (Some(start), Some(end)) => Some(start..end),
                    _ => None,
                },
            },
            BindingImeEvent::CompositionCommit { text } => Self::CompositionCommit { text },
            BindingImeEvent::CompositionEnd => Self::CompositionEnd,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingWindowEvent {
    CloseRequested,
    Resized(Size),
    Moved(Point),
    ScaleFactorChanged {
        scale_factor: f64,
        raw_dpi: Option<f32>,
        suggested_size: Option<Size>,
    },
    Focused(bool),
    Occluded(bool),
    SafeAreaChanged {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    },
    ExternalFileHovered(String),
    ExternalFileHoverCancelled,
    ExternalFileDropped(String),
    RedrawRequested,
}

impl From<&WindowEvent> for BindingWindowEvent {
    fn from(value: &WindowEvent) -> Self {
        match value {
            WindowEvent::CloseRequested => Self::CloseRequested,
            WindowEvent::Resized(size) => Self::Resized(*size),
            WindowEvent::Moved(position) => Self::Moved(*position),
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                raw_dpi,
                suggested_size,
            } => Self::ScaleFactorChanged {
                scale_factor: *scale_factor,
                raw_dpi: *raw_dpi,
                suggested_size: *suggested_size,
            },
            WindowEvent::Focused(focused) => Self::Focused(*focused),
            WindowEvent::Occluded(occluded) => Self::Occluded(*occluded),
            WindowEvent::SafeAreaChanged(insets) => Self::SafeAreaChanged {
                left: insets.left,
                top: insets.top,
                right: insets.right,
                bottom: insets.bottom,
            },
            WindowEvent::ExternalFileHovered(path) => {
                Self::ExternalFileHovered(path.to_string_lossy().into_owned())
            }
            WindowEvent::ExternalFileHoverCancelled => Self::ExternalFileHoverCancelled,
            WindowEvent::ExternalFileDropped(path) => {
                Self::ExternalFileDropped(path.to_string_lossy().into_owned())
            }
            WindowEvent::RedrawRequested => Self::RedrawRequested,
        }
    }
}

impl From<BindingWindowEvent> for WindowEvent {
    fn from(value: BindingWindowEvent) -> Self {
        match value {
            BindingWindowEvent::CloseRequested => Self::CloseRequested,
            BindingWindowEvent::Resized(size) => Self::Resized(size),
            BindingWindowEvent::Moved(position) => Self::Moved(position),
            BindingWindowEvent::ScaleFactorChanged {
                scale_factor,
                raw_dpi,
                suggested_size,
            } => Self::ScaleFactorChanged {
                scale_factor,
                raw_dpi,
                suggested_size,
            },
            BindingWindowEvent::Focused(focused) => Self::Focused(focused),
            BindingWindowEvent::Occluded(occluded) => Self::Occluded(occluded),
            BindingWindowEvent::SafeAreaChanged {
                left,
                top,
                right,
                bottom,
            } => Self::SafeAreaChanged(sui::SafeAreaInsets::new(left, top, right, bottom)),
            BindingWindowEvent::ExternalFileHovered(path) => Self::ExternalFileHovered(path.into()),
            BindingWindowEvent::ExternalFileHoverCancelled => Self::ExternalFileHoverCancelled,
            BindingWindowEvent::ExternalFileDropped(path) => Self::ExternalFileDropped(path.into()),
            BindingWindowEvent::RedrawRequested => Self::RedrawRequested,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingCustomEvent {
    pub kind: String,
    pub payload: Option<String>,
}

impl From<&CustomEvent> for BindingCustomEvent {
    fn from(value: &CustomEvent) -> Self {
        Self {
            kind: value.kind.clone(),
            payload: value.payload.clone(),
        }
    }
}

impl From<BindingCustomEvent> for CustomEvent {
    fn from(value: BindingCustomEvent) -> Self {
        Self {
            kind: value.kind,
            payload: value.payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingEvent {
    Pointer(BindingPointerEvent),
    RawMouseMotion(BindingRawMouseMotionEvent),
    Keyboard(BindingKeyboardEvent),
    Ime(BindingImeEvent),
    Window(BindingWindowEvent),
    Custom(BindingCustomEvent),
    Unsupported { kind: String },
}

impl BindingEvent {
    pub fn kind(&self) -> &str {
        match self {
            Self::Pointer(_) => "pointer",
            Self::RawMouseMotion(_) => "raw_mouse_motion",
            Self::Keyboard(_) => "keyboard",
            Self::Ime(_) => "ime",
            Self::Window(_) => "window",
            Self::Custom(_) => "custom",
            Self::Unsupported { kind } => kind,
        }
    }

    pub fn into_sui_event(self) -> Result<Event, String> {
        match self {
            Self::Pointer(event) => Ok(Event::Pointer(event.into())),
            Self::RawMouseMotion(event) => Ok(Event::RawMouseMotion(event.into())),
            Self::Keyboard(event) => Ok(Event::Keyboard(event.into())),
            Self::Ime(event) => Ok(Event::Ime(event.into())),
            Self::Window(event) => Ok(Event::Window(event.into())),
            Self::Custom(event) => Ok(Event::Custom(event.into())),
            Self::Unsupported { kind } => {
                Err(format!("{kind} events cannot be dispatched from bindings"))
            }
        }
    }
}

impl From<&Event> for BindingEvent {
    fn from(value: &Event) -> Self {
        match value {
            Event::Pointer(event) => Self::Pointer(BindingPointerEvent::from(event)),
            Event::RawMouseMotion(event) => {
                Self::RawMouseMotion(BindingRawMouseMotionEvent::from(event))
            }
            Event::Keyboard(event) => Self::Keyboard(BindingKeyboardEvent::from(event)),
            Event::Ime(event) => Self::Ime(BindingImeEvent::from(event)),
            Event::Window(event) => Self::Window(BindingWindowEvent::from(event)),
            Event::Custom(event) => Self::Custom(BindingCustomEvent::from(event)),
            Event::Drag(_) => Self::Unsupported {
                kind: "drag".to_string(),
            },
            Event::Semantics(_) => Self::Unsupported {
                kind: "semantics".to_string(),
            },
            Event::Wake(_) => Self::Unsupported {
                kind: "wake".to_string(),
            },
        }
    }
}
