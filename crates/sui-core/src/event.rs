use crate::{Point, Size, Vector};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        shift: false,
        control: false,
        alt: false,
        meta: false,
    };

    pub const fn any(self) -> bool {
        self.shift || self.control || self.alt || self.meta
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PointerButtons(u8);

impl PointerButtons {
    const PRIMARY: u8 = 1 << 0;
    const SECONDARY: u8 = 1 << 1;
    const MIDDLE: u8 = 1 << 2;
    const BACK: u8 = 1 << 3;
    const FORWARD: u8 = 1 << 4;

    pub const NONE: Self = Self(0);

    pub const fn new(bits: u8) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, button: PointerButton) -> bool {
        match Self::mask(button) {
            Some(mask) => self.0 & mask != 0,
            None => false,
        }
    }

    pub fn insert(&mut self, button: PointerButton) {
        if let Some(mask) = Self::mask(button) {
            self.0 |= mask;
        }
    }

    const fn mask(button: PointerButton) -> Option<u8> {
        match button {
            PointerButton::Primary => Some(Self::PRIMARY),
            PointerButton::Secondary => Some(Self::SECONDARY),
            PointerButton::Middle => Some(Self::MIDDLE),
            PointerButton::Back => Some(Self::BACK),
            PointerButton::Forward => Some(Self::FORWARD),
            PointerButton::Other(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerKind {
    #[default]
    Mouse,
    Touch,
    Pen,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEventKind {
    Down,
    Up,
    Move,
    Scroll,
    Enter,
    Leave,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollDelta {
    Lines(Vector),
    Pixels(Vector),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointerEvent {
    pub pointer_id: u64,
    pub kind: PointerEventKind,
    pub position: Point,
    pub delta: Vector,
    pub scroll_delta: Option<ScrollDelta>,
    pub button: Option<PointerButton>,
    pub buttons: PointerButtons,
    pub modifiers: Modifiers,
    pub pointer_kind: PointerKind,
    pub is_primary: bool,
}

impl PointerEvent {
    pub fn new(kind: PointerEventKind, position: Point) -> Self {
        Self {
            pointer_id: 0,
            kind,
            position,
            delta: Vector::ZERO,
            scroll_delta: None,
            button: None,
            buttons: PointerButtons::NONE,
            modifiers: Modifiers::NONE,
            pointer_kind: PointerKind::Mouse,
            is_primary: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardEvent {
    pub key: String,
    pub code: String,
    pub state: KeyState,
    pub modifiers: Modifiers,
    pub repeat: bool,
    pub is_composing: bool,
}

impl KeyboardEvent {
    pub fn new(key: impl Into<String>, state: KeyState) -> Self {
        let key = key.into();

        Self {
            code: key.clone(),
            key,
            state,
            modifiers: Modifiers::NONE,
            repeat: false,
            is_composing: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImeEvent {
    CompositionStart,
    CompositionUpdate { text: String },
    CompositionCommit { text: String },
    CompositionEnd,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowEvent {
    CloseRequested,
    Resized(Size),
    ScaleFactorChanged {
        scale_factor: f64,
        suggested_size: Option<Size>,
    },
    Focused(bool),
    Occluded(bool),
    RedrawRequested,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomEvent {
    pub kind: String,
    pub payload: Option<String>,
}

impl CustomEvent {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            payload: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Pointer(PointerEvent),
    Keyboard(KeyboardEvent),
    Ime(ImeEvent),
    Window(WindowEvent),
    Custom(CustomEvent),
}
