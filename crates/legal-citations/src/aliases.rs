//! Reporter parallels supported by the direct ALR/A2AJ evidence table.
//! Direct observed spellings may identify a decision even without a structural
//! key. Contradictory identities and invalid spellings cannot authorize a link.

use crate::{key, registry, Citation, Form, Format};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// One citation's complete alias closure from an installed source index.
/// An ambiguous index supplies only the citation's own key.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct SourceAliasGroup {
    pub index: usize,
    pub keys: Vec<String>,
}

/// Beaver's reciprocal-closure check, formerly `parallelCaseKeys` in
/// authoritiesImport.ts. An observed key must agree with every other member's
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
    /// Original ALR evidence-record identifiers in registry/aliases.json.
    /// These are provenance references, never runtime lookup keys.
    pub records: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Table {
    key_version: String,
    targets: HashMap<String, AliasTarget>,
    blocked_forms: HashSet<String>,
    #[serde(default)]
    observed: HashMap<String, Observed>,
    #[serde(skip)]
    target_courts: HashMap<String, String>,
}

#[derive(Deserialize)]
struct Observed {
    #[serde(flatten)]
    target: AliasTarget,
    court: String,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let mut table: Table = serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/aliases.json")))
        .expect("validated reporter aliases");
    assert_eq!(table.key_version, key::KEY_VERSION, "alias keys must match this engine");
    let record_courts: HashMap<_, _> = table.observed.values().flat_map(|evidence|
        evidence.target.records.iter().map(move |id| (id, &evidence.court))).collect();
    for (key, target) in &table.targets {
        let courts: Option<HashSet<_>> = target.records.iter().map(|id| record_courts.get(id)).collect();
        if let Some(courts) = courts.filter(|courts| courts.len() == 1) {
            table.target_courts.insert(key.clone(), (***courts.iter().next().unwrap()).clone());
        }
    }
    table
});

/// ALR A2AJClient._reporter_alias: an observed spelling is source evidence,
/// including misspellings that have no structural reporter interpretation.
pub(crate) fn observed_form(text: &str) -> bool {
    TABLE.observed.contains_key(&registry::fold(text))
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

pub(crate) fn for_key(citation: &Citation, structural_key: &str) -> Option<&'static AliasTarget> {
    if citation.is_ambiguous() || TABLE.blocked_forms.contains(&registry::fold(&citation.span.text)) {
        return None;
    }
    if TABLE.target_courts.get(structural_key).is_some_and(|court| !supports_court(citation, court)) {
        return None;
    }
    TABLE.targets.get(structural_key)
}

fn observed(citation: &Citation) -> Option<&'static Observed> {
    let form = registry::fold(&citation.span.text);
    if TABLE.blocked_forms.contains(&form) { return None; }
    let evidence = TABLE.observed.get(&form)?;
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
    if !supports_court(citation, &evidence.court) { return None; }
    Some(evidence)
}

pub(crate) fn observed_court(citation: &Citation) -> Option<&'static registry::Court> {
    registry::registry().court(&observed(citation)?.court)
}

pub fn resolve(citation: &Citation) -> Option<&'static AliasTarget> {
    if citation.form != Form::Full || citation.is_ambiguous() { return None; }
    let structural = key::key_in(citation, registry::registry())
        .and_then(|key| for_key(citation, &key));
    if let Some(evidence) = observed(citation) {
        // Keep the full provenance for equivalent structural spellings. An
        // exact observed form still controls when the structural reading differs.
        return structural.filter(|target| target.key == evidence.target.key).or(Some(&evidence.target));
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
        for (alias_key, target) in &TABLE.targets {
            assert!(alias_key.starts_with(&format!("{}:", key::KEY_VERSION)));
            assert!(target.key.starts_with(&format!("{}:", key::KEY_VERSION)));
            assert!(!target.records.is_empty());
            if let Some(next) = TABLE.targets.get(&target.key) {
                assert_eq!(next.key, target.key, "unreviewed identity chain");
            }
            accounted.extend(target.records.iter().map(String::as_str));
        }
        assert_eq!(accounted, aliases.keys().map(String::as_str).collect());
        for review in data["reviews"].as_object().unwrap().values() {
            assert!(!review["evidence"].as_array().unwrap().is_empty());
        }
    }
}
