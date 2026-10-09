//! JavaScript functions as binding-core actions.
//!
//! Each helper takes an optional callback, as factories receive them, and
//! converts the action's Rust arguments the same way the Node addon does:
//! ids become strings (they may exceed JavaScript's safe integers), indices
//! become numbers, and colors become `Color` objects.

use crate::prelude::*;

/// Call `callback` with `args`; a thrown exception becomes a callback
/// failure for the binding core.
pub(crate) fn call_js(callback: &Function, args: &[JsValue]) -> ForeignCallbackResult<JsValue> {
    let args = args.iter().collect::<Array>();
    callback
        .apply(&JsValue::UNDEFINED, &args)
        .map_err(callback_failure)
}

/// Wrap an optional callback into an action with `make`.
fn wrap<A>(callback: Option<Function>, make: impl FnOnce(Local<Function>) -> A) -> Option<A> {
    callback.map(|callback| make(Local(callback)))
}

pub(crate) fn js_action(callback: Option<Function>) -> Option<BindingAction> {
    wrap(callback, |callback| {
        BindingAction::new(move || call_js(&callback, &[]).map(drop))
    })
}

pub(crate) fn js_bool_action(callback: Option<Function>) -> Option<BindingBoolAction> {
    wrap(callback, |callback| {
        BindingBoolAction::new(move |value| call_js(&callback, &[value.into()]).map(drop))
    })
}

pub(crate) fn js_number_action(callback: Option<Function>) -> Option<BindingNumberAction> {
    wrap(callback, |callback| {
        BindingNumberAction::new(move |value| call_js(&callback, &[value.into()]).map(drop))
    })
}

pub(crate) fn js_id_action(callback: Option<Function>) -> Option<BindingIdAction> {
    wrap(callback, |callback| {
        BindingIdAction::new(move |id| call_js(&callback, &[id.to_string().into()]).map(drop))
    })
}

pub(crate) fn js_string_action(callback: Option<Function>) -> Option<BindingStringAction> {
    wrap(callback, |callback| {
        BindingStringAction::new(move |value| call_js(&callback, &[value.into()]).map(drop))
    })
}

pub(crate) fn js_strings_action(callback: Option<Function>) -> Option<BindingStringsAction> {
    wrap(callback, |callback| {
        BindingStringsAction::new(move |values| {
            let values = values.into_iter().map(JsValue::from).collect::<Array>();
            call_js(&callback, &[values.into()]).map(drop)
        })
    })
}

pub(crate) fn js_select_action(callback: Option<Function>) -> Option<BindingSelectAction> {
    wrap(callback, |callback| {
        BindingSelectAction::new(move |index, value| {
            call_js(&callback, &[(index as u32).into(), value.into()]).map(drop)
        })
    })
}

pub(crate) fn js_color_action(callback: Option<Function>) -> Option<BindingColorAction> {
    wrap(callback, |callback| {
        BindingColorAction::new(move |color| {
            call_js(&callback, &[JsColor::from(color).into()]).map(drop)
        })
    })
}

pub(crate) fn js_color_select_action(
    callback: Option<Function>,
) -> Option<BindingColorSelectAction> {
    wrap(callback, |callback| {
        BindingColorSelectAction::new(move |index, name, color| {
            call_js(
                &callback,
                &[
                    (index as u32).into(),
                    name.into(),
                    JsColor::from(color).into(),
                ],
            )
            .map(drop)
        })
    })
}

pub(crate) fn js_reorder_action(callback: Option<Function>) -> Option<BindingReorderAction> {
    wrap(callback, |callback| {
        BindingReorderAction::new(move |item, from, to| {
            call_js(
                &callback,
                &[
                    (item as u32).into(),
                    (from as u32).into(),
                    (to as u32).into(),
                ],
            )
            .map(drop)
        })
    })
}

pub(crate) fn js_id_number_action(callback: Option<Function>) -> Option<BindingIdNumberAction> {
    wrap(callback, |callback| {
        BindingIdNumberAction::new(move |id, value| {
            call_js(&callback, &[id.to_string().into(), value.into()]).map(drop)
        })
    })
}

pub(crate) fn js_message_action(callback: Function) -> BindingMessageAction {
    let callback = Local(callback);
    BindingMessageAction::new(move |payload| {
        call_js(&callback, &[binding_value_to_js(payload)]).map(drop)
    })
}
