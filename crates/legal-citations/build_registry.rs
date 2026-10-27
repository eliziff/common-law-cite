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
fn strings(value: &Value) -> Vec<String> {
    value.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()
}
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
/// Whether the crate is built with the jurisdiction `jurisdiction` (`ca-on`, `us`, `uk-ew`) belongs to:
/// its feature (`canada`, `us`, `uk`, ...). International and unplaced rows are always kept.
fn enabled(jurisdiction: &str) -> bool {
    let feature = match jurisdiction.split('-').next().unwrap_or("") {
        "ca" => "CANADA", "us" => "US", "uk" => "UK", "au" => "AUSTRALIA", "nz" => "NEW_ZEALAND",
        "ie" => "IRELAND", "hk" => "HONG_KONG", "sg" => "SINGAPORE", "za" => "SOUTH_AFRICA",
        _ => return true,
    };
    std::env::var_os(format!("CARGO_FEATURE_{feature}")).is_some()
}

/// The jurisdiction a registry row belongs to: its own, else `us` for a reporters-db record.
fn row_jurisdiction(row: &Value) -> &str {
    match string(&row["jurisdiction"]) {
        "" if string(&row["source"]) == "reporters-db" => "us",
        "" => string(&row["region"]),
        jurisdiction => jurisdiction,
    }
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

/// The years `registry/journal-years.json` gives journals (their run and the years volumes appeared, each with its
/// source), on every journal row written as its key is: the journal a citation key names, folded.
fn journal_years(rows: &mut [Value]) {
    let years = read("registry/journal-years.json");
    let years = years.as_object().unwrap();
    let fold = |text: &str| text.chars().filter(|character| character.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
    for row in rows {
        let names = std::iter::once(string(&row["abbreviation"]).to_owned()).chain(strings(&row["variations"]));
        let Some(entry) = names.map(|name| fold(&name)).find_map(|name| years.get(&name)) else { continue };
        for bound in ["start", "end"] {
            if let Some(year) = entry[bound]["year"].as_u64() { row[bound] = json!(year); }
        }
        let anchors: Vec<Value> = entry["anchors"].as_array().into_iter().flatten().map(|anchor| {
            let mut indexed = json!({"volume": anchor["volume"], "year": anchor["year"]});
            if anchor["last_year"].is_number() { indexed["last_year"] = anchor["last_year"].clone(); }
            indexed
        }).collect();
        if !anchors.is_empty() { row["anchors"] = json!(anchors); }
    }
}

/// reporters-db's template variables (`regexes.json`, as reporters_db.utils.process_variables reads them) and the
/// authored ones beside them (`registry/regexes.json`): nested keys joined with `_`, notes (`#`) left out, each with
/// an `_optional` twin, then each expanded until it names no other. A standard page is the one the eyecite port reads
/// (`us_page`), as `tools/sync-upstream.py` gave its extractors.
fn template_variables() -> BTreeMap<String, String> {
    fn flatten(values: &Value, prefix: &str, into: &mut BTreeMap<String, String>) {
        for (key, value) in values.as_object().unwrap() {
            if key.ends_with('#') { continue; }
            let name = [prefix, key].iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join("_");
            match value {
                Value::Object(_) => flatten(value, &name, into),
                _ => { into.insert(name, string(value).to_owned()); }
            }
        }
    }
    let mut upstream = read("registry/upstream/reporters-db/regexes.json");
    let tables = legal_grammar::load_tables().expect("grammar corpus");
    upstream["page"][""] = json!(format!("(?P<page>{})", tables["cite.us.reporter.full"].defs["us_page"]));
    let mut variables = BTreeMap::new();
    flatten(&upstream, "", &mut variables);
    let mut authored = BTreeMap::new();
    flatten(&read("registry/regexes.json"), "", &mut authored);
    for (name, value) in authored {
        assert!(variables.insert(name.clone(), value).is_none(), "registry/regexes.json: {name} shadows reporters-db's");
    }
    let optional: Vec<_> = variables.iter().map(|(name, value)| (format!("{name}_optional"), format!("(?:{value} ?)?"))).collect();
    variables.extend(optional);
    let names: Vec<String> = variables.keys().cloned().collect();
    for name in names {
        let expanded = expand(&variables[&name], &variables);
        variables.insert(name, expanded);
    }
    variables
}

/// Python's `string.Template(value).safe_substitute(variables)`, repeated until nothing changes: `$name` (the
/// longest run of letters, digits and underscores, not starting with a digit) becomes its value when it has one.
fn expand(value: &str, variables: &BTreeMap<String, String>) -> String {
    let mut value = value.to_owned();
    for _ in 0..100 {
        let mut expanded = String::with_capacity(value.len());
        let mut rest = value.as_str();
        while let Some(at) = rest.find('$') {
            expanded.push_str(&rest[..at]);
            let after = &rest[at + 1..];
            if let Some(after) = after.strip_prefix('$') {
                expanded.push('$');
                rest = after;
                continue;
            }
            let length = after.bytes().enumerate()
                .take_while(|&(index, byte)| byte == b'_' || byte.is_ascii_alphabetic() || index > 0 && byte.is_ascii_digit())
                .count();
            match variables.get(&after[..length]).filter(|_| length > 0) {
                Some(variable) => expanded.push_str(variable),
                None => { expanded.push('$'); expanded.push_str(&after[..length]); }
            }
            rest = &after[length..];
        }
        expanded.push_str(rest);
        if expanded == value { return value; }
        value = expanded;
    }
    panic!("template exceeds reporters-db's recursion depth: {value}")
}

/// One named row: a registry row a citation names, its names (`exact`) and variations, and its templates.
#[allow(clippy::too_many_arguments)]
fn named_row(table: &str, id: Option<&Value>, edition: &str, source: &str, jurisdiction: &str, dates: &Value,
    variations: Vec<String>, templates: Vec<String>, examples: Vec<Value>) -> Value {
    let mut row = json!({"table": table, "edition": edition, "source": source, "jurisdiction": jurisdiction,
        "variations": variations, "templates": templates, "examples": examples});
    if let Some(id) = id.filter(|id| id.is_string()) { row["id"] = id.clone(); }
    for bound in ["start", "end"] {
        if let Some(date) = dates.get(bound).filter(|date| date.is_string()) { row[bound] = date.clone(); }
    }
    row
}

/// The named rows of an authored registry row. A row's own `regexes` (a reporter's on each edition) are its
/// templates; without them a reporter takes reporters-db's standard citation and, when its volume is a year, the
/// year-volume report, a court its neutral citation, and a law its kind's, Commonwealth legislation alone having one.
fn authored_named(table: &str, row: &Value, into: &mut Vec<Value>) {
    let jurisdiction = string(&row[if table == "courts" { "region" } else { "jurisdiction" }]);
    if !enabled(jurisdiction) { return; }
    let examples = row["examples"].as_array().cloned().unwrap_or_default();
    match table {
        "courts" => {
            let neutral = strings(&row["neutral"]);
            let Some((name, variations)) = neutral.split_first() else { return };
            let templates = row.get("regexes").map_or_else(||
                vec!["$commonwealth_neutral".into(), "$commonwealth_neutral_bracketed".into()], strings);
            into.push(named_row(table, row.get("id"), name, "authored", jurisdiction, &Value::Null,
                variations.to_vec(), templates, examples));
        }
        "reporters" => for (edition, record) in row["editions"].as_object().unwrap() {
            // A reporter numbered by year in any period is also cited by its year alone.
            let by_year = row["year_volume"].as_bool() == Some(true) || record["numbering"].as_array().into_iter().flatten()
                .any(|period| period["year_volume"].as_bool() == Some(true));
            let templates = match record.get("regexes") {
                Some(regexes) => strings(regexes),
                None if by_year => vec!["$full_cite".into(), "$commonwealth_report_year_volume".into()],
                None => vec!["$full_cite".into()],
            };
            let mut variations: Vec<String> = row["variations"].as_object().into_iter().flatten()
                .filter(|(_, canonical)| string(canonical) == edition).map(|(surface, _)| surface.clone()).collect();
            variations.sort();
            let examples = record["examples"].as_array().cloned().unwrap_or_else(|| examples.clone());
            into.push(named_row(table, row.get("citation_id"), edition, string(&row["source"]), jurisdiction, record,
                variations, templates, examples));
        },
        "series" => {
            let templates = match row.get("regexes") {
                Some(regexes) => strings(regexes),
                None if jurisdiction.starts_with("us") => Vec::new(),
                None => match string(&row["kind"]) {
                    "revised_statutes" | "annual_statutes" => vec!["$commonwealth_statute".into()],
                    "regulations" => vec!["$commonwealth_regulation".into()],
                    _ => Vec::new(),
                },
            };
            into.push(named_row("laws", row.get("citation_id"), string(&row["abbreviation"]), "authored", jurisdiction,
                &Value::Null, strings(&row["variations"]), templates, examples));
        }
        _ => {
            let templates = row.get("regexes").map_or_else(|| vec!["$full_cite".into()], strings);
            into.push(named_row(table, row.get("citation_id"), string(&row["abbreviation"]), string(&row["source"]), jurisdiction,
                &Value::Null, strings(&row["variations"]), templates, examples));
        }
    }
}

/// The named rows of a reporters-db record, each edition of a reporter its own, with its regexes or else the
/// standard citation, as eyecite reads them; a law's section mark may be doubled or closed up.
fn upstream_named(table: &str, key: &str, original: &Value, codes: &BTreeMap<String, String>, into: &mut Vec<Value>) {
    // A reporter's place is its region; a law's is the state its jurisdiction names, as for the series index.
    let jurisdiction = match table {
        "reporters" => match string(&original["region"]) { "" => "us".to_owned(), region => region.to_owned() },
        "series" => codes.get(&string(&original["jurisdiction"]).to_lowercase()).map_or_else(|| "us".into(), |code| format!("us-{code}")),
        _ => "us".to_owned(),
    };
    let jurisdiction = jurisdiction.as_str();
    if !enabled(jurisdiction) { return; }
    let examples = original["examples"].as_array().cloned().unwrap_or_default();
    let templates = |record: &Value| {
        let templates = strings(&record["regexes"]);
        let templates = if templates.is_empty() { vec!["$full_cite".to_owned()] } else { templates };
        templates.into_iter().map(|template| if table == "series" { template.replace("§ ", "§§? ?") } else { template }).collect()
    };
    let named_table = if table == "series" { "laws" } else { table };
    if table == "reporters" {
        for (edition, record) in original["editions"].as_object().unwrap() {
            let mut variations: Vec<String> = original["variations"].as_object().into_iter().flatten()
                .filter(|(_, canonical)| string(canonical) == edition).map(|(surface, _)| surface.clone()).collect();
            variations.sort();
            into.push(named_row(named_table, original.get("citation_id"), edition, "reporters-db", jurisdiction, record,
                variations, templates(record), examples.clone()));
        }
    } else {
        into.push(named_row(named_table, original.get("citation_id"), key, "reporters-db", jurisdiction, original,
            strings(&original["variations"]), templates(original), examples));
    }
}

/// The named rows src/named.rs reads: each row's templates expanded and shared (`shapes`), the row naming them by
/// position.
fn compile_named(out: &Path, rows: Vec<Value>) {
    let variables = template_variables();
    let mut shapes: Vec<String> = Vec::new();
    let mut positions: BTreeMap<String, usize> = BTreeMap::new();
    let rows: Vec<Value> = rows.into_iter().map(|mut row| {
        let templates = strings(&row["templates"]);
        let indices: Vec<usize> = templates.iter().map(|template| {
            let shape = expand(template, &variables);
            assert!(!shape.contains('$') || shape.contains("(?P<reporter>$edition)"), "template {template} names an unknown variable: {shape}");
            *positions.entry(shape.clone()).or_insert_with(|| { shapes.push(shape); shapes.len() - 1 })
        }).collect();
        let object = row.as_object_mut().unwrap();
        object.remove("templates");
        object.insert("shapes".into(), json!(indices));
        row
    }).collect();
    let conventions = json!({"year_before": expand("$commonwealth_year_before", &variables),
        "issue": expand("$commonwealth_issue", &variables), "volume": expand("$volume", &variables)});
    super::compress(out.join("named.json.gz"), &json!({"shapes": shapes, "rows": rows, "conventions": conventions}));
}

pub fn compile(out: &Path) {
    let mut named = Vec::new();
    let states = read("registry/upstream/courts-db/states.json");
    let mut codes: BTreeMap<String, String> = states.as_object().unwrap().iter()
        .filter(|(code, _)| code.len() == 2).map(|(code, name)| (string(name).to_lowercase(), code.to_lowercase())).collect();
    codes.insert("washington d.c.".into(), "dc".into());
    codes.insert("district of columbia".into(), "dc".into());
    let mut jurisdictions = read("registry/jurisdictions.json");
    jurisdictions.as_array_mut().unwrap().retain(|row| enabled(string(&row["id"])));
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
            for court in authored { authored_named(table, court, &mut named); }
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
            all.retain(|court| enabled(row_jurisdiction(court)));
            source[file] = json!(all);
        } else {
            let upstream = read(&format!("registry/upstream/reporters-db/{file}.json"));
            // Edition years reporters-db gets wrong, each with the page that states it, by its key and edition.
            let corrected = read("registry/edition-years.json");
            let mut all = Map::new();
            for row in authored { authored_named(table, row, &mut named); }
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
                            for bound in ["start", "end"] {
                                let value = corrected[key][abbreviation][bound]["year"].as_u64().and_then(|year| u16::try_from(year).ok()).or_else(|| year(&dates[bound]));
                                if let Some(value) = value { edition[bound] = json!(value); }
                            }
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
                    upstream_named(table, key, &original, &codes, &mut named);
                    all.entry(key).or_insert_with(|| json!([])).as_array_mut().unwrap().push(original);
                }
            }
            // An upstream (reporters-db) record is US unless its own region says otherwise.
            for entries in all.values_mut() {
                entries.as_array_mut().unwrap().retain(|row| enabled(match row_jurisdiction(row) {
                    "" if row.get("cite_type").is_some() => "us",
                    jurisdiction => jurisdiction,
                }));
            }
            all.retain(|_, entries| !entries.as_array().unwrap().is_empty());
            source[file] = Value::Object(all);
        }
        let fields: &[&str] = match table {
            "courts" => &["id", "name", "jurisdiction", "level", "neutral", "aliases", "citation_string", "source_dates", "canlii", "canlii_fr", "canlii_french", "canlii_codes", "canlii_codes_fr", "start", "end", "parent"],
            "reporters" => &["id", "name", "kind", "jurisdiction", "courts", "default_court", "editions", "variations", "year_volume", "source", "verified"],
            "series" => &["id", "name", "kind", "jurisdiction", "abbreviation", "variations", "language", "canlii"],
            "journals" => &["id", "name", "abbreviation", "variations", "jurisdiction", "source", "verified", "start", "end", "anchors"],
            _ => unreachable!(),
        };
        rows.retain(|row| enabled(row_jurisdiction(row)));
        if table == "journals" { journal_years(&mut rows); }
        for row in &mut rows { row.as_object_mut().unwrap().retain(|key, _| fields.contains(&key.as_str())); }
        index[table] = json!(rows);
    }
    super::compress(out.join("registry-index.json.gz"), &index);
    compile_named(out, named);
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
