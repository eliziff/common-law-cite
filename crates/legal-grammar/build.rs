use flate2::{write::GzEncoder, Compression};
use serde_json::Value;
use std::io::Write;
use std::{env, fs, path::PathBuf};

#[allow(dead_code)]
#[path = "src/table.rs"]
mod table;

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let source = "data/grammar-corpus.json";
    println!("cargo:rerun-if-changed={source}");
    let mut compressed = GzEncoder::new(Vec::new(), Compression::best());
    compressed.write_all(&fs::read(source).expect("grammar source")).expect("compress grammar");
    fs::write(output.join("grammar-corpus.json.gz"), compressed.finish().expect("finish grammar"))
        .expect("embedded grammar");

    if env::var_os("CARGO_FEATURE_US").is_some() {
        us_extractors(&output);
    }
}

/// The compiled US extractors src/us_extractors.rs reads.
fn us_extractors(output: &std::path::Path) {
    let source = "data/us-extractors.json";
    println!("cargo:rerun-if-changed={source}");
    let data: Value = serde_json::from_slice(&fs::read(source).expect("US extractors")).expect("valid US extractors");
    let mut strings = table::StringsWriter::default();
    let (mut extractors, mut pool, mut editions) = (Vec::new(), Vec::new(), Vec::new());
    let run = |values: Vec<u32>, pool: &mut Vec<u32>| {
        let start = pool.len() as u32;
        let len = values.len() as u32;
        pool.extend(values);
        [start, len]
    };
    for extractor in data["extractors"].as_array().unwrap() {
        let indices = |field: &str| extractor[field].as_array().unwrap().iter()
            .map(|index| u32::try_from(index.as_u64().unwrap()).unwrap()).collect::<Vec<_>>();
        let flag = |field: &str, bit: u32| if extractor.get(field).and_then(Value::as_bool).unwrap_or(false) { bit } else { 0 };
        extractors.push(strings.id(extractor["pattern"].as_str().unwrap()));
        extractors.extend(run(indices("exact"), &mut pool));
        extractors.extend(run(indices("variations"), &mut pool));
        let names = extractor["strings"].as_array().unwrap().iter().map(|name| strings.id(name.as_str().unwrap())).collect();
        extractors.extend(run(names, &mut pool));
        extractors.push(flag("short", 1) | flag("standard", 2) | flag("ecmascript", 4));
    }
    for edition in data["editions"].as_array().unwrap() {
        let reporter = &edition["reporter"];
        for value in [&edition["short_name"], &reporter["short_name"], &reporter["name"], &reporter["cite_type"], &reporter["source"]] {
            editions.push(strings.id(value.as_str().unwrap()));
        }
        for date in [&edition["start"], &edition["end"]] {
            editions.push(date.as_str().map_or(u32::MAX, |date| strings.id(date)));
        }
    }
    let [offsets, arena] = strings.sections();
    fs::write(output.join("us-extractors.bin"),
        table::blob(&[offsets, arena, table::words(extractors), table::words(pool), table::words(editions)]))
        .expect("compiled US extractors");
}
