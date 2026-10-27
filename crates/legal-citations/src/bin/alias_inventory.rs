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
//!   --digital-agreement     a pair only digital-born texts write binds on two of them, or one and a PDF text, not one
//!   --no-anchor             a pair may join two decisions A2AJ's records hold apart
//!   --no-distinct           a pair may join citations the engine reads as two decisions (`parallel::distinct`)
//!   --no-order              pairs bind in the store's order, not the most written first
//!
//! `--hold-out` seeds each A2AJ decision by its first citation alone and scores its other citations (A2AJ's second
//! citations) against the binding: bound to its decision, bound elsewhere, or unbound. `--ablate` scores every gate
//! off alone, all on and all off. `--split` measures the identities A2AJ cannot judge by split-half agreement: bound
//! from one half of the text sources, read against the other half's parallels.
//!
//!   cargo run --release --features alias-evidence --bin alias-inventory -- EVIDENCE.sqlite… [gates]
//!       --write registry/aliases.json [--from registry/aliases.json]
//!
//! `--judge PAGES.tsv` scores the binding under each configuration (`--ablate`) or the chosen gates against decisions
//! whose own citations a provider prints, one decision a line (`provider <TAB> text`), by [`judge`].
//!
//! `--write` writes the alias table the engine compiles in from the binding under the chosen gates and from the table
//! it replaces (`--from`, by default the one it writes), whose ALR records, reviews and blocked forms it keeps.
use legal_citations::{aliases, extract, key, parallel, registry, Authority, Citation, Form, Options};
use rusqlite::{Connection, OpenFlags};
use std::collections::{BTreeMap, HashMap, HashSet};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy, PartialEq)]
enum Kind { Record, Decision, Digital, Article, Corpus, CorpusDigital }

#[derive(Clone, Copy)]
struct Gates {
    journal_agreement: bool, decision_agreement: bool, digital_agreement: bool, anchor: bool, distinct: bool, order: bool,
    /// The most years two identities' decisions may lie apart and join ([`years`]).
    years: Option<i32>,
}

impl Default for Gates {
    fn default() -> Self {
        Self { journal_agreement: true, decision_agreement: false, digital_agreement: false, anchor: true, distinct: true, order: true,
            years: None }
    }
}

#[derive(Default, Clone, Copy)]
struct Support { records: u32, decisions: u32, digital: u32, articles: u32, files: u32, digital_files: u32 }

struct Evidence {
    keys: Vec<String>,
    forms: Vec<String>,
    /// Each key's best rank as written (`parallel::rank`).
    ranks: Vec<u8>,
    /// The years each key's decision can be from ([`years`]).
    years: Vec<Option<(i32, i32)>>,
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
    let (mut keys, mut forms, mut ranks) = (Vec::new(), Vec::new(), Vec::<u8>::new());
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
    let mut statement = connection.prepare("SELECT s.id, s.kind, m.parallel_id, m.key, m.form, m.rank FROM member m
        JOIN parallel p ON p.id = m.parallel_id JOIN source s ON s.id = p.source_id ORDER BY s.id, m.parallel_id, m.rowid")?;
    let mut rows = statement.query([])?;
    // Each store's ids apart from the others'.
    let offset = (store as i64) << 40;
    while let Some(row) = rows.next()? {
        let (source_id, kind_name, parallel_id, key, form, rank): (i64, String, i64, String, String, u8) =
            (offset + row.get::<_, i64>(0)?, row.get(1)?, offset + row.get::<_, i64>(2)?, row.get(3)?, row.get(4)?, row.get(5)?);
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
        let id = *ids.entry(key.clone()).or_insert_with(|| { keys.push(key); forms.push(form); ranks.push(rank); keys.len() as u32 - 1 });
        ranks[id as usize] = ranks[id as usize].min(rank);
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
    let years = years(&keys, &pairs);
    Ok(Evidence { keys, forms, ranks, years, records, pairs })
}

/// What a key says of its decision's year: the year it writes (a neutral, CanLII or database citation's, a report's
/// numbered by its year) or the edition and volume of a report numbered by volume.
enum Dated { Year(i32), Volume(String, u32), Unknown }

fn dated(key: &str) -> Dated {
    let parts: Vec<&str> = key.split(':').collect();
    let year = |part: &&str| part.len() == 4 && part.parse::<i32>().is_ok_and(|year| (1700..=2100).contains(&year));
    let at = |index: usize| parts.get(index).copied().filter(year).and_then(|part| part.parse().ok()).map_or(Dated::Unknown, Dated::Year);
    match (parts.get(1).copied(), parts.len()) {
        (Some("neutral" | "canlii"), _) => at(2),
        (Some("database"), _) => at(3),
        (Some("reporter"), 7) => at(4),
        (Some("reporter"), 6) => match at(4) {
            Dated::Unknown => parts[4].parse().map_or(Dated::Unknown, |volume| Dated::Volume(format!("{}:{}", parts[2], parts[3]), volume)),
            dated => dated,
        },
        _ => Dated::Unknown,
    }
}

/// The years each key's decision can be from: the year it writes, or for a report numbered by volume the year its
/// volume appeared, the median of the years the citations written as its volume's parallels give (A2AJ's records and
/// the evidence's pairs), and for a volume none dates, the years between the nearest dated volumes before and after it.
fn years(keys: &[String], pairs: &[((u32, u32), Support)]) -> Vec<Option<(i32, i32)>> {
    let dated: Vec<Dated> = keys.iter().map(|key| dated(key)).collect();
    let mut seen: HashMap<(&str, u32), Vec<i32>> = HashMap::new();
    for ((left, right), support) in pairs {
        let weight = (support.records + support.decisions + support.digital + support.digital_files + support.articles
            + support.files).min(10);
        for (a, b) in [(left, right), (right, left)] {
            if let (Dated::Year(year), Dated::Volume(edition, volume)) = (&dated[*a as usize], &dated[*b as usize]) {
                seen.entry((edition.as_str(), *volume)).or_default().extend(std::iter::repeat_n(*year, weight as usize));
            }
        }
    }
    let mut volumes: HashMap<&str, BTreeMap<u32, i32>> = HashMap::new();
    for ((edition, volume), mut years) in seen {
        years.sort_unstable();
        volumes.entry(edition).or_default().insert(volume, years[years.len() / 2]);
    }
    dated.iter().map(|dated| match dated {
        Dated::Year(year) => Some((*year, *year)),
        Dated::Volume(edition, volume) => {
            let dates = volumes.get(edition.as_str())?;
            if let Some(&year) = dates.get(volume) { return Some((year, year)); }
            let before = dates.range(..volume).next_back().map(|(_, &year)| year)?;
            let after = dates.range(volume..).next().map(|(_, &year)| year)?;
            Some((before.min(after), before.max(after)))
        }
        Dated::Unknown => None,
    }).collect()
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
    // Each identity's years, at its root.
    let mut span = evidence.years.clone();
    for &&((left, right), support) in &order {
        let records = if hold_out { 0 } else { support.records };
        let digital = support.digital + support.digital_files;
        let binds = records > 0
            || support.decisions >= if gates.decision_agreement { 2 } else { 1 }
            || if gates.digital_agreement { digital >= 2 || digital >= 1 && support.articles + support.files >= 1 } else { digital >= 1 }
            || support.articles + support.files >= if gates.journal_agreement { 2 } else { 1 };
        if !binds { continue; }
        let (a, b) = (find(&mut root, left), find(&mut root, right));
        if a == b { continue; }
        if gates.anchor && matches!((anchor[a as usize], anchor[b as usize]), (Some(x), Some(y)) if x != y) {
            refused.push((left, right, "anchor"));
            continue;
        }
        if let (Some(apart), Some((a_first, a_last)), Some((b_first, b_last))) = (gates.years, span[a as usize], span[b as usize]) {
            if a_first.max(b_first) - a_last.min(b_last) > apart {
                refused.push((left, right, "years"));
                continue;
            }
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
        span[large as usize] = match (span[large as usize], span[small as usize]) {
            (Some((a, b)), Some((c, d))) => Some((a.min(c), b.max(d))),
            (one, other) => one.or(other),
        };
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

/// The decisions of `path` (`provider <TAB> text` a line), each with its provider and the keys of its case citations.
fn pages(path: &str) -> Result<Vec<(String, Vec<String>)>> {
    let mut pages = Vec::new();
    for line in std::fs::read_to_string(path)?.lines() {
        let Some((provider, text)) = line.split_once('\t') else { continue };
        let mut keys: Vec<String> = extract(text, &Options { resolve: false, parallel: false, ..Options::default() }).iter()
            .filter(|citation| citation.form == Form::Full && citation.authority == Authority::Case)
            .filter_map(|citation| key::key_in(citation, registry::registry())).collect();
        keys.sort();
        keys.dedup();
        pages.push((provider.to_owned(), keys));
    }
    Ok(pages)
}

/// Each pair of the case citations a provider prints for one decision, against `binding`, by provider: linked (one
/// identity), split (two identities), wrong (two identities that hold two different A2AJ decisions) or unbound (one of
/// them in no identity). Each wrong pair goes to stderr.
fn judge(evidence: &Evidence, binding: &Binding, pages: &[(String, Vec<String>)]) -> BTreeMap<String, [u64; 4]> {
    let ids: HashMap<&str, u32> = evidence.keys.iter().enumerate().map(|(id, key)| (key.as_str(), id as u32)).collect();
    let mut size: HashMap<u32, u32> = HashMap::new();
    for &root in &binding.root { *size.entry(root).or_default() += 1; }
    let mut decision: HashMap<u32, u32> = HashMap::new();
    for (first, _) in &evidence.records { decision.entry(binding.root[*first as usize]).or_insert(*first); }
    let mut tally: BTreeMap<String, [u64; 4]> = BTreeMap::new();
    for (provider, keys) in pages {
        let roots: Vec<Option<u32>> = keys.iter()
            .map(|key| ids.get(key.as_str()).map(|&id| binding.root[id as usize]).filter(|root| size[root] > 1)).collect();
        let counts = tally.entry(provider.clone()).or_default();
        for (at, left) in roots.iter().enumerate() {
            for (right, other) in roots.iter().zip(keys).skip(at + 1) {
                counts[match (left, right) {
                    (Some(a), Some(b)) if a == b => 0,
                    (Some(a), Some(b)) if decision.get(a).zip(decision.get(b)).is_some_and(|(x, y)| x != y) => {
                        eprintln!("wrong\t{provider}\t{}\t{other}\t{}\t{}", keys[at], evidence.keys[decision[a] as usize],
                            evidence.keys[decision[b] as usize]);
                        2
                    }
                    (Some(_), Some(_)) => 1,
                    _ => 3,
                }] += 1;
            }
        }
    }
    tally
}

/// Every key the alias table names, with its identity: the binding's keys first, then those only the ALR records name.
struct Identities {
    ids: HashMap<String, u32>,
    keys: Vec<String>,
    root: Vec<u32>,
    /// At an identity's root, the A2AJ decision (its first key) the identity holds.
    decision: Vec<Option<u32>>,
    /// At an identity's root, the neutral citation and the CanLII identifier it holds: a decision has one of each.
    named: Vec<[Option<u32>; 2]>,
    rank: Vec<u8>,
}

/// Which of [`Identities::named`] a key is: its neutral citation (0) or its CanLII identifier (1).
fn named_slot(key: &str) -> Option<usize> {
    match key.split(':').nth(1) { Some("neutral") => Some(0), Some("canlii") => Some(1), _ => None }
}

impl Identities {
    fn id(&mut self, key: &str) -> u32 {
        if let Some(&id) = self.ids.get(key) { return id; }
        let id = self.keys.len() as u32;
        self.ids.insert(key.to_owned(), id);
        self.keys.push(key.to_owned());
        self.root.push(id);
        self.decision.push(None);
        let mut named = [None; 2];
        if let Some(slot) = named_slot(key) { named[slot] = Some(id); }
        self.named.push(named);
        self.rank.push(u8::MAX);
        id
    }

    /// Joins the identities of `a` and `b`, unless they hold two different decisions: two of A2AJ's, or two neutral
    /// citations or CanLII identifiers ("181 D.L.R. (4th) 522", an ALR record of "[1999] 1 SCR 420", is 1999 CanLII
    /// 5311's, not 1999 CanLII 715's).
    fn join(&mut self, a: u32, b: u32) -> bool {
        let (a, b) = (find(&mut self.root, a), find(&mut self.root, b));
        if a == b { return true; }
        if matches!((self.decision[a as usize], self.decision[b as usize]), (Some(x), Some(y)) if x != y) { return false; }
        let (left, right) = (self.named[a as usize], self.named[b as usize]);
        if left.iter().zip(&right).any(|pair| matches!(pair, (Some(x), Some(y)) if x != y)) { return false; }
        self.root[a as usize] = b;
        self.decision[b as usize] = self.decision[b as usize].or(self.decision[a as usize]);
        self.named[b as usize] = [right[0].or(left[0]), right[1].or(left[1])];
        true
    }
}

/// Writes the alias table (`registry/aliases.json`) from `binding` and `from`, the table it replaces. The ALR records
/// `from` holds keep their reviews and blocked forms and are read again by this engine: each record's citation joins
/// its decision's identity, or, where it would join two decisions, is a conflict and joins none; one this engine reads
/// no citation or identity in is unresolved. Each identity's citation is the A2AJ decision's it holds, else its
/// best-ranked member's ([`parallel::rank`]); every other member is a target of it, and each ALR record's written form
/// is observed for it.
fn write(evidence: &Evidence, binding: &Binding, from: &str, out: &str, paths: &[String], gates: Gates) -> Result<()> {
    use serde_json::{json, Value};
    let previous: Value = serde_json::from_str(&std::fs::read_to_string(from)?)?;
    let records = previous["aliases"].as_object().ok_or("aliases")?;
    let reviews = previous["reviews"].as_object().ok_or("reviews")?;
    let blocked: HashSet<&str> = previous["blockedForms"].as_array().ok_or("blockedForms")?.iter().filter_map(Value::as_str).collect();
    let mut identities = Identities {
        ids: evidence.keys.iter().enumerate().map(|(id, key)| (key.clone(), id as u32)).collect(),
        keys: evidence.keys.clone(), root: binding.root.clone(), decision: vec![None; evidence.keys.len()],
        named: vec![[None; 2]; evidence.keys.len()], rank: evidence.ranks.clone(),
    };
    for (first, _) in &evidence.records {
        let root = find(&mut identities.root, *first);
        identities.decision[root as usize].get_or_insert(*first);
    }
    for id in 0..evidence.keys.len() as u32 {
        if let Some(slot) = named_slot(&evidence.keys[id as usize]) {
            let root = find(&mut identities.root, id);
            identities.named[root as usize][slot].get_or_insert(id);
        }
    }
    let (mut unresolved, mut conflicts) = (BTreeMap::<String, &str>::new(), BTreeMap::<String, Vec<String>>::new());
    let mut citations: HashMap<u32, String> = HashMap::new();
    let mut own_records: HashMap<u32, Vec<String>> = HashMap::new();
    // The keys an ALR record and the binding give two decisions: neither is taken.
    let mut refused: HashSet<u32> = HashSet::new();
    // Each record kept, with its decision's key and its court.
    let mut kept: Vec<(&String, u32, String)> = Vec::new();
    for (id, record) in records {
        let review = reviews.get(id);
        if blocked.contains(id.as_str()) || review.is_some_and(|review| review["status"] != "corrected") { continue; }
        let citation = review.and_then(|review| review["canonicalCitation"].as_str())
            .or_else(|| record["canonical_citation"].as_str()).ok_or("canonical_citation")?;
        let Ok(canonical) = key::key_for_text(citation) else { unresolved.insert(id.clone(), "no_canonical_identity"); continue };
        let canonical = identities.id(&canonical);
        citations.entry(canonical).or_insert_with(|| citation.to_owned());
        let root = find(&mut identities.root, canonical);
        identities.decision[root as usize].get_or_insert(canonical);
        match key::key_for_text(record["alias"].as_str().ok_or("alias")?) {
            Ok(own) => {
                let own = identities.id(&own);
                if !identities.join(own, canonical) {
                    conflicts.entry(identities.keys[own as usize].clone()).or_default().push(id.clone());
                    refused.insert(own);
                    continue;
                }
                own_records.entry(own).or_default().push(id.clone());
            }
            Err(key::KeyError::NoIdentity) => { unresolved.insert(id.clone(), "no_identity"); }
            Err(key::KeyError::NoCitation) => { unresolved.insert(id.clone(), "no_citation"); continue; }
            Err(key::KeyError::Multiple(_)) => { unresolved.insert(id.clone(), "several_citations"); continue; }
        }
        let court = previous["observed"][id.as_str()]["court"].as_str().map(str::to_owned)
            .unwrap_or_else(|| record["dataset"].as_str().unwrap_or_default().to_lowercase());
        kept.push((id, canonical, court));
    }
    let mut members: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for id in 0..identities.keys.len() as u32 {
        members.entry(find(&mut identities.root, id)).or_default().push(id);
    }
    let (mut targets, mut heads) = (BTreeMap::<String, Value>::new(), HashMap::<u32, (String, String)>::new());
    for (root, ids) in &members {
        if ids.len() < 2 { continue; }
        let head = identities.decision[*root as usize].unwrap_or_else(|| *ids.iter()
            .min_by_key(|&&id| (identities.rank[id as usize], &identities.keys[id as usize])).unwrap());
        let citation = citations.get(&head).or(evidence.forms.get(head as usize)).ok_or("identity citation")?.clone();
        let key = identities.keys[head as usize].clone();
        for &id in ids.iter().filter(|&&id| id != head && !refused.contains(&id)) {
            let provenance = own_records.get(&id).cloned().unwrap_or_else(|| vec!["inventory".to_owned()]);
            targets.insert(identities.keys[id as usize].clone(), json!({"key": key, "citation": citation, "records": provenance}));
        }
        heads.insert(*root, (key, citation));
    }
    let mut observed = BTreeMap::<String, Value>::new();
    for (id, canonical, court) in kept {
        let root = find(&mut identities.root, canonical);
        let (key, citation) = heads.get(&root).cloned()
            .unwrap_or_else(|| (identities.keys[canonical as usize].clone(), citations[&canonical].clone()));
        observed.insert(id.clone(), json!({"key": key, "citation": citation, "records": [id], "court": court}));
    }
    let evidence_files: Vec<String> = paths.iter().filter_map(|path| std::path::Path::new(path).file_name())
        .map(|name| name.to_string_lossy().into_owned()).collect();
    let table = json!({
        "keyVersion": key::KEY_VERSION,
        "source": previous["source"],
        "inventory": {
            "engine": env!("CARGO_PKG_VERSION"), "evidence": evidence_files,
            "gates": {"journalAgreement": gates.journal_agreement, "decisionAgreement": gates.decision_agreement,
                "digitalAgreement": gates.digital_agreement, "yearsApart": gates.years,
                "anchor": gates.anchor, "distinct": gates.distinct, "order": gates.order},
            "identities": heads.len(), "targets": targets.len(),
        },
        "aliases": previous["aliases"], "reviews": previous["reviews"],
        "unresolved": unresolved, "conflicts": conflicts, "blockedForms": previous["blockedForms"],
        "targets": targets, "observed": observed,
    });
    let mut text = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut text, serde_json::ser::PrettyFormatter::with_indent(b" "));
    serde::Serialize::serialize(&table, &mut serializer)?;
    text.push(b'\n');
    std::fs::write(out, text)?;
    eprintln!("{} identities, {} targets, {} observed, {} unresolved, {} conflicts", heads.len(), table["targets"].as_object().map_or(0, |targets| targets.len()),
        table["observed"].as_object().map_or(0, |observed| observed.len()), table["unresolved"].as_object().map_or(0, |unresolved| unresolved.len()),
        table["conflicts"].as_object().map_or(0, |conflicts| conflicts.len()));
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let valued = ["--write", "--from", "--judge", "--years"];
    let value = |name: &str| args.iter().position(|arg| arg == name).and_then(|at| args.get(at + 1)).cloned();
    let paths: Vec<String> = args.iter().enumerate()
        .filter(|(at, arg)| !arg.starts_with("--") && !(*at > 0 && valued.contains(&args[at - 1].as_str())))
        .map(|(_, arg)| arg.clone()).collect();
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
            ("digital agreement on", Gates { digital_agreement: true, ..on }),
            ("decision and digital agreement on", Gates { decision_agreement: true, digital_agreement: true, ..on }),
            ("years over 2 apart refused", Gates { years: Some(2), ..on }),
            ("years over 1 apart refused", Gates { years: Some(1), ..on }),
            ("anchor off", Gates { anchor: false, ..on }),
            ("distinct off", Gates { distinct: false, ..on }),
            ("order off", Gates { order: false, ..on }),
            ("all gates off", Gates { journal_agreement: false, decision_agreement: false, digital_agreement: false, anchor: false,
                distinct: false, order: false, years: None }),
        ]
    } else {
        vec![("chosen gates", Gates {
            journal_agreement: !flag("--no-journal-agreement"), decision_agreement: flag("--decision-agreement"),
            digital_agreement: flag("--digital-agreement"),
            anchor: !flag("--no-anchor"), distinct: !flag("--no-distinct"), order: !flag("--no-order"),
            years: value("--years").and_then(|apart| apart.parse().ok()),
        })]
    };
    if let Some(path) = value("--judge") {
        let pages = pages(&path)?;
        println!("| configuration | provider | linked | split | wrong | unbound | identities | aliases |");
        println!("|---|---|---|---|---|---|---|---|");
        for (name, gates) in &configurations {
            let binding = bind(&evidence, *gates, false, &mut readings);
            let score = score(&evidence, &binding);
            for (provider, [linked, split, wrong, unbound]) in judge(&evidence, &binding, &pages) {
                println!("| {name} | {provider} | {linked} | {split} | {wrong} | {unbound} | {} | {} |", score.identities, score.aliases);
            }
        }
        return Ok(());
    }
    if let Some(out) = value("--write") {
        let gates = configurations[0].1;
        let binding = bind(&evidence, gates, false, &mut readings);
        return write(&evidence, &binding, &value("--from").unwrap_or_else(|| out.clone()), &out, &paths, gates);
    }
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
        ("all gates off", Gates { journal_agreement: false, decision_agreement: false, digital_agreement: false, anchor: false,
            distinct: false, order: false, years: None })];
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
