//! Reads every positive vector of an old grammar corpus with today's engine and prints the ones no citation it
//! extracts covers, by entry: what an earlier grammar read that the engine no longer does.
//!
//! cargo run --example old_vectors -- <grammar-corpus.json>...
use std::collections::BTreeMap;

fn main() {
    let options = legal_citations::Options::default();
    let mut lost: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let (mut total, mut seen) = (0, std::collections::BTreeSet::new());
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).expect("corpus");
        let corpus: serde_json::Value = serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes)).expect("json");
        for table in corpus["tables"].as_object().expect("tables").values() {
            for entry in table["entries"].as_array().into_iter().flatten() {
                let id = entry["id"].as_str().unwrap_or_default();
                if !id.starts_with("cite.") { continue; }
                for vector in entry["vectors"].as_array().into_iter().flatten() {
                    let input = vector["input"].as_str().unwrap_or_default();
                    if vector["groups"].is_null() || !seen.insert((id.to_owned(), input.to_owned())) { continue; }
                    total += 1;
                    if legal_citations::extract(input, &options).is_empty() {
                        lost.entry(id.to_owned()).or_default().push(input.to_owned());
                    }
                }
            }
        }
    }
    println!("positive vectors: {total}; read by no citation today: {}", lost.values().map(Vec::len).sum::<usize>());
    for (id, inputs) in &lost {
        println!("{id} ({}):", inputs.len());
        for input in inputs { println!("    {input:?}"); }
    }
}
