//! Resolve a file of probe citations and print, per line, the key, the probe, the court the grammar
//! read, and the CanLII link the builder makes for it: evidence for the registry's coverage.
//!
//!     cargo run --release --example probe_courts -- PROBES.tsv > OUT.tsv
//!
//! PROBES.tsv: `key<TAB>text` per line (the key is carried through unchanged).
use legal_citations::format::Language;
use legal_citations::url::canlii_case;
use legal_citations::{extract, Options};

fn main() {
    let path = std::env::args().nth(1).expect("PROBES.tsv");
    let text = std::fs::read_to_string(&path).expect("probe file");
    let options = Options::default();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let Some((key, probe)) = line.split_once('\t') else { continue };
        let citations = extract(probe, &options);
        let first = citations.iter().find(|c| c.court.is_some()).or(citations.first());
        let court = first.and_then(|c| c.court.as_ref()).map_or("-", |c| c.id.as_str());
        let kind = first.map_or("none".to_owned(), |c| format!("{:?}", c.authority));
        let url = first.and_then(|c| canlii_case(c, Language::En)).unwrap_or_else(|| "-".to_owned());
        println!("{key}\t{probe}\t{court}\t{kind}\t{url}");
    }
}
