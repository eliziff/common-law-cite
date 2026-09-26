//! WebAssembly entry point for the npm package `legal-citations`.
//!
//! One export, [`call`], over the shared JSON API; the hand-written ESM
//! wrapper in `js/` turns it into `extract()`, `key()`, `format()`, ... and
//! defaults offsets to UTF-16 code units so spans index JavaScript strings.
//! Passing JSON strings instead of JS objects keeps the module free of
//! serde-wasm-bindgen and the glue code small.

use wasm_bindgen::prelude::*;

/// Dispatch one API call. Returns the response JSON; throws an `Error` whose
/// message is the `{"code", "message"}` error JSON.
#[wasm_bindgen]
pub fn call(method: &str, request_json: &str) -> Result<String, JsError> {
    legal_citations::api::call(method, request_json).map_err(|error| JsError::new(&error.to_json()))
}

/// The crate version this module was built from.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
