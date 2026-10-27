//! Reporter parallels supported by the direct A2AJ evidence table.
//! Direct observed spellings may identify a decision even without a structural
//! key. Contradictory identities and invalid spellings cannot authorize a link.

use crate::{key, registry, Citation, Form, Format};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use legal_grammar::table::{Blob, Rows, Strings, Words};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

static IN_USE: AtomicBool = AtomicBool::new(true);

/// Whether citations are read with this table's evidence (the default) or from the grammar alone, for the whole
/// process: the alias evidence collector (the `alias-evidence` bin) reads the printed citations its table is made from.
pub fn use_evidence(in_use: bool) {
    IN_USE.store(in_use, Ordering::Relaxed);
}

fn in_use() -> bool { IN_USE.load(Ordering::Relaxed) }

/// One citation's complete alias closure from an installed source index.
/// An ambiguous index supplies only the citation's own key.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct SourceAliasGroup {
    pub index: usize,
    pub keys: Vec<String>,
}

/// Reciprocal-closure check: an observed key must agree with every other member's
/// closure; asymmetric or conflicting source inventories cannot merge cases.
pub(crate) fn source_links(citations: &mut [Citation], evidence: &[SourceAliasGroup]) -> Vec<(usize, usize)> {
    if evidence.is_empty() { return Vec::new(); }
    let candidates: HashMap<_, _> = citations.iter().filter(|citation|
        citation.form == Form::Full && !citation.is_ambiguous()
            && matches!(citation.authority, crate::Authority::Case | crate::Authority::Unknown)
            && citation.key.is_some()).map(|citation| (citation.index, citation)).collect();
    let observed: HashSet<_> = candidates.values().filter_map(|citation| citation.key.as_ref()).collect();
    let mut groups: HashMap<Vec<String>, Vec<&Citation>> = HashMap::new();
    let mut signatures: HashMap<&String, HashSet<Vec<String>>> = HashMap::new();
    for item in evidence {
        let Some(&citation) = candidates.get(&item.index) else { continue };
        let key = citation.key.as_ref().unwrap();
        let mut closure = item.keys.clone();
        closure.sort();
        closure.dedup();
        if !closure.contains(key) { continue; }
        signatures.entry(key).or_default().insert(closure.clone());
        groups.entry(closure).or_default().push(citation);
    }
    let mut links = Vec::new();
    let mut conflicts = HashSet::new();
    for (closure, mut group) in groups {
        group.sort_by_key(|citation| (citation.span.start, citation.index));
        let mut keys = HashSet::new();
        let unique: Vec<_> = group.iter().filter(|citation| keys.insert(citation.key.as_ref().unwrap())).collect();
        if closure.iter().any(|key| observed.contains(key)
                && signatures.get(key).is_none_or(|signatures| signatures.len() != 1 || !signatures.contains(&closure)))
            || unique.iter().enumerate().any(|(index, left)| unique[index + 1..].iter()
                .any(|right| crate::parallel::distinct(left, right))) {
            conflicts.extend(group.iter().map(|citation| citation.index));
            continue;
        }
        if keys.len() < 2 || !group.iter().any(|citation| citation.authority == crate::Authority::Case) { continue; }
        links.extend(group[1..].iter().map(|citation| (group[0].index, citation.index)));
    }
    for citation in citations {
        if conflicts.contains(&citation.index) && !citation.reasons.iter().any(|reason| reason == "source_alias_conflict") {
            citation.reasons.push("source_alias_conflict".into());
        }
    }
    links
}

pub const DATA: &str = include_str!("../registry/aliases.json");

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct AliasTarget {
    pub key: String,
    pub citation: String,
    /// Original evidence-record identifiers in registry/aliases.json.
    /// These are provenance references, never runtime lookup keys.
    pub records: Vec<String>,
}

/// The alias mappings build.rs compiles from registry/aliases.json: targets by structural key and
/// observed spellings by folded form (each with its target, records and court), and the blocked forms.
struct Table {
    strings: Strings,
    targets: Rows,
    observed: Rows,
    records: Words,
    blocked: Rows,
}

const NO_COURT: u32 = u32::MAX;

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let blob = Blob::new(include_bytes!(concat!(env!("OUT_DIR"), "/aliases.bin")));
    let strings = blob.strings(0);
    assert_eq!(strings.get(blob.words(2).get(0)), key::KEY_VERSION, "alias keys must match this engine");
    Table { strings, targets: blob.rows(3, 6), observed: blob.rows(4, 6), records: blob.words(5), blocked: blob.rows(6, 1) }
});

impl Table {
    /// The target of a `rows` row: its key, citation and records.
    fn target(&self, rows: Rows, row: usize) -> AliasTarget {
        AliasTarget {
            key: self.strings.get(rows.get(row, 1)).to_owned(),
            citation: self.strings.get(rows.get(row, 2)).to_owned(),
            records: self.records.run(rows.get(row, 3), rows.get(row, 4))
                .map(|id| self.strings.get(id).to_owned()).collect(),
        }
    }

    fn court(&self, rows: Rows, row: usize) -> Option<&'static str> {
        Some(rows.get(row, 5)).filter(|&id| id != NO_COURT).map(|id| self.strings.get(id))
    }

    fn blocked(&self, form: &str) -> bool {
        self.blocked.find(self.strings, 0, form).is_some()
    }
}

/// An observed spelling is source evidence,
/// including misspellings that have no structural reporter interpretation.
pub(crate) fn observed_form(text: &str) -> bool {
    in_use() && TABLE.observed.find(TABLE.strings, 0, &registry::fold(text)).is_some()
}

fn supports_court(citation: &Citation, court: &str) -> bool {
    let registry = registry::registry();
    let reporter = citation.fields.reporter_id.as_deref()
        .and_then(|id| registry.reporters.iter().find(|reporter| reporter.id == id));
    if reporter.zip(registry.court(court)).is_some_and(|(reporter, court)|
        !crate::classify::reporter_supports_court(reporter, court)) { return false; }
    crate::classify::explicit_courts(citation).is_none_or(|courts|
        courts.iter().any(|candidate| candidate.id == court))
        && citation.court.as_ref().is_none_or(|candidate| candidate.id == court)
}

pub(crate) fn for_key(citation: &Citation, structural_key: &str) -> Option<AliasTarget> {
    if !in_use() || citation.is_ambiguous() || TABLE.blocked(&registry::fold(&citation.span.text)) {
        return None;
    }
    let row = TABLE.targets.find(TABLE.strings, 0, structural_key)?;
    if TABLE.court(TABLE.targets, row).is_some_and(|court| !supports_court(citation, court)) {
        return None;
    }
    Some(TABLE.target(TABLE.targets, row))
}

/// The observed row of the citation's written form, when that form identifies its record and court.
fn observed(citation: &Citation) -> Option<usize> {
    if !in_use() { return None; }
    let form = registry::fold(&citation.span.text);
    if TABLE.blocked(&form) { return None; }
    let evidence = TABLE.observed.find(TABLE.strings, 0, &form)?;
    // A bare volume/page can recur when a reporter uses the year as its
    // volume or changes numbering regimes. The source index's observed row
    // identifies its record, but the shorter written form does not identify
    // that record without a publication year.
    if citation.format == Some(Format::Reporter) && citation.fields.year.is_none() {
        let registry = registry::registry();
        let surface = citation.fields.reporter.as_deref()?;
        if registry.reporters_by_surface(surface).len() == 1
            && key::key_in(citation, registry).is_none() { return None; }
    }
    if !supports_court(citation, observed_court_id(evidence)) { return None; }
    Some(evidence)
}

fn observed_court_id(row: usize) -> &'static str {
    TABLE.court(TABLE.observed, row).expect("observed court")
}

pub(crate) fn observed_court(citation: &Citation) -> Option<&'static registry::Court> {
    registry::registry().court(observed_court_id(observed(citation)?))
}

pub fn resolve(citation: &Citation) -> Option<AliasTarget> {
    if citation.form != Form::Full || citation.is_ambiguous() { return None; }
    let structural = key::key_in(citation, registry::registry())
        .and_then(|key| for_key(citation, &key));
    if let Some(evidence) = observed(citation) {
        // Keep the full provenance for equivalent structural spellings. An
        // exact observed form still controls when the structural reading differs.
        let key = TABLE.strings.get(TABLE.observed.get(evidence, 1));
        return structural.filter(|target| target.key == key).or_else(|| Some(TABLE.target(TABLE.observed, evidence)));
    }
    structural
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn every_source_record_is_preserved_and_accounted_for() {
        use sha2::{Digest, Sha256};
        let data: Value = serde_json::from_str(DATA).unwrap();
        let aliases = data["aliases"].as_object().unwrap();
        assert_eq!(aliases.len(), 26_546);
        assert_eq!(data["source"]["sha256"], "d5c65c1d481e8439d6f4508b3ba62199b06fb6d9c6702b4feb83081d8ddcaf29");
        assert_eq!(format!("{:x}", Sha256::digest(data["aliases"].to_string().as_bytes())),
            "239e47e1d23c6383c141a47fb36d4f7ea4e913100b5153cfc457c28f5ce6b3d7");
        let mut accounted: HashSet<&str> = HashSet::new();
        for section in ["reviews", "unresolved"] {
            accounted.extend(data[section].as_object().unwrap().keys().map(String::as_str));
        }
        for members in data["conflicts"].as_object().unwrap().values() {
            accounted.extend(members.as_array().unwrap().iter().map(|id| id.as_str().unwrap()));
        }
        let mut records = Vec::new();
        for row in 0..TABLE.targets.len() {
            let (alias_key, target) = (TABLE.strings.get(TABLE.targets.get(row, 0)), TABLE.target(TABLE.targets, row));
            assert!(alias_key.starts_with(&format!("{}:", key::KEY_VERSION)));
            assert!(target.key.starts_with(&format!("{}:", key::KEY_VERSION)));
            assert!(!target.records.is_empty());
            if let Some(next) = TABLE.targets.find(TABLE.strings, 0, &target.key) {
                assert_eq!(TABLE.target(TABLE.targets, next).key, target.key, "unreviewed identity chain");
            }
            records.extend(target.records);
        }
        for row in 0..TABLE.observed.len() {
            records.extend(TABLE.target(TABLE.observed, row).records);
        }
        // A target only the alias inventory's evidence names cites it (`inventory`), no ALR record.
        accounted.extend(records.iter().map(String::as_str).filter(|record| *record != "inventory"));
        assert!(data["inventory"].is_object() || !records.iter().any(|record| record == "inventory"));
        assert_eq!(accounted, aliases.keys().map(String::as_str).collect());
        for review in data["reviews"].as_object().unwrap().values() {
            assert!(!review["evidence"].as_array().unwrap().is_empty());
        }
    }
}

/// Build this module's lazily built statics now ([`crate::warm`]).
pub(crate) fn warm() {
    crate::warm_statics!(
        TABLE
    );
}
