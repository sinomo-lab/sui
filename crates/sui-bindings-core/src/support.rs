use crate::errors::{
    ForeignCallbackError, ForeignCallbackPhase, ForeignCallbackResult, ForeignErrorSink,
    ForeignWidgetId,
};
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::sync::Mutex;
use sui::InvalidationKind;
use sui::InvalidationRequest;
use sui::InvalidationTarget;
use sui::WidgetId;

pub(crate) fn run_foreign_callback<T>(
    id: ForeignWidgetId,
    errors: &ForeignErrorSink,
    phase: ForeignCallbackPhase,
    fallback: T,
    callback: impl FnOnce() -> ForeignCallbackResult<T>,
) -> T {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            errors.push(ForeignCallbackError::new(id, phase, error.message));
            fallback
        }
        Err(payload) => {
            errors.push(ForeignCallbackError::new(id, phase, panic_message(payload)));
            fallback
        }
    }
}

pub(crate) fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return format!("foreign callback panicked: {message}");
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return format!("foreign callback panicked: {message}");
    }
    "foreign callback panicked with a non-string payload".to_string()
}

pub(crate) fn recover_lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn widget_invalidation(widget_id: WidgetId, kind: InvalidationKind) -> InvalidationRequest {
    InvalidationRequest::new(InvalidationTarget::Widget(widget_id), kind)
}
