//! Port of ALR verifier_core/supra_fallbacks.py's short-form inference.
//! Patterns retain their source definitions in the grammar corpus.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::LazyLock};
use unicode_casefold::UnicodeCaseFold;

macro_rules! pattern {
    ($name:ident, $id:literal) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| {
            let entry = &legal_grammar::load_tables().unwrap()[$id].entry;
            // These source routines use Python's Unicode regex mode, not
            // the ECMAScript/ASCII mode used by the citation tables.
            regex::RegexBuilder::new(&entry.pattern)
                .case_insensitive(entry.flags.contains('i')).build().unwrap()
        });
    };
}
pattern!(SIGNAL, "ref.infer.signal");
pattern!(EXPLICIT_SKIP, "ref.infer.explicit-skip");
pattern!(EXPLICIT, "ref.infer.explicit");
pattern!(CASE_CITE, "ref.infer.case-cite");
pattern!(ACT, "ref.infer.act");
pattern!(PARTIES, "ref.infer.parties");
pattern!(CROWN, "ref.infer.crown");
pattern!(REFERENCE, "ref.token");
pattern!(AUTHORS, "ref.author-separator");
pattern!(AUTHOR_TOKEN, "ref.author-token");
pattern!(HINT, "ref.infer.hint");
pattern!(HINT_SIGNAL, "ref.infer.hint-signal");
pattern!(NOTE, "ref.infer.note");
pattern!(REGISTRY_NOTE, "ref.registry.note");
pattern!(REGISTRY_N, "ref.registry.n");
pattern!(REGISTRY_NN, "ref.registry.nn");
pattern!(REGISTRY_SIGNAL, "ref.registry.signal");
pattern!(BRACKET_SKIP, "ref.registry.bracket-skip");
pattern!(SUPRA, "ref.registry.supra");
pattern!(SUPRA_HINT_SIGNAL, "ref.registry.hint-signal");
pattern!(SUPRA_HINT_START, "ref.registry.hint-start");
pattern!(SUPRA_HINT_ANY, "ref.registry.hint-any");
pattern!(HEREINAFTER, "ref.registry.hereinafter");
pattern!(ANCHOR_PARAGRAPH, "ref.anchor.paragraph");
pattern!(ANCHOR_PROVISION, "ref.anchor.provision");
pattern!(ANCHOR_SCOPE, "ref.anchor.scope");
pattern!(IBID, "ref.anchor.ibid");

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct InferredShortForm {
    pub value: String,
    pub rule: &'static str,
}

pub fn normalize(value: &str) -> String {
    // Python's Unicode \w is letters, numbers and underscore; unlike Rust
    // regex \w it does not include combining marks or join controls.
    static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}\p{N}_]").unwrap());
    WORD.find_iter(value).map(|matched| matched.as_str()).collect::<String>()
        .as_str().case_fold().collect()
}

fn has_explicit(text: &str) -> bool {
    EXPLICIT.captures_iter(text).any(|captures| {
        let value = captures[1].trim();
        value.chars().any(|character| character.is_ascii_alphabetic()) && !EXPLICIT_SKIP.is_match(value)
    })
}

pub(crate) fn surname(name: &str) -> Option<String> {
    AUTHOR_TOKEN.find_iter(name).map(|token| token.as_str()).filter(|token|
            !matches!(token.case_fold().collect::<String>().as_str(), "et" | "al" | "eds" | "ed" | "kc" | "qc"))
            .last().map(|surname| surname.trim_matches('.').to_owned())
}

fn surname_list(prefix: &str) -> Vec<String> {
    AUTHORS.split(prefix).filter_map(surname).collect()
}

fn author_short_form(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [left, right] => format!("{left}, {right}"),
        many => format!("{} & {}", many[..many.len() - 1].join(", "), many.last().unwrap()),
    }
}

pub fn infer(text: &str, kind: &str) -> Vec<InferredShortForm> {
    if text.is_empty() || REFERENCE.is_match(text) || has_explicit(text) { return Vec::new(); }
    let cleaned = SIGNAL.replace(text.trim(), "");
    let clean = cleaned.trim();
    let mut forms = Vec::new();
    let mut add = |value: &str, rule| forms.push(InferredShortForm { value: value.to_owned(), rule });
    let kind = kind.case_fold().collect::<String>();
    if matches!(kind.as_str(), "case" | "unreported") {
        let style = CASE_CITE.find(clean).map_or_else(
            || clean.split(',').next().unwrap_or(clean).trim(),
            |citation| clean[..citation.start()].trim_end_matches([' ', ',', '.', ';']));
        if let Some(parts) = PARTIES.captures(style) {
            let (left, right) = (parts[1].trim(), parts[2].trim());
            add(style, "case_style");
            if !CROWN.is_match(left) { add(left, "case_party"); }
            add(right, "case_party");
        }
    }
    if matches!(kind.as_str(), "statute" | "legislation" | "regulation") {
        if let Some(captures) = ACT.captures(clean) {
            let title = captures[1].trim();
            add(title, "legislation_title");
            let words = title.split_whitespace().collect::<Vec<_>>();
            let acronym = words.iter().filter_map(|word| word.chars().next())
                .filter(|character| character.is_uppercase()).collect::<String>();
            if words.len() >= 3 && acronym.chars().count() >= 2 { add(&acronym, "legislation_acronym"); }
        }
    }
    if matches!(kind.as_str(), "journal" | "book" | "essay_collection" | "report" | "article" | "website" | "news") {
        let prefix = clean.find(['"', '\u{201c}']).map_or_else(
            || clean.split(',').next().unwrap_or(clean).trim(),
            |position| clean[..position].trim_matches([' ', ',']));
        let short = author_short_form(&surname_list(prefix));
        if !short.is_empty() { add(&short, "secondary_authors"); }
    }
    let mut seen = HashSet::new();
    forms.retain(|form| !form.value.is_empty() && seen.insert(normalize(&form.value)));
    forms
}

pub fn reference_candidates(text: &str) -> Vec<String> {
    let hint = HINT.captures_iter(text).last().map(|captures|
        captures[1].trim_matches([' ', ',', ';', ':', '.']).to_owned()).unwrap_or_default();
    reference_candidates_for_hint(&hint)
}

pub(crate) fn reference_candidates_for_hint(hint: &str) -> Vec<String> {
    let hint = HINT_SIGNAL.replace(hint, "");
    let mut candidates = Vec::new();
    if !hint.is_empty() { candidates.push(hint.to_string()); }
    candidates.extend(infer(&hint, "case").into_iter().map(|form| form.value));
    let author = author_short_form(&surname_list(&hint));
    if !author.is_empty() { candidates.push(author); }
    let mut seen = HashSet::new();
    candidates.retain(|value| {
        let key = normalize(value);
        !key.is_empty() && seen.insert(key)
    });
    candidates
}

/// Source records already produced by the caller's retrieval pipeline.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ReferenceSource {
    pub note: serde_json::Value,
    pub sequence: Option<u32>,
    pub verbatim: Option<String>,
    pub link: Option<String>,
    pub short_form: Option<String>,
    pub short_form_norm: Option<String>,
    pub rule: Option<String>,
}

impl ReferenceSource {
    fn link(&self) -> &str { self.link.as_deref().unwrap_or("") }
    fn usable(&self) -> bool {
        let link = self.link().trim();
        !link.is_empty() && link.case_fold().collect::<String>() != "other"
    }
    fn base(&self) -> String {
        self.link().split('#').next().unwrap_or("").trim_end_matches('/')
            .case_fold().collect()
    }
    fn note(&self) -> String {
        match &self.note {
            serde_json::Value::String(note) => note.clone(),
            serde_json::Value::Number(note) if note.as_f64() != Some(0.0) => note.to_string(),
            _ => String::new(),
        }
    }
}

fn ref_normalize(value: &str) -> String {
    value.split(crate::text::python_whitespace).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
        .replace(['‘', '’'], "'").replace(['“', '”'], "\"")
        .replace(['–', '—'], "-").to_lowercase()
}

/// ALR's reference markers, preserving the note/n/nn pattern precedence.
/// Decimal strings let each binding retain Python's unbounded note integers.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ReferenceInfo {
    pub kind: &'static str,
    pub notes: Vec<String>,
    pub normalized: String,
}

pub fn reference_info(text: &str) -> ReferenceInfo {
    let notes = [&*REGISTRY_NOTE, &*REGISTRY_N, &*REGISTRY_NN].into_iter()
        .flat_map(|pattern| pattern.captures_iter(text).map(|found| found[1].to_owned()))
        .collect();
    ReferenceInfo {
        kind: if IBID.is_match(text) { "ibid" } else if SUPRA.is_match(text) { "supra" } else { "" },
        notes,
        normalized: ref_normalize(text),
    }
}

fn ref_tokens(value: &str) -> Vec<String> {
    ref_normalize(value).trim_matches(['[', ']', '(', ')'])
        .split(|c: char| !c.is_ascii_lowercase() && !c.is_ascii_digit())
        .filter(|token| token.len() >= 3 && !matches!(*token,
            "see" | "also" | "but" | "and" | "the" | "note" | "supra" | "ibid" |
            "generally" | "contra" | "compare" | "with" | "above" | "e.g" | "eg"))
        .map(str::to_owned).collect()
}

pub fn supra_hint(text: &str, aggressive: bool) -> String {
    let cleaned = SUPRA_HINT_SIGNAL.replace(text, "");
    let trim_hint = |hint: &str| hint
        .trim_start_matches(|c: char| c.is_whitespace() || "([{\"'“”".contains(c))
        .trim_end_matches(|c: char| c.is_whitespace() || ")]}\"'“”".contains(c))
        .split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(found) = SUPRA_HINT_START.captures(&cleaned) {
        return trim_hint(&found[1]);
    }
    if aggressive {
        if let Some(found) = SUPRA_HINT_ANY.captures(text) {
            let hint = trim_hint(&found[1]);
            if hint.chars().any(|c| c.is_ascii_alphabetic()) { return hint; }
        }
    }
    String::new()
}

pub fn fallback_hint(text: &str) -> String {
    let Some(found) = SUPRA.find(text) else { return String::new(); };
    let mut prefix = text[..found.start()].trim();
    while let Some(signal) = REGISTRY_SIGNAL.find(prefix) {
        prefix = prefix[signal.end()..].trim();
    }
    let prefix = prefix.trim_matches([' ', ',', ';', ':', '.']);
    prefix.chars().skip(prefix.chars().count().saturating_sub(80)).collect()
}

pub(crate) fn supra_position(text: &str) -> Option<usize> {
    SUPRA.find(text).map(|found| found.start())
}

fn match_text(entry: &ReferenceSource) -> String {
    ref_normalize(&format!("{} {}", entry.short_form.as_deref().unwrap_or(""),
        entry.verbatim.as_deref().unwrap_or("").chars().take(200).collect::<String>()))
}

fn ref_base(link: &str) -> String {
    // The strict registry source trims whitespace, not trailing slashes.
    link.split('#').next().unwrap_or("").trim().to_lowercase()
}

/// ALR's strict registry resolver. Keep numbered-note, short-form and final
/// bracket-definition tiers in source order, including unlinked-target vetoes.
pub fn resolve_registry(text: &str, registry: &[ReferenceSource], aggressive: bool) -> (String, &'static str) {
    resolve_registry_scoped(text, registry, aggressive, None)
}

pub(crate) fn resolve_registry_scoped(text: &str, registry: &[ReferenceSource], aggressive: bool,
    sequence: Option<u32>) -> (String, &'static str) {
    let hint = supra_hint(text, aggressive);
    let hint = if hint.is_empty() { fallback_hint(text) } else { hint };
    resolve_registry_hint_scoped(text, &hint, registry, sequence)
}

/// The same registry tiers for a name already captured by citation discovery.
pub(crate) fn resolve_registry_hint(text: &str, hint: &str, registry: &[ReferenceSource]) -> (String, &'static str) {
    resolve_registry_hint_scoped(text, hint, registry, None)
}

fn resolve_registry_hint_scoped(text: &str, hint: &str, registry: &[ReferenceSource],
    sequence: Option<u32>) -> (String, &'static str) {
    let normalized = ref_normalize(hint);
    let normalized = normalized.trim_matches(['[', ']', '(', ')', ' ']);
    let tokens = ref_tokens(&hint);
    if normalized.is_empty() && tokens.is_empty() { return (String::new(), "abstain_no_hint"); }
    let linked = registry.iter().filter(|entry| {
        let link = entry.link().trim();
        !link.is_empty() && link.to_lowercase() != "other"
    }).collect::<Vec<_>>();
    let matches_tokens = |value: &str| tokens.iter().all(|token| value.contains(token.as_str()));
    if !tokens.is_empty() {
        let number = [&*REGISTRY_NOTE, &*REGISTRY_N, &*REGISTRY_NN].into_iter()
            .find_map(|pattern| pattern.captures(text))
            .and_then(|found| crate::text::decimal(&found[1])).map(|number| number.to_string());
        if let Some(number) = number {
            let local = sequence.filter(|&sequence| registry.iter().any(|entry|
                entry.note.as_str() == Some(number.as_str()) && entry.sequence == Some(sequence)));
            if local.is_none() && sequence.is_some() && registry.iter().filter(|entry|
                entry.note.as_str() == Some(number.as_str()))
                .filter_map(|entry| entry.sequence).collect::<HashSet<_>>().len() > 1 {
                return (String::new(), "abstain_ambiguous_note_number_scope");
            }
            // Source records have string note labels. Numeric JSON notes do
            // not satisfy the original Python equality against str(note_n).
            let in_note = |entry: &&ReferenceSource| entry.note.as_str() == Some(number.as_str())
                && local.is_none_or(|sequence| entry.sequence == Some(sequence));
            let matching = linked.iter().copied().filter(in_note)
                .filter(|entry| matches_tokens(&match_text(entry))).collect::<Vec<_>>();
            let bases = matching.iter().map(|entry| ref_base(entry.link())).collect::<HashSet<_>>();
            if bases.len() == 1 {
                return (matching[0].link().to_owned(), "note_number");
            }
            if bases.len() > 1 {
                return match bracket_definition(normalized, matching.into_iter()) {
                    Some(link) => (link, "bracket_definition"),
                    None => (String::new(), "abstain_ambiguous_note_number"),
                };
            }
            let suffix = registry.iter().filter(in_note).filter(|entry| {
                let short = ref_tokens(entry.short_form.as_deref().unwrap_or(""));
                short.len() >= 2 && tokens.ends_with(&short)
            }).collect::<Vec<_>>();
            let linked_suffix = suffix.iter().copied().filter(|entry| {
                let link = entry.link().trim();
                !link.is_empty() && link.to_lowercase() != "other"
            }).collect::<Vec<_>>();
            let bases = linked_suffix.iter().map(|entry| ref_base(entry.link())).collect::<HashSet<_>>();
            if bases.len() == 1 { return (linked_suffix[0].link().to_owned(), "note_number_short_form_suffix"); }
            if !linked_suffix.is_empty() { return (String::new(), "abstain_ambiguous_note_number_suffix"); }
            if !suffix.is_empty() || registry.iter().filter(in_note).any(|entry| matches_tokens(&match_text(entry))) {
                return (String::new(), "abstain_unlinked_target");
            }
        }
    }
    let mut abstain = "abstain_no_match";
    for (method, ambiguous) in [
        ("exact_sf", "abstain_ambiguous_exact_sf"),
        ("token_sf", "abstain_ambiguous_token_sf"),
        ("token_verb", "abstain_ambiguous_token_verb"),
    ] {
        let pool = linked.iter().copied().filter(|entry| match method {
            "exact_sf" => !normalized.is_empty() && ref_normalize(entry.short_form.as_deref().unwrap_or(""))
                .trim_matches(['[', ']', '(', ')', ' ']) == normalized,
            "token_sf" => !tokens.is_empty() && entry.short_form.as_ref().is_some_and(|short|
                !short.is_empty() && matches_tokens(&ref_normalize(short))),
            _ => !tokens.is_empty() && entry.verbatim.as_ref().is_some_and(|verbatim|
                !verbatim.is_empty() && matches_tokens(&match_text(entry))),
        }).collect::<Vec<_>>();
        if pool.is_empty() { continue; }
        let bases = pool.iter().map(|entry| ref_base(&entry.link().split_whitespace()
            .collect::<Vec<_>>().join(" "))).collect::<HashSet<_>>();
        if bases.len() == 1 { return (pool[0].link().to_owned(), method); }
        abstain = ambiguous;
        break;
    }
    if let Some(link) = bracket_definition(normalized, linked.into_iter()) {
        return (link, "bracket_definition");
    }
    (String::new(), abstain)
}

fn bracket_definition<'a>(normalized: &str, entries: impl Iterator<Item = &'a ReferenceSource>) -> Option<String> {
    if normalized.is_empty() { return None; }
    let mut bases = HashSet::new();
    let mut best = "";
    for entry in entries {
        for bracket in EXPLICIT.captures_iter(entry.verbatim.as_deref().unwrap_or("")) {
            if BRACKET_SKIP.is_match(bracket[1].trim()) { continue; }
            for piece in bracket[1].split(';') {
                let piece = HEREINAFTER.replace(piece, "");
                if ref_normalize(&piece).trim_matches(['[', ']', '(', ')', ' ']) == normalized {
                    bases.insert(ref_base(&entry.link().split_whitespace().collect::<Vec<_>>().join(" ")));
                    best = entry.link();
                }
            }
        }
    }
    (bases.len() == 1).then(|| best.to_owned())
}

/// ALR's resolve_after_strict_abstention, retaining its two existing tiers.
pub fn resolve_after_strict_abstention(
    text: &str, registry: &[ReferenceSource], inferred_forms: &[ReferenceSource],
) -> (String, String) {
    let candidates = reference_candidates(text);
    if let Some(note) = NOTE.captures(text).filter(|_| candidates.is_empty()) {
        let pool = registry.iter().filter(|item| item.note() == note[1]
            && !REFERENCE.is_match(item.verbatim.as_deref().unwrap_or(""))).collect::<Vec<_>>();
        if let [item] = pool.as_slice() {
            if item.usable() { return (item.link().to_owned(), "bare_note_unique_citation".into()); }
        }
    }
    resolve_inferred_candidates(&candidates, registry, inferred_forms)
}

pub(crate) fn resolve_inferred_candidates(
    candidates: &[String], registry: &[ReferenceSource], inferred_forms: &[ReferenceSource],
) -> (String, String) {
    let keys = candidates.iter().map(|value| normalize(value)).collect::<HashSet<_>>();
    let inferred = inferred_forms.iter().filter(|item|
        item.short_form_norm.as_ref().is_some_and(|key| keys.contains(key))).collect::<Vec<_>>();
    let Some(chosen) = inferred.last() else { return (String::new(), String::new()); };
    let authoritative = registry.iter().filter(|item| item.usable()
        && keys.contains(&normalize(item.short_form.as_deref().unwrap_or(""))));
    let bases = inferred.iter().copied().chain(authoritative).map(ReferenceSource::base).collect::<HashSet<_>>();
    if bases.len() != 1 { return (String::new(), String::new()); }
    (chosen.link().to_owned(), format!("inferred_short_form:{}", chosen.rule.as_deref().unwrap_or("")))
}

/// ALR's _reanchor_ref_link: own-scope pinpoints replace inherited fragments;
/// bare supra removes the fragment, while bare ibid retains it verbatim.
pub fn reanchor_reference(origin_link: &str, text: &str) -> String {
    let link = origin_link.split(crate::text::python_whitespace)
        .filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ");
    if link.is_empty() || link.to_lowercase() == "other" || !link.to_lowercase().contains("canlii.org") {
        return link;
    }
    let base = link.split('#').next().unwrap_or("").trim();
    let scope = ANCHOR_SCOPE.split(text).find(|scope| REFERENCE.is_match(scope))
        .unwrap_or_else(|| ANCHOR_SCOPE.split(text).next().unwrap_or(""));
    if base.to_lowercase().contains("/doc/") {
        if let Some(pin) = ANCHOR_PARAGRAPH.captures(scope) { return format!("{base}#par{}", &pin[1]); }
    }
    if base.to_lowercase().contains("/laws/") {
        if let Some(pin) = ANCHOR_PROVISION.captures(scope) {
            if let Some(number) = pin.name("section").or_else(|| pin.name("rule")).or_else(|| pin.name("article")) {
                return format!("{base}#sec{}", number.as_str());
            }
        }
    }
    if !IBID.is_match(text) && SUPRA.is_match(text) { base.to_owned() } else { link }
}
