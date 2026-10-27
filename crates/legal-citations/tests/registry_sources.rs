use legal_citations::{api, registry::source_registry};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[test]
fn every_upstream_field_is_preserved_in_the_public_registry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("registry/upstream");
    let source = source_registry();
    for table in ["reporters", "laws", "journals"] {
        let original: Value = serde_json::from_slice(&fs::read(root.join(format!("reporters-db/{table}.json"))).unwrap()).unwrap();
        for (key, records) in original.as_object().unwrap() {
            let published = source[table][key].as_array().unwrap();
            for record in records.as_array().unwrap() {
                assert!(published.iter().any(|candidate| record.as_object().unwrap().iter()
                    .all(|(field, value)| candidate.get(field) == Some(value))), "{table}/{key}: upstream fields changed");
            }
        }
    }
    let courts: Value = serde_json::from_slice(&fs::read(root.join("courts-db/courts.json")).unwrap()).unwrap();
    for court in courts.as_array().unwrap() {
        let published = source["courts"].as_array().unwrap().iter().find(|row| row["id"] == court["id"]).unwrap();
        for (field, value) in court.as_object().unwrap() {
            assert_eq!(published.get(field), Some(value), "court {}: {field}", court["id"]);
        }
    }
    for (table, file) in [("states", "courts-db/states.json"), ("regexes", "reporters-db/regexes.json")] {
        let original: Value = serde_json::from_slice(&fs::read(root.join(file)).unwrap()).unwrap();
        assert_eq!(source[table], original);
    }
}

#[test]
fn surface_lookups_return_source_records_with_optional_extensions() {
    let court = api::call_value("registry", json!({"table":"courts", "surface":"SCC"})).unwrap();
    assert_eq!(court[0]["name"], "Supreme Court of Canada");
    assert_eq!(court[0]["hierarchy"], "apex");
    let reporters = api::call_value("registry", json!({"table":"reporters", "surface":"U.S."})).unwrap();
    assert!(reporters.as_array().unwrap().iter().any(|row| row["reporter"]["editions"].get("U.S.").is_some()));
    let laws = api::call_value("registry", json!({"table":"laws", "surface":"RSC"})).unwrap();
    assert_eq!(laws[0]["abbreviation"], "RSC");
}
