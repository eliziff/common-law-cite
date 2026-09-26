//! Decide whether a quoted excerpt is prose, an authority list or a mix, for
//! citator treatment extraction.

use crate::find::{citation_hits, has_core_citation};
use crate::text::{trim_javascript_whitespace, utf16_len, ScalarText};
use regex::Regex;
use serde::Serialize;
use std::ops::Range;
use std::sync::LazyLock;

type Hit = Range<usize>;

fn ecmascript(pattern: &str, flags: &str) -> Regex {
    legal_grammar::compile_ecmascript_pattern("excerpt", pattern, flags).expect("frozen excerpt regex")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcerptClassification {
    pub kind: &'static str,
    pub cite_tokens: usize,
    pub cite_runs: usize,
    pub cite_char_coverage: f64,
    pub function_words: usize,
    pub prose_window: Option<String>,
    pub rule: &'static str,
}

fn function_word(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "of"
            | "to"
            | "that"
            | "in"
            | "a"
            | "an"
            | "and"
            | "is"
            | "was"
            | "were"
            | "be"
            | "as"
            | "for"
            | "on"
            | "with"
            | "by"
            | "it"
            | "this"
            | "not"
            | "or"
            | "which"
            | "would"
            | "may"
            | "must"
            | "court"
            | "judge"
            | "held"
            | "found"
            | "stated"
    )
}

static NAME_LEADIN: LazyLock<Regex> = LazyLock::new(|| {
    ecmascript(
        r"(?:[A-Z][\w.'’()‑-]*(?:\s+[\w.'’()‑-]+){0,6}\s+(?:v|c)\.\s+[A-Z][\w.'’()‑-]*(?:\s+[\w.'’()‑-]+){0,6},?\s*)$",
        "",
    )
});
static PINPOINT: LazyLock<Regex> = LazyLock::new(|| {
    ecmascript(
        r"^\s*(?:,\s*)?(?:at\s+)?para?s?\.?\s+\d+(?:\s*[-–]\s*\d+)?",
        "",
    )
});
static GLUE: LazyLock<Regex> = LazyLock::new(|| ecmascript(r"^[\s,;]*(?:and\s+)?$", ""));

fn citation_spans(text: &str, document: &ScalarText<'_>) -> (Vec<Hit>, usize) {
    let hits = citation_hits(text, true);
    let tokens = hits.len();
    let mut spans = Vec::with_capacity(tokens);
    for hit in hits {
        let mut start = hit.start;
        let mut end = hit.end;
        if let Some(leadin) = NAME_LEADIN.find(&text[..start]) {
            start = leadin.start();
        }
        if let Some(tail) = PINPOINT.find(&text[end..]) {
            end += tail.end();
        }
        spans.push(start..end);
    }
    spans.sort_by_key(|span| span.start);
    let mut merged: Vec<Hit> = Vec::new();
    for span in spans {
        if let Some(last) = merged.last_mut().filter(|last| {
            document.utf16_at_byte(span.start).unwrap()
                <= document.utf16_at_byte(last.end).unwrap() + 6
                && GLUE.is_match(&text[last.end..span.start.max(last.end)])
        }) {
            last.end = last.end.max(span.end);
        } else {
            merged.push(span);
        }
    }
    (merged, tokens)
}

fn refusal(rule: &'static str) -> ExcerptClassification {
    ExcerptClassification {
        kind: "insufficient",
        cite_tokens: 0,
        cite_runs: 0,
        cite_char_coverage: 0.0,
        function_words: 0,
        prose_window: None,
        rule,
    }
}

fn lowercase_words(text: &str) -> usize {
    static WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}'’-]+").unwrap());
    static LOWERCASE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\p{Ll}$").unwrap());
    WORDS
        .find_iter(text)
        .filter(|word| {
            let word = word.as_str();
            let first = word.chars().next().unwrap();
            if word.is_ascii() {
                word.len() >= 3 && first.is_ascii_lowercase()
            } else {
                utf16_len(word) >= 3
                    && first.len_utf16() == 1
                    && LOWERCASE.is_match(&word[..first.len_utf8()])
            }
        })
        .count()
}

fn trim_window_edges(text: &str) -> String {
    static FIRST: LazyLock<Regex> = LazyLock::new(|| ecmascript(r"^\S*\s+", ""));
    static LAST: LazyLock<Regex> = LazyLock::new(|| ecmascript(r"\s+\S*$", ""));
    LAST.replace(&FIRST.replace(text, ""), "").into_owned()
}

pub fn classify_citator_excerpt(excerpt: &str) -> ExcerptClassification {
    let text = trim_javascript_whitespace(excerpt);
    let document = ScalarText::new(text);
    let text_length = document.utf16_len();
    if text_length < 60 {
        return refusal("shorter_than_min_excerpt");
    }
    let (spans, cite_tokens) = citation_spans(text, &document);
    let cite_chars = spans
        .iter()
        .map(|span| {
            document.utf16_at_byte(span.end).unwrap() - document.utf16_at_byte(span.start).unwrap()
        })
        .sum::<usize>();
    let cite_char_coverage = cite_chars as f64 / text_length as f64;
    let cite_runs = text
        .split(';')
        .filter(|segment| has_core_citation(segment))
        .count();
    let mut segments = Vec::new();
    let mut cursor = 0;
    for span in &spans {
        if span.start > cursor {
            segments.push(&text[cursor..span.start]);
        }
        cursor = cursor.max(span.end);
    }
    if cursor < text.len() {
        segments.push(&text[cursor..]);
    }
    let words = segments.join(" ").to_lowercase();
    let function_words = words
        .split(|character: char| !character.is_ascii_lowercase() && character != '\'')
        .filter(|word| function_word(word))
        .count();
    let best = segments
        .iter()
        .flat_map(|segment| segment.split('\n'))
        .map(|line| trim_javascript_whitespace(line))
        .map(|line| (line, lowercase_words(line), utf16_len(line)))
        .reduce(|best, candidate| {
            if (candidate.1, candidate.2) > (best.1, best.2) {
                candidate
            } else {
                best
            }
        });
    let prose_window = best
        .filter(|(_, score, length)| *score >= 4 && *length >= 40)
        .map(|(line, _, _)| trim_window_edges(line));
    if cite_runs >= 3 && function_words < cite_runs * 4 {
        return ExcerptClassification {
            kind: "authority_list",
            cite_tokens,
            cite_runs,
            cite_char_coverage,
            function_words,
            prose_window: None,
            rule: "cite_runs>=3_low_function_words",
        };
    }
    if cite_char_coverage > 0.5 && function_words < 8 {
        return ExcerptClassification {
            kind: "authority_list",
            cite_tokens,
            cite_runs,
            cite_char_coverage,
            function_words,
            prose_window: None,
            rule: "cite_coverage>0.5_low_function_words",
        };
    }
    let Some(prose_window) = prose_window else {
        return refusal("no_prose_window");
    };
    let prose = cite_char_coverage <= 0.15 && cite_tokens <= 2;
    ExcerptClassification {
        kind: if prose { "prose" } else { "mixed" },
        cite_tokens,
        cite_runs,
        cite_char_coverage,
        function_words,
        prose_window: Some(prose_window),
        rule: if prose {
            "low_cite_coverage"
        } else {
            "prose_window_with_citations"
        },
    }
}




