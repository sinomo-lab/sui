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

impl BindingPointerEventKind {
    /// Parse the case-sensitive binding vocabulary without normalizing input.
    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "down" => Some(Self::Down),
            "up" => Some(Self::Up),
            "move" => Some(Self::Move),
            "scroll" => Some(Self::Scroll),
            "enter" => Some(Self::Enter),
            "leave" => Some(Self::Leave),
            "cancel" => Some(Self::Cancel),
            _ => None,
        }
    }

    /// Return the canonical binding name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
            Self::Move => "move",
            Self::Scroll => "scroll",
            Self::Enter => "enter",
            Self::Leave => "leave",
            Self::Cancel => "cancel",
        }
    }
}

impl BindingPointerKind {
    /// Parse the case-sensitive binding vocabulary without normalizing input.
    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "mouse" => Some(Self::Mouse),
            "touch" => Some(Self::Touch),
            "pen" => Some(Self::Pen),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    /// Return the canonical binding name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Mouse => "mouse",
            Self::Touch => "touch",
            Self::Pen => "pen",
            Self::Unknown => "unknown",
        }
    }
}

impl BindingKeyState {
    /// Parse the case-sensitive binding vocabulary without normalizing input.
    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "pressed" | "down" => Some(Self::Pressed),
            "released" | "up" => Some(Self::Released),
            _ => None,
        }
    }

    /// Return the canonical binding name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Pressed => "pressed",
            Self::Released => "released",
        }
    }
}

impl BindingPointerButton {
    /// Parse the case-sensitive binding vocabulary without normalizing input.
    pub fn from_name(value: &str) -> Result<Self, &'static str> {
        match value {
            "primary" | "left" => Ok(Self::Primary),
            "secondary" | "right" => Ok(Self::Secondary),
            "middle" => Ok(Self::Middle),
            "back" => Ok(Self::Back),
            "forward" => Ok(Self::Forward),
            _ => {
                if let Some(value) = value.strip_prefix("other:") {
                    return value
                        .parse::<u16>()
                        .map(Self::Other)
                        .map_err(|_| "other pointer button must fit in u16");
                }
                Err(
                    "pointer button must be 'primary', 'secondary', 'middle', 'back', 'forward', or 'other:<u16>'",
                )
            }
        }
    }

    /// Return the canonical binding name.
    pub fn name(self) -> String {
        match self {
            Self::Primary => "primary".to_owned(),
            Self::Secondary => "secondary".to_owned(),
            Self::Middle => "middle".to_owned(),
            Self::Back => "back".to_owned(),
            Self::Forward => "forward".to_owned(),
            Self::Other(button) => format!("other:{button}"),
        }
    }
}

impl BindingImeEvent {
    /// Construct an IME event using the JavaScript camelCase/snake_case aliases.
    pub fn from_js_name(
        kind: &str,
        text: Option<String>,
        cursor_start: Option<usize>,
        cursor_end: Option<usize>,
    ) -> Result<Self, &'static str> {
        Ok(match kind {
            "compositionStart" | "composition_start" | "start" => BindingImeEvent::CompositionStart,
            "compositionUpdate" | "composition_update" | "update" => {
                BindingImeEvent::CompositionUpdate {
                    text: text.unwrap_or_default(),
                    cursor_start,
                    cursor_end,
                }
            }
            "compositionCommit" | "composition_commit" | "commit" => {
                BindingImeEvent::CompositionCommit {
                    text: text.unwrap_or_default(),
                }
            }
            "compositionEnd" | "composition_end" | "end" => BindingImeEvent::CompositionEnd,
            _ => {
                return Err(
                    "IME kind must be 'compositionStart', 'compositionUpdate', 'compositionCommit', or 'compositionEnd'",
                );
            }
        })
    }
}

#[cfg(test)]
mod vocabulary_tests {
    use super::*;

    #[test]
    fn event_names_preserve_aliases_and_reject_normalization() {
        for name in ["down", "up", "move", "scroll", "enter", "leave", "cancel"] {
            assert_eq!(
                BindingPointerEventKind::from_name(name).unwrap().name(),
                name
            );
        }
        for name in ["mouse", "touch", "pen", "unknown"] {
            assert_eq!(BindingPointerKind::from_name(name).unwrap().name(), name);
        }
        for (alias, canonical) in [
            ("down", "pressed"),
            ("up", "released"),
            ("pressed", "pressed"),
            ("released", "released"),
        ] {
            assert_eq!(BindingKeyState::from_name(alias).unwrap().name(), canonical);
        }
        for name in ["", "DOWN", " down", "down "] {
            assert!(BindingPointerEventKind::from_name(name).is_none());
            assert!(BindingPointerKind::from_name(name).is_none());
            assert!(BindingKeyState::from_name(name).is_none());
        }
    }

    #[test]
    fn pointer_button_aliases_and_numeric_boundaries_are_preserved() {
        for (alias, canonical) in [
            ("left", "primary"),
            ("right", "secondary"),
            ("primary", "primary"),
            ("secondary", "secondary"),
            ("middle", "middle"),
            ("back", "back"),
            ("forward", "forward"),
            ("other:0", "other:0"),
            ("other:65535", "other:65535"),
            ("other:+1", "other:1"),
        ] {
            let button = BindingPointerButton::from_name(alias).unwrap();
            assert_eq!(button.name(), canonical);
            assert_eq!(BindingPointerButton::from_name(&button.name()), Ok(button));
        }
        for value in ["other:", "other:-1", "other:65536", "other: 1"] {
            assert_eq!(
                BindingPointerButton::from_name(value),
                Err("other pointer button must fit in u16")
            );
        }
        assert!(BindingPointerButton::from_name("LEFT").is_err());
    }

    #[test]
    fn javascript_ime_aliases_preserve_payloads_and_defaults() {
        for aliases in [
            ["compositionStart", "composition_start", "start"],
            ["compositionUpdate", "composition_update", "update"],
            ["compositionCommit", "composition_commit", "commit"],
            ["compositionEnd", "composition_end", "end"],
        ] {
            let expected =
                BindingImeEvent::from_js_name(aliases[0], Some("文".into()), Some(1), Some(2))
                    .unwrap();
            for alias in aliases {
                assert_eq!(
                    BindingImeEvent::from_js_name(alias, Some("文".into()), Some(1), Some(2))
                        .unwrap(),
                    expected
                );
            }
        }
        assert_eq!(
            BindingImeEvent::from_js_name("update", None, Some(1), None).unwrap(),
            BindingImeEvent::CompositionUpdate {
                text: String::new(),
                cursor_start: Some(1),
                cursor_end: None
            }
        );
        for invalid in ["composition-start", "START", " start", ""] {
            assert!(BindingImeEvent::from_js_name(invalid, None, None, None).is_err());
        }
    }
}

impl BindingImeEvent {
    /// Return the canonical JavaScript event action name.
    pub fn js_name(&self) -> &'static str {
        match self {
            Self::CompositionStart => "compositionStart",
            Self::CompositionUpdate { .. } => "compositionUpdate",
            Self::CompositionCommit { .. } => "compositionCommit",
            Self::CompositionEnd => "compositionEnd",
        }
    }
}

impl BindingWindowEvent {
    /// Return the canonical JavaScript event action name.
    pub fn js_name(&self) -> &'static str {
        match self {
            Self::CloseRequested => "closeRequested",
            Self::Resized(_) => "resized",
            Self::Moved(_) => "moved",
            Self::ScaleFactorChanged { .. } => "scaleFactorChanged",
            Self::Focused(_) => "focused",
            Self::Occluded(_) => "occluded",
            Self::SafeAreaChanged { .. } => "safeAreaChanged",
            Self::ExternalFileHovered(_) => "externalFileHovered",
            Self::ExternalFileHoverCancelled => "externalFileHoverCancelled",
            Self::ExternalFileDropped(_) => "externalFileDropped",
            Self::RedrawRequested => "redrawRequested",
        }
    }
}
