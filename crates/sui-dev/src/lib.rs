mod app;

pub use app::{build_dev_application, build_dev_application_with_widget_book_bounds};

pub fn run_desktop() -> sui::Result<()> {
    build_dev_application().run()
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    run_desktop().map_err(|error| JsValue::from_str(&error.to_string()))
}
