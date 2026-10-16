//! Compile the source registry into the small records used by citation matching.
//! Source fields and keys remain intact in the separately embedded registry API.
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::{fs, path::Path};
use unicode_normalization::UnicodeNormalization;

fn read(path: &str) -> Value {
    println!("cargo:rerun-if-changed={path}");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn string(value: &Value) -> &str { value.as_str().unwrap_or("") }
fn year(value: &Value) -> Option<u16> { string(value).get(..4)?.parse().ok() }
fn slug(value: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for c in value.nfkd().filter(|c| !unicode_normalization::char::is_combining_mark(*c)) {
        if matches!(c, '.' | '\'' | '’') { continue; }
        if c.is_ascii_alphanumeric() {
            if separator && !result.is_empty() { result.push('-'); }
            result.push(c.to_ascii_lowercase());
            separator = false;
        } else { separator = true; }
    }
    if result.is_empty() { "x".into() } else { result }
}
fn unique(base: &str, taken: &mut BTreeSet<String>, suffix: &str) -> String {
    if taken.insert(base.into()) { return base.into(); }
    let base = if suffix.is_empty() { base.into() } else { format!("{base}-{suffix}") };
    if taken.insert(base.clone()) { return base; }
    for n in 2.. {
        let id = format!("{base}-{n}");
        if taken.insert(id.clone()) { return id; }
    }
    unreachable!()
}
fn region(values: &Value) -> String {
    // Preserve the engine's national scope for upstream reporters. Detailed
    // Juris-M jurisdictions remain available unchanged in the source record.
    let values: BTreeSet<String> = values.as_array().into_iter().flatten().map(|value| {
        let parts: Vec<_> = string(value).split(';').next().unwrap_or("").split(':').collect();
        match parts.as_slice() {
            ["gb" | "en", ..] => "uk-ew".into(),
            _ => "us".into(),
        }
    }).collect();
    if values.len() == 1 { values.into_iter().next().unwrap() } else { "us".into() }
}
fn names(row: &Value) -> Value {
    let mut result = json!({"en": row["name"]});
    if let Some(fr) = row.get("name_fr") { result["fr"] = fr.clone(); }
    result
}

fn court_hierarchy(court: &Value) -> &'static str {
    let level = string(&court["level"]);
    let kind = string(&court["type"]);
    if string(&court["id"]) == "scotus" || level == "colr" { "apex" }
    else if level == "iac" { "appellate" }
    else if level.starts_with("gjc") { "superior_trial" }
    else if level == "ljc" { "inferior_trial" }
    else if kind == "appellate" { "appellate" }
    else if kind.starts_with("trial") { "superior_trial" }
    else if kind == "bankruptcy" { "inferior_trial" } else { "tribunal" }
}

/// Authored extensions carry citation identity independently of the upstream key.
fn authored_index(table: &str, row: &Value) -> Value {
    let mut result = row.clone();
    let object = result.as_object_mut().unwrap();
    if table != "courts" {
        let id = object.remove("citation_id").unwrap();
        object.insert("id".into(), id);
    }
    if table != "journals" { result["name"] = names(row); }
    if table == "courts" {
        result["jurisdiction"] = row["region"].clone();
        result["level"] = json!(court_hierarchy(row));
    }
    if table == "reporters" {
        result["editions"] = Value::Array(row["editions"].as_object().unwrap().iter().map(|(key, edition)| {
            let mut edition = edition.clone();
            edition["abbreviation"] = json!(key);
            for bound in ["start", "end"] {
                if let Some(value) = year(&edition[bound]) { edition[bound] = json!(value); }
                else { edition.as_object_mut().unwrap().remove(bound); }
            }
            edition
        }).collect());
        let order = row["edition_order"].as_array().unwrap();
        result["editions"].as_array_mut().unwrap().sort_by_key(|edition|
            order.iter().position(|key| key == &edition["abbreviation"]).unwrap());
    }
    result
}

pub fn compile(out: &Path) {
    let states = read("registry/upstream/courts-db/states.json");
    let mut codes: BTreeMap<String, String> = states.as_object().unwrap().iter()
        .filter(|(code, _)| code.len() == 2).map(|(code, name)| (string(name).to_lowercase(), code.to_lowercase())).collect();
    codes.insert("washington d.c.".into(), "dc".into());
    codes.insert("district of columbia".into(), "dc".into());
    let jurisdictions = read("registry/jurisdictions.json");
    let mut index = json!({"jurisdictions": jurisdictions});
    let mut source = json!({"jurisdictions": jurisdictions, "states": states,
        "regexes": read("registry/upstream/reporters-db/regexes.json")});
    let mut source_positions = json!({});
    for (table, file) in [("courts", "courts"), ("reporters", "reporters"), ("series", "laws"), ("journals", "journals")] {
        let authored = read(&format!("registry/{file}.json"));
        let authored = authored.as_array().unwrap();
        let mut rows: Vec<Value> = authored.iter().map(|row| authored_index(table, row)).collect();
        let mut taken: BTreeSet<String> = rows.iter().map(|row| string(&row["id"]).to_owned()).collect();
        let authored_ids = taken.clone();
        if table == "courts" {
            let upstream = read("registry/upstream/courts-db/courts.json");
            let mut upstream = upstream.as_array().unwrap().clone();
            upstream.sort_by_key(|row| string(&row["id"]).to_owned());
            let mut all = authored.clone();
            for court in &mut all { court["hierarchy"] = json!(court_hierarchy(court)); }
            for court in upstream {
                let id = string(&court["id"]);
                assert!(taken.insert(id.into()), "duplicate court {id}");
                let hierarchy = court_hierarchy(&court);
                let location = string(&court["location"]).to_lowercase();
                let region = match string(&court["system"]) {
                    "federal" => "us".into(),
                    "international" => if location == "england" { "uk-ew".into() } else { "int".into() },
                    _ => codes.get(&location).map_or_else(|| "us".into(), |code| format!("us-{code}")),
                };
                let mut row = json!({"id": id, "name": {"en": court["name"]}, "jurisdiction": region, "level": hierarchy});
                let mut aliases = Vec::new();
                for field in ["citation_string", "name_abbreviation"] {
                    let alias = string(&court[field]);
                    if !alias.is_empty() && alias != string(&court["name"]) && !aliases.contains(&alias) { aliases.push(alias); }
                }
                row["aliases"] = json!(aliases);
                if !string(&court["citation_string"]).is_empty() { row["citation_string"] = court["citation_string"].clone(); }
                let dates = court["dates"].as_array();
                let bounds = |name: &str| -> Option<Vec<u16>> { dates.filter(|dates| !dates.is_empty())?.iter().map(|date| year(&date[name])).collect() };
                let start = bounds("start").and_then(|values| values.into_iter().min());
                let end = bounds("end").and_then(|values| values.into_iter().max());
                if start.zip(end).is_some_and(|(start, end)| start > end) { row["source_dates"] = court["dates"].clone(); }
                else {
                    if let Some(start) = start { row["start"] = json!(start); }
                    if let Some(end) = end { row["end"] = json!(end); }
                }
                if !string(&court["parent"]).is_empty() { row["parent"] = court["parent"].clone(); }
                rows.push(row);
                let mut court = court;
                court["region"] = json!(region);
                court["hierarchy"] = json!(hierarchy);
                all.push(court);
            }
            source[file] = json!(all);
        } else {
            let upstream = read(&format!("registry/upstream/reporters-db/{file}.json"));
            let mut all = Map::new();
            for row in authored {
                let key = string(&row["abbreviation"]);
                all.entry(key).or_insert_with(|| json!([])).as_array_mut().unwrap().push(row.clone());
            }
            for (key, entries) in upstream.as_object().unwrap() {
                let replacement = authored.iter().find(|row| row.get("upstream_key").and_then(Value::as_str) == Some(key));
                for original in entries.as_array().unwrap() {
                    let mut original = original.clone();
                    if let Some(replacement) = replacement {
                        // Preserve the original record even when corrected authored data wins matching.
                        original["citation_id"] = replacement["citation_id"].clone();
                    } else if table == "reporters" {
                        let place = region(&original["mlz_jurisdiction"]);
                        let mut base = slug(key);
                        if authored_ids.contains(&base) { base = format!("{base}-{}", place.split('-').next().unwrap()); }
                        let id = unique(&base, &mut taken, "");
                        let kind = if key == "U.S." { "official" } else { match string(&original["cite_type"]) {
                            "state" | "neutral" | "scotus_early" => "official", "federal" | "state_regional" => "general",
                            "specialty_west" | "specialty_lexis" => "database", _ => "specialty",
                        }};
                        let mut editions: Vec<Value> = original["editions"].as_object().unwrap().iter().map(|(abbreviation, dates)| {
                            let mut edition = json!({"abbreviation": abbreviation});
                            for bound in ["start", "end"] { if let Some(value) = year(&dates[bound]) { edition[bound] = json!(value); } }
                            edition
                        }).collect();
                        editions.sort_by_key(|edition| (edition["start"].as_u64().unwrap_or(0), string(&edition["abbreviation"]).to_owned()));
                        let mut row = json!({"id": id, "name": {"en": original["name"]}, "kind": kind,
                            "jurisdiction": place, "editions": editions, "year_volume": false, "source": "reporters-db"});
                        if original["variations"].is_object() { row["variations"] = original["variations"].clone(); }
                        let cite_type = string(&original["cite_type"]).to_lowercase();
                        if cite_type.contains("scotus") || (cite_type == "federal" && string(&original["name"]).to_lowercase().contains("supreme")) {
                            row["default_court"] = json!("scotus");
                        }
                        original["citation_id"] = json!(id);
                        original["kind"] = json!(kind);
                        original["region"] = json!(place);
                        rows.push(row);
                    } else {
                        let kind = match string(&original["cite_type"]) {
                            "leg_statute" => Some("code"), "leg_session" => Some("annual_statutes"),
                            "admin_compilation" | "admin_register" => Some("regulations"), _ => None,
                        };
                        if table == "journals" || kind.is_some() {
                            let id = unique(&slug(key), &mut taken, "us");
                            let mut row = json!({"id": id, "name": original["name"], "abbreviation": key});
                            let variations: BTreeSet<_> = original["variations"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                            row["variations"] = json!(variations);
                            if table == "journals" { row["source"] = json!("reporters-db"); }
                            else {
                                row["name"] = json!({"en": original["name"]});
                                row["kind"] = json!(kind.unwrap());
                                let place = string(&original["jurisdiction"]).to_lowercase();
                                row["jurisdiction"] = json!(codes.get(&place).map_or_else(|| "us".into(), |code| format!("us-{code}")));
                            }
                            original["citation_id"] = json!(id);
                            rows.push(row);
                        }
                    }
                    all.entry(key).or_insert_with(|| json!([])).as_array_mut().unwrap().push(original);
                }
            }
            source[file] = Value::Object(all);
        }
        let fields: &[&str] = match table {
            "courts" => &["id", "name", "jurisdiction", "level", "neutral", "aliases", "citation_string", "source_dates", "canlii", "canlii_fr", "canlii_french", "start", "end", "parent"],
            "reporters" => &["id", "name", "kind", "jurisdiction", "courts", "default_court", "editions", "variations", "year_volume", "source", "verified"],
            "series" => &["id", "name", "kind", "jurisdiction", "abbreviation", "variations", "language", "canlii"],
            "journals" => &["id", "name", "abbreviation", "variations", "jurisdiction", "source", "verified"],
            _ => unreachable!(),
        };
        for row in &mut rows { row.as_object_mut().unwrap().retain(|key, _| fields.contains(&key.as_str())); }
        index[table] = json!(rows);
    }
    super::compress(out.join("registry-index.json.gz"), &index);
    for table in ["reporters", "laws", "journals"] {
        let mut positions = Map::new();
        for (key, rows) in source[table].as_object_mut().unwrap() {
            for (position, row) in rows.as_array_mut().unwrap().iter_mut().enumerate() {
                if let Some(id) = row.as_object_mut().unwrap().remove("citation_id") {
                    positions.entry(string(&id)).or_insert_with(|| json!([key, position]));
                }
            }
        }
        source_positions[table] = Value::Object(positions);
    }
    super::compress(out.join("registry-source.json.gz"), &json!({"tables": source, "positions": source_positions}));
}
