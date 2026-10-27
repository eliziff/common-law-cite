//! `conformance/schema/citation.schema.json` must describe what `extract`
//! actually returns: every response for every conformance input validates
//! (`additionalProperties: false` catches a model field the schema lacks), and
//! every enum value the schema lists deserializes into the model.
//!
//! The validator covers the schema subset the file uses: `$ref` to `$defs`,
//! `type`, `enum`, `properties`, `required`, `additionalProperties: false`,
//! `items`, `minimum`.

use legal_citations::api;
use legal_citations::{Authority, Form, Format, OffsetUnit, ParentheticalKind, PinpointKind};
use serde_json::{json, Value};
use std::path::Path;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn schema() -> Value {
    let path = root().join("conformance/schema/citation.schema.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn resolve<'a>(schema: &'a Value, node: &'a Value) -> &'a Value {
    match node.get("$ref").and_then(Value::as_str) {
        Some(reference) => {
            let name = reference.strip_prefix("#/$defs/").expect("local $defs ref");
            resolve(schema, &schema["$defs"][name])
        }
        None => node,
    }
}

fn type_matches(expected: &str, value: &Value) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.is_u64() || value.is_i64(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        other => panic!("schema type {other} not supported by this validator"),
    }
}

fn validate(schema: &Value, node: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    let node = resolve(schema, node);
    if let Some(expected) = node.get("type").and_then(Value::as_str) {
        if !type_matches(expected, value) {
            errors.push(format!("{path}: expected {expected}, got {value}"));
            return;
        }
    }
    if let Some(choices) = node.get("enum").and_then(Value::as_array) {
        if !choices.contains(value) {
            errors.push(format!(
                "{path}: {value} is not in the schema enum {choices:?}"
            ));
        }
    }
    if let (Some(minimum), Some(number)) =
        (node.get("minimum").and_then(Value::as_f64), value.as_f64())
    {
        if number < minimum {
            errors.push(format!("{path}: {number} < {minimum}"));
        }
    }
    if let Some(object) = value.as_object() {
        let properties = node.get("properties").and_then(Value::as_object);
        for required in node
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if !object.contains_key(required.as_str().unwrap()) {
                errors.push(format!("{path}: missing required {required}"));
            }
        }
        for (key, item) in object {
            match properties.and_then(|properties| properties.get(key)) {
                Some(child) => validate(schema, child, item, &format!("{path}.{key}"), errors),
                None if node.get("additionalProperties") == Some(&Value::Bool(false)) => {
                    errors.push(format!("{path}: property {key:?} is not in the schema"))
                }
                None => {}
            }
        }
    }
    if let (Some(items), Some(array)) = (node.get("items"), value.as_array()) {
        for (index, item) in array.iter().enumerate() {
            validate(schema, items, item, &format!("{path}[{index}]"), errors);
        }
    }
}

fn conformance_inputs() -> Vec<(String, Value)> {
    let mut inputs = Vec::new();
    let mut files = std::fs::read_dir(root().join("conformance/cases"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    files.sort();
    for file in files {
        let document: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        for case in document["cases"].as_array().unwrap() {
            if case.get("method").is_some_and(|method| method != "extract")
                || case.get("input").is_none()
            {
                continue;
            }
            let mut options = case.get("options").cloned().unwrap_or_else(|| json!({}));
            let unit = options
                .as_object_mut()
                .and_then(|options| options.remove("offsetUnit"))
                .unwrap_or_else(|| json!("char"));
            inputs.push((
                format!(
                    "{}::{}",
                    file.file_stem().unwrap().to_string_lossy(),
                    case["name"].as_str().unwrap()
                ),
                json!({"text": case["input"], "options": options, "offsetUnit": unit}),
            ));
        }
    }
    inputs
}

#[test]
fn extract_output_matches_the_schema() {
    let schema = schema();
    let mut errors = Vec::new();
    let inputs = conformance_inputs();
    assert!(!inputs.is_empty());
    for (label, request) in inputs {
        let response = api::call_value("extract", request).unwrap();
        validate(&schema, &schema, &response, &label, &mut errors);
    }
    errors.truncate(40);
    assert!(errors.is_empty(), "schema drift:\n{}", errors.join("\n"));
}

#[test]
fn schema_enums_exist_in_the_model() {
    let schema = schema();
    let defs = &schema["$defs"];
    fn each<T: for<'de> serde::Deserialize<'de>>(values: &Value, name: &str) {
        for value in values.as_array().unwrap() {
            serde_json::from_value::<T>(value.clone()).unwrap_or_else(|error| {
                panic!("schema {name} value {value} is not in the model: {error}")
            });
        }
    }
    each::<Form>(&defs["form"]["enum"], "form");
    each::<Authority>(&defs["authority"]["enum"], "authority");
    each::<Format>(&defs["format"]["enum"], "format");
    each::<PinpointKind>(&defs["pinpointKind"]["enum"], "pinpointKind");
    each::<ParentheticalKind>(
        &defs["parenthetical"]["properties"]["kind"]["enum"],
        "parenthetical kind",
    );
    each::<OffsetUnit>(&defs["offsetUnit"]["enum"], "offsetUnit");
    assert_eq!(
        schema["properties"]["schemaVersion"]["enum"][0],
        api::SCHEMA_VERSION
    );
}
