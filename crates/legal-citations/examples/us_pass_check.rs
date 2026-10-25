//! What reading without the US reporter and law grammars (`Options::jurisdictions` without `us`) loses and saves:
//! every full citation the default reading finds and the other does not, by its reporter or form, and the time each
//! reading takes.
//!
//!   cargo run --release --example us_pass_check -- TEXTS [LOST.tsv]
//!
//! TEXTS holds the texts separated by U+001E.
use legal_citations::{extract, Form, Options};
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).expect("TEXTS");
    let text = std::fs::read_to_string(path).expect("readable texts");
    let texts: Vec<&str> = text.split('\u{1e}').collect();
    let with_us = Options { resolve: false, ..Options::default() };
    let without_us = Options { jurisdictions: Some(vec!["ca".into(), "uk".into(), "au".into(), "nz".into(), "ie".into()]), ..with_us.clone() };
    // Warm both readings' grammars before timing either.
    let _ = (extract(texts[0], &with_us), extract(texts[0], &without_us));
    let full = |options: &Options| -> (Vec<Vec<(String, String)>>, u128) {
        let started = Instant::now();
        let found = texts.iter().map(|text| extract(text, options).into_iter().filter(|citation| citation.form == Form::Full)
            .map(|citation| (citation.span.text.clone(), citation.key.clone().unwrap_or_default())).collect()).collect();
        (found, started.elapsed().as_millis())
    };
    let (default, default_ms) = full(&with_us);
    let (narrowed, narrowed_ms) = full(&without_us);
    let kind_of = |key: &str| { let kind = key.split(':').take(3).collect::<Vec<_>>().join(":"); if kind.is_empty() { "(no key)".to_owned() } else { kind } };
    let (mut lost, mut gained): (BTreeMap<String, Vec<String>>, BTreeMap<String, Vec<String>>) = Default::default();
    let mut total = 0usize;
    for (before, after) in default.iter().zip(&narrowed) {
        total += before.len();
        let kept: HashSet<&(String, String)> = after.iter().collect();
        let had: HashSet<&(String, String)> = before.iter().collect();
        for citation in after.iter().filter(|citation| !had.contains(citation)) {
            gained.entry(kind_of(&citation.1)).or_default().push(citation.0.clone());
        }
        for citation in before.iter().filter(|citation| !kept.contains(citation)) {
            lost.entry(kind_of(&citation.1)).or_default().push(citation.0.clone());
        }
    }
    let count = |map: &BTreeMap<String, Vec<String>>| map.values().map(Vec::len).sum::<usize>();
    println!("{} texts; full citations {total}; lost without the US grammars {}; gained {}", texts.len(), count(&lost), count(&gained));
    println!("time: default {default_ms} ms, without US {narrowed_ms} ms");
    for (label, map) in [("lost", &lost), ("gained", &gained)] {
        println!("\n{label}:");
        let mut kinds: Vec<_> = map.iter().collect();
        kinds.sort_by_key(|(_, forms)| std::cmp::Reverse(forms.len()));
        for (kind, forms) in kinds.iter().take(20) {
            println!("{:6} {kind}  e.g. {:?}", forms.len(), forms.iter().take(4).collect::<Vec<_>>());
        }
    }
    if let Some(out) = std::env::args().nth(2) {
        let mut out = std::io::BufWriter::new(std::fs::File::create(out).expect("lost file"));
        for (kind, forms) in &lost { for form in forms { let _ = writeln!(out, "{kind}\t{form}"); } }
    }
}
