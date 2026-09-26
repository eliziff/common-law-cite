//! Runs every case in `conformance/cases/*.json` through `api::call_value`.
//!
//! A `pass` case must match; a `pending` case must *not* match, so the pending
//! list only ever shrinks: when the engine starts satisfying one, flip it to
//! `pass` (`python conformance/run.py --cli <binary> --promote` does that).
//!
//! `CONFORMANCE_FILTER=<substring>` limits the run to matching file or case
//! names; `CONFORMANCE_VERBOSE=1` prints the first mismatch of every case.
//!
//! The matcher semantics are shared with `conformance/run.py` and
//! `conformance/run.mjs`; see `conformance/README.md`.

use legal_citations::api;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn cases_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases")
}

/// The API request a case describes.
fn request(case: &Value) -> (String, Value) {
    let method = case
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("extract")
        .to_owned();
    if let Some(request) = case.get("request") {
        return (method, request.clone());
    }
    let mut options = case
        .get("options")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let unit = options
        .remove("offsetUnit")
        .unwrap_or_else(|| json!("char"));
    let request = json!({
        "text": case["input"],
        "options": Value::Object(options),
        "offsetUnit": unit,
    });
    (method, request)
}

fn same_scalar(expected: &Value, actual: &Value) -> bool {
    match (expected.as_f64(), actual.as_f64()) {
        (Some(left), Some(right)) if expected.is_number() && actual.is_number() => left == right,
        _ => expected == actual,
    }
}

/// `None` when `actual` satisfies `expected`, else the first mismatch.
fn mismatch(expected: &Value, actual: Option<&Value>, path: &str) -> Option<String> {
    let actual_or_null = actual.unwrap_or(&Value::Null);
    match expected {
        Value::Null => (!actual_or_null.is_null())
            .then(|| format!("{path}: expected absent, got {actual_or_null}")),
        Value::Object(object) if object.len() == 1 && object.contains_key("$contains") => {
            let Value::Array(wanted) = &object["$contains"] else {
                return Some(format!("{path}: $contains takes an array"));
            };
            let Some(items) = actual_or_null.as_array() else {
                return Some(format!("{path}: expected an array, got {actual_or_null}"));
            };
            let mut cursor = 0;
            for (position, item) in wanted.iter().enumerate() {
                let found = items[cursor..]
                    .iter()
                    .position(|candidate| mismatch(item, Some(candidate), path).is_none());
                match found {
                    Some(offset) => cursor += offset + 1,
                    None => {
                        return Some(format!(
                            "{path}: no element (in order) matches $contains[{position}] = {item}"
                        ))
                    }
                }
            }
            None
        }
        Value::Object(object) if object.len() == 1 && object.contains_key("$in") => {
            let Value::Array(choices) = &object["$in"] else {
                return Some(format!("{path}: $in takes an array"));
            };
            (!choices
                .iter()
                .any(|choice| mismatch(choice, actual, path).is_none()))
            .then(|| format!("{path}: {actual_or_null} is not one of {}", object["$in"]))
        }
        Value::Object(object) => {
            let Some(actual) = actual_or_null.as_object() else {
                return Some(format!("{path}: expected an object, got {actual_or_null}"));
            };
            object
                .iter()
                .find_map(|(key, value)| mismatch(value, actual.get(key), &format!("{path}.{key}")))
        }
        Value::Array(items) => {
            let Some(actual) = actual_or_null.as_array() else {
                return Some(format!("{path}: expected an array, got {actual_or_null}"));
            };
            if actual.len() != items.len() {
                return Some(format!(
                    "{path}: expected {} elements, got {}",
                    items.len(),
                    actual.len()
                ));
            }
            items
                .iter()
                .zip(actual)
                .enumerate()
                .find_map(|(index, (item, actual))| {
                    mismatch(item, Some(actual), &format!("{path}[{index}]"))
                })
        }
        scalar => (!same_scalar(scalar, actual_or_null))
            .then(|| format!("{path}: expected {scalar}, got {actual_or_null}")),
    }
}

#[derive(Default)]
struct Tally {
    pass: usize,
    pending: usize,
}

#[test]
fn conformance() {
    let filter = std::env::var("CONFORMANCE_FILTER").ok();
    let verbose = std::env::var_os("CONFORMANCE_VERBOSE").is_some();
    let mut files = std::fs::read_dir(cases_dir())
        .expect("conformance/cases exists")
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    files.sort();
    assert!(!files.is_empty(), "no conformance case files");

    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    let mut failures = Vec::new();
    for file in files {
        let name = file.file_stem().unwrap().to_string_lossy().into_owned();
        let document: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
        assert_eq!(
            document["format"], "legal-citations-conformance:v1",
            "{name}: unknown case file format"
        );
        let tally = tallies.entry(name.clone()).or_default();
        for case in document["cases"].as_array().expect("cases array") {
            let case_name = case["name"].as_str().expect("case name");
            let label = format!("{name}::{case_name}");
            if filter
                .as_deref()
                .is_some_and(|filter| !label.contains(filter))
            {
                continue;
            }
            let (method, request) = request(case);
            let actual = match api::call_value(&method, request) {
                Ok(value) => value,
                Err(error) => {
                    let mut object = Map::new();
                    object.insert("error".into(), serde_json::to_value(&error).unwrap());
                    Value::Object(object)
                }
            };
            let problem = mismatch(&case["expect"], Some(&actual), "$");
            match case["status"].as_str() {
                Some("pass") => {
                    tally.pass += 1;
                    if let Some(problem) = problem {
                        failures.push(format!("REGRESSION {label}: {problem}"));
                    }
                }
                Some("pending") => {
                    tally.pending += 1;
                    match problem {
                        None => failures.push(format!(
                            "NOW PASSING {label}: set \"status\": \"pass\" (conformance/run.py --promote)"
                        )),
                        Some(problem) if verbose => eprintln!("pending {label}: {problem}"),
                        Some(_) => {}
                    }
                }
                other => failures.push(format!(
                    "{label}: status must be pass or pending, got {other:?}"
                )),
            }
        }
    }

    eprintln!("conformance (Rust api):");
    let (mut pass, mut pending) = (0, 0);
    for (name, tally) in &tallies {
        eprintln!(
            "  {name:<24} pass {:>4}  pending {:>4}",
            tally.pass, tally.pending
        );
        pass += tally.pass;
        pending += tally.pending;
    }
    eprintln!("  {:<24} pass {pass:>4}  pending {pending:>4}", "total");
    assert!(
        failures.is_empty(),
        "{} conformance failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn matcher_semantics() {
    let actual = json!({"citations": [{"form": "full", "pinpoints": [{"first": "1"}, {"first": "2"}]}, {"form": "ibid"}]});
    assert!(mismatch(
        &json!({"citations": [{"form": "full"}, {"form": "ibid"}]}),
        Some(&actual),
        "$"
    )
    .is_none());
    assert!(mismatch(
        &json!({"citations": [{"form": "full"}]}),
        Some(&actual),
        "$"
    )
    .is_some());
    assert!(mismatch(
        &json!({"citations": {"$contains": [{"form": "ibid"}]}}),
        Some(&actual),
        "$"
    )
    .is_none());
    assert!(mismatch(
        &json!({"citations": {"$contains": [{"form": "ibid"}, {"form": "full"}]}}),
        Some(&actual),
        "$"
    )
    .is_some());
    assert!(mismatch(
        &json!({"citations": [{"antecedent": null}, {"form": {"$in": ["supra", "ibid"]}}]}),
        Some(&actual),
        "$"
    )
    .is_none());
    assert!(mismatch(&json!({"missing": null}), Some(&actual), "$").is_none());
}
