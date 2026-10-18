use flate2::{write::GzEncoder, Compression};
use legal_grammar::table::{blob, words, StringsWriter};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::{env, fs, path::PathBuf};
mod build_registry;

fn compress(path: PathBuf, value: &serde_json::Value) {
    let mut compressed = GzEncoder::new(Vec::new(), Compression::best());
    compressed.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
    fs::write(path, compressed.finish().unwrap()).unwrap();
}

/// The alias mappings src/aliases.rs reads: strings (sections 0 and 1), the key version (2), target rows (3)
/// by structural key and observed rows (4) by folded form, each of target key, citation, records (a run in
/// the record pool, 5: start and length) and court (`u32::MAX` for none), then the blocked forms (6), sorted.
/// A target's court is the one court every one of its records was observed in, when there is one.
fn compile_aliases(out: &std::path::Path) {
    println!("cargo:rerun-if-changed=registry/aliases.json");
    let source: serde_json::Value =
        serde_json::from_slice(&fs::read("registry/aliases.json").expect("alias evidence"))
            .expect("valid alias evidence");
    let text = |value: &serde_json::Value| value.as_str().expect("alias string").to_owned();
    let mut record_courts: HashMap<String, String> = HashMap::new();
    for evidence in source["observed"].as_object().unwrap().values() {
        for record in evidence["records"].as_array().unwrap() {
            let court = text(&evidence["court"]);
            let previous = record_courts.insert(text(record), court.clone());
            assert!(previous.is_none_or(|previous| previous == court), "record observed in two courts");
        }
    }
    let mut strings = StringsWriter::default();
    let mut pool = Vec::new();
    let mut rows = |table: &serde_json::Value, court: &dyn Fn(&serde_json::Value) -> Option<String>| {
        let table: BTreeMap<&String, &serde_json::Value> = table.as_object().unwrap().iter().collect();
        let mut rows = Vec::new();
        for (key, target) in table {
            let records: Vec<String> = target["records"].as_array().unwrap().iter().map(text).collect();
            rows.extend([strings.id(key), strings.id(&text(&target["key"])), strings.id(&text(&target["citation"])),
                pool.len() as u32, records.len() as u32]);
            pool.extend(records.iter().map(|record| strings.id(record)));
            rows.push(court(target).map_or(u32::MAX, |court| strings.id(&court)));
        }
        rows
    };
    let targets = rows(&source["targets"], &|target| {
        let courts: Option<std::collections::HashSet<&String>> = target["records"].as_array().unwrap().iter()
            .map(|record| record_courts.get(record.as_str().unwrap())).collect();
        courts.filter(|courts| courts.len() == 1).map(|courts| courts.into_iter().next().unwrap().clone())
    });
    let observed = rows(&source["observed"], &|evidence| Some(text(&evidence["court"])));
    let mut blocked: Vec<String> = source["blockedForms"].as_array().unwrap().iter().map(text).collect();
    blocked.sort();
    blocked.dedup();
    let blocked: Vec<u32> = blocked.iter().map(|form| strings.id(form)).collect();
    let version = strings.id(&text(&source["keyVersion"]));
    let [offsets, arena] = strings.sections();
    fs::write(out.join("aliases.bin"), blob(&[offsets, arena, words([version]), words(targets), words(observed),
        words(pool), words(blocked)])).expect("compiled alias mappings");
}

/// The statute titles src/reference_names.rs reads (registry/statute-titles.txt, tools/statute-titles.py), compressed.
fn compress_statute_titles(out: &std::path::Path) {
    println!("cargo:rerun-if-changed=registry/statute-titles.txt");
    let mut compressed = GzEncoder::new(Vec::new(), Compression::best());
    compressed.write_all(&fs::read("registry/statute-titles.txt").expect("statute titles")).unwrap();
    fs::write(out.join("statute-titles.txt.gz"), compressed.finish().unwrap()).unwrap();
}

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    build_registry::compile(&out);
    compile_aliases(&out);
    compress_statute_titles(&out);
}
