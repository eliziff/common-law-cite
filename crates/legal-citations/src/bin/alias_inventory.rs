//! Binds the alias evidence (`alias-evidence`'s store) into identities: citations written as parallels of one
//! another are one decision. Each gate is a switch, so the raw binding is always one flag away.
//!
//!   cargo run --release --features alias-evidence --bin alias-inventory -- EVIDENCE.sqlite… [gates] [--hold-out]
//!   cargo run --release --features alias-evidence --bin alias-inventory -- EVIDENCE.sqlite… --ablate
//!
//! Several evidence stores (`alias-evidence` runs over different sources) are read as one.
//!
//! Gates (all but `--decision-agreement` on by default):
//!   --no-journal-agreement  a pair only text extracted from PDFs writes (a PDF-text journal article, a corpus
//!                           document read as OCR or unclassified) binds on one such text, not two; a digital-native
//!                           journal article or digital-born corpus document binds on one, as A2AJ's text does
//!   --decision-agreement    a pair only A2AJ decisions write binds on two decisions, not one
//!   --no-anchor             a pair may join two decisions A2AJ's records hold apart
//!   --no-distinct           a pair may join citations the engine reads as two decisions (`parallel::distinct`)
//!   --no-order              pairs bind in the store's order, not the most written first
//!
//! `--hold-out` seeds each A2AJ decision by its first citation alone and scores its other citations (A2AJ's second
//! citations) against the binding: bound to its decision, bound elsewhere, or unbound. `--ablate` scores every gate
//! off alone, all on and all off. `--split` measures the identities A2AJ cannot judge by split-half agreement: bound
//! from one half of the text sources, read against the other half's parallels.
use legal_citations::{aliases, extract, key, parallel, registry, Citation, Options};
use rusqlite::{Connection, OpenFlags};
use std::collections::{HashMap, HashSet};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy, PartialEq)]
enum Kind { Record, Decision, Digital, Article, Corpus, CorpusDigital }

#[derive(Clone, Copy)]
struct Gates { journal_agreement: bool, decision_agreement: bool, anchor: bool, distinct: bool, order: bool }

impl Default for Gates {
    fn default() -> Self { Self { journal_agreement: true, decision_agreement: false, anchor: true, distinct: true, order: true } }
}

#[derive(Default, Clone, Copy)]
struct Support { records: u32, decisions: u32, digital: u32, articles: u32, files: u32, digital_files: u32 }

struct Evidence {
    keys: Vec<String>,
    forms: Vec<String>,
    /// Each A2AJ decision's first key and its other keys.
    records: Vec<(u32, Vec<u32>)>,
    /// Text pairs, by how many sources of each kind write them (records' own pairs apart).
    pairs: Vec<((u32, u32), Support)>,
}

/// The text sources a load keeps: all, or those of one half by their id's parity (A2AJ's records always).
#[derive(Clone, Copy, PartialEq)]
enum Half { All, Even, Odd }

fn load(paths: &[String], half: Half) -> Result<Evidence> {
    let mut ids: HashMap<String, u32> = HashMap::new();
    let (mut keys, mut forms) = (Vec::new(), Vec::new());
    let mut records = Vec::new();
    let mut support: HashMap<(u32, u32), Support> = HashMap::new();
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    let (mut source, mut group, mut kind, mut members): (i64, i64, Kind, Vec<u32>) = (-1, -1, Kind::Record, Vec::new());
    let mut flush = |kind: Kind, members: &mut Vec<u32>, seen: &mut HashSet<(u32, u32)>| {
        if kind == Kind::Record {
            if let Some(&first) = members.first() {
                let others: Vec<u32> = members.iter().copied().filter(|id| *id != first).collect::<HashSet<_>>().into_iter().collect();
                records.push((first, others));
            }
        } else {
            for (at, &left) in members.iter().enumerate() {
                for &right in &members[at + 1..] {
                    if left == right { continue; }
                    let pair = (left.min(right), left.max(right));
                    // A source counts once for a pair, however often it writes it.
                    if !seen.insert(pair) { continue; }
                    let entry = support.entry(pair).or_default();
                    match kind {
                        Kind::Decision => entry.decisions += 1,
                        Kind::Digital => entry.digital += 1,
                        Kind::Corpus => entry.files += 1,
                        Kind::CorpusDigital => entry.digital_files += 1,
                        _ => entry.articles += 1,
                    }
                }
            }
        }
        members.clear();
    };
    for (store, path) in paths.iter().enumerate() {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare("SELECT s.id, s.kind, m.parallel_id, m.key, m.form FROM member m
        JOIN parallel p ON p.id = m.parallel_id JOIN source s ON s.id = p.source_id ORDER BY s.id, m.parallel_id, m.rowid")?;
    let mut rows = statement.query([])?;
    // Each store's ids apart from the others'.
    let offset = (store as i64) << 40;
    while let Some(row) = rows.next()? {
        let (source_id, kind_name, parallel_id, key, form): (i64, String, i64, String, String) =
            (offset + row.get::<_, i64>(0)?, row.get(1)?, offset + row.get::<_, i64>(2)?, row.get(3)?, row.get(4)?);
        if parallel_id != group {
            flush(kind, &mut members, &mut seen);
            group = parallel_id;
        }
        if source_id != source { seen.clear(); source = source_id; }
        kind = match kind_name.as_str() {
            "a2aj_record" => Kind::Record,
            "a2aj_decision" => Kind::Decision,
            "journal_article_digital" => Kind::Digital,
            "corpus_document_digital" => Kind::CorpusDigital,
            "corpus_document" | "corpus_document_ocr" => Kind::Corpus,
            _ => Kind::Article,
        };
        let kept = match half { Half::All => true, Half::Even => source_id % 2 == 0, Half::Odd => source_id % 2 == 1 };
        if kind != Kind::Record && !kept { continue; }
        let id = *ids.entry(key.clone()).or_insert_with(|| { keys.push(key); forms.push(form); keys.len() as u32 - 1 });
        members.push(id);
    }
    }
    flush(kind, &mut members, &mut seen);
    // A record's own citations are a pair A2AJ writes.
    for (first, others) in &records {
        for &other in others {
            support.entry(((*first).min(other), (*first).max(other))).or_default().records += 1;
        }
    }
    let mut pairs: Vec<((u32, u32), Support)> = support.into_iter().collect();
    pairs.sort_by_key(|(pair, _)| *pair);
    Ok(Evidence { keys, forms, records, pairs })
}

struct Binding { root: Vec<u32>, refused: Vec<(u32, u32, &'static str)> }

fn find(root: &mut [u32], mut node: u32) -> u32 {
    while root[node as usize] != node {
        let parent = root[node as usize];
        root[node as usize] = root[parent as usize];
        node = parent;
    }
    node
}

/// The engine's reading of a key's written form, read from the grammar alone.
fn reading(key: &str, form: &str) -> Option<Citation> {
    extract(form, &Options { resolve: false, parallel: false, ..Options::default() }).into_iter()
        .find(|citation| key::key_in(citation, registry::registry()).as_deref() == Some(key))
}

fn bind(evidence: &Evidence, gates: Gates, hold_out: bool, readings: &mut HashMap<u32, Option<Citation>>) -> Binding {
    let count = evidence.keys.len();
    let mut root: Vec<u32> = (0..count as u32).collect();
    let mut members: Vec<Vec<u32>> = (0..count as u32).map(|id| vec![id]).collect();
    // Each key A2AJ files under exactly one decision is that decision's (by the decision's first key).
    let mut filed: HashMap<u32, HashSet<u32>> = HashMap::new();
    for (first, others) in &evidence.records {
        filed.entry(*first).or_default().insert(*first);
        if !hold_out { for other in others { filed.entry(*other).or_default().insert(*first); } }
    }
    let mut anchor: Vec<Option<u32>> = (0..count as u32).map(|id| filed.get(&id).filter(|set| set.len() == 1)
        .and_then(|set| set.iter().next().copied())).collect();
    let mut order: Vec<&((u32, u32), Support)> = evidence.pairs.iter()
        .filter(|(_, support)| !(hold_out && support.decisions + support.digital + support.digital_files + support.articles
            + support.files == 0)).collect();
    if gates.order {
        order.sort_by_key(|(_, support)| std::cmp::Reverse((support.records,
            support.decisions + support.digital + support.digital_files + support.articles + support.files,
            support.decisions + support.digital + support.digital_files)));
    }
    let mut refused = Vec::new();
    for &&((left, right), support) in &order {
        let records = if hold_out { 0 } else { support.records };
        let binds = records > 0
            || support.decisions >= if gates.decision_agreement { 2 } else { 1 }
            || support.digital + support.digital_files >= 1
            || support.articles + support.files >= if gates.journal_agreement { 2 } else { 1 };
        if !binds { continue; }
        let (a, b) = (find(&mut root, left), find(&mut root, right));
        if a == b { continue; }
        if gates.anchor && matches!((anchor[a as usize], anchor[b as usize]), (Some(x), Some(y)) if x != y) {
            refused.push((left, right, "anchor"));
            continue;
        }
        let (small, large) = if members[a as usize].len() < members[b as usize].len() { (a, b) } else { (b, a) };
        if gates.distinct {
            for &id in members[small as usize].iter().chain(&members[large as usize]) {
                readings.entry(id).or_insert_with(|| reading(&evidence.keys[id as usize], &evidence.forms[id as usize]));
            }
            if members[small as usize].iter().any(|x| members[large as usize].iter().any(|y|
                matches!((&readings[x], &readings[y]), (Some(x), Some(y)) if parallel::distinct(x, y)))) {
                refused.push((left, right, "distinct"));
                continue;
            }
        }
        let moved = std::mem::take(&mut members[small as usize]);
        members[large as usize].extend(moved);
        root[small as usize] = large;
        anchor[large as usize] = anchor[large as usize].or(anchor[small as usize]);
    }
    for id in 0..count as u32 { find(&mut root, id); }
    Binding { root, refused }
}

#[derive(Default)]
struct Score { reproduced: u64, elsewhere: u64, unbound: u64, unwritten: u64, aliases: u64, identities: u64, merged: u64 }

/// Held-out citations no text writes as a parallel (a tribunal's French neutral twin, numbered apart) cannot be
/// reproduced by any binding; they are counted apart.
fn score(evidence: &Evidence, binding: &Binding) -> Score {
    let mut size: HashMap<u32, u64> = HashMap::new();
    for &root in &binding.root { *size.entry(root).or_default() += 1; }
    let mut score = Score::default();
    for &members in size.values() {
        if members > 1 { score.identities += 1; score.aliases += members - 1; }
    }
    // Over-merging, which agreement alone rewards: identities holding two or more decisions A2AJ holds apart.
    let mut decisions: HashMap<u32, HashSet<u32>> = HashMap::new();
    for (first, _) in &evidence.records { decisions.entry(binding.root[*first as usize]).or_default().insert(*first); }
    score.merged = decisions.values().filter(|held| held.len() > 1).count() as u64;
    let written: HashSet<u32> = evidence.pairs.iter().filter(|(_, support)| support.decisions + support.digital + support.digital_files + support.articles + support.files > 0)
        .flat_map(|((left, right), _)| [*left, *right]).collect();
    for (first, others) in &evidence.records {
        for &other in others {
            if !written.contains(&other) { score.unwritten += 1; continue; }
            let (a, b) = (binding.root[*first as usize], binding.root[other as usize]);
            if a == b { score.reproduced += 1 } else if size[&b] > 1 { score.elsewhere += 1 } else { score.unbound += 1 }
        }
    }
    score
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths: Vec<String> = args.iter().filter(|arg| !arg.starts_with("--")).cloned().collect();
    if paths.is_empty() { return Err("EVIDENCE.sqlite".into()); }
    let flag = |name: &str| args.iter().any(|arg| arg == name);
    aliases::use_evidence(false);
    let started = std::time::Instant::now();
    if flag("--split") { return split(&paths); }
    let evidence = load(&paths, Half::All)?;
    eprintln!("{} keys, {} pairs, {} records in {:.0} s", evidence.keys.len(), evidence.pairs.len(), evidence.records.len(),
        started.elapsed().as_secs_f64());
    let mut readings = HashMap::new();
    let configurations: Vec<(&str, Gates)> = if flag("--ablate") {
        let on = Gates::default();
        vec![
            ("all gates on", on),
            ("journal agreement off", Gates { journal_agreement: false, ..on }),
            ("decision agreement on", Gates { decision_agreement: true, ..on }),
            ("anchor off", Gates { anchor: false, ..on }),
            ("distinct off", Gates { distinct: false, ..on }),
            ("order off", Gates { order: false, ..on }),
            ("all gates off", Gates { journal_agreement: false, decision_agreement: false, anchor: false, distinct: false, order: false }),
        ]
    } else {
        vec![("chosen gates", Gates {
            journal_agreement: !flag("--no-journal-agreement"), decision_agreement: flag("--decision-agreement"),
            anchor: !flag("--no-anchor"), distinct: !flag("--no-distinct"), order: !flag("--no-order"),
        })]
    };
    let hold_out = flag("--hold-out") || flag("--ablate");
    let mut unwritten = 0;
    println!("| configuration | bound to its decision | bound to another identity | unbound | precision | recall | identities holding two A2AJ decisions | identities | aliases | seconds |");
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for (name, gates) in configurations {
        let run = std::time::Instant::now();
        let binding = bind(&evidence, gates, hold_out, &mut readings);
        let score = score(&evidence, &binding);
        let precision = score.reproduced as f64 / (score.reproduced + score.elsewhere).max(1) as f64;
        let recall = score.reproduced as f64 / (score.reproduced + score.elsewhere + score.unbound).max(1) as f64;
        println!("| {name} | {} | {} | {} | {:.2}% | {:.2}% | {} | {} | {} | {:.0} |", score.reproduced, score.elsewhere, score.unbound,
            100.0 * precision, 100.0 * recall, score.merged, score.identities, score.aliases, run.elapsed().as_secs_f64());
        unwritten = score.unwritten;
    }
    println!("\nHeld-out citations no text writes as a parallel, not scored: {unwritten}");
    if flag("--explain") {
        explain(&evidence, &bind(&evidence, Gates::default(), true, &mut readings), &mut readings);
    }
    Ok(())
}

/// Split-half agreement, for the identities A2AJ cannot judge: bind from the even half of the text sources, then read
/// the odd half's parallels. A pair whose citations both sit in identities the even half bound agrees (one identity)
/// or contradicts it (two), by the kind of source in the odd half that writes it.
fn split(paths: &[String]) -> Result<()> {
    let (even, odd) = (load(paths, Half::Even)?, load(paths, Half::Odd)?);
    let mut readings = HashMap::new();
    let configurations = [("all gates on", Gates::default()), ("journal agreement off", Gates { journal_agreement: false, ..Gates::default() }),
        ("distinct off", Gates { distinct: false, ..Gates::default() }),
        ("all gates off", Gates { journal_agreement: false, decision_agreement: false, anchor: false, distinct: false, order: false })];
    println!("| configuration | odd half written by | agrees | contradicts | agreement |");
    println!("|---|---|---|---|---|");
    let id_of: HashMap<&str, u32> = even.keys.iter().enumerate().map(|(id, key)| (key.as_str(), id as u32)).collect();
    for (name, gates) in configurations {
        let binding = bind(&even, gates, false, &mut readings);
        let mut size: HashMap<u32, u32> = HashMap::new();
        for &root in &binding.root { *size.entry(root).or_default() += 1; }
        for (label, written) in [("A2AJ decisions", 0usize), ("digital-native articles", 1), ("PDF-text articles", 2),
            ("OCR or unclassified corpus documents", 3), ("digital-born corpus documents", 4)] {
            let (mut agrees, mut contradicts) = (0u64, 0u64);
            for ((left, right), support) in &odd.pairs {
                let by = [support.decisions, support.digital, support.articles, support.files, support.digital_files];
                if by[written] == 0 { continue; }
                let (Some(&a), Some(&b)) = (id_of.get(odd.keys[*left as usize].as_str()), id_of.get(odd.keys[*right as usize].as_str()))
                    else { continue };
                let (a, b) = (binding.root[a as usize], binding.root[b as usize]);
                if a == b { agrees += 1 } else if size[&a] > 1 && size[&b] > 1 { contradicts += 1 }
            }
            println!("| {name} | {label} | {agrees} | {contradicts} | {:.2}% |", 100.0 * agrees as f64 / (agrees + contradicts).max(1) as f64);
        }
    }
    // The pairs journal agreement decides: written in the even half by one text extracted from a PDF and nothing else,
    // judged by the odd half bound with every gate on, by the kind of that text.
    let odd_binding = bind(&odd, Gates::default(), false, &mut readings);
    let mut size: HashMap<u32, u32> = HashMap::new();
    for &root in &odd_binding.root { *size.entry(root).or_default() += 1; }
    let odd_id: HashMap<&str, u32> = odd.keys.iter().enumerate().map(|(id, key)| (key.as_str(), id as u32)).collect();
    for (label, from_corpus) in [("one PDF-text journal article", false), ("one OCR or unclassified corpus document", true)] {
    let (mut confirmed, mut contradicted, mut silent) = (0u64, 0u64, 0u64);
    let mut examples = Vec::new();
    for ((left, right), support) in &even.pairs {
        let alone = if from_corpus { support.files == 1 && support.articles == 0 } else { support.articles == 1 && support.files == 0 };
        if !(alone && support.records + support.decisions + support.digital + support.digital_files == 0) { continue; }
        let (Some(&a), Some(&b)) = (odd_id.get(even.keys[*left as usize].as_str()), odd_id.get(even.keys[*right as usize].as_str()))
            else { silent += 1; continue };
        let (a, b) = (odd_binding.root[a as usize], odd_binding.root[b as usize]);
        if a == b { confirmed += 1 } else if size[&a] > 1 && size[&b] > 1 {
            contradicted += 1;
            if examples.len() < 30 { examples.push(format!("{} | {}", even.forms[*left as usize], even.forms[*right as usize])); }
        } else { silent += 1 }
    }
    println!("\nPairs {label} alone writes (even half), judged by the odd half: confirmed {confirmed}, contradicted \
        {contradicted}, no evidence {silent}; precision where judged {:.2}%", 100.0 * confirmed as f64 / (confirmed + contradicted).max(1) as f64);
    for example in examples.iter().take(12) { println!("  contradicted: {example}"); }
    }
    Ok(())
}

/// What a reading carries that `parallel::distinct` compares.
fn facts(citation: &Option<Citation>) -> String {
    citation.as_ref().map_or_else(|| "unread".to_owned(), |citation| format!("year {:?}, court {:?}, reporter {:?} {:?}:{:?}",
        citation.fields.year, citation.court.as_ref().map(|court| court.id.as_str()),
        citation.fields.reporter_canonical.as_deref().or(citation.fields.reporter.as_deref()), citation.fields.volume, citation.fields.page))
}

/// Each held-out citation the gates leave apart from its decision, with the refused pairs that touch it.
fn explain(evidence: &Evidence, binding: &Binding, readings: &mut HashMap<u32, Option<Citation>>) {
    let mut touching: HashMap<u32, Vec<(u32, &str)>> = HashMap::new();
    for &(left, right, gate) in &binding.refused {
        touching.entry(left).or_default().push((right, gate));
        touching.entry(right).or_default().push((left, gate));
    }
    println!("\n| held out | its decision | refused pair | gate | held-out reading | other reading |");
    println!("|---|---|---|---|---|---|");
    for (first, others) in &evidence.records {
        for &other in others {
            if binding.root[*first as usize] == binding.root[other as usize] { continue; }
            for &(partner, gate) in touching.get(&other).map_or(&[][..], Vec::as_slice) {
                for id in [other, partner] {
                    readings.entry(id).or_insert_with(|| reading(&evidence.keys[id as usize], &evidence.forms[id as usize]));
                }
                println!("| {} | {} | {} | {gate} | {} | {} |", evidence.forms[other as usize], evidence.forms[*first as usize],
                    evidence.forms[partner as usize], facts(&readings[&other]), facts(&readings[&partner]));
            }
        }
    }
}
