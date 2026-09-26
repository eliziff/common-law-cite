//! Pinpoint ranges, parentheticals, subsequent history, parties and court.
//!
//! [`crate::find`] walks the text after each core with [`tail`] to fix the
//! citation's extent; [`attach`] then reads what the extent holds: the court a
//! parenthetical names, the history phrase that links a decision to the next
//! citation, and the parties of a style of cause.

use crate::find::span;
use crate::model::{
    Citation, CourtRef, Form, History, Parenthetical, ParentheticalKind, Parties, Pinpoint,
    PinpointKind,
};
use crate::registry::registry;
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
static SHORT_FORM_SUFFIX: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("shortform.splitter"));
static SOURCE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("parenthetical.source"));
static COURT: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("parenthetical.court"));
static HISTORY: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("ref.history"));
static VERSUS: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("party.versus"));
static BRACKETED_PARAGRAPH: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("pinpoint.bracketed-paragraph"));

/// A parenthetical may hold nested parentheses but never runs on for pages.
const MAX_PARENTHETICAL: usize = 600;

/// What [`tail`] may read after a core.
#[derive(Clone, Copy, Default)]
pub(crate) struct TailRules {
    /// A Bluebook pinpoint with no keyword: `410 U.S. 113, 153`.
    pub bare_page: bool,
    /// OSCOLA pinpoints after `(n 4)`: a bare page (`353`) or a bracketed
    /// paragraph (`[12]`).
    pub oscola: bool,
    /// Pinpoints that sit inside the styled part (`Canadian Charter ..., s 7,
    /// Part I ...`) and were read before the core.
    pub inner: Option<(usize, usize)>,
}

/// The pinpoints, parentheticals and bracketed short form after a core.
#[derive(Default)]
pub(crate) struct Tail {
    pub pinpoints: Vec<Pinpoint>,
    pub parentheticals: Vec<Parenthetical>,
    pub short: Option<String>,
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
) -> Option<(Vec<Pinpoint>, usize)> {
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
    Some((pinpoints, end))
}

/// A balanced parenthetical opening after optional spaces and a comma.
fn parenthetical_at(text: &str, start: usize, limit: usize) -> Option<Range<usize>> {
    let rest = &text[start..limit];
    let open = start + rest.len()
        - rest
            .trim_start_matches(|character: char| javascript_whitespace(character) || character == ',')
            .len();
    if !text[open..limit].starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    for (offset, character) in text[open..limit].char_indices() {
        if offset > MAX_PARENTHETICAL || character == '\n' {
            return None;
        }
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open..open + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// The court a parenthetical names, with its date.
pub(crate) struct CourtReading {
    pub court: Option<String>,
    pub date: Option<String>,
}

pub(crate) fn read_court(content: &str) -> Option<CourtReading> {
    let captures = COURT.captures(content.trim()).ok()??;
    let court = captures.name("court").map(|value| value.as_str().trim().to_owned());
    let date = captures.name("date").map(|value| value.as_str().to_owned());
    (court.as_deref().is_some_and(|court| !court.is_empty()) || date.is_some())
        .then_some(CourtReading { court, date })
}

fn parenthetical(text: &str, range: Range<usize>) -> Parenthetical {
    let content = text[range.start + 1..range.end - 1].trim().to_owned();
    let kind = if SOURCE.is_match(&content) {
        ParentheticalKind::Source
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

/// `[Hansman]` (and a following period) right after the pinpoints.
fn explicit_short_form(text: &str, start: usize, limit: usize) -> Option<(String, usize)> {
    let tail = &text[start..limit];
    let close = tail.find(']')?;
    let mut end = close + 1;
    let remainder = &tail[end..];
    let after_space = remainder.trim_start_matches(javascript_whitespace);
    if after_space.starts_with('.') {
        end += remainder.len() - after_space.len() + 1;
    }
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

/// Walk pinpoints, parentheticals and a bracketed short form from `start`,
/// never past `limit` (the next citation's core).
pub(crate) fn tail(text: &str, start: usize, limit: usize, rules: TailRules) -> Tail {
    let mut result = Tail {
        end: start,
        ..Tail::default()
    };
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
    loop {
        if let Some((pinpoints, end)) = pinpoint_group(text, cursor, limit, bare_page) {
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
        if let Some(range) = parenthetical_at(text, cursor, limit) {
            cursor = range.end;
            result.parentheticals.push(parenthetical(text, range));
            continue;
        }
        break;
    }
    result.end = cursor;
    if let Some((short, end)) = explicit_short_form(text, cursor, limit) {
        result.short = Some(short);
        result.end = end;
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
        plaintiff,
        defendant,
    })
}

/// Resolve a parenthetical's court surface (`ON CA`, `HL`, `2d Cir.`) against
/// the registry's aliases.
fn court_of(content: &str) -> Option<(CourtRef, String)> {
    let reading = read_court(content)?;
    let surface = reading.court?;
    let court = registry().court_by_surface(&surface)?;
    Some((
        CourtRef {
            id: court.id.clone(),
            text: surface,
        },
        court.jurisdiction.clone(),
    ))
}

/// Parties, court and date from parentheticals, and history links.
pub fn attach(text: &str, citations: &mut [Citation]) {
    for citation in citations.iter_mut() {
        if citation.form == Form::Full && citation.parties.is_none() {
            if let Some(style) = &citation.style {
                if citation.authority == crate::model::Authority::Case {
                    citation.parties = parties(&style.text);
                }
            }
        }
        for parenthetical in &citation.parentheticals {
            if parenthetical.kind != ParentheticalKind::Court
                && parenthetical.kind != ParentheticalKind::Date
            {
                continue;
            }
            if citation.court.is_none() {
                if let Some((court, jurisdiction)) = court_of(&parenthetical.content) {
                    citation.court = Some(court);
                    // The court's jurisdiction refines a series-wide one
                    // (CarswellOnt `ca` -> Ontario Superior Court `ca-on`).
                    let refines = citation.jurisdiction.as_deref().is_none_or(|current| {
                        jurisdiction.starts_with(current) && jurisdiction.len() > current.len()
                    });
                    if refines {
                        citation.jurisdiction = Some(jurisdiction);
                    }
                    citation.reasons.push("court_parenthetical".to_owned());
                }
            }
            if citation.fields.year.is_none() {
                if let Some(date) = read_court(&parenthetical.content).and_then(|reading| reading.date) {
                    let year = date.chars().rev().take(4).collect::<Vec<_>>();
                    citation.fields.year = Some(year.into_iter().rev().collect());
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
