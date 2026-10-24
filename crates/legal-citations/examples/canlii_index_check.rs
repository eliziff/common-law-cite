//! Every case in CanLII's own index, through the grammar and the link builder, offline.
//!
//!     cargo run --release --example canlii_index_check -- CASES.tsv [REPORT.tsv]
//!
//! CASES.tsv: `databaseId<TAB>caseId` per line (from canlii.db's cases table). The citation CanLII
//! prints is rebuilt from the id ("2025onsc1050" -> "2025 ONSC 1050", "2012canlii345" -> "2012 CanLII
//! 345"), read by the grammar, and the link the builder makes is compared with the id: its slug must
//! be the case id. Prints totals and, per database, how many ids read, resolve to a court, link, and
//! link to the right slug; REPORT.tsv gets one line per (database, designator) with those counts.
use legal_citations::format::Language;
use legal_citations::url::canlii_case;
use legal_citations::{extract, Options};
use std::collections::BTreeMap;

#[derive(Default, Clone, Copy)]
struct Tally {
    ids: u64,
    parsed: u64,
    cited: u64,
    court: u64,
    linked: u64,
    slug_ok: u64,
}

fn split_id(id: &str) -> Option<(&str, &str, &str)> {
    let bytes = id.as_bytes();
    if bytes.len() < 6 || !bytes[..4].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let letters = bytes[4..].iter().take_while(|b| b.is_ascii_alphabetic()).count();
    if letters == 0 {
        return None;
    }
    let rest = &id[4 + letters..];
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((&id[..4], &id[4..4 + letters], rest))
}

fn main() {
    let path = std::env::args().nth(1).expect("CASES.tsv");
    let report = std::env::args().nth(2);
    let text = std::fs::read_to_string(&path).expect("cases file");
    let rows: Vec<(&str, &str)> = text
        .lines()
        .filter_map(|line| line.trim_end_matches('\r').split_once('\t'))
        .collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(12);
    let chunk = rows.len().div_ceil(threads).max(1);
    let started = std::time::Instant::now();
    let results: Vec<(BTreeMap<(String, String), Tally>, Vec<String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = rows
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    let options = Options::default();
                    let mut tallies = BTreeMap::<(String, String), Tally>::new();
                    let mut examples = Vec::new();
                    for &(database, id) in part {
                        let Some((year, code, number)) = split_id(id) else {
                            let t = tallies.entry((database.to_owned(), "?".to_owned())).or_default();
                            t.ids += 1;
                            continue;
                        };
                        let printed = if code == "canlii" { format!("{year} CanLII {number}") } else { format!("{year} {} {number}", code.to_uppercase()) };
                        let t = tallies.entry((database.to_owned(), code.to_owned())).or_default();
                        t.ids += 1;
                        t.parsed += 1;
                        let citations = extract(&printed, &options);
                        let Some(first) = citations.first() else { continue };
                        t.cited += 1;
                        if first.court.is_some() {
                            t.court += 1;
                        }
                        if let Some(url) = canlii_case(first, Language::En) {
                            t.linked += 1;
                            let slug = url.rsplit('/').next().unwrap_or("").trim_end_matches(".html");
                            if slug.eq_ignore_ascii_case(id) {
                                t.slug_ok += 1;
                            } else if examples.len() < 2000 {
                                examples.push(format!("{database}\t{id}\t{printed}\t{url}"));
                            }
                        }
                    }
                    (tallies, examples)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("worker")).collect()
    });
    let mut tallies = BTreeMap::<(String, String), Tally>::new();
    let mut examples = Vec::new();
    for (part, more) in results {
        for (key, t) in part {
            let e = tallies.entry(key).or_default();
            e.ids += t.ids; e.parsed += t.parsed; e.cited += t.cited; e.court += t.court; e.linked += t.linked; e.slug_ok += t.slug_ok;
        }
        examples.extend(more);
    }
    let total = tallies.values().fold(Tally::default(), |mut a, t| { a.ids += t.ids; a.parsed += t.parsed; a.cited += t.cited; a.court += t.court; a.linked += t.linked; a.slug_ok += t.slug_ok; a });
    let pct = |n: u64| format!("{:.3}%", 100.0 * n as f64 / total.parsed.max(1) as f64);
    println!("{} ids in {:.1}s; {} with a year-code-number id", total.ids, started.elapsed().as_secs_f64(), total.parsed);
    println!("read as a citation {} ({}), court resolved {} ({}), link built {} ({}), link slug is the id {} ({})",
        total.cited, pct(total.cited), total.court, pct(total.court), total.linked, pct(total.linked), total.slug_ok, pct(total.slug_ok));
    let mut worst: Vec<_> = tallies.iter().filter(|(_, t)| t.slug_ok < t.parsed).collect();
    worst.sort_by_key(|(_, t)| std::cmp::Reverse(t.parsed - t.slug_ok));
    println!("(database, designator) pairs {}; pairs with any id not linking to itself {}; worst by ids lost:", tallies.len(), worst.len());
    for ((db, code), t) in worst.iter().take(40) {
        println!("  {:<10} {:<10} ids {:>7} cited {:>7} court {:>7} linked {:>7} slug ok {:>7}", db, code, t.parsed, t.cited, t.court, t.linked, t.slug_ok);
    }
    if let Some(report) = report {
        let mut out = String::from("database\tdesignator\tids\tcited\tcourt\tlinked\tslug_ok\n");
        for ((db, code), t) in &tallies {
            out.push_str(&format!("{db}\t{code}\t{}\t{}\t{}\t{}\t{}\n", t.parsed, t.cited, t.court, t.linked, t.slug_ok));
        }
        out.push_str("\n# wrong slug examples: database\tid\tprinted\turl\n");
        for e in examples.iter().take(400) { out.push_str(e); out.push('\n'); }
        std::fs::write(&report, out).expect("report");
    }
}
