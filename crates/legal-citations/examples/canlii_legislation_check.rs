//! Every statute and regulation CanLII lists, read from the citation CanLII prints for it: does the grammar read it
//! whole, and does the link builder make exactly CanLII's address?
//!
//! `cargo run --release --example canlii_legislation_check -- LEGISLATION.tsv [MISSES.tsv]`, where each line is
//! `database \t jurisdiction \t id \t citation` (CanLII's legislation list).
use legal_citations::format::Language;
use legal_citations::{extract, url, Authority, Options};
use std::collections::BTreeMap;
use std::io::Write;

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    // `--time FILE`: how long extraction takes over FILE's texts (separated by U+001E), best of three.
    if args.peek().is_some_and(|arg| arg == "--time") {
        let options = Options { resolve: false, parallel: false, ..Default::default() };
        let text = std::fs::read_to_string(args.nth(1).expect("FILE")).expect("readable texts");
        let texts = text.split('\u{1e}').collect::<Vec<_>>();
        let _ = extract(texts[0], &options);
        let best = (0..3).map(|_| {
            let started = std::time::Instant::now();
            let found = texts.iter().map(|text| extract(text, &options).len()).sum::<usize>();
            (started.elapsed().as_millis(), found)
        }).min().unwrap();
        println!("{} texts, {} citations, {} ms", texts.len(), best.1, best.0);
        return;
    }
    // `--probe TEXT…`: what the parser reads in each text, and the CanLII page it builds.
    if args.peek().is_some_and(|arg| arg == "--probe") {
        let options = Options { resolve: false, parallel: false, ..Default::default() };
        for text in args.skip(1) {
            for found in extract(&text, &options) {
                println!("{text} => {:?} {:?} {:?} \"{}\" (whole \"{}\") {:?} -> {:?}", found.authority, found.format, found.form,
                    found.span.text, found.full_span.text, found.fields, url::canlii_legislation(&found, Language::En));
                println!("    as a consolidated record -> {:?}", url::canlii_consolidated_legislation(&found, Language::En));
            }
        }
        return;
    }
    let list = std::fs::read_to_string(args.next().expect("LEGISLATION.tsv")).expect("readable list");
    let mut misses = args.next().map(|path| std::fs::File::create(path).expect("writable report"));
    let options = Options { resolve: false, parallel: false, ..Default::default() };
    // Every page CanLII lists, by jurisdiction, kind and id: an annual act is often both as enacted and consolidated.
    let listed = list.lines().filter_map(|line| {
        let [database, jurisdiction, id, _] = line.splitn(4, '\t').collect::<Vec<_>>()[..] else { return None };
        Some(format!("{jurisdiction}/{}/{id}", kind_of(database)))
    }).collect::<std::collections::HashSet<_>>();
    let (mut total, mut linked) = (0usize, 0usize);
    let mut databases: BTreeMap<&str, (usize, usize, Vec<String>)> = BTreeMap::new();
    for line in list.lines() {
        let [database, jurisdiction, id, citation] = line.splitn(4, '\t').collect::<Vec<_>>()[..] else { continue };
        total += 1;
        let read = extract(citation, &options).into_iter()
            .find(|found| matches!(found.authority, Authority::Statute | Authority::Regulation));
        let built = read.as_ref().and_then(|found| url::canlii_legislation(found, Language::En));
        let page = built.as_deref().and_then(url::canlii_law_page);
        let entry = databases.entry(database).or_default();
        entry.0 += 1;
        let kind = kind_of(database);
        if page.as_ref().is_some_and(|page| page.jurisdiction == jurisdiction && page.id == id
            && listed.contains(&format!("{}/{}/{}", page.jurisdiction, page.kind, page.id))) {
            linked += 1;
            entry.1 += 1;
            continue;
        }
        let got = match (&read, &page) {
            (None, _) => "unread".to_owned(),
            (Some(found), None) => format!("read {:?} \"{}\", no link", found.fields.series, found.span.text),
            (Some(found), Some(page)) => format!("\"{}\" -> {}/{}/{}", found.span.text, page.jurisdiction, page.kind, page.id),
        };
        if entry.2.len() < 4 {
            entry.2.push(format!("{citation} => {got} (want {kind}/{id})"));
        }
        if let Some(file) = misses.as_mut() {
            writeln!(file, "{database}\t{citation}\t{id}\t{got}").unwrap();
        }
    }
    let mut rows = databases.into_iter().filter(|(_, (n, ok, _))| ok < n).collect::<Vec<_>>();
    rows.sort_by_key(|(_, (n, ok, _))| std::cmp::Reverse(n - ok));
    for (database, (n, ok, examples)) in rows {
        println!("{database} {ok}/{n}\n    {}", examples.join("\n    "));
    }
    println!("linked {linked}/{total} ({:.3}%)", 100.0 * linked as f64 / total as f64);
}

/// CanLII's database id ends in its pages' kind: s(tat), a(stat), r(egu), h(stat).
fn kind_of(database: &str) -> &'static str {
    match database.as_bytes().last() { Some(b's') => "stat", Some(b'a') => "astat", Some(b'r') => "regu", Some(b'h') => "hstat", _ => "" }
}
