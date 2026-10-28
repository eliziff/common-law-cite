//! Short-form inference.
//! Patterns retain their source definitions in the grammar corpus.
//! Canadian/McGill-oriented author and title inference is an aggressive
//! fallback. Exact source identity, note scope and ambiguity are general
//! resolution contracts and do not depend on that inference or on retrieval.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, collections::HashSet, sync::LazyLock};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

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
pattern!(IBID_HINT, "ref.registry.ibid-hint");
pattern!(BARE_PROVISION, "ref.provision.bare");
pattern!(SOURCE_YEAR, "ref.source.year");
pattern!(HEREINAFTER, "ref.registry.hereinafter");
pattern!(ANCHOR_PARAGRAPH, "ref.anchor.paragraph");
pattern!(ANCHOR_PROVISION, "ref.anchor.provision");
pattern!(ANCHOR_SCOPE, "ref.anchor.scope");
pattern!(IBID, "ref.anchor.ibid");
pattern!(READING_SIGNAL, "ref.registry.reading-signal");
pattern!(PUBLICATION_FACTS, "ref.registry.publication-facts");
pattern!(SHORT_FORM_WORDING, "ref.registry.short-form-wording");

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct InferredShortForm {
    pub value: String,
    pub rule: &'static str,
}

// Python's Unicode \w is letters, numbers and underscore; unlike Rust
// regex \w it does not include combining marks or join controls.
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}\p{N}_]").unwrap());
pub fn normalize(value: &str) -> String {
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

/// Sources already identified by the caller. Targets are opaque authority or
/// source identifiers; matching never depends on a URL or retrieval status.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ReferenceSource {
    pub note: serde_json::Value,
    pub sequence: Option<u32>,
    pub verbatim: Option<String>,
    pub target: Option<String>,
    /// Names of the identified source, also inherited by references to it.
    pub names: Vec<String>,
    pub short_form: Option<String>,
    pub short_form_norm: Option<String>,
    pub rule: Option<String>,
    /// The source part the entry was registered from, in a note splitter's rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    /// The note (its position among the resolver's notes) the entry was registered from, which tells
    /// apart notes that carry the same number.
    #[serde(skip)]
    pub note_index: Option<usize>,
    /// Whether the work is cited without a name of its own ("2022 SCC 36", its style of cause in the text).
    #[serde(skip)]
    pub nameless: bool,
}

impl ReferenceSource {
    pub(crate) fn target(&self) -> &str { self.target.as_deref().unwrap_or("") }
    /// Whether the entry's note, as a decimal string (empty when it has none), is `number`.
    pub(crate) fn note_is(&self, number: &str) -> bool {
        match &self.note {
            serde_json::Value::String(note) => note == number,
            serde_json::Value::Number(note) if note.as_f64() != Some(0.0) => note.to_string() == number,
            _ => number.is_empty(),
        }
    }
}

fn ref_normalize(value: &str) -> String {
    value.split(crate::text::python_whitespace).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
        .replace(['‘', '’'], "'").replace(['“', '”'], "\"")
        .replace(['–', '—'], "-").to_lowercase()
}

/// Reference markers, preserving the note/n/nn pattern precedence.
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

/// The name a named ibid writes before it ("Blue, ibid at para 51" names Blue); a bare ibid names none.
pub(crate) fn ibid_hint(text: &str) -> String {
    let cleaned = SUPRA_HINT_SIGNAL.replace(text, "");
    IBID_HINT.captures(&cleaned).map(|found| found[1].split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

/// The provisions of statutes a text writes ("s. 503", "ss 31, 31.1 and 213", "section 12 of the Charter", "(see
/// s 33(5))"): each one's span and the statute it names, if it names one. A letter or apostrophe before the "s" makes
/// it a word's ending ("Canada’s 2020"), no provision.
pub(crate) fn provision_mentions(text: &str) -> Vec<(std::ops::Range<usize>, Option<String>)> {
    BARE_PROVISION.captures_iter(text).filter_map(|found| {
        let whole = found.get(0)?;
        let before = text[..whole.start()].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '’' | '\'' | '‘')) { return None; }
        let name = found.name("name").map(|name| name.as_str().trim().trim_end_matches([',', ';']).to_owned());
        Some((whole.range(), name.filter(|name| !name.is_empty())))
    }).collect()
}

/// Whether a part names a year, as a cited work does and plain prose seldom does.
pub(crate) fn names_year(text: &str) -> bool {
    SOURCE_YEAR.is_match(text)
}

/// The source a name names by its short form, a bracketed definition or a party's name; never by words
/// that merely occur in a source's text ("Poirier" in "… which Johanne Poirier described as …").
pub(crate) fn resolve_named<R: Borrow<ReferenceSource>>(hint: &str, registry: &[R]) -> Option<String> {
    let (resource, method) = resolve_registry_hint_scoped("", hint, registry, None, false);
    if !resource.is_empty() && matches!(method, "exact_sf" | "token_sf" | "bracket_definition") {
        return Some(resource);
    }
    let name = ref_normalize(hint);
    let name = name.trim_matches(['[', ']', '(', ')', ' ', '.']);
    if name.is_empty() { return None; }
    unique_target(entries(registry).filter(|entry| entry.names.iter()
        .any(|party| ref_normalize(party).trim_matches(['[', ']', '(', ')', ' ', '.']) == name))).map(str::to_owned)
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
    ref_normalize(&format!("{} {} {}", entry.short_form.as_deref().unwrap_or(""), entry.names.join(" "),
        entry.verbatim.as_deref().unwrap_or("").chars().take(200).collect::<String>()))
}

/// A registry's entries, whether the caller holds them or refers to them.
fn entries<R: Borrow<ReferenceSource>>(registry: &[R]) -> impl Iterator<Item = &ReferenceSource> + Clone {
    registry.iter().map(Borrow::borrow)
}

fn unique_target<'a>(entries: impl IntoIterator<Item = &'a ReferenceSource>) -> Option<&'a str> {
    let targets = entries.into_iter().map(ReferenceSource::target).collect::<HashSet<_>>();
    (targets.len() == 1).then(|| *targets.iter().next().unwrap()).filter(|target| !target.is_empty())
}

/// Match note numbers, short forms, names and bracket definitions to source
/// identities. An unresolved source remains in the ambiguity pool.
pub fn resolve_registry<R: Borrow<ReferenceSource>>(text: &str, registry: &[R], aggressive: bool) -> (String, &'static str) {
    resolve_registry_scoped(text, registry, aggressive, None, false)
}

/// `by_name`: a supra whose numbered note cites another work, or none, is read by its name, as safe
/// linking reads it, whatever note number the author wrote.
pub(crate) fn resolve_registry_scoped<R: Borrow<ReferenceSource>>(text: &str, registry: &[R], aggressive: bool,
    sequence: Option<u32>, by_name: bool) -> (String, &'static str) {
    let hint = supra_hint(text, aggressive);
    let hint = if hint.is_empty() { fallback_hint(text) } else { hint };
    resolve_registry_hint_scoped(text, &hint, registry, sequence, by_name)
}

/// The same registry tiers for a name already captured by citation discovery.
pub(crate) fn resolve_registry_hint<R: Borrow<ReferenceSource>>(text: &str, hint: &str, registry: &[R]) -> (String, &'static str) {
    resolve_registry_hint_scoped(text, hint, registry, None, false)
}

fn resolve_registry_hint_scoped<R: Borrow<ReferenceSource>>(text: &str, hint: &str, registry: &[R],
    sequence: Option<u32>, by_name: bool) -> (String, &'static str) {
    resolve_hint(text, hint, registry, sequence, by_name, &Matching::default())
}

fn resolve_hint<R: Borrow<ReferenceSource>>(text: &str, hint: &str, registry: &[R],
    sequence: Option<u32>, by_name: bool, rules: &Matching) -> (String, &'static str) {
    let normalized = ref_normalize(hint);
    let normalized = normalized.trim_matches(['[', ']', '(', ')', ' ']);
    // Name matching reads whole words, in any script; registry tokens are ASCII runs of three letters or more.
    let tokens = if rules.matching { word_tokens(hint, 3) } else { ref_tokens(&hint) };
    let matches_tokens = |value: &str| tokens.iter().all(|token| value.contains(token.as_str()));
    let matches_entry = |entry: &ReferenceSource| if rules.matching {
        let words = work_words(entry);
        tokens.iter().all(|token| words.contains(token))
    } else { matches_tokens(&match_text(entry)) };
    // Where the rules read names, a name and a short form compare without their quotation marks.
    let keyed = rules.short_forms || rules.matching;
    let wanted = if keyed { name_key(hint) } else { normalized.to_owned() };
    let short_of = |entry: &ReferenceSource| {
        let short = entry.short_form.as_deref().unwrap_or("");
        let short = if rules.short_forms { clean_short_form(short) } else { short.to_owned() };
        if keyed { name_key(&short) } else { ref_normalize(&short).trim_matches(['[', ']', '(', ')', ' ']).to_owned() }
    };
    let number = [&*REGISTRY_NOTE, &*REGISTRY_N, &*REGISTRY_NN].into_iter()
        .find_map(|pattern| pattern.captures(text))
        .and_then(|found| crate::text::decimal(&found[1])).map(|number| number.to_string());
    let numbered = number.map(|number| (|| -> (String, &'static str) {
            let local = sequence.filter(|&sequence| entries(registry).any(|entry|
                entry.note_is(&number) && entry.sequence == Some(sequence)));
            if local.is_none() && sequence.is_some() && entries(registry).filter(|entry|
                entry.note_is(&number))
                .filter_map(|entry| entry.sequence).collect::<HashSet<_>>().len() > 1 {
                return (String::new(), "abstain_ambiguous_note_number_scope");
            }
            let in_note = |entry: &&ReferenceSource| entry.note_is(&number)
                && local.is_none_or(|sequence| entry.sequence == Some(sequence));
            let mut note_sources = entries(registry).filter(in_note).collect::<Vec<_>>();
            if note_sources.is_empty() { return (String::new(), "note_without_authority"); }
            // A work the reference's pinpoint cannot be in is no work it names. A name none of the note's works bears
            // says the note is not the one it means ("Johnson, supra note 243" written for note 242), and the pinpoint
            // then settles nothing.
            let unnamed = normalized.is_empty() && tokens.is_empty();
            let named_here = || note_sources.iter().any(|entry| matches_entry(entry) || short_of(entry) == wanted);
            if let Some(fits) = rules.fits.filter(|_| unnamed || named_here()) {
                let fitting = note_sources.iter().copied().filter(|entry| entry.target().is_empty() || fits(entry))
                    .collect::<Vec<_>>();
                if fitting.iter().all(|entry| entry.target().is_empty()) && !note_sources.iter().all(|entry| entry.target().is_empty()) {
                    return (String::new(), "locator_conflict");
                }
                note_sources = fitting;
            }
            if normalized.is_empty() && tokens.is_empty() {
                // Of a note's several works, the one the pinpoint is likely in.
                let preferred = || rules.prefers.and_then(|prefers|
                    unique_identified(note_sources.iter().copied().filter(|entry| prefers(entry))));
                let spoken = || rules.body.and_then(|body| body_named(body, &note_sources));
                return match pick(rules.matching, note_sources.clone()) {
                    Some(target) => (target.to_owned(), "note_only"),
                    None if spoken().is_some() => spoken().map(|(target, reason)| (target.to_owned(), reason)).unwrap(),
                    None if preferred().is_some() => (preferred().unwrap().to_owned(), "locator_preferred"),
                    None if rules.first_of_note => note_sources.iter().map(|entry| entry.target())
                        .find(|target| !target.is_empty())
                        .map_or((String::new(), "ambiguous_note"), |target| (target.to_owned(), "note_first")),
                    None => (String::new(), "ambiguous_note"),
                };
            }
            // A work's own short form names it before words that occur in another's text ("Chesler" over a
            // sentence that mentions her; "Renvoi manitobain (C.A.)" over "Renvoi manitobain").
            if rules.matching {
                let exact = note_sources.iter().copied().filter(|entry| {
                    let short = short_of(entry);
                    !short.is_empty() && short == wanted
                }).collect::<Vec<_>>();
                if let Some(target) = unique_identified(exact) { return (target.to_owned(), "note_short_form"); }
            }
            let matching = note_sources.iter().copied()
                .filter(|entry| matches_entry(entry)).collect::<Vec<_>>();
            if let Some(target) = pick(rules.matching, matching.clone()) {
                return (target.to_owned(), "note_number");
            }
            // Where the name's words fit several works, the one that holds the most of them, its
            // short words and court included ("(C.A.)", "Part 2").
            if rules.distinguishing && matching.len() > 1 {
                let words = word_tokens(hint, 1);
                let scored = matching.iter().map(|entry| {
                    let held = work_words(entry);
                    (words.iter().filter(|word| held.contains(*word)).count(), entry.target())
                }).filter(|(_, target)| !target.is_empty()).collect::<Vec<_>>();
                let best = scored.iter().map(|(score, _)| *score).max().unwrap_or(0);
                let leaders = scored.iter().filter(|(score, _)| *score == best)
                    .map(|(_, target)| *target).collect::<HashSet<_>>();
                if best > 0 && leaders.len() == 1 {
                    return ((*leaders.iter().next().unwrap()).to_owned(), "distinguishing_words");
                }
            }
            // Of several works the name fits, the one the pinpoint is likely in.
            if let Some(prefers) = rules.prefers.filter(|_| matching.len() > 1) {
                if let Some(target) = unique_identified(matching.iter().copied().filter(|entry| prefers(entry))) {
                    return (target.to_owned(), "locator_preferred");
                }
            }
            // A supra names the work its note cites, not a reference in that note to another work.
            if let Some(target) = pick(rules.matching, matching.iter().copied().filter(|entry|
                !REFERENCE.is_match(entry.verbatim.as_deref().unwrap_or(""))).collect()) {
                return (target.to_owned(), "note_number");
            }
            let suffix = note_sources.iter().copied().filter(|entry| {
                let short = ref_tokens(entry.short_form.as_deref().unwrap_or(""));
                short.len() >= 2 && tokens.ends_with(&short)
            }).collect::<Vec<_>>();
            if !matching.is_empty() {
                // "R v Mann & Mann, supra" names the source whose short form it ends with.
                return match bracket_definition(normalized, matching.into_iter(), rules.short_forms)
                    .or_else(|| unique_target(suffix.iter().copied()).map(str::to_owned)) {
                    Some(target) => (target, "bracket_definition"),
                    None => (String::new(), "abstain_ambiguous_note_number"),
                };
            }
            if let Some(target) = unique_target(suffix.iter().copied()) {
                return (target.to_owned(), "note_number_short_form_suffix");
            }
            if !suffix.is_empty() { return (String::new(), "abstain_ambiguous_note_number_suffix"); }
            if let Some(target) = bracket_definition(normalized, note_sources.iter().copied(), rules.short_forms) {
                return (target, "bracket_definition");
            }
            // A name one letter off the only work of the note that writes it so ("Campell" for Campbell).
            if rules.near {
                let words = word_tokens(hint, 5);
                if let [word] = words.as_slice() {
                    let near = note_sources.iter().copied().filter(|entry|
                        work_words(entry).iter().any(|held| one_letter_apart(word, held))).collect::<Vec<_>>();
                    if let Some(target) = unique_identified(near) { return (target.to_owned(), "near_name"); }
                }
            }
            // A name that names no work anywhere ("The English statute") leaves the note to say which: its
            // one work.
            if rules.descriptive && !tokens.is_empty() && !entries(registry).any(|entry| {
                let held = work_words(entry);
                tokens.iter().all(|token| held.contains(token))
            }) {
                if let Some(target) = unique_identified(note_sources.iter().copied()) {
                    return (target.to_owned(), "note_single_work");
                }
            }
            // A note citing one work, and that without a name of its own, is what the name stands for: its
            // style of cause was the text's ("Annapolis, supra note 1" with "¹ 2022 SCC 36").
            if rules.nameless {
                if let Some(target) = unique_identified(note_sources.iter().copied()) {
                    if note_sources.iter().filter(|entry| entry.target() == target).all(|entry| entry.nameless) {
                        return (target.to_owned(), "note_nameless");
                    }
                }
            }
            (String::new(), "note_name_conflict")
    })());
    let numbered = match numbered {
        Some((target, reason)) if !(by_name && target.is_empty()
            && matches!(reason, "note_name_conflict" | "note_without_authority")) => return (target, reason),
        numbered => numbered.map(|(_, reason)| reason),
    };
    if normalized.is_empty() && tokens.is_empty() { return (String::new(), numbered.unwrap_or("abstain_no_hint")); }
    let mut abstain = "abstain_no_match";
    for (method, ambiguous) in [
        ("exact_sf", "abstain_ambiguous_exact_sf"),
        ("token_sf", "abstain_ambiguous_token_sf"),
        ("token_verb", "abstain_ambiguous_token_verb"),
    ] {
        // A name read past its numbered note is matched to a short form, not to words in a source's text.
        if numbered.is_some() && method == "token_verb" { continue; }
        let pool = entries(registry).filter(|entry| rules.fits.is_none_or(|fits| entry.target().is_empty() || fits(entry)))
            .filter(|entry| match method {
            "exact_sf" => !wanted.is_empty() && short_of(entry) == wanted,
            "token_sf" => !tokens.is_empty() && entry.short_form.as_ref().is_some_and(|short| !short.is_empty() && if rules.matching {
                let held = word_tokens(short, 1);
                tokens.iter().all(|token| held.contains(token)) && names_article(&tokens, short, |value| word_tokens(value, 3))
            } else { matches_tokens(&ref_normalize(short)) && names_article(&tokens, short, ref_tokens) }),
            _ => !tokens.is_empty() && matches_entry(entry),
        }).collect::<Vec<_>>();
        if pool.is_empty() { continue; }
        if let Some(target) = pick(rules.matching, pool) { return (target.to_owned(), method); }
        abstain = ambiguous;
        break;
    }
    if let Some(target) = bracket_definition(normalized, entries(registry), rules.short_forms) {
        return (target, "bracket_definition");
    }
    (String::new(), numbered.unwrap_or(abstain))
}

/// Whether a name's words can name the article a short form writes whole, its authors, a comma and its title ("Catherine
/// Ordway et al, “Human Rights … in Elite Sport …”"): they hold an author's name or open the title, as an article's
/// short form does, not a word from within the title ("Sport, supra note 8"). Any other short form they can.
fn names_article(tokens: &[String], short: &str, words: impl Fn(&str) -> Vec<String>) -> bool {
    let Some(quote) = short.find(['“', '"', '«']).filter(|&quote| short[..quote].trim_end().ends_with(',')) else { return true; };
    let authors = words(&short[..quote]);
    tokens.iter().any(|token| authors.contains(token)) || words(&short[quote..]).starts_with(tokens)
}

/// The work whose bracketed definition is the name; with `clean`, a definition read without its McGill
/// wording ([`clean_short_form`]).
fn bracket_definition<'a>(normalized: &str, entries: impl Iterator<Item = &'a ReferenceSource>, clean: bool) -> Option<String> {
    if normalized.is_empty() { return None; }
    let mut bases = HashSet::new();
    let mut best = "";
    for entry in entries {
        for bracket in EXPLICIT.captures_iter(entry.verbatim.as_deref().unwrap_or("")) {
            if BRACKET_SKIP.is_match(bracket[1].trim()) { continue; }
            for piece in bracket[1].split(';') {
                let piece = if clean { clean_short_form(piece).into() } else { HEREINAFTER.replace(piece, "") };
                if ref_normalize(&piece).trim_matches(['[', ']', '(', ')', ' ']) == normalized {
                    bases.insert(entry.target());
                    best = entry.target();
                }
            }
        }
    }
    (bases.len() == 1 && !best.is_empty()).then(|| best.to_owned())
}

/// Resolution after strict abstention, retaining its two existing tiers.
pub fn resolve_after_strict_abstention<R: Borrow<ReferenceSource>>(
    text: &str, registry: &[R], inferred_forms: &[R],
) -> (String, String) {
    let candidates = reference_candidates(text);
    let note_numbers = reference_info(text).notes;
    let in_note = |item: &&ReferenceSource| note_numbers.is_empty() || note_numbers.iter().any(|number| item.note_is(number));
    let registry = entries(registry).filter(in_note).collect::<Vec<_>>();
    let inferred_forms = entries(inferred_forms).filter(in_note).collect::<Vec<_>>();
    if let Some(note) = NOTE.captures(text).filter(|_| candidates.is_empty()) {
        let pool = registry.iter().copied().filter(|item| item.note_is(&note[1])
            && !REFERENCE.is_match(item.verbatim.as_deref().unwrap_or(""))).collect::<Vec<_>>();
        if let Some(target) = unique_target(pool) {
            return (target.to_owned(), "bare_note_unique_citation".into());
        }
    }
    resolve_inferred_candidates(&candidates, &registry, &inferred_forms)
}

pub(crate) fn resolve_inferred_candidates<R: Borrow<ReferenceSource>>(
    candidates: &[String], registry: &[R], inferred_forms: &[R],
) -> (String, String) {
    let keys = candidates.iter().map(|value| normalize(value)).collect::<HashSet<_>>();
    let inferred = entries(inferred_forms).filter(|item|
        item.short_form_norm.as_ref().is_some_and(|key| keys.contains(key))).collect::<Vec<_>>();
    let Some(chosen) = inferred.last() else { return (String::new(), String::new()); };
    let authoritative = entries(registry).filter(|item|
        keys.contains(&normalize(item.short_form.as_deref().unwrap_or(""))));
    let Some(target) = unique_target(inferred.iter().copied().chain(authoritative))
        else { return (String::new(), String::new()); };
    (target.to_owned(), format!("inferred_short_form:{}", chosen.rule.as_deref().unwrap_or("")))
}

/// Own-scope pinpoints replace inherited fragments;
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

/// What a split resolver's linking rules ask of the registry's name tiers ([`crate::Linking`]).
#[derive(Default)]
pub(crate) struct Matching<'a> {
    /// [`crate::Linking::name_reading`]: a reference's names as [`reading_hints`] reads them.
    pub reading: bool,
    /// [`crate::Linking::name_matching`].
    pub matching: bool,
    /// [`crate::Linking::distinguishing_words`].
    pub distinguishing: bool,
    /// [`crate::Linking::near_names`].
    pub near: bool,
    /// [`crate::Linking::descriptive_names`].
    pub descriptive: bool,
    /// [`crate::Linking::first_of_note`].
    pub first_of_note: bool,
    /// [`crate::Linking::short_forms`]: short forms read without their McGill wording.
    pub short_forms: bool,
    /// [`crate::Linking::locators`]: whether a registered work can hold the reference's pinpoint, to refuse it.
    pub fits: Option<&'a dyn Fn(&ReferenceSource) -> bool>,
    /// [`crate::Linking::locators`]: whether the pinpoint is likely in a registered work, to choose among several.
    pub prefers: Option<&'a dyn Fn(&ReferenceSource) -> bool>,
    /// [`crate::Linking::body_names`]: the text's clause before the reference's own note marker.
    pub body: Option<&'a str>,
    /// [`crate::Linking::nameless_notes`].
    pub nameless: bool,
}

/// A short form without the McGill wording its brackets hold around it ("hereinafter “Caron, SCC”",
/// "Lavoie avec renvois aux N.S.R.") and the quotation marks it is written in.
pub(crate) fn clean_short_form(value: &str) -> String {
    let value = SHORT_FORM_WORDING.replace_all(value, "");
    value.trim_matches(|c: char| c.is_whitespace() || "\"'“”«»‹›‘’".contains(c)).to_owned()
}

/// A name's words, case-folded, at least `min` letters or digits long, without the words that write a
/// reference rather than name a work.
fn word_tokens(value: &str, min: usize) -> Vec<String> {
    value.split(|c: char| !c.is_alphanumeric()).filter(|word| word.chars().count() >= min)
        .map(|word| word.case_fold().collect::<String>())
        .filter(|word| !matches!(word.as_str(), "see" | "also" | "but" | "and" | "the" | "note" | "supra" | "ibid"
            | "generally" | "contra" | "compare" | "with" | "above" | "voir" | "aussi" | "et" | "of" | "de"))
        .collect()
}

/// The words a registered work is named by: its short form, names and text, without its publication facts but
/// with their year, which tells editions apart ("Nadeau & Nadeau, 1971").
fn work_words(entry: &ReferenceSource) -> HashSet<String> {
    let verbatim = entry.verbatim.as_deref().unwrap_or("");
    let mut years = String::new();
    let verbatim = PUBLICATION_FACTS.replace_all(verbatim, |facts: &regex::Captures| {
        for word in facts[0].split(|c: char| !c.is_ascii_digit()).filter(|word| word.len() == 4) { years.push(' '); years.push_str(word); }
        " ".to_owned()
    });
    word_tokens(&format!("{} {} {}{}", entry.short_form.as_deref().unwrap_or(""), entry.names.join(" "),
        verbatim.chars().take(240).collect::<String>(), years), 1).into_iter().collect()
}

/// A name as names are compared: lower case, spaced once, without its quotation marks and outer brackets.
fn name_key(value: &str) -> String {
    ref_normalize(value).chars().filter(|c| !"\"'“”«»‹›‘’".contains(*c)).collect::<String>()
        .trim_matches(|c: char| c.is_whitespace() || "[](),.;:".contains(c)).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Of a note's several works, the one the text names before the reference's note marker: the one whose short
/// form it writes, else the one a distinctive word of whose name (a party's, a statute's) it writes and no other
/// work's name holds. A bare supra's sentence speaks of its work by a party or the short form the note set.
fn body_named<'a>(body: &str, pool: &[&'a ReferenceSource]) -> Option<(&'a str, &'static str)> {
    let fold = |value: &str| value.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty())
        .map(|word| word.nfkd().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()).collect::<Vec<_>>();
    let said = fold(body);
    let spoken = format!(" {} ", said.join(" "));
    let identified = pool.iter().copied().filter(|entry| !entry.target().is_empty()).collect::<Vec<_>>();
    let short = identified.iter().copied().filter(|entry| {
        let short = fold(&clean_short_form(entry.short_form.as_deref().unwrap_or("")));
        !short.is_empty() && spoken.contains(&format!(" {} ", short.join(" ")))
    });
    if let Some(target) = unique_identified(short) { return Some((target, "body_short_form")); }
    let words = identified.iter().map(|entry| {
        let name = entry.names.first().cloned().or_else(|| entry.verbatim.clone()).unwrap_or_default();
        crate::format::distinctive_party_words(&name).into_iter()
            .filter(|word| word.chars().count() >= 4 && word.chars().all(char::is_alphabetic)).collect::<HashSet<_>>()
    }).collect::<Vec<_>>();
    let named = identified.iter().enumerate().filter(|&(at, _)| words[at].iter().any(|word| said.contains(word)
        && !words.iter().enumerate().any(|(other, theirs)| other != at && theirs.contains(word)))).map(|(_, entry)| *entry);
    unique_identified(named).map(|target| (target, "body_name"))
}

/// The one identity a pool names: with `identified`, entries with none aside ([`unique_identified`]).
fn pick<'a>(identified: bool, pool: Vec<&'a ReferenceSource>) -> Option<&'a str> {
    if identified { unique_identified(pool) } else { unique_target(pool) }
}

/// The one identity entries name, entries with none aside.
fn unique_identified<'a>(entries: impl IntoIterator<Item = &'a ReferenceSource>) -> Option<&'a str> {
    let targets = entries.into_iter().map(ReferenceSource::target).filter(|target| !target.is_empty())
        .collect::<HashSet<_>>();
    (targets.len() == 1).then(|| *targets.iter().next().unwrap())
}

/// Whether two words are one letter apart (one added, dropped or changed).
fn one_letter_apart(left: &str, right: &str) -> bool {
    let (left, right) = (left.chars().collect::<Vec<_>>(), right.chars().collect::<Vec<_>>());
    if left == right || left.len().abs_diff(right.len()) > 1 { return false; }
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < left.len() && j < right.len() {
        if left[i] == right[j] { i += 1; j += 1; continue; }
        edits += 1;
        if edits > 1 { return false; }
        match left.len().cmp(&right.len()) {
            std::cmp::Ordering::Greater => i += 1,
            std::cmp::Ordering::Less => j += 1,
            std::cmp::Ordering::Equal => { i += 1; j += 1; }
        }
    }
    edits + (left.len() - i) + (right.len() - j) <= 1
}

/// The words a lead-in writes before a name, which are no part of it ("In Lewis v. Bankole",
/// "Both Professor Iacobucci", "C'est … dans Lavoie").
fn lead_in(word: &str) -> bool {
    matches!(word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase().as_str(),
        "in" | "dans" | "both" | "professor" | "prof" | "justice" | "j" | "dr" | "mr" | "ms" | "mrs" | "mme"
        | "m" | "me" | "chief" | "lord" | "lady" | "sir" | "madam" | "honourable" | "the" | "la" | "le" | "les"
        | "see" | "voir" | "also" | "aussi" | "and" | "et" | "of" | "de" | "du" | "des")
}

/// The words a name joins its capitalized words with ("R v Oakes", "Re Ulrike Roth", "Procureur général du Québec").
fn connector(word: &str) -> bool {
    matches!(word.to_lowercase().as_str(), "v" | "v." | "vs" | "vs." | "c" | "c." | "et" | "&" | "and" | "de"
        | "du" | "des" | "la" | "le" | "les" | "l'" | "d'" | "of" | "the" | "for" | "re" | "ex" | "parte"
        | "van" | "von" | "der" | "den" | "da" | "di" | "y" | "en" | "à" | "au" | "aux" | "sur" | "on")
}

/// The name that ends a lead-in ("This assertion stemmed from Sheehy's research, see Froc & Sheehy" ends with
/// "Froc & Sheehy"): the capitalized words and their joiners last in it. None where the words are a title
/// (written in quotation marks or after a colon), or where they are all the hint.
fn tail_name(hint: &str) -> Option<String> {
    if hint.contains(['“', '”', '«', '»', '"', ':']) { return None; }
    let words = hint.split_whitespace().collect::<Vec<_>>();
    let mut start = words.len();
    while start > 0 {
        let word = words[start - 1];
        let first = word.trim_start_matches(|c: char| "([\"'“«".contains(c)).chars().next();
        if first.is_some_and(|c| c.is_uppercase() || c.is_numeric()) || connector(word) { start -= 1; } else { break; }
    }
    if start == 0 || start == words.len() { return None; }
    let mut tail = &words[start..];
    while tail.first().is_some_and(|word| lead_in(word) || connector(word)) { tail = &tail[1..]; }
    (!tail.is_empty()).then(|| tail.join(" ").trim_matches([',', ';', ':', '.', ' ']).to_owned())
        .filter(|tail| !tail.is_empty() && tail.as_str() != hint)
}

/// The words of a reference's own clause before it: after the last semicolon, or within a parenthesis still open
/// before it ("R v Hoy, 2010 ABQB 575 (citing M(A), supra …"; "Brown (SCC), supra" keeps its closed one), without the
/// signals that open it and the quotation marks around it.
fn clause_name(prior: &str) -> String {
    let (mut start, mut open) = (0, Vec::new());
    for (at, c) in prior.char_indices() {
        match c {
            '(' => open.push(at),
            ')' => { open.pop(); }
            ';' if open.is_empty() => start = at + 1,
            _ => {}
        }
    }
    if let Some(&at) = open.last() { start = start.max(at + 1); }
    let mut rest = prior[start..].trim_start_matches(|c: char| c.is_whitespace() || "[{".contains(c));
    while let Some(signal) = READING_SIGNAL.find(rest).filter(|signal| signal.start() == 0 && signal.end() > 0) {
        rest = &rest[signal.end()..];
    }
    rest.trim_matches(|c: char| c.is_whitespace() || ",;:.[{]}\"'“”«»‹›".contains(c))
        .split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The names an ibid written after another citation in its note is given ("…; Tham, “Anti-Assignment Clauses”, ibid
/// at 290", "noted in J.J., ibid"): its clause's words and the name a lead-in ends with, where they run up to it or to
/// a comma before it. None where a colon or a parenthesis leads into the ibid (": ibid at para 44", "(ibid at 40)").
pub(crate) fn ibid_names(prior: &str) -> Vec<String> {
    let prior = prior.trim_end();
    if prior.is_empty() || prior.ends_with([':', '(', '[', '.', ';']) { return Vec::new(); }
    let hint = clause_name(prior);
    [Some(hint.clone()), tail_name(&hint)].into_iter().flatten().filter(|name| !name.is_empty())
        .fold(Vec::new(), |mut names, name| { if !names.contains(&name) { names.push(name); } names })
}

/// A supra's names as a reader reads them ([`crate::Linking::name_reading`]): the words of its own clause
/// before it, without the signals that open it and the quotation marks around it, and the name that ends
/// them where they lead in to one. None where the supra opens its own sentence ("… flawed.” Supra note 58").
pub(crate) fn reading_hints(text: &str) -> Vec<String> {
    let Some(found) = SUPRA.find(text) else { return Vec::new(); };
    let prior = text[..found.start()].trim_end();
    let opens = text[found.start()..].starts_with(|c: char| c.is_uppercase());
    if prior.is_empty() || (opens && prior.ends_with(['.', '!', '?', '”', '"', ')', ']', '’', '»'])) {
        return Vec::new();
    }
    let hint = clause_name(prior);
    // Then the name as the supra-hint reader reads it, and the name a lead-in ends with.
    let reader_hint = supra_hint(text, true);
    let reader_hint = if reader_hint.is_empty() { fallback_hint(text) } else { reader_hint };
    let mut hints = Vec::new();
    for candidate in [Some(hint.clone()), Some(reader_hint), tail_name(&hint)].into_iter().flatten() {
        if !candidate.is_empty() && !hints.contains(&candidate) { hints.push(candidate); }
    }
    hints
}

/// The registry's name tiers for a supra its linking rules read ([`Matching`]): each of its names in turn, the
/// first that names a work winning; a bare supra read by its note alone.
pub(crate) fn resolve_registry_with<R: Borrow<ReferenceSource>>(text: &str, registry: &[R], aggressive: bool,
    sequence: Option<u32>, by_name: bool, matching: &Matching) -> (String, &'static str) {
    if !matching.reading {
        let hint = supra_hint(text, aggressive);
        let hint = if hint.is_empty() { fallback_hint(text) } else { hint };
        return resolve_hint(text, &hint, registry, sequence, by_name, matching);
    }
    let hints = reading_hints(text);
    if hints.is_empty() { return resolve_hint(text, "", registry, sequence, by_name, matching); }
    let mut first = None;
    for hint in &hints {
        let result = resolve_hint(text, hint, registry, sequence, by_name, matching);
        if !result.0.is_empty() { return result; }
        first.get_or_insert(result);
    }
    first.unwrap()
}

/// Build this module's lazily built statics now ([`crate::warm`]).
pub(crate) fn warm() {
    crate::warm_statics!(
        SIGNAL, EXPLICIT_SKIP, EXPLICIT, CASE_CITE, ACT, PARTIES, CROWN, REFERENCE, AUTHORS,
        AUTHOR_TOKEN, HINT, HINT_SIGNAL, NOTE, REGISTRY_NOTE, REGISTRY_N, REGISTRY_NN,
        REGISTRY_SIGNAL, BRACKET_SKIP, SUPRA, SUPRA_HINT_SIGNAL, SUPRA_HINT_START, SUPRA_HINT_ANY,
        IBID_HINT, BARE_PROVISION, SOURCE_YEAR, HEREINAFTER, ANCHOR_PARAGRAPH, ANCHOR_PROVISION, ANCHOR_SCOPE,
        IBID, WORD, READING_SIGNAL, PUBLICATION_FACTS, SHORT_FORM_WORDING
    );
}
