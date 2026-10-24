//! Every citation in every extracted document of a folder, one line each: document, authority kind,
//! the citation's lookup key (as the A2AJ store indexes citations), its court, whether it carries a
//! neutral citation, and its text. For joining against a store offline.
//!
//!     cargo run --release --example corpus_keys -- INPUTS_DIR > KEYS.tsv
use legal_citations::{extract, Options};
use std::io::{Read, Write};

fn main() {
    let dir = std::env::args().nth(1).expect("INPUTS_DIR");
    let options = Options::default();
    let out = std::io::stdout();
    let mut out = std::io::BufWriter::new(out.lock());
    let mut entries: Vec<_> = std::fs::read_dir(&dir).expect("inputs dir").flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(sha) = name.strip_suffix(".json.gz") else { continue };
        let Ok(file) = std::fs::File::open(entry.path()) else { continue };
        let mut json = String::new();
        if flate2::read::GzDecoder::new(file).read_to_string(&mut json).is_err() { continue; }
        let Ok(doc) = serde_json::from_str::<serde_json::Value>(&json) else { continue };
        let mut text = String::new();
        for page in doc["pages"].as_array().into_iter().flatten() {
            for line in page["lines"].as_array().into_iter().flatten() {
                if let Some(t) = line["text"].as_str() { text.push_str(t); text.push('\n'); }
            }
        }
        for c in extract(&text, &options) {
            let key = legal_citations::key::key(&c).unwrap_or_default();
            let court = c.court.as_ref().map_or("-", |c| c.id.as_str());
            let cite = text[c.span.start..c.span.end].replace(['\t', '\n'], " ");
            let _ = writeln!(out, "{}\t{:?}\t{}\t{}\t{}", &sha[..12], c.authority, key, court, cite.trim());
        }
    }
}
