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
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

/// A registry surface as a pattern: dotted and undotted, spaced or not
/// (`S.C.R.`, `SCR`, `S C R`), as the legacy tables spelled their members.
fn surface_pattern(surface: &str) -> Option<String> {
    let surface = surface.split('(').next().unwrap_or(surface).trim();
    let tokens = surface
        .split_whitespace()
        .map(|token| token.trim_matches('.'))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let letters = tokens.iter().map(|token| token.chars().filter(|c| c.is_alphanumeric()).count()).sum::<usize>();
    if tokens.is_empty() || letters < 2 {
        return None;
    }
    let pattern = tokens
        .iter()
        .map(|token| {
            let bare = token.replace('.', "");
            if bare.chars().count() > 1 && bare.chars().all(|character| character.is_ascii_uppercase()) {
                bare.chars()
                    .map(|character| format!("{character}\\.?"))
                    .collect::<Vec<_>>()
                    .join(r"\s*")
            } else {
                format!("{}\\.?", regex::escape(token))
            }
        })
        .collect::<Vec<_>>()
        .join(r"\s+");
    Some(pattern)
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

static DEFS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let tables = legal_grammar::load_tables().expect("grammar corpus");
    let mut defs = (*tables["cue.citation"].defs).clone();
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

fn cue(id: &str) -> CompiledEcmascriptGrammar {
    let tables = legal_grammar::load_tables().expect("grammar corpus");
    let entry = &tables[id].entry;
    let pattern = legal_grammar::expand_pattern(&entry.pattern, &DEFS).expect("cue defs");
    let mut builder = regex::RegexBuilder::new(
        &legal_grammar::expand_ecmascript_portable(&pattern).expect("portable cue"),
    );
    builder
        .case_insensitive(entry.flags.contains('i'))
        .multi_line(entry.flags.contains('m'))
        .dot_matches_new_line(entry.flags.contains('s'))
        .size_limit(64 << 20);
    builder.build().unwrap_or_else(|error| panic!("{id}: {error}"))
}

fn backtracking(id: &str) -> CompiledGrammar {
    legal_grammar::compile_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

static CITATION: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| cue("cue.citation"));
static CONTINUATION: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| cue("cue.continuation"));
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
static SIGNAL_CASED: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| cue("cue.signal.cased"));
static SIGNAL_FOLDED: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| cue("cue.signal.folded"));
static REPORTER_CANDIDATE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| cue("cue.reporter-candidate"));
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

/// Whether a line opens with a report series, continuing a citation broken
/// across lines (`S.C.R. 631`).
pub fn is_citation_continuation(text: &str) -> bool {
    CONTINUATION.is_match(text)
}

/// The byte spans of citation text whose digits must not be read as footnote
/// markers: report, neutral and CanLII citations, statute chapters,
/// periodical blocks, pinpoints and U.S. Code sections, grouped by kind in
/// that order.
pub fn protected_spans(text: &str) -> Vec<Range<usize>> {
    if !text.chars().any(char::is_numeric) {
        return Vec::new();
    }
    let mut spans = Vec::new();
    for pattern in PROTECTED.iter() {
        let mut offset = 0;
        while let Some(captures) = pattern.captures_at(text, offset) {
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

fn digit_runs(text: &str) -> usize {
    let mut runs = 0;
    let mut inside = false;
    for character in text.chars() {
        let digit = character.is_ascii_digit();
        if digit && !inside {
            runs += 1;
        }
        inside = digit;
    }
    runs
}

/// A volume/reporter/page shape whose reporter is an authored report series.
fn has_registry_reporter_citation(text: &str) -> bool {
    REPORTER_CANDIDATE.captures_iter(text).any(|captures| {
        registry()
            .reporters_by_surface(&captures["reporter"])
            .iter()
            .any(|(reporter, _)| reporter.source != "reporters-db")
    })
}

/// Whether text (after NFKC normalization) holds a citation signal: a
/// neutral, report or statute citation, a two-party case name, a section,
/// paragraph or page pinpoint, `supra note`/`ibid`, or a periodical block.
pub fn has_citation_signal(text: &str) -> bool {
    let normalized = normalize(text);
    if SIGNAL_CASED.is_match(&normalized) || SIGNAL_FOLDED.is_match(&normalized) {
        return true;
    }
    digit_runs(&normalized) >= 2 && has_registry_reporter_citation(&normalized)
}

/// Whether a short line is plausibly a heading: capitalized, not ending in a
/// number, carrying no citation cue or signal, and all caps or title case.
pub fn heading_text_plausible(value: &str) -> bool {
    let text = value.trim();
    if text.is_empty() || text.chars().count() > 100 {
        return false;
    }
    let Some(first) = text.chars().next() else {
        return false;
    };
    if !first.is_alphabetic() || !first.is_uppercase() || TRAILING_DIGIT.is_match(text) {
        return false;
    }
    let citation_text = POSSESSIVE.replace_all(text, "$1");
    if has_citation_cue(&citation_text) || has_citation_signal(&citation_text) {
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
