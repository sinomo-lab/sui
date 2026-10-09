//! WebAssembly bindings for SUI's JavaScript API.
//!
//! The browser entry point of the `@sui/ui` package (`crates/sui-js/web.js`)
//! loads this module. It exports the same classes and functions as the Node
//! addon in `crates/sui-js`, so the shared JavaScript layer runs on both.
#![cfg(target_arch = "wasm32")]
#![allow(
    clippy::large_enum_variant,
    clippy::too_many_arguments,
    clippy::wrong_self_convention
)]

mod animation;
mod app;
mod callbacks;
mod convert;
mod events;
mod generated_widgets;
mod geometry;
mod interop;
mod paint;
mod support;
mod testing;
mod widget;

/// Imports shared by the binding modules.
mod prelude {
    pub(crate) use ::sui as sui_crate;
    pub(crate) use js_sys::{Array, Function, Object, Reflect, Uint8Array};
    pub(crate) use std::cell::RefCell;
    pub(crate) use std::collections::HashMap;
    pub(crate) use std::rc::Rc;
    pub(crate) use std::sync::{Arc, Mutex};
    pub(crate) use sui_bindings_core::*;
    pub(crate) use sui_crate::{
        Axis, Color, ColorSpace, Constraints, Event, FontStretch, FontStyle, FontWeight, GridTrack,
        Path, PathBuilder, Point, Rect, RegisteredImage, RuntimeApplication, SceneCommand,
        SemanticsNode, SemanticsRole, SemanticsValue, ShadowParams, Size, StrokeStyle, TextStyle,
        ToggleState, Transform, Vector, WidgetId, WindowBuilder,
    };
    pub(crate) use wasm_bindgen::prelude::*;

    pub(crate) use crate::app::*;
    pub(crate) use crate::callbacks::*;
    pub(crate) use crate::convert::*;
    pub(crate) use crate::events::*;
    pub(crate) use crate::generated_widgets::*;
    pub(crate) use crate::geometry::*;
    pub(crate) use crate::interop::*;
    pub(crate) use crate::paint::*;
    pub(crate) use crate::support::*;
    pub(crate) use crate::testing::*;
    pub(crate) use crate::widget::*;
}

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

/// The version of the native module; the package exports it as `version`.
#[wasm_bindgen(js_name = "nativeVersion")]
pub fn native_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
