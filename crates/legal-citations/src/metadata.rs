//! Pinpoint ranges, parentheticals, subsequent history, parties and court.
//!
//! [`crate::find`] walks the text after each core with [`tail`] to fix the
//! citation's extent; [`attach`] then reads what the extent holds: the court a
//! parenthetical names, the history phrase that links a decision to the next
//! citation, and the parties of a style of cause.

use crate::find::span;
use crate::model::{
    Citation, History, Parenthetical, ParentheticalKind, Parties, Pinpoint,
    PinpointKind,
};
use crate::text::javascript_whitespace;
use legal_grammar::{CompiledEcmascriptGrammar, CompiledGrammar};
use std::ops::Range;
use std::sync::LazyLock;

fn backtracking(id: &str) -> CompiledGrammar {
    legal_grammar::compile_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

fn linear(id: &str) -> CompiledEcmascriptGrammar {
    legal_grammar::compile_ecmascript_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

// The locator needs lookahead so a lettered locator ("12a") never eats the
// first letter of a following word, which is the backtracking dialect.
static LOCATOR: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("pinpoint.locator"));
static ITEM: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("pinpoint.item"));
static TOKEN: LazyLock<regex::Regex> = LazyLock::new(|| {
    let tables = legal_grammar::load_tables().unwrap();
    regex::Regex::new(&tables["pinpoint.token"].entry.pattern).unwrap()
});
static PINPOINT_PHRASES: LazyLock<[(PinpointKind, CompiledEcmascriptGrammar); 3]> = LazyLock::new(|| [
    (PinpointKind::Paragraph, linear("pinpoint.para.toa")),
    (PinpointKind::Section, linear("pinpoint.section.toa")),
    (PinpointKind::Page, linear("pinpoint.page.toa")),
]);
static PINPOINT_BRIDGE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("pinpoint.bridge"));

/// LSP pinpoint_hits: first eligible group, with its original tie ordering.
fn pinpoint_phrase(text: &str, start: usize, limit: usize) -> Option<(PinpointKind, crate::Span)> {
    let tail = &text[start..limit];
    let mut selected = None;
    for (kind, pattern) in PINPOINT_PHRASES.iter() {
        let Some(captures) = pattern.captures(tail) else { continue };
        let matched = captures.get(0).unwrap();
        if !PINPOINT_BRIDGE.is_match(&tail[..matched.start()]) { continue; }
        if selected.is_none_or(|(_, at, _)| matched.start() < at) {
            selected = Some((*kind, matched.start(), matched.end()));
        }
    }
    let (kind, _, end) = selected?;
    Some((kind, clean_pin_cite(text, start..start + end)?))
}

/// Legal Structure Parser's numeric-token projection of a parsed locator.
/// Keep the full range in `Pinpoint`; callers needing individual source tokens
/// receive their original text and byte offsets here.
pub fn pinpoint_tokens(span: &crate::Span) -> impl Iterator<Item = crate::Span> + '_ {
    TOKEN.find_iter(&span.text).map(|matched| crate::Span {
        text: matched.as_str().to_owned(),
        start: span.start + matched.start(),
        end: span.start + matched.end(),
    })
}

/// Eyecite helpers.clean_pin_cite, retaining the cleaned text's source offsets.
pub(crate) fn clean_pin_cite(text: &str, range: Range<usize>) -> Option<crate::Span> {
    let raw = &text[range.clone()];
    let trimmed = raw.trim_matches([',', ' ']);
    let start = range.start + raw.len() - raw.trim_start_matches([',', ' ']).len();
    (!trimmed.is_empty()).then(|| span(text, start..start + trimmed.len()))
}
static SHORT_FORM_SUFFIX: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("shortform.splitter"));
static SOURCE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("parenthetical.source"));
static REMARK: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("parenthetical.remark"));
static RECORD: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("parenthetical.record"));
static COURT: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("parenthetical.court"));
static LAW_PUBLICATION: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("parenthetical.law"));
static POST_CITATION: LazyLock<[CompiledGrammar; 3]> = LazyLock::new(|| {
    ["parenthetical.us.post-full", "parenthetical.us.post-law",
        "parenthetical.us.post-journal"].map(|id|
        legal_grammar::compile_python_table_entry(id).expect("pinned post-citation grammar"))
});
static SOURCE_YEAR: LazyLock<CompiledGrammar> = LazyLock::new(||
    legal_grammar::compile_python_table_entry("parenthetical.us.year").expect("pinned parenthetical year"));
static HISTORY: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("ref.history"));
static VERSUS: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("party.versus"));
static BRACKETED_PARAGRAPH: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("pinpoint.bracketed-paragraph"));

/// A parenthetical may hold nested parentheses but never runs on for pages.
const MAX_PARENTHETICAL: usize = 600;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PostCitation { Case, Law, Journal }

/// What [`tail`] may read after a core.
#[derive(Clone, Copy, Default)]
pub(crate) struct TailRules {
    /// Native core boundary when the source locator starts inside that core.
    pub core_end: Option<usize>,
    /// Pinned metadata grammar, source token end and the farthest point a
    /// full case's date may follow: the paragraph boundary or the first later
    /// citation not joined to it as a parallel. Only a full case's date can
    /// follow tokens beyond the next core boundary.
    pub post_citation: Option<(PostCitation, usize, usize)>,
    /// A Bluebook pinpoint with no keyword: `410 U.S. 113, 153`.
    pub bare_page: bool,
    /// OSCOLA pinpoints after `(n 4)`: a bare page (`353`) or a bracketed
    /// paragraph (`[12]`).
    pub oscola: bool,
    /// Eyecite pinpoints preceding the citation token.
    pub inner: Option<(usize, usize)>,
}

/// The pinpoints, parentheticals and bracketed short form after a core.
#[derive(Default)]
pub(crate) struct Tail {
    pub source_end: Option<usize>,
    pub source_pin: Option<crate::Span>,
    pub source_pin_end: Option<usize>,
    pub source_parenthetical: Option<String>,
    pub extra: Option<String>,
    pub publisher: Option<String>,
    pub court_date: Option<CourtReading>,
    pub pinpoints: Vec<Pinpoint>,
    pub pin_cite: Option<crate::Span>,
    pub pin_cite_kind: Option<PinpointKind>,
    pub parentheticals: Vec<Parenthetical>,
    pub short: Option<String>,
    pub short_span: Option<crate::Span>,
    pub end: usize,
}

fn pinpoint_kind(keyword: Option<&str>) -> PinpointKind {
    let Some(keyword) = keyword else {
        return PinpointKind::Page;
    };
    let word = keyword.trim_end_matches('.').to_lowercase();
    match word.as_str() {
        "¶" | "¶¶" | "para" | "paras" | "paragraph" | "paragraphs" | "par" | "pars" => {
            PinpointKind::Paragraph
        }
        "p" | "pp" | "page" | "pages" => PinpointKind::Page,
        "sub" | "subs" | "subsection" | "subsections" => PinpointKind::Subsection,
        "art" | "arts" | "article" | "articles" => PinpointKind::Article,
        "r" | "rr" | "rule" | "rules" => PinpointKind::Rule,
        "n" | "nn" | "note" | "notes" | "fn" | "fns" | "footnote" | "footnotes" => {
            PinpointKind::Footnote
        }
        "cl" | "cls" | "clause" | "clauses" => PinpointKind::Clause,
        _ if word.starts_with("sch") || word.starts_with("annexe") => PinpointKind::Schedule,
        _ => PinpointKind::Section,
    }
}

fn compact(value: &str) -> String {
    value.chars().filter(|character| !character.is_whitespace()).collect()
}

/// `7(2)` and `(4)` → `7(4)`; `191` and `92` → `192`; otherwise `last` as written.
fn range_end(first: &str, last: &str) -> String {
    if last.starts_with('(') {
        let base = first.find('(').map_or(first, |at| &first[..at]);
        return format!("{base}{last}");
    }
    let digits = |value: &str| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
    if digits(first) && digits(last) && last.len() < first.len() {
        let expanded = format!("{}{last}", &first[..first.len() - last.len()]);
        if expanded.parse::<u64>().ok() > first.parse::<u64>().ok() {
            return expanded;
        }
    }
    last.to_owned()
}

fn is_roman(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| b"ivxlc".contains(&byte))
}

/// One keyword-led pinpoint group at `start`: `at paras 62-64 and 70`,
/// `, s 7(2)`, `au para 23`, `at 553, 559`.
fn pinpoint_group(
    text: &str,
    start: usize,
    limit: usize,
    bare_page: bool,
) -> Option<(Vec<Pinpoint>, crate::Span)> {
    let captures = LOCATOR.captures(&text[start..limit]).ok()??;
    let keyword = captures.name("keyword").map(|value| value.as_str());
    let led = captures.name("at").is_some() || captures.name("elided").is_some();
    let lead = captures.name("lead").map_or("", |value| value.as_str());
    if keyword.is_none() && !led && !(bare_page && lead.contains(',')) {
        return None;
    }
    let kind = pinpoint_kind(keyword);
    let values = captures.name("values")?;
    let base = start + values.start();
    let mut pinpoints = Vec::new();
    for item in ITEM.captures_iter(values.as_str()).flatten() {
        let whole = item.get(0)?;
        let first = compact(item.name("first")?.as_str());
        if is_roman(&first) && kind != PinpointKind::Page {
            break;
        }
        let last = item
            .name("last")
            .map(|last| range_end(&first, &compact(last.as_str())));
        pinpoints.push(Pinpoint {
            kind,
            span: span(text, base + whole.start()..base + whole.end()),
            first,
            last,
        });
    }
    let end = pinpoints.last()?.span.end;
    Some((pinpoints, clean_pin_cite(text, start..end)?))
}

/// A balanced parenthetical opening after optional spaces and a comma.
fn parenthetical_at(text: &str, start: usize, limit: usize) -> Option<Range<usize>> {
    let rest = &text[start..limit];
    let open = start + rest.len()
        - rest
            .trim_start_matches(|character: char| javascript_whitespace(character) || character == ',')
            .len();
    let opening = text[open..limit].chars().next()?;
    let closing = match opening { '(' => ')', '[' => ']', _ => return None };
    let mut depth = 0usize;
    // A following core may itself be cited inside this parenthetical.
    // `limit` constrains where a trailing component starts, not where an
    // already-open parenthetical closes (eyecite.process_parenthetical).
    for (offset, character) in text[open..].char_indices() {
        if offset > MAX_PARENTHETICAL || character == '\n' {
            return None;
        }
        match character {
            value if value == opening => depth += 1,
            value if value == closing => {
                depth -= 1;
                if depth == 0 {
                    if opening == '[' {
                        let reading = read_court(&text[open + 1..open + offset])?;
                        if reading.date.is_none() && reading.court.as_deref().is_none_or(|court|
                            crate::registry::registry().courts_by_surface(court).is_empty()) {
                            return None;
                        }
                    }
                    return Some(open..open + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// A parenthetical that belongs to the citation: the author's own sentence
/// after it ("(Mr. Moss did not argue the point)") ends the citation.
fn citation_parenthetical(text: &str, start: usize, limit: usize) -> Option<Range<usize>> {
    parenthetical_at(text, start, limit).filter(|range| !REMARK.is_match(text[range.start + 1..range.end - 1].trim()))
}

/// The court a parenthetical names, with its date.
pub(crate) struct CourtReading {
    pub court: Option<String>,
    pub date: Option<String>,
    pub year: Option<String>,
    pub month: Option<String>,
    pub day: Option<String>,
}

pub(crate) fn read_court(content: &str) -> Option<CourtReading> {
    let captures = COURT.captures(content.trim()).ok()??;
    let court = captures.name("court").map(|value| value.as_str().trim().to_owned());
    let date = captures.name("date").map(|value| value.as_str().to_owned());
    let year = captures.name("year").map(|value| value.as_str().to_owned());
    let month = captures.name("month").map(|value| value.as_str().to_owned());
    let day = captures.name("day").or_else(|| captures.name("day_before")).map(|value| value.as_str().to_owned());
    (court.as_deref().is_some_and(|court| !court.is_empty()) || date.is_some())
        .then_some(CourtReading { court, date, year, month, day })
}

fn parenthetical(text: &str, range: Range<usize>) -> Parenthetical {
    let content = text[range.start + 1..range.end - 1].trim().to_owned();
    let kind = if SOURCE.is_match(&content) {
        ParentheticalKind::Source
    } else if RECORD.is_match(&content) {
        ParentheticalKind::Record
    } else if !content.is_empty() && content.bytes().all(|byte| byte.is_ascii_digit()) {
        // A bare numbered subdivision after a pinpoint is not a court.
        ParentheticalKind::Explanatory
    } else if let Some(reading) = read_court(&content) {
        if reading.court.is_some() {
            ParentheticalKind::Court
        } else {
            ParentheticalKind::Date
        }
    } else {
        ParentheticalKind::Explanatory
    };
    Parenthetical {
        kind,
        span: span(text, range),
        content,
    }
}

/// `[Hansman]` right after the pinpoints. The sentence period after the
/// bracket ends the sentence, not the citation.
fn explicit_short_form(text: &str, start: usize, limit: usize) -> Option<(String, usize)> {
    let tail = &text[start..limit];
    let end = tail.find(']')? + 1;
    let captures = SHORT_FORM_SUFFIX.captures(&tail[..end])?;
    if captures.get(0).unwrap().start() != 0 {
        return None;
    }
    let short = captures.name("short").unwrap().as_str().trim();
    if short.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    Some((short.to_owned(), start + end))
}

/// OSCOLA's page pinpoint after `(n 4)`: a bare number (`Jordan (n 4) 353`).
fn oscola_page(text: &str, start: usize, limit: usize) -> Option<(Pinpoint, usize)> {
    let rest = &text[start..limit];
    let body = rest.trim_start_matches([' ', '\u{a0}']);
    let at = start + rest.len() - body.len();
    let digits = body.bytes().take_while(u8::is_ascii_digit).count();
    if at == start || digits == 0 || body[digits..].chars().next().is_some_and(char::is_alphanumeric) {
        return None;
    }
    Some((
        Pinpoint {
            kind: PinpointKind::Page,
            span: span(text, at..at + digits),
            first: body[..digits].to_owned(),
            last: None,
        },
        at + digits,
    ))
}

/// `at [87]`, `[87]-[90]`: the bracketed paragraph of UK, Australian and New
/// Zealand judgments.
fn bracketed_paragraph(text: &str, start: usize, limit: usize) -> Option<(Pinpoint, usize)> {
    let captures = BRACKETED_PARAGRAPH.captures(&text[start..limit])?;
    let whole = captures.get(0)?;
    let first = captures.name("first")?;
    Some((
        Pinpoint {
            kind: PinpointKind::Paragraph,
            span: span(text, start + first.start() - 1..start + whole.end()),
            first: first.as_str().to_owned(),
            last: captures.name("last").map(|last| last.as_str().to_owned()),
        },
        start + whole.end(),
    ))
}

/// Whether only the first citation's pinpoints and commas separate two
/// citation cores (`410 U.S. 113, 153, 93 S. Ct. 705`), which is how
/// parallel citations are joined.
pub(crate) fn parallel_gap(text: &str, start: usize, end: usize) -> bool {
    let mut cursor = start;
    while let Some(next) = pinpoint_group(text, cursor, end, true).map(|(_, phrase)| phrase.end)
        .or_else(|| bracketed_paragraph(text, cursor, end).map(|(_, next)| next))
        .filter(|next| *next > cursor) {
        cursor = next;
    }
    text[cursor..end].chars().all(|character| character == ',' || javascript_whitespace(character))
}

/// Original LSP pinpoint and explicit-short-form extent, shared by both views.
pub(crate) fn native_tail(text: &str, start: usize, limit: usize) -> Tail {
    let mut result = Tail {
        end: start,
        ..Tail::default()
    };
    if let Some((kind, phrase)) = pinpoint_phrase(text, start, limit) {
        result.pin_cite = Some(phrase);
        result.pin_cite_kind = Some(kind);
    }
    let local_end = result.pin_cite.as_ref().map_or(start, |pin| pin.end);
    if let Some((short, end)) = explicit_short_form(text, local_end, limit) {
        result.short = Some(short);
        result.short_span = Some(span(text, local_end..end));
    }
    result.end = result.short_span.as_ref().map_or(local_end, |short| short.end);
    result
}

/// Walk pinpoints and parentheticals after the native extent. An explanatory
/// parenthetical can contain another citation and end beyond `limit`.
pub(crate) fn tail(text: &str, start: usize, limit: usize, rules: TailRules) -> Tail {
    let mut result = native_tail(text, rules.core_end.unwrap_or(start), limit);
    if let Some((inner_start, inner_end)) = rules.inner {
        if let Some((pinpoints, _)) = pinpoint_group(text, inner_start, inner_end, false) {
            result.pinpoints.extend(pinpoints);
        }
    }
    let mut cursor = start;
    let mut bare_page = rules.bare_page;
    if rules.oscola {
        if let Some((pinpoint, end)) = oscola_page(text, cursor, limit) {
            result.pinpoints.push(pinpoint);
            cursor = end;
        }
    }
    while cursor < limit {
        if let Some((pinpoints, phrase)) = pinpoint_group(text, cursor, limit, bare_page) {
            let end = phrase.end;
            result.pinpoints.extend(pinpoints);
            cursor = end;
            bare_page = false;
            continue;
        }
        if let Some((pinpoint, end)) = bracketed_paragraph(text, cursor, limit) {
            result.pinpoints.push(pinpoint);
            cursor = end;
            continue;
        }
        if let Some(range) = citation_parenthetical(text, cursor, limit) {
            // The source-specific post-citation grammar below owns any
            // explanatory extent crossing the next recognized citation.
            if range.end > limit {
                break;
            }
            cursor = range.end;
            result.parentheticals.push(parenthetical(text, range));
            continue;
        }
        break;
    }
    if let Some((source, start, reach)) = rules.post_citation {
        result.source_end = Some(start);
        // Full cases may span parallel citation tokens. Other source forms
        // stop at the next citation as well as the paragraph boundary.
        let window = &text[start..if source == PostCitation::Case { reach } else { reach.min(limit) }];
        let end = window.char_indices().nth(300).map_or(window.len(), |(at, _)| at);
        if let Some(captures) = POST_CITATION[source as usize].captures(&window[..end]).expect("post-citation match") {
            {
                let mut source_end = start + captures.get(0).unwrap().end();
                if let Some(part) = captures.name("parenthetical") {
                    result.source_parenthetical = process_parenthetical(part.as_str());
                    if source == PostCitation::Case {
                        if let Some(value) = &result.source_parenthetical {
                            source_end -= part.as_str().len() - value.len();
                        }
                    }
                }
                result.source_end = Some(source_end);
                if let Some(pin) = captures.name("pin_cite").or_else(|| captures.name("pin_cite_2")) {
                    if !pin.as_str().is_empty() { result.source_pin_end = Some(start + pin.as_str().len()); }
                    result.source_pin = clean_pin_cite(text, start + pin.start()..start + pin.end());
                }
            }
            if let Some(extra) = captures.name("extra") {
                let group = |name| captures.name(name).map(|value| value.as_str().to_owned());
                result.court_date = Some(CourtReading {
                    court: group("court").map(|value| value.trim().to_owned()).filter(|value| !value.is_empty()),
                    date: group("year_2"), year: group("year_3"),
                    month: group("month_3"), day: group("day"),
                });
                let extra_text = extra.as_str().trim_matches(crate::text::python_whitespace);
                result.extra = (!extra_text.is_empty()).then(|| extra_text.to_owned());
                let matched_end = start + captures.get(0).unwrap().end();
                let mut at = start + extra.end();
                // The source grammar has a court/date parenthetical and at
                // most one following explanation. Reuse the shared balanced
                // extent reader so an explanation can contain another cite.
                for _ in 0..2 {
                    let Some(range) = citation_parenthetical(text, at, matched_end) else { break };
                    at = range.end;
                    if !result.parentheticals.iter().any(|part| part.span.start == range.start) {
                        result.parentheticals.push(parenthetical(text, range));
                    }
                }
                cursor = cursor.max(at);
            } else if source != PostCitation::Case {
                let group = |name| captures.name(name).map(|value| value.as_str().to_owned());
                result.publisher = group("publisher");
                result.court_date = Some(CourtReading {
                    court: None, date: group("year"), year: group("year"),
                    month: group("month"), day: group("day"),
                });
                cursor = cursor.max(start + captures.get(0).unwrap().end());
            }
        }
    }
    result.end = cursor.max(result.end);
    result
}

/// Eyecite helpers.process_parenthetical, shared by full and short forms.
fn process_parenthetical(value: &str) -> Option<String> {
    let mut balance = 0;
    for (at, character) in value.char_indices() {
        if character == '(' { balance += 1; }
        if character == ')' { balance -= 1; }
        if balance < 0 { return (!value[..at].is_empty()).then(|| value[..at].to_owned()); }
    }
    (!value.is_empty() && !SOURCE_YEAR.is_match(value).expect("source parenthetical year"))
        .then(|| value.to_owned())
}

/// Eyecite extract_pin_cite: prepend the token's page and stop at the caller's
/// first non-string token. Prefix offsets are subtracted from the matched extent.
pub(crate) fn short_reference(text: &str, token: Range<usize>, limit: usize, prefix: &str) -> crate::SourceCaseName {
    static POST_SHORT: LazyLock<CompiledGrammar> = LazyLock::new(||
        legal_grammar::compile_python_table_entry("parenthetical.us.post-short").unwrap());
    let window: String = prefix.chars().chain(text[token.end..limit].chars()).take(300).collect();
    let mut result = crate::SourceCaseName {
        full_span_start: token.start, full_span_end: Some(token.end),
        token_span: Some(span(text, token.clone())), reference_span: Some(span(text, token.clone())),
        ..Default::default()
    };
    if let Some(captures) = POST_SHORT.captures(&window).expect("source short metadata") {
        let base = token.end - prefix.len();
        let mut extra = 0;
        if let Some(pin) = captures.name("pin_cite").filter(|pin| !pin.as_str().is_empty()) {
            let raw = pin.as_str();
            let cleaned = raw.trim_matches([',', ' ']);
            extra = raw.trim_end_matches([',', ' ']).len();
            let start = base + pin.start() + raw.len() - raw.trim_start_matches([',', ' ']).len();
            result.pin_cite = (!cleaned.is_empty()).then(|| crate::Span {
                text: cleaned.to_owned(), start, end: start + cleaned.len(),
            });
        }
        let span_end = base + extra;
        result.reference_span = Some(span(text, token.start..if span_end == 0 { token.end } else { span_end }));
        result.full_span_end = Some(span_end.max(token.end));
        result.parenthetical = captures.name("parenthetical").and_then(|part| process_parenthetical(part.as_str()));
    }
    result
}

/// Normalized relation of a history phrase.
fn relation(phrase: &str) -> &'static str {
    let lower = phrase.to_lowercase().replace('’', "'");
    let words = lower.split_whitespace().collect::<Vec<_>>();
    if lower.starts_with("leave") || lower.starts_with("autorisation") {
        return match words.last().copied() {
            Some("granted" | "allowed" | "accordée") => "leave_granted",
            _ => "leave_refused",
        };
    }
    if lower.starts_with("appeal") {
        return match words.last().copied() {
            Some("allowed") => "appeal_allowed",
            Some("quashed") => "appeal_quashed",
            Some("abandoned") => "appeal_abandoned",
            _ => "appeal_dismissed",
        };
    }
    if lower.starts_with("cert") {
        return match words.last().copied() {
            Some("granted") => "cert_granted",
            Some("dismissed") => "cert_dismissed",
            _ => "cert_denied",
        };
    }
    let stem = words.first().copied().unwrap_or_default();
    let ing = stem.ends_with('g') || stem.ends_with("ing");
    match stem.chars().next() {
        Some('a') if ing => "affirming",
        Some('a') => "affirmed",
        Some('c') => "affirmed",
        Some('r') if ing => "reversing",
        Some('r') | Some('i') => "reversed",
        Some('v') if ing => "varying",
        Some('v') | Some('m') => "varied",
        Some('o') if ing => "overruling",
        Some('o') => "overruled",
        _ => "related",
    }
}

/// The history phrase at the start of the gap after `citation`, and whether
/// the gap holds nothing else before `next_start`.
fn history(text: &str, from: usize, to: usize) -> Option<(History, bool)> {
    let gap = &text[from..to];
    let captures = HISTORY.captures(gap)?;
    let matched = captures.get(0)?;
    let phrase = captures.name("relation")?;
    let end = captures
        .name("court")
        .map(|court| court.end() + 1)
        .or_else(|| captures.name("qualifier").map(|qualifier| qualifier.end()))
        .unwrap_or(phrase.end());
    let reaches_next = gap[matched.end()..]
        .trim_matches(|character: char| javascript_whitespace(character) || character == ',')
        .is_empty();
    Some((
        History {
            relation: relation(phrase.as_str()).to_owned(),
            span: span(text, from + phrase.start()..from + end.min(gap.len())),
            target: None,
        },
        reaches_next,
    ))
}

/// A style of cause with one party: `Re Moore`, `Reference re Secession of
/// Quebec`, `Renvoi relatif à ...`, `Moore (Re)`, `Ex parte Smith`.
pub(crate) fn single_party(style: &str) -> bool {
    let lower = style.trim_start().to_lowercase();
    ["re ", "reference re", "renvoi", "in re ", "in the matter of", "ex parte", "ex p "]
        .iter()
        .any(|lead| lower.starts_with(lead))
        || lower.ends_with("(re)")
        || lower.ends_with("(renvoi)")
}

/// Split a two-party style of cause on its last `v`/`v.`/`c.`/`vs`.
pub(crate) fn parties(style: &str) -> Option<Parties> {
    if single_party(style) {
        return None;
    }
    let trim = |value: &str| {
        value
            .trim_matches(|character: char| javascript_whitespace(character) || character == ',')
            .to_owned()
    };
    let versus = VERSUS
        .find_iter(style)
        .flatten()
        .filter(|matched| {
            matched.start() > 0
                && style[matched.end()..]
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_uppercase() || character.is_numeric())
        })
        .last()?;
    let plaintiff = trim(&style[..versus.start()]);
    let defendant = trim(&style[versus.end()..]);
    (!plaintiff.is_empty() && !defendant.is_empty()).then_some(Parties {
        plaintiff: Some(plaintiff),
        defendant: Some(defendant),
    })
}

/// The first dated court/date parenthetical, using the same parsed fields
/// for classification and metadata attachment.
pub(crate) fn parenthetical_date(citation: &Citation) -> Option<CourtReading> {
    citation.parentheticals.iter()
        .filter(|parenthetical| matches!(parenthetical.kind, ParentheticalKind::Court | ParentheticalKind::Date))
        .find_map(|parenthetical| {
            read_court(&parenthetical.content).filter(|reading| reading.date.is_some())
        })
}

/// Dates and history links. Registry court selection is owned by classification.
pub fn attach(text: &str, citations: &mut [Citation]) {
    for citation in citations.iter_mut() {
        if let Some(date) = parenthetical_date(citation) {
            citation.fields.year = citation.fields.year.take().or(date.year);
            citation.fields.month = citation.fields.month.take().or(date.month);
            citation.fields.day = citation.fields.day.take().or(date.day);
        }
        if citation.authority.is_legislation() && citation.fields.source_groups.is_empty() {
            if let Some(publication) = citation.parentheticals.iter().find_map(|parenthetical|
                LAW_PUBLICATION.captures(&parenthetical.content).expect("law publication metadata")) {
                for (name, field) in [("publisher", &mut citation.fields.publisher),
                    ("month", &mut citation.fields.month), ("day", &mut citation.fields.day),
                    ("year", &mut citation.fields.year)] {
                    if field.is_none() { *field = publication.name(name).map(|value| value.as_str().to_owned()); }
                }
            }
        }
    }
    for index in 0..citations.len() {
        let from = citations[index].full_span.end;
        let (to, next) = match citations.get(index + 1) {
            Some(next) => (
                next.signal.as_ref().map_or(next.full_span.start, |signal| signal.start),
                Some(next.index),
            ),
            None => (text.len(), None),
        };
        if from > to {
            continue;
        }
        if let Some((mut entry, reaches_next)) = history(text, from, to) {
            if reaches_next {
                entry.target = next;
            }
            citations[index].history.push(entry);
        }
    }
}
