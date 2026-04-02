#![forbid(unsafe_code)]

use std::error::Error as StdError;
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for Error {}

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(raw: u64) -> Self {
                Self(raw)
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

define_id!(WidgetId);
define_id!(WindowId);
define_id!(SurfaceId);
define_id!(ImageHandle);
define_id!(FontHandle);

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl Color {
    pub const fn rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEventKind {
    Down,
    Up,
    Move,
    Wheel,
    Enter,
    Leave,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointerEvent {
    pub position: Point,
    pub delta: Point,
    pub button: Option<PointerButton>,
    pub kind: PointerEventKind,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardEvent {
    pub key: String,
    pub state: KeyState,
    pub modifiers: Modifiers,
    pub repeat: bool,
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
    ScaleFactorChanged(f64),
    Focused(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomEvent {
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Pointer(PointerEvent),
    Keyboard(KeyboardEvent),
    Ime(ImeEvent),
    Window(WindowEvent),
    Custom(CustomEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticsRole {
    Window,
    GenericContainer,
    Button,
    Text,
    TextInput,
    Canvas,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticsAction {
    Focus,
    Blur,
    Activate,
    Increment,
    Decrement,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SemanticsState {
    pub disabled: bool,
    pub focused: bool,
    pub hidden: bool,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub busy: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticsNode {
    pub id: WidgetId,
    pub role: SemanticsRole,
    pub name: Option<String>,
    pub description: Option<String>,
    pub state: SemanticsState,
    pub actions: Vec<SemanticsAction>,
    pub bounds: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationKind {
    Layout,
    Paint,
    HitTest,
    Text,
    Semantics,
    Resources,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirtyRegion {
    pub area: Rect,
    pub kind: InvalidationKind,
}