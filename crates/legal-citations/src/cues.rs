//! Cheap citation cues for layout heuristics: whether a line carries,
//! continues or signals a citation, which spans' digits are citation text
//! rather than note labels, and the short form in front of a note
//! cross-reference.
//!
//! The grammar is the corpus `cue.*` family. Its court codes, report series
//! and statute series are the frozen legacy tables joined with the registry's
//! Canadian neutral identifiers, authored report series and Canadian statute
//! series, so a row added to the registry reaches these predicates too and no
//! legacy cue is lost.

use crate::registry::{registry, ReporterKind, SeriesKind};
use crate::text::last_scalars;
use legal_grammar::{CompiledEcmascriptGrammar, CompiledGrammar};
use regex::Regex;
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::sync::{LazyLock, OnceLock};
use unicode_normalization::UnicodeNormalization;

/// Legal Structure's partial SCR/RCS page cue for printed pagination.
pub fn canadian_report_start(text: &str) -> Option<u32> {
    static REPORT: LazyLock<CompiledGrammar> = LazyLock::new(||
        legal_grammar::compile_table_entry("header.scr.start-page").unwrap());
    REPORT.captures(text).expect("SCR first-page cue")?
        .name("page")?.as_str().parse().ok()
}

/// AuthoritiesHelper's SCR/RCS running-head evidence. The caller selects the
/// opening PDF lines; citation identity and reporter spelling stay here.
pub fn matches_reporter_header(text: &str, citations: &[crate::Citation]) -> bool {
    static REPORTER: LazyLock<CompiledGrammar> = LazyLock::new(||
        legal_grammar::compile_ecmascript_backtracking_table_entry("header.scr.reporter").unwrap());
    static PAGE: LazyLock<CompiledGrammar> = LazyLock::new(||
        legal_grammar::compile_ecmascript_backtracking_table_entry("header.scr.page").unwrap());
    citations.iter().filter(|citation| citation.form == crate::Form::Full
        && !citation.is_ambiguous() && citation.fields.reporter_id.as_deref() == Some("scr"))
        .any(|citation| {
            let fields = &citation.fields;
            REPORTER.captures_iter(text).any(|matched| {
                let matched = matched.expect("SCR running-head reporter");
                matched.name("volume").map(|value| value.as_str()) == fields.volume.as_deref()
                    && fields.year.as_deref().is_none_or(|year|
                        matched.name("year").is_some_and(|value| value.as_str() == year))
            }) && PAGE.captures_iter(text).any(|matched|
                matched.expect("SCR running-head page").name("page").map(|value| value.as_str()) == fields.page.as_deref())
        })
}

/// Original legal-pdf-support abbreviation spelling rules, including editions,
/// punctuation, apostrophes and spaced capital abbreviations.
fn python_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        if "()[]{}?*+-|^$\\.&~# \t\n\r\u{000b}\u{000c}".contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

pub fn reporter_abbreviation_regex(abbreviation: &str) -> String {
    let mut result = String::new();
    let characters: Vec<char> = abbreviation.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character.is_whitespace() {
            while index < characters.len() && characters[index].is_whitespace() {
                index += 1;
            }
            result.push_str(r"\s+");
        } else if character.is_alphanumeric() {
            let start = index;
            while index < characters.len() && characters[index].is_alphanumeric() {
                index += 1;
            }
            let token: String = characters[start..index].iter().collect();
            if token.chars().count() > 1 && token.chars().all(char::is_uppercase) {
                result.push_str(
                    &token
                        .chars()
                        .map(|value| format!("{}\\.?", python_escape(&value.to_string())))
                        .collect::<Vec<_>>()
                        .join(r"\s*"),
                );
            } else {
                result.push_str(&python_escape(&token));
            }
        } else if character == '(' {
            let end = characters[index + 1..]
                .iter()
                .position(|value| *value == ')')
                .map_or(characters.len(), |offset| index + offset + 1);
            let inner: String = characters[index + 1..end].iter().collect();
            result.push_str(r"\(\s*");
            result.push_str(&python_escape(&inner).replace(r"\ ", r"\s+"));
            result.push_str(r"\s*\)");
            index = (end + 1).min(characters.len());
        } else {
            match character {
                '&' => result.push_str(r"\s*&\s*"),
                '-' | '/' => result.push_str(r"\s*[-/]\s*"),
                '\'' | '\u{2019}' => result.push_str("['\u{2019}]"),
                '.' => result.push_str(r"\.?"),
                _ => result.push_str(&python_escape(&character.to_string())),
            }
            index += 1;
        }
    }
    result
}

fn surface_pattern(surface: &str) -> Option<String> {
    (!surface.is_empty()).then(|| reporter_abbreviation_regex(surface))
}

/// The legacy alternation joined with registry surfaces, longest first.
fn joined(legacy: &str, surfaces: BTreeSet<String>) -> String {
    let mut surfaces = surfaces.into_iter().collect::<Vec<_>>();
    surfaces.sort_by_key(|surface| std::cmp::Reverse(surface.len()));
    if surfaces.is_empty() {
        return legacy.to_owned();
    }
    format!("{legacy}|{}", surfaces.join("|"))
}

static BASE_DEFS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let tables = legal_grammar::load_tables().expect("grammar corpus");
    (*tables["cue.citation"].defs).clone()
});

static DEFS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let mut defs = (*BASE_DEFS).clone();
    let registry = registry();
    let courts = registry
        .courts
        .iter()
        .filter(|court| court.jurisdiction.starts_with("ca"))
        .flat_map(|court| &court.neutral)
        .filter(|surface| surface.chars().all(|character| character.is_ascii_alphanumeric() || character == ' '))
        .map(|surface| regex::escape(surface).replace(' ', r"\s+"))
        .collect();
    let reporters = registry
        .reporters
        .iter()
        .filter(|reporter| {
            reporter.source != "reporters-db"
                && reporter.kind != ReporterKind::Database
                && reporter
                    .jurisdiction
                    .as_deref()
                    .is_none_or(|jurisdiction| jurisdiction.starts_with("ca") || jurisdiction.starts_with("uk"))
        })
        .flat_map(|reporter| {
            reporter
                .editions
                .iter()
                .map(|edition| edition.abbreviation.as_str())
                .chain(reporter.variations.keys().map(String::as_str))
        })
        // A single mixed-case word ("Dy", "Ow") is prose more often than a
        // nominate report; the legacy table keeps the few that matter.
        .filter(|surface| {
            surface.contains(['.', ' '])
                || surface.chars().filter(|character| character.is_alphabetic()).all(char::is_uppercase)
        })
        .filter_map(surface_pattern)
        .collect();
    let statutes = registry
        .series
        .iter()
        .filter(|series| {
            series.jurisdiction.starts_with("ca")
                && matches!(series.kind, SeriesKind::RevisedStatutes | SeriesKind::AnnualStatutes)
        })
        .flat_map(|series| std::iter::once(&series.abbreviation).chain(&series.variations))
        .filter(|surface| surface.chars().all(|character| character.is_alphanumeric() || character == '.'))
        .map(|surface| regex::escape(surface))
        .collect();
    for (name, surfaces) in [
        ("cue_court_codes", courts),
        ("cue_reporter_tokens", reporters),
        ("cue_statute_sources", statutes),
    ] {
        let legacy = defs[name].clone();
        defs.insert(name.to_owned(), joined(&legacy, surfaces));
    }
    defs
});

fn cue_with_defs(id: &str, defs: &HashMap<String, String>) -> CompiledEcmascriptGrammar {
    let tables = legal_grammar::load_tables().expect("grammar corpus");
    let entry = &tables[id].entry;
    let pattern = legal_grammar::expand_pattern(&entry.pattern, defs).expect("cue defs");
    // These primitives came from native Rust regexes; retain their Unicode
    // word, digit and whitespace semantics at the new owner.
    let mut builder = regex::RegexBuilder::new(&pattern);
    builder
        .case_insensitive(entry.flags.contains('i'))
        .multi_line(entry.flags.contains('m'))
        .dot_matches_new_line(entry.flags.contains('s'))
        .size_limit(64 << 20);
    builder.build().unwrap_or_else(|error| panic!("{id}: {error}"))
}

fn cue(id: &str) -> CompiledEcmascriptGrammar {
    cue_with_defs(id, &DEFS)
}

fn layout_cue(id: &str) -> CompiledEcmascriptGrammar {
    cue_with_defs(id, &BASE_DEFS)
}

fn backtracking(id: &str) -> CompiledGrammar {
    legal_grammar::compile_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

static CITATION: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| cue("cue.citation"));
static CONTINUATION: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| cue("cue.continuation"));
static LAYOUT_CITATION: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| layout_cue("cue.citation"));
static LAYOUT_CONTINUATION: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| layout_cue("cue.continuation"));
static PROTECTED: LazyLock<[CompiledEcmascriptGrammar; 6]> = LazyLock::new(|| {
    [
        "cue.protected.reporter",
        "cue.protected.neutral",
        "cue.protected.statute",
        "cue.protected.journal",
        "cue.protected.pinpoint",
        "cue.protected.code",
    ]
    .map(cue)
});
static LAYOUT_PROTECTED: LazyLock<[CompiledEcmascriptGrammar; 6]> = LazyLock::new(|| {
    [
        "cue.protected.reporter", "cue.protected.neutral", "cue.protected.statute",
        "cue.protected.journal", "cue.protected.pinpoint", "cue.protected.code",
    ].map(layout_cue)
});
static SIGNAL_CASED: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| cue("cue.signal.cased"));
static SIGNAL_FOLDED: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| cue("cue.signal.folded"));
static LAYOUT_SIGNAL_CASED: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| layout_cue("cue.signal.cased"));
static LAYOUT_SIGNAL_FOLDED: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| layout_cue("cue.signal.folded"));
static CITATION_TAIL: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| cue("cue.citation-tail"));
// `\w` in a cross-reference short form is any letter, which is the Unicode
// (backtracking) dialect.
static CROSSREF_SHORT_FORM: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("cue.crossref-short-form"));
static CROSSREF_STOPWORD: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| cue("cue.crossref-stopword"));
static COUNTER_NOUN: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| cue("cue.counter-noun"));
static TRAILING_DIGIT: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| cue("cue.heading-trailing-digit"));
static TRAILING_DATE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| cue("cue.heading-trailing-date"));
static POSSESSIVE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| cue("cue.heading-possessive"));

const ALL_CAPS_MIN_RATIO: f64 = 0.85;
const TITLECASE_MIN_RATIO: f64 = 0.6;

/// Whether a line carries or starts a legal citation: a reference or pinpoint
/// word, a signal, a versus token, a court code, a report series, a bracketed
/// or parenthesized year, or a U.S. Code section.
pub fn has_citation_cue(text: &str) -> bool {
    CITATION.is_match(text) || PROTECTED[5].is_match(text)
}

/// The frozen PDF layout cue uses the original table. Registry additions are
/// valid citations, but a broader cue here can hide a document heading.
pub fn layout_has_citation_cue(text: &str) -> bool {
    LAYOUT_CITATION.is_match(text) || LAYOUT_PROTECTED[5].is_match(text)
}

/// Whether a line opens with a report series, continuing a citation broken
/// across lines (`S.C.R. 631`).
pub fn is_citation_continuation(text: &str) -> bool {
    CONTINUATION.is_match(text)
}

pub fn layout_is_citation_continuation(text: &str) -> bool {
    LAYOUT_CONTINUATION.is_match(text)
}

fn has_pinpoint_prefix(text: &str) -> bool {
    if !text.is_ascii() {
        return true;
    }
    const CUES: [(&str, bool); 9] = [
        ("at", false),
        ("p", true),
        ("pp", true),
        ("page", true),
        ("pages", true),
        ("para", true),
        ("paras", true),
        ("s", true),
        ("ss", true),
    ];
    text.char_indices().any(|(index, character)| {
        if !character.is_ascii_alphabetic()
            || (index > 0
                && text[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|previous| previous.is_ascii_alphanumeric()))
        {
            return false;
        }
        CUES.iter().any(|(cue, allows_dot)| {
            let rest = &text[index..];
            let Some(prefix) = rest.get(..cue.len()) else {
                return false;
            };
            if !prefix.eq_ignore_ascii_case(cue) {
                return false;
            }
            let mut tail = &rest[cue.len()..];
            if *allows_dot && tail.starts_with('.') {
                tail = &tail[1..];
            }
            let mut characters = tail.chars();
            characters.next().is_some_and(char::is_whitespace)
                && characters
                    .find(|next| !next.is_whitespace())
                    .is_some_and(char::is_numeric)
        })
    })
}

/// The byte spans of citation text whose digits must not be read as footnote
/// markers: report, neutral and CanLII citations, statute chapters,
/// periodical blocks, pinpoints and U.S. Code sections, grouped by kind in
/// that order.
pub fn protected_spans(text: &str) -> Vec<Range<usize>> {
    protected_spans_in(text, true)
}

pub fn layout_protected_spans(text: &str) -> Vec<Range<usize>> {
    protected_spans_in(text, false)
}

fn protected_spans_in(text: &str, expanded: bool) -> Vec<Range<usize>> {
    if !text.chars().any(|character| character.is_numeric()) {
        return Vec::new();
    }
    let mut digit_runs = 0;
    let mut inside_digits = false;
    for character in text.chars() {
        if character.is_numeric() {
            if !inside_digits {
                digit_runs += 1;
            }
            inside_digits = true;
        } else {
            inside_digits = false;
        }
    }
    let statute_source = !text.is_ascii()
        || text
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|token| {
                (if expanded { &DEFS } else { &BASE_DEFS })["cue_statute_sources"]
                    .split('|')
                    .any(|source| token.eq_ignore_ascii_case(source))
            });
    let pinpoint_prefix = has_pinpoint_prefix(text);
    let mut spans = Vec::new();
    for index in 0..6 {
        if (index < 2 && digit_runs < 2)
            || (index == 2 && !statute_source)
            || (index == 3 && digit_runs < 3)
            || (index == 4 && !pinpoint_prefix)
        {
            continue;
        }
        let regex = if expanded { &PROTECTED[index] } else { &LAYOUT_PROTECTED[index] };
        let mut offset = 0;
        while let Some(captures) = regex.captures_at(text, offset) {
            let found = captures.name("span").expect("protected span capture");
            spans.push(found.start()..found.end());
            offset = found.end();
        }
    }
    spans
}

/// NFKC, with typographic quotes, dashes and no-break spaces folded to ASCII.
fn normalize(text: &str) -> Cow<'_, str> {
    if text.is_ascii() {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.nfkc()
            .filter_map(|character| match character {
                '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' => Some('\''),
                '\u{201c}' | '\u{201d}' | '\u{201e}' => Some('"'),
                '\u{2010}'..='\u{2014}' => Some('-'),
                '\u{00a0}' => Some(' '),
                '\u{feff}' => None,
                _ => Some(character),
            })
            .collect(),
    )
}

fn reporter_citation_re(first: u8, expanded: bool) -> Option<&'static Regex> {
    static RES: OnceLock<[OnceLock<Regex>; 26]> = OnceLock::new();
    static LAYOUT_RES: OnceLock<[OnceLock<Regex>; 26]> = OnceLock::new();
    static ABBREVIATIONS: OnceLock<Vec<String>> = OnceLock::new();
    static LAYOUT_ABBREVIATIONS: OnceLock<Vec<String>> = OnceLock::new();
    let index = first.checked_sub(b'A')? as usize;
    let slots = if expanded { &RES } else { &LAYOUT_RES };
    let slot = slots
        .get_or_init(|| std::array::from_fn(|_| OnceLock::new()))
        .get(index)?;
    let source = if expanded { &ABBREVIATIONS } else { &LAYOUT_ABBREVIATIONS };
    let abbreviations = source.get_or_init(|| {
        let mut values: Vec<String> = serde_json::from_str(include_str!("../registry/mcgill-inventory.json"))
            .expect("source McGill abbreviation inventory");
        if expanded {
            // Retain original records and order; authored additions use the same
            // proven spelling routine, without certifying the inventory entries.
            let mut seen = values.iter().cloned().collect::<BTreeSet<_>>();
            for reporter in &registry().reporters {
                if reporter.source == "reporters-db" { continue; }
                for surface in reporter.editions.iter().map(|edition| &edition.abbreviation)
                    .chain(reporter.variations.keys()) {
                    if seen.insert(surface.clone()) { values.push(surface.clone()); }
                }
            }
        }
        values
    });
    abbreviations
        .iter()
        .any(|value| value.as_bytes().first() == Some(&first))
        .then(|| {
            slot.get_or_init(|| {
                let reporters = abbreviations
                    .iter()
                    .filter(|value| value.as_bytes().first() == Some(&first))
                    .map(|value| reporter_abbreviation_regex(value))
                    .collect::<Vec<_>>()
                    .join("|");
                let tables = legal_grammar::load_tables().expect("citation grammar");
                let entry = &tables["cue.reporter-inventory"].entry;
                Regex::new(&entry.pattern.replace("{{inventory_reporters}}", &reporters))
                    .expect("source reporter inventory grammar")
            })
        })
}

fn has_reporter_citation(text: &str, expanded: bool) -> bool {
    static PREFIX: OnceLock<Regex> = OnceLock::new();
    let prefix = PREFIX.get_or_init(|| {
        let tables = legal_grammar::load_tables().expect("citation grammar");
        Regex::new(&tables["cue.reporter-prefix"].entry.pattern).expect("source reporter prefix")
    });
    let mut tried = 0_u32;
    prefix.captures_iter(text).any(|captures| {
        let first = captures[1].as_bytes()[0];
        let bit = 1 << (first - b'A');
        if tried & bit != 0 {
            return false;
        }
        tried |= bit;
        reporter_citation_re(first, expanded).is_some_and(|regex| regex.is_match(text))
    })
}

/// Whether text (after NFKC normalization) holds a citation signal: a
/// neutral, report or statute citation, a two-party case name, a section,
/// paragraph or page pinpoint, `supra note`/`ibid`, or a periodical block.
pub fn has_citation_signal(text: &str) -> bool {
    has_citation_signal_in(text, true)
}

pub fn layout_has_citation_signal(text: &str) -> bool {
    has_citation_signal_in(text, false)
}

fn has_citation_signal_in(text: &str, expanded: bool) -> bool {
    let normalized = normalize(text);
    let (cased, folded) = if expanded {
        (&SIGNAL_CASED, &SIGNAL_FOLDED)
    } else {
        (&LAYOUT_SIGNAL_CASED, &LAYOUT_SIGNAL_FOLDED)
    };
    if cased.is_match(&normalized) || folded.is_match(&normalized) {
        return true;
    }
    static DIGIT_RUN: OnceLock<Regex> = OnceLock::new();
    DIGIT_RUN.get_or_init(|| Regex::new(r"\d+").expect("digit run regex"))
        .find_iter(&normalized).take(2).count() >= 2
        && has_reporter_citation(&normalized, expanded)
}

/// Whether a short line is plausibly a heading: capitalized, not ending in a
/// number, carrying no citation cue or signal, and all caps or title case.
pub fn heading_text_plausible(value: &str) -> bool {
    heading_text_plausible_in(value, true)
}

pub fn layout_heading_text_plausible(value: &str) -> bool {
    heading_text_plausible_in(value, false)
}

fn heading_text_plausible_in(value: &str, expanded: bool) -> bool {
    let text = value.trim();
    if text.is_empty() || text.chars().count() > 100 {
        return false;
    }
    let Some(first) = text.chars().next() else {
        return false;
    };
    // A line ending in a number is a list item or a citation, unless the number
    // is the year of the date a title bears ("ORDER DATED 3 MARCH 2030"): two
    // or more capitalized words run on into the date, not a place or a label
    // set before it ("Ottawa, 3 March 2030", "DATE: 3 MARCH 2030").
    let dated_title = TRAILING_DATE.find(text).is_some_and(|date| {
        let title = text[..date.start()].trim_end();
        !title.ends_with([',', ':']) && title.split_whitespace()
            .filter(|word| word.chars().next().is_some_and(char::is_uppercase)).count() >= 2
    });
    if !first.is_alphabetic() || !first.is_uppercase() || (TRAILING_DIGIT.is_match(text) && !dated_title) {
        return false;
    }
    let citation_text = POSSESSIVE.replace_all(text, "$1");
    let cue = if expanded { has_citation_cue(&citation_text) } else { layout_has_citation_cue(&citation_text) };
    if cue || has_citation_signal_in(&citation_text, expanded) {
        return false;
    }
    let letters = text.chars().filter(|character| character.is_alphabetic()).collect::<Vec<_>>();
    let all_caps = letters.len() >= 4
        && letters.iter().filter(|character| character.is_uppercase()).count() as f64
            / letters.len() as f64
            >= ALL_CAPS_MIN_RATIO;
    let words = text
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphabetic))
        .collect::<Vec<_>>();
    let titlecase = !words.is_empty()
        && words
            .iter()
            .filter(|word| word.chars().next().is_some_and(char::is_uppercase))
            .count() as f64
            / words.len() as f64
            >= TITLECASE_MIN_RATIO;
    all_caps || titlecase
}

/// Whether the text after a candidate note marker reads as the rest of a
/// citation (` SCR 631`), so the digit before it is not a marker.
pub fn is_citation_shaped_tail(text: &str) -> bool {
    CITATION_TAIL.is_match(text)
}

/// The short form written just before a note cross-reference at `byte_start`
/// (`Jordan` in `Jordan, supra note 4`), or an empty string.
pub fn crossref_short_form(text: &str, byte_start: usize) -> String {
    let Some(captures) = CROSSREF_SHORT_FORM
        .captures(last_scalars(&text[..byte_start], 70))
        .ok()
        .flatten()
    else {
        return String::new();
    };
    let short = captures
        .name("short")
        .map_or("", |value| value.as_str())
        .trim()
        .trim_end_matches([',', '.', ';', ':'])
        .to_owned();
    if CROSSREF_STOPWORD.is_match(&short) {
        String::new()
    } else {
        short
    }
}

/// Whether a word counts the number after it (`note 12`, `para 5`, `Figure 3`).
pub fn is_counter_noun(word: &str) -> bool {
    COUNTER_NOUN.is_match(word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    // The legacy legal-pdf-parser tables and regexes, kept as an oracle: the
    // corpus-and-registry predicates must accept everything these accepted.
    const COURT_CODE_PATTERN: &str = "SCC|FCA|FC|TCC|CMAC|BCCA|BCSC|BCPC|ABCA|ABQB|ABKB|ABPC|SKCA|SKQB|SKKB|SKPC|MBCA|MBQB|ONCA|ONSC|ONCJ|QCCA|QCCS|QCCQ|NBCA|NBQB|NSSC|NSCA|PECA|PESC|NLCA|NLSC|YKCA|YKSC|NWTCA|NWTSC|NUCA|NUCJ";
    const REPORTER_TOKEN_PATTERN: &str = r"S\.?\s*C\.?\s*R\.?|D\.?\s*L\.?\s*R\.?|C\.?\s*C\.?\s*C\.?|O\.?\s*R\.?|W\.?\s*W\.?\s*R\.?|C\.?\s*R\.?|All\s+E\.?\s*R\.?|A\.?\s*C\.?|K\.?\s*B\.?|Q\.?\s*B\.?|Q\.?\s*B\.?\s*D\.?|Ch(?:\s+D)?\.?|App\s+Cas|W\.?\s*L\.?\s*R\.?|E\.?\s*R\.?|T\.?\s*L\.?\s*R\.?|Cox\s+C\.?\s*C\.?|Cr\s+App\s+R\.?|Ex\.?|Eq\.?|H\.?\s*L\.?\s*Cas\.?";

    fn legacy_cue(text: &str) -> bool {
        Regex::new(&format!(
            r"(?i)\b(?:ibid|id\.?|ibidem|supra|infra|op\s+cit|note|notes|para\.?|paras\.?|paragraphs?|pp?\.?|pages?|ss?\.?|secs?\.?|sections?|art\.?|arts\.?|at|see|cf\.?|e\.?g\.?|accord|contra|R\.?\s*v\.?|Rex|Regina|v\.?|vs\.?|CanLII|SCC|SCR|DLR|(?:{COURT_CODE_PATTERN})|(?:{REPORTER_TOKEN_PATTERN}))\b|\[(?:17|18|19|20)\d{{2}}\]|\((?:17|18|19|20)\d{{2}}\)"
        ))
        .unwrap()
        .is_match(text)
    }

    const LINES: [&str; 16] = [
        "R v Jordan, 2016 SCC 27 at para 5",
        "[2016] 1 SCR 631",
        "(1994) 117 DLR (4th) 577",
        "Criminal Code, RSC 1985, c C-46, s 718",
        "42 U.S.C. § 1983",
        "Ibid at 7.",
        "Smith, supra note 4",
        "the Court of Appeal for Ontario",
        "INTRODUCTION",
        "Background and Facts",
        "S.C.R. 631, 12 C.C.C. (3d) 1",
        "(2019) 97 Can Bar Rev 1",
        "Tuesday afternoon we ate",
        "à la p. 45",
        "2020 ONSC 1234; 2021 QCCA 5",
        "12 All ER 5",
    ];

    #[test]
    fn cues_keep_every_legacy_cue() {
        for line in LINES {
            if legacy_cue(line) {
                assert!(has_citation_cue(line), "{line}");
            }
        }
        assert!(has_citation_cue("R. v. Jordan"));
        assert!(has_citation_cue("see 42 U.S.C. § 1983"));
        assert!(!has_citation_cue("Tuesday afternoon we ate"));
        // A registry court the legacy table lacked.
        assert!(has_citation_cue("2020 BCSC 5 and 2019 NSCA 7"));
    }

    #[test]
    fn continuation_lines_open_with_a_report_series() {
        assert!(is_citation_continuation("S.C.R. 631"));
        assert!(is_citation_continuation("  D.L.R. (4th) 577"));
        assert!(is_citation_continuation("P.2d 456"));
        assert!(is_citation_continuation("N.R. 1"));
        assert!(!is_citation_continuation("The court held"));
    }

    #[test]
    fn protected_spans_cover_citation_digits_in_bytes() {
        let text = "🦫 R v Jordan, 2016 SCC 27, [2016] 1 SCR 631 at paras 20-25; RSC 1985, c C-46; 42 U.S.C. § 1983.";
        let spans = protected_spans(text);
        let texts = spans.iter().map(|span| &text[span.clone()]).collect::<Vec<_>>();
        assert!(texts.contains(&"[2016] 1 SCR 631"), "{texts:?}");
        assert!(texts.contains(&"2016 SCC 27"), "{texts:?}");
        assert!(texts.contains(&"RSC 1985, c C-46"), "{texts:?}");
        assert!(texts.contains(&"paras 20-25"), "{texts:?}");
        assert!(texts.contains(&"42 U.S.C. § 1983"), "{texts:?}");
        assert!(protected_spans("no digits here").is_empty());
        assert!(protected_spans("footnote 12 only").is_empty());
    }

    #[test]
    fn citation_signals_normalize_and_use_the_registry() {
        assert!(has_citation_signal("R v Jordan, 2016 SCC 27"));
        assert!(has_citation_signal("[2016] 1 S.C.R. 631"));
        assert!(has_citation_signal("Smith v. Jones"));
        assert!(has_citation_signal("Ibid."));
        assert!(has_citation_signal("RSC 1985, c C-46"));
        assert!(has_citation_signal("at\u{a0}p.\u{a0}45"));
        assert!(has_citation_signal("(1990) 71 OR (2d) 725"));
        // A McGill report series outside the legacy token table.
        assert!(has_citation_signal("(1985) 12 Admin LR 5"));
        assert!(!has_citation_signal("the parties met twice in 2019"));
    }

    #[test]
    fn headings_tails_short_forms_and_counter_nouns() {
        assert!(heading_text_plausible("BACKGROUND AND FACTS"));
        assert!(heading_text_plausible("Standard of Review"));
        assert!(!heading_text_plausible("R v Jordan, 2016 SCC 27"));
        assert!(!heading_text_plausible("Part 3"));
        assert!(!heading_text_plausible("the court held that"));
        assert!(layout_heading_text_plausible("ORDER DATED 3 MARCH 2030"));
        assert!(heading_text_plausible("Reasons Delivered March 3, 2030"));
        assert!(!layout_heading_text_plausible("DATE: 3 MARCH 2030"));
        assert!(!layout_heading_text_plausible("Ottawa, Ontario, 3 March 2030"));
        assert!(!layout_heading_text_plausible("Contents 12"));
        assert!(is_citation_shaped_tail(" SCR 631"));
        assert!(!is_citation_shaped_tail(" the court held"));
        let text = "as held in Jordan, supra note 4";
        assert_eq!(crossref_short_form(text, text.find("supra").unwrap()), "Jordan");
        // Like the legacy grammar, a capitalized signal stays in the short form.
        let text = "See Jordan, supra note 4";
        assert_eq!(crossref_short_form(text, text.find("supra").unwrap()), "See Jordan");
        let text = "see supra note 4";
        assert_eq!(crossref_short_form(text, text.find("supra").unwrap()), "");
        assert!(is_counter_noun("paras"));
        assert!(is_counter_noun("Figures"));
        assert!(!is_counter_noun("court"));
    }
}
