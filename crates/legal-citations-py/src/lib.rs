//! The `legal_citations._native` extension module.
//!
//! Deliberately one function: every method goes through
//! [`legal_citations::api::call`], so the Python package (`python/legal_citations`)
//! stays a thin, pure-Python layer over the same JSON surface as the CLI and
//! the npm package.

use pyo3::prelude::*;

/// `call(method, request_json) -> (ok, response_json)`.
///
/// On success `ok` is `True` and the second item is the response JSON; on
/// failure it is `False` and the second item is `{"code", "message"}`. The GIL
/// is released while the engine runs, so threads extract in parallel.
#[pyfunction]
fn call(py: Python<'_>, method: &str, request_json: &str) -> (bool, String) {
    let (method, request_json) = (method.to_owned(), request_json.to_owned());
    py.detach(
        move || match legal_citations::api::call(&method, &request_json) {
            Ok(response) => (true, response),
            Err(error) => (false, error.to_json()),
        },
    )
}

#[pymodule]
#[pyo3(name = "_native")]
fn native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(call, module)?)?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
