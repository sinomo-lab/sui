//! Conversions between JavaScript values and the binding core.
//!
//! wasm-bindgen passes exported classes by value (moving them out of their
//! JavaScript objects) or by plain reference. Arrays, optional arguments, and
//! option objects need references to classes inside arbitrary JavaScript
//! values, so this module looks instances up by their class constructors,
//! which `web.js` registers once with `__registerClasses`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;

use js_sys::{Array, Function, Object, Reflect};
use sui_bindings_core::{
    BindingBool, BindingNumber, BindingState, BindingText, BindingValue, ForeignCallbackFailure,
};
use wasm_bindgen::convert::RefFromWasmAbi;
use wasm_bindgen::prelude::*;

pub(crate) type Result<T> = std::result::Result<T, JsValue>;

/// An error for an argument the API cannot accept. Like the Node addon's
/// errors, it is an `Error` whose `code` is `"InvalidArg"`.
pub(crate) fn invalid_arg(message: impl ToString) -> JsValue {
    coded_error(message, "InvalidArg")
}

/// An error for an operation that failed, with `code` `"GenericFailure"`.
pub(crate) fn runtime_error(message: impl ToString) -> JsValue {
    coded_error(message, "GenericFailure")
}

fn coded_error(message: impl ToString, code: &str) -> JsValue {
    let error = js_sys::Error::new(&message.to_string());
    let _ = Reflect::set(&error, &JsValue::from_str("code"), &JsValue::from_str(code));
    error.into()
}

/// The message of a thrown JavaScript value.
pub(crate) fn error_message(error: &JsValue) -> String {
    if let Some(error) = error.dyn_ref::<js_sys::Error>() {
        return String::from(error.message());
    }
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

/// Turn a JavaScript exception into a callback failure for the binding core.
pub(crate) fn callback_failure(error: JsValue) -> ForeignCallbackFailure {
    ForeignCallbackFailure::new(error_message(&error))
}

/// A JavaScript value the binding core may hold in `Send + Sync` callbacks.
///
/// `wasm32-unknown-unknown` without the `atomics` target feature has a single
/// thread, so the value is never shared across threads.
pub(crate) struct Local<T>(pub(crate) T);

// SAFETY: see the type documentation; this build has exactly one thread.
unsafe impl<T> Send for Local<T> {}
// SAFETY: as above.
unsafe impl<T> Sync for Local<T> {}

impl<T> Deref for Local<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone> Clone for Local<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// ---- Exported classes inside JavaScript values ----------------------------

thread_local! {
    static CLASSES: RefCell<HashMap<String, Function>> = RefCell::new(HashMap::new());
}

/// Record the exported class constructors so instances can be recognized
/// inside arrays and option objects. `web.js` calls this with the module
/// namespace right after loading the WebAssembly.
#[wasm_bindgen(js_name = "__registerClasses")]
pub fn register_classes(namespace: &JsValue) -> Result<()> {
    let keys = Object::keys(namespace.unchecked_ref::<Object>());
    CLASSES.with(|classes| {
        let mut classes = classes.borrow_mut();
        for key in keys.iter() {
            let Some(name) = key.as_string() else {
                continue;
            };
            if let Ok(value) = Reflect::get(namespace, &key)
                && let Ok(constructor) = value.dyn_into::<Function>()
            {
                classes.insert(name, constructor);
            }
        }
    });
    Ok(())
}

/// The ABI wasm-bindgen 0.2.129 uses for exported classes: a pointer to the
/// reference-counted cell holding the Rust value.
type ClassAbi<T> = wasm_bindgen::__rt::WasmPtr<wasm_bindgen::__rt::WasmRefCell<T>>;

/// An exported class that can be found inside JavaScript values.
pub trait JsClass: RefFromWasmAbi<Abi = ClassAbi<Self>> + Sized {
    /// The class's JavaScript name.
    const NAME: &'static str;
}

/// Implement [`JsClass`] and [`IntoJs`] for exported classes.
macro_rules! js_class {
    ($($ty:ty => $name:literal),* $(,)?) => {
        $(impl $crate::convert::JsClass for $ty {
            const NAME: &'static str = $name;
        }

        impl $crate::convert::IntoJs for $ty {
            fn into_js(self) -> ::wasm_bindgen::JsValue {
                self.into()
            }
        })*
    };
}
pub(crate) use js_class;

/// Whether `value` is an instance of the exported class `name`.
pub(crate) fn is_instance(value: &JsValue, name: &str) -> bool {
    if !value.is_object() {
        return false;
    }
    CLASSES.with(|classes| {
        classes.borrow().get(name).is_some_and(|constructor| {
            Reflect::get(constructor, &JsValue::from_str("prototype"))
                .ok()
                .and_then(|prototype| prototype.dyn_into::<Object>().ok())
                .is_some_and(|prototype| prototype.is_prototype_of(value))
        })
    })
}

/// Borrow the Rust value behind an instance of `T`'s class.
pub(crate) fn class_ref<T: JsClass>(value: &JsValue) -> Result<T::Anchor> {
    if !is_instance(value, T::NAME) {
        return Err(invalid_arg(format!("expected a {}", T::NAME)));
    }
    let pointer = Reflect::get(value, &JsValue::from_str("__wbg_ptr"))?
        .as_f64()
        .unwrap_or(0.0);
    if pointer == 0.0 {
        return Err(invalid_arg(format!("the {} has been freed", T::NAME)));
    }
    let pointer = ClassAbi::<T>::from_ptr(pointer as usize as *mut _);
    // SAFETY: `value` is an instance of `T`'s exported class, and a nonzero
    // `__wbg_ptr` is the live pointer wasm-bindgen stores for that instance.
    // Borrowing takes another reference count; the object keeps its own.
    Ok(unsafe { T::ref_from_abi(pointer) })
}

/// A copy of the Rust value behind an instance of `T`'s class.
pub(crate) fn class_value<T: JsClass + Clone>(value: &JsValue) -> Result<T> {
    Ok(class_ref::<T>(value)?.clone())
}

/// Copies of the Rust values behind an array of `T` instances.
pub(crate) fn class_array<T: JsClass + Clone>(value: &JsValue) -> Result<Vec<T>> {
    let array = value
        .dyn_ref::<Array>()
        .ok_or_else(|| invalid_arg(format!("expected an array of {}", T::NAME)))?;
    array.iter().map(|item| class_value(&item)).collect()
}

/// A JavaScript array of exported class instances.
pub(crate) fn to_array<T: Into<JsValue>>(items: impl IntoIterator<Item = T>) -> Array {
    items.into_iter().map(Into::into).collect()
}

// ---- Option objects -------------------------------------------------------

/// A value read from a JavaScript argument or option-object field.
pub(crate) trait FromJs: Sized {
    fn from_js(value: &JsValue) -> Result<Self>;
}

impl FromJs for f64 {
    fn from_js(value: &JsValue) -> Result<Self> {
        value
            .as_f64()
            .ok_or_else(|| invalid_arg("expected a number"))
    }
}

impl FromJs for f32 {
    fn from_js(value: &JsValue) -> Result<Self> {
        f64::from_js(value).map(|value| value as f32)
    }
}

impl FromJs for u32 {
    fn from_js(value: &JsValue) -> Result<Self> {
        let number = f64::from_js(value)?;
        if number.fract() != 0.0 || !(0.0..=f64::from(u32::MAX)).contains(&number) {
            return Err(invalid_arg("expected a non-negative integer"));
        }
        Ok(number as u32)
    }
}

impl FromJs for i32 {
    fn from_js(value: &JsValue) -> Result<Self> {
        let number = f64::from_js(value)?;
        if number.fract() != 0.0 || !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&number) {
            return Err(invalid_arg("expected an integer"));
        }
        Ok(number as i32)
    }
}

impl FromJs for bool {
    fn from_js(value: &JsValue) -> Result<Self> {
        value
            .as_bool()
            .ok_or_else(|| invalid_arg("expected a boolean"))
    }
}

impl FromJs for String {
    fn from_js(value: &JsValue) -> Result<Self> {
        value
            .as_string()
            .ok_or_else(|| invalid_arg("expected a string"))
    }
}

impl FromJs for JsValue {
    fn from_js(value: &JsValue) -> Result<Self> {
        Ok(value.clone())
    }
}

impl FromJs for Function {
    fn from_js(value: &JsValue) -> Result<Self> {
        value
            .dyn_ref::<Function>()
            .cloned()
            .ok_or_else(|| invalid_arg("expected a function"))
    }
}

impl<T: FromJs> FromJs for Option<T> {
    fn from_js(value: &JsValue) -> Result<Self> {
        if value.is_undefined() || value.is_null() {
            Ok(None)
        } else {
            T::from_js(value).map(Some)
        }
    }
}

impl<T: FromJs> FromJs for Vec<T> {
    fn from_js(value: &JsValue) -> Result<Self> {
        let array = value
            .dyn_ref::<Array>()
            .ok_or_else(|| invalid_arg("expected an array"))?;
        array.iter().map(|item| T::from_js(&item)).collect()
    }
}

impl FromJs for HashMap<String, String> {
    fn from_js(value: &JsValue) -> Result<Self> {
        let object = value
            .dyn_ref::<Object>()
            .ok_or_else(|| invalid_arg("expected an object of strings"))?;
        Object::entries(object)
            .iter()
            .map(|entry| {
                let entry = Array::from(&entry);
                Ok((
                    String::from_js(&entry.get(0))?,
                    String::from_js(&entry.get(1))?,
                ))
            })
            .collect()
    }
}

impl<T: JsClass + Clone> FromJs for T {
    fn from_js(value: &JsValue) -> Result<Self> {
        class_value(value)
    }
}

/// A value returned to JavaScript inside an object or array.
pub(crate) trait IntoJs {
    fn into_js(self) -> JsValue;
}

macro_rules! into_js_via_from {
    ($($ty:ty),*) => {
        $(impl IntoJs for $ty {
            fn into_js(self) -> JsValue {
                JsValue::from(self)
            }
        })*
    };
}

into_js_via_from!(f64, f32, u32, i32, bool, String, JsValue);

impl<T: IntoJs> IntoJs for Option<T> {
    fn into_js(self) -> JsValue {
        self.map_or(JsValue::UNDEFINED, IntoJs::into_js)
    }
}

impl<T: IntoJs> IntoJs for Vec<T> {
    fn into_js(self) -> JsValue {
        self.into_iter()
            .map(IntoJs::into_js)
            .collect::<Array>()
            .into()
    }
}

impl IntoJs for HashMap<String, String> {
    fn into_js(self) -> JsValue {
        let object = Object::new();
        for (key, value) in self {
            let _ = Reflect::set(&object, &JsValue::from(key), &JsValue::from(value));
        }
        object.into()
    }
}

/// Read field `name` of an option object; a missing field reads as
/// `undefined`.
pub(crate) fn field<T: FromJs>(object: &JsValue, name: &str) -> Result<T> {
    let value = if object.is_undefined() || object.is_null() {
        JsValue::UNDEFINED
    } else {
        Reflect::get(object, &JsValue::from_str(name))?
    };
    T::from_js(&value).map_err(|error| invalid_arg(format!("{name}: {}", error_message(&error))))
}

/// Declare a struct read from a JavaScript option object, one field per
/// property: `rust_name: Type = "jsName"`.
macro_rules! js_object {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($(#[$field_meta:meta])* $field:ident: $ty:ty = $js:literal),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $($(#[$field_meta])* pub $field: $ty,)*
        }

        impl $crate::convert::FromJs for $name {
            fn from_js(value: &::wasm_bindgen::JsValue) -> $crate::convert::Result<Self> {
                if !(value.is_object() || value.is_undefined() || value.is_null()) {
                    return Err($crate::convert::invalid_arg(concat!(
                        "expected ", stringify!($name), " to be an object"
                    )));
                }
                Ok(Self {
                    $($field: $crate::convert::field(value, $js)?,)*
                })
            }
        }

        impl $crate::convert::IntoJs for $name {
            fn into_js(self) -> ::wasm_bindgen::JsValue {
                let object = ::js_sys::Object::new();
                $(let _ = ::js_sys::Reflect::set(
                    &object,
                    &::wasm_bindgen::JsValue::from_str($js),
                    &$crate::convert::IntoJs::into_js(self.$field),
                );)*
                object.into()
            }
        }
    };
}
pub(crate) use js_object;

// ---- Binding values -------------------------------------------------------

/// A string, number, or boolean as a binding value.
pub(crate) fn binding_value_from_js(value: &JsValue) -> Result<BindingValue> {
    if let Some(text) = value.as_string() {
        Ok(BindingValue::String(text))
    } else if let Some(number) = value.as_f64() {
        Ok(BindingValue::Number(number))
    } else if let Some(flag) = value.as_bool() {
        Ok(BindingValue::Bool(flag))
    } else {
        Err(invalid_arg(
            "binding values must be a string, number, or boolean",
        ))
    }
}

pub(crate) fn binding_value_to_js(value: BindingValue) -> JsValue {
    match value {
        BindingValue::String(value) => JsValue::from_str(&value),
        BindingValue::Number(value) => JsValue::from_f64(value),
        BindingValue::Integer(value) => JsValue::from_f64(value as f64),
        BindingValue::Bool(value) => JsValue::from_bool(value),
    }
}

/// A `State` or a plain value.
pub(crate) enum StateOrValue {
    State(BindingState),
    Value(BindingValue),
}

pub(crate) fn state_or_value(value: &JsValue) -> Result<StateOrValue> {
    if is_instance(value, "State") {
        let state = class_ref::<crate::app::JsState>(value)?;
        return Ok(StateOrValue::State(state.inner.clone()));
    }
    binding_value_from_js(value)
        .map(StateOrValue::Value)
        .map_err(|_| invalid_arg("binding values must be a State, string, number, or boolean"))
}

/// A `State | string | number | boolean` argument.
pub(crate) fn binding_text(value: &JsValue) -> Result<BindingText> {
    Ok(match state_or_value(value)? {
        StateOrValue::State(state) => BindingText::State(state),
        StateOrValue::Value(value) => BindingText::Static(value.as_label_text()),
    })
}

/// A `State | boolean | number` argument.
pub(crate) fn binding_bool(value: &JsValue) -> Result<BindingBool> {
    match state_or_value(value)? {
        StateOrValue::State(state) => Ok(BindingBool::State(state)),
        StateOrValue::Value(BindingValue::Bool(value)) => Ok(BindingBool::Static(value)),
        StateOrValue::Value(BindingValue::Number(value)) => Ok(BindingBool::Static(value != 0.0)),
        StateOrValue::Value(BindingValue::Integer(value)) => Ok(BindingBool::Static(value != 0)),
        StateOrValue::Value(BindingValue::String(_)) => Err(invalid_arg(
            "boolean bindings must be a State, boolean, or number",
        )),
    }
}

/// A `State | number | boolean` argument.
pub(crate) fn binding_number(value: &JsValue) -> Result<BindingNumber> {
    match state_or_value(value)? {
        StateOrValue::State(state) => Ok(BindingNumber::State(state)),
        StateOrValue::Value(BindingValue::Number(value)) => Ok(BindingNumber::Static(value)),
        StateOrValue::Value(BindingValue::Integer(value)) => {
            Ok(BindingNumber::Static(value as f64))
        }
        StateOrValue::Value(BindingValue::Bool(value)) => {
            Ok(BindingNumber::Static(if value { 1.0 } else { 0.0 }))
        }
        StateOrValue::Value(BindingValue::String(_)) => Err(invalid_arg(
            "number bindings must be a State, number, or boolean",
        )),
    }
}
