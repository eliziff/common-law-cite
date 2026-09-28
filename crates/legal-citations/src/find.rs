//! Discover citation cores and their styled extent: full citations, U.S.
//! short forms, `Ibid`/`Id.`, `supra`/`above n`/`(n 4)`/`précité`, bare
//! case-name references, bare section symbols, and the introductory signal in
//! front of each. Classification and metadata happen in later stages.
//!
//! Full spans can overlap when an explanatory parenthetical contains another
//! citation. Document splitting owns source boundaries independently.

use crate::metadata::{self, TailRules};
use crate::model::{
    Authority, Citation, Fields, Form, NoteDirection, NoteReference, Pinpoint, PinpointKind, Span,
};
use crate::text::javascript_whitespace;
use crate::Options;
use legal_grammar::{CompiledEcmascriptGrammar, CompiledGrammar};
use regex::Regex;
use std::collections::HashSet;
use std::ops::Range;
use std::sync::LazyLock;

pub(crate) type Hit = Range<usize>;

fn linear(id: &str) -> CompiledEcmascriptGrammar {
    legal_grammar::compile_ecmascript_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

fn backtracking(id: &str) -> CompiledGrammar {
    legal_grammar::compile_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

static CITATION_PATTERN: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.in-text"));
// The style grammars spell Unicode letter classes with the corpus's Latin
// letter defs, since the corpus bans \p{} classes.
static CASE_NAME: LazyLock<Regex> = LazyLock::new(|| linear("style.case-name"));
static SIGNAL_PREFIX: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("signal.prefix.toa"));
static INTRODUCTORY_SIGNAL: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("signal.introductory"));
static INTRODUCTORY_PREFIX: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("signal.introductory.prefix"));
// Précité ends in a non-ASCII letter, so its word boundary needs the Unicode
// (backtracking) dialect.
static BACK_REFERENCE: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("ref.back"));
static NOTE_CROSS_REFERENCE: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("ref.note-cross"));
static ANTECEDENT_NAME: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("ref.antecedent-name"));
static SECTION_SYMBOL: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.section-symbol"));
// Complete existing cores, or discover a report whose abbreviation the
// registry independently verifies. The splitter alone is not evidence.
static REPORTER_PATTERN: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("cite.reporter.splitter"));
pub(crate) static REPORTER_PARTS: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("cite.reporter.parts"));
pub(crate) fn reporter_parts(core: &str) -> Option<legal_grammar::GrammarCaptures<'_>> {
    REPORTER_PARTS.captures(core).ok().flatten()
}
pub(crate) static JOURNAL_CUE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.journal.title-cue"));
pub(crate) static TREATY: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.treaty"));
pub(crate) static PARLIAMENTARY_COMMONWEALTH: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.parliamentary.commonwealth"));
pub(crate) static DATABASE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.database"));
static CHARTER: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.ca.charter"));
static LAW_SUBDIVISION: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.us.law.subdivision"));
// Secondary-source first references. Case law reaches the citation grammar
// through a reporter, a neutral citation or a docket; every other authority
// announces itself with a publication block instead, one family per entry.
// These only ever add anchors: a secondary hit that touches a case-law hit is
// dropped, so the case lane is byte-identical with and without them.
static SECONDARY_ECMASCRIPT: LazyLock<[(&'static str, &'static str, CompiledEcmascriptGrammar); 9]> =
    LazyLock::new(|| {
        [
            // Multi-word tribunal identifiers (2020 Comp Trib 6, 2000 CIRB LD
            // 213) are out of cite.in-text's reach.
            ("case", "neutral_grammar", "cite.neutral.tribunal"),
            ("case", "neutral_grammar", "cite.neutral.bracketed"),
            ("statute", "ca_statute_grammar", "cite.ca.statute.first"),
            ("statute", "titled_statute_grammar", "cite.statute.titled"),
            ("book", "book_grammar", "cite.book.imprint"),
            ("parliamentary", "parliamentary_grammar", "cite.parliamentary.paper"),
            ("parliamentary", "westminster_grammar", "cite.parliamentary.commonwealth"),
            ("treaty", "treaty_grammar", "cite.treaty"),
            ("case", "database_grammar", "cite.database"),
        ]
        .map(|(kind, reason, id)| (kind, reason, linear(id)))
    });
// Article without a first page ("… (2020) The Journal of Value Inquiry at 1")
// needs a lookahead so the pinpoint stays outside the core, which is the
// backtracking dialect rather than the linear one.
static JOURNAL_ARTICLE: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("cite.journal.article"));
static ONLINE_SOURCE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.url"));
// The frozen URL grammar also matches adjacent sentence punctuation. Keep
// balanced parentheses in the URL while leaving trailing prose outside it.
pub(crate) fn online_source_end(value: &str) -> usize {
    let mut end = value.len();
    while end > 0 {
        match value.as_bytes()[end - 1] {
            b'.' | b',' | b';' | b'>' => end -= 1,
            b')' => {
                let mut open = 0usize;
                for byte in value[..end - 1].bytes() {
                    match byte {
                        b'(' => open += 1,
                        b')' => open = open.saturating_sub(1),
                        _ => {}
                    }
                }
                if open == 0 { end -= 1; } else { break; }
            }
            _ => break,
        }
    }
    end
}
// ALR's source evidence for secondary authorities without a reporter, book
// imprint or routable URL. These matches remain citation extents, not document
// splitting boundaries.
static QUOTED_SOURCE: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("cite.quoted"));
static SECONDARY_SOURCE: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("cite.secondary"));
static CASE_VERSUS: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("party.versus"));
// A balanced, uppercase-first parenthetical ("Quebec (Attorney General)")
// counts as one party token; "(1998)", "(2d)" and "(see below)" do not.
static CASE_LEFT: LazyLock<Regex> = LazyLock::new(|| linear("style.case-left"));
// What can open a numbered paragraph ahead of its first case: the paragraph's
// own label ("12.", "[12]", "(a)") and a leading "In". Neither is part of a
// party name, although the party grammar accepts numbers and capitals.
static CASE_LEAD_IN: LazyLock<Regex> = LazyLock::new(|| linear("style.case-lead-in"));
// The hard delimiters between two authorities in one footnote: a semicolon,
// or a sentence period that is not an abbreviation or an initial. No styled
// span reaches back across one, so widening a span can never swallow the
// boundary the next authority is split on. Only top-level delimiters count
// (see `top_level`), and a corporate suffix's period is not a sentence end.
static STYLED_FLOOR: LazyLock<Regex> = LazyLock::new(|| linear("style.floor"));
static CORPORATE_SUFFIX: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("boundary.corporate-suffix"));
// "Reference re Secession of Quebec" / "Re Residential Tenancies Act" /
// "Renvoi relatif à la sécession du Québec" / "Moore (Re)": a style of cause
// with one party instead of two.
static CASE_RE_STYLE: LazyLock<Regex> = LazyLock::new(|| linear("style.case-single-party"));
static PLACEHOLDER: LazyLock<CompiledGrammar> = LazyLock::new(|| {
    legal_grammar::compile_python_table_entry("cite.us.placeholder").expect("source placeholder grammar")
});
// The title a statute or treaty citation is styled with, ending in the
// instrument word and optionally carrying its own regnal year and jurisdiction.
static STATUTE_TITLE: LazyLock<Regex> = LazyLock::new(|| linear("style.statute-title"));
static TRAILING_DATE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("style.trailing-date"));
// "Author, \u{201c}Article Title\u{201d}" (and any "in Editor, ed," lead-in):
// the styled part of a secondary source sitting in front of its publication
// block.
static QUOTED_WORK: LazyLock<Regex> = LazyLock::new(|| linear("style.quoted-work"));
// The same styled part when the work carries no quoted title: a monograph, a
// debate record, a dictionary.
static PLAIN_WORK: LazyLock<Regex> = LazyLock::new(|| linear("style.plain-work"));
static STANDARD_CANDIDATE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.us.reporter.candidate"));
fn surfaces(names: &[&str]) -> HashSet<String> {
    let tables = legal_grammar::load_tables().expect("shared US citation grammar");
    let entry = tables
        .get("cite.us.reporter.standard.full")
        .expect("shared reporter grammar");
    names
        .iter()
        .flat_map(|name| split_literal_alternation(&entry.defs[*name]))
        .map(decode_surface)
        .collect()
}
static US_JOURNAL_SURFACES: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let reporters = surfaces(&["us_reporters"]);
    surfaces(&["us_journals"]).into_iter()
        .filter(|surface| !reporters.contains(surface))
        .collect()
});

/// Names that open a style of cause but never identify a case on their own.
const GENERIC_PARTIES: [&str; 18] = [
    "r", "r.", "rex", "regina", "the queen", "her majesty the queen", "his majesty the king",
    "the king", "queen", "king", "crown", "canada", "united states", "state", "people",
    "commonwealth", "the state", "attorney general",
];

fn split_literal_alternation(source: &str) -> Vec<&str> {
    let inner = source
        .strip_prefix("(?:")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(source);
    let mut values = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (index, character) in inner.char_indices() {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '|' {
            values.push(&inner[start..index]);
            start = index + 1;
        }
    }
    values.push(&inner[start..]);
    values
}

fn decode_surface(source: &str) -> String {
    let source = source.replace(r"\s*", "").replace(' ', "");
    let mut decoded = String::with_capacity(source.len());
    let mut characters = source.chars();
    while let Some(character) = characters.next() {
        decoded.push(if character == '\\' {
            characters.next().unwrap_or(character)
        } else {
            character
        });
    }
    decoded
}

fn compact_surface(reporter: &str) -> String {
    reporter
        .chars()
        .filter(|character| !javascript_whitespace(*character))
        .collect()
}

/// Whether a whole core is a standard U.S. journal citation (`100 Harv. L.
/// Rev. 1234`), which the reporter grammars would otherwise read as a case.
pub(crate) fn us_journal(core: &str) -> bool {
    STANDARD_CANDIDATE.captures(core).is_some_and(|captures| {
        let citation = captures.name("citation").unwrap();
        citation.start() == 0
            && citation.end() == core.len()
            && US_JOURNAL_SURFACES.contains(&compact_surface(&captures["reporter"]))
    })
}

/// A discovered authority anchor carries the family and fields recognized by
/// its grammar. Unclassified spans are read when the anchor is materialized.
/// `style_start` and `inner` carry a styled part
/// and pinpoints the grammar itself located (the Charter's full form).
#[derive(Default)]
struct Anchor {
    span: Hit,
    family: Option<(&'static str, &'static str)>,
    reading: Option<crate::classify::Reading>,
    style_start: Option<usize>,
    inner: Option<(usize, usize)>,
}

impl Anchor {
    fn new(text: &str, span: Hit, family: Option<(&'static str, &'static str)>) -> Self {
        let reading = crate::classify::read(&text[span.clone()], family.map_or("", |(_, reason)| reason), "");
        let family = family.or_else(|| reading.as_ref().map(|reading| reading.family()));
        Self {
            reading,
            span,
            family,
            style_start: None,
            inner: None,
        }
    }
}

fn resolve(mut found: Vec<Anchor>) -> Vec<Anchor> {
    found.sort_by(|left, right| {
        left.span.start
            .cmp(&right.span.start)
            .then_with(|| right.span.end.cmp(&left.span.end))
            .then_with(|| right.reading.is_some().cmp(&left.reading.is_some()))
    });
    let mut resolved: Vec<Anchor> = Vec::new();
    for hit in found {
        if resolved
            .last()
            .is_some_and(|previous| hit.span.start < previous.span.end)
        {
            continue;
        }
        resolved.push(hit);
    }
    resolved
}

/// The Charter's McGill form: the title is the style, the pinpoint sits
/// before the enacting instrument, and the instrument is the core.
fn charter_hits(value: &str) -> Vec<Anchor> {
    CHARTER
        .captures_iter(value)
        .map(|captures| {
            let source = captures.name("source").unwrap();
            let title = captures.name("title").unwrap();
            Anchor {
                span: source.start()..source.end(),
                family: Some(("statute", "charter_grammar")),
                reading: crate::classify::read(source.as_str(), "charter_grammar", title.as_str()),
                style_start: Some(title.start()),
                inner: captures.name("pin").map(|pin| (pin.start(), pin.end())),
            }
        })
        .collect()
}

/// First references to the authorities that never carry a reporter: statutes
/// and regulations, journal articles, monographs and edited collections,
/// parliamentary papers, treaties, database identifiers and online-only sources.
fn secondary_hits(value: &str) -> Vec<Anchor> {
    let mut found = Vec::new();
    for anchor in charter_hits(value) {
        found.push(anchor);
    }
    for (kind, reason, pattern) in SECONDARY_ECMASCRIPT.iter() {
        found.extend(
            pattern
                .find_iter(value)
                .map(|matched| Anchor::new(value, matched.start()..matched.end(), Some((*kind, *reason)))),
        );
    }
    found.extend(JOURNAL_ARTICLE.find_iter(value).flatten().map(|matched| {
        Anchor::new(value, matched.start()..matched.end(), Some(("journal", "article_grammar")))
    }));
    found.extend(ONLINE_SOURCE.find_iter(value).map(|matched| {
        Anchor::new(value, matched.start()..matched.start() + online_source_end(matched.as_str()), Some(("other", "online_grammar")))
    }));
    found.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then_with(|| right.span.end.cmp(&left.span.end))
    });
    let mut resolved: Vec<Anchor> = Vec::new();
    for anchor in found {
        let claimed_from = |previous: &Anchor| previous.style_start.unwrap_or(previous.span.start);
        if resolved.last().is_some_and(|previous| {
            anchor.span.start < previous.span.end
                || (previous.style_start.is_some()
                    && anchor.span.start >= claimed_from(previous)
                    && anchor.span.start < previous.span.end)
        }) {
            continue;
        }
        // A Charter title claims the text in front of its source.
        if anchor.style_start.is_some() {
            let from = anchor.style_start.unwrap();
            resolved.retain(|previous| previous.span.end <= from);
        }
        resolved.push(anchor);
    }
    // A quoted/date source is already a first reference in ALR's splitter.
    // Keep the more precise citation grammars above: a source frame does not
    // create a second authority around an existing core or its styled name.
    for (reason, pattern) in [("quoted_source_grammar", &*QUOTED_SOURCE),
        ("secondary_source_grammar", &*SECONDARY_SOURCE)] {
        for matched in pattern.find_iter(value).flatten() {
            let span = matched.start()..matched.end();
            if resolved.iter().any(|anchor| {
                let start = if anchor.family == Some(("other", "online_grammar")) {
                    work_style_start(value, anchor.span.start, 0)
                } else { anchor.style_start.unwrap_or(anchor.span.start) };
                start < span.end && span.start < anchor.span.end
            }) { continue; }
            resolved.push(Anchor { span, family: Some(("other", reason)), ..Anchor::default() });
        }
    }
    resolved.sort_by_key(|anchor| anchor.span.start);
    resolved
}

/// A title cited with its own year ("Constitution Act, 1867") directly in
/// front of a statute source ("(UK), 30 & 31 Vict, c 3") is that statute's
/// style, not a second authority.
fn title_before_source(value: &str, anchor: &Anchor, next: &Anchor) -> bool {
    if anchor.family != Some(("statute", "titled_statute_grammar"))
        || next.family.is_none_or(|(kind, _)| kind != "statute")
        || value[anchor.span.clone()].contains('[')
    {
        return false;
    }
    let gap = value[anchor.span.end..next.span.start].trim_matches(javascript_whitespace);
    let gap = match gap.strip_prefix('(') {
        Some(rest) => match rest.find(')') {
            Some(close) if close <= 20 && !rest[..close].contains(['(', '\n']) => {
                rest[close + 1..].trim_start_matches(javascript_whitespace)
            }
            _ => return false,
        },
        None => gap,
    };
    gap == ","
}

fn citation_anchors(value: &str, extended_us: bool, scopes: &[usize]) -> Vec<Anchor> {
    let primary: Vec<_> = primary_anchors(value, extended_us).into_iter()
        .map(|anchor| if anchor.reading.is_some() { anchor }
            else { Anchor::new(value, anchor.span, anchor.family) }).collect();
    // A case claims its style of cause, and a style of cause can read as a
    // statute title ("Re Residential Tenancies Act, 1979, [1981] 1 SCR 714")
    // or as a work title; nothing inside that prefix is a second authority.
    let mut claimed = Vec::with_capacity(primary.len());
    let mut floor = 0;
    for hit in &primary {
        let start = if hit.family.is_some_and(|(kind, _)| kind == "case") {
            case_style_start(value, hit.span.start, floor.max(scope_floor(scopes, hit.span.start)))
        } else {
            hit.span.start
        };
        claimed.push(start..hit.span.end);
        floor = hit.span.end;
    }
    // A closed grammar that contains a generic primary hit reads it better
    // ("Can TS 1976 No 47" over "1976 No 47").
    let (containing, secondary): (Vec<_>, Vec<_>) = secondary_hits(value)
        .into_iter()
        .partition(|anchor| {
            matches!(
                anchor.family,
                Some((_, "treaty_grammar" | "database_grammar" | "neutral_grammar"))
            ) && primary.iter().any(|hit| {
                anchor.span.start <= hit.span.start
                    && hit.span.end <= anchor.span.end
                    && anchor.span.len() > hit.span.len()
            })
        });
    let primary = primary
        .into_iter()
        .filter(|hit| {
            !containing
                .iter()
                .any(|anchor| anchor.span.start <= hit.span.start && hit.span.end <= anchor.span.end)
        })
        .collect::<Vec<_>>();
    let mut anchors = secondary
        .into_iter()
        .filter(|anchor| {
            let start = anchor.style_start.unwrap_or(anchor.span.start);
            !claimed
                .iter()
                .any(|hit| start < hit.end && hit.start < anchor.span.end)
        })
        .collect::<Vec<_>>();
    anchors.extend(containing);
    anchors.extend(primary);
    anchors.sort_by_key(|anchor| anchor.span.start);
    let mut kept: Vec<Anchor> = Vec::with_capacity(anchors.len());
    let mut anchors = anchors.into_iter().peekable();
    while let Some(anchor) = anchors.next() {
        if anchors
            .peek()
            .is_some_and(|next| title_before_source(value, &anchor, next))
            || kept
                .last()
                .is_some_and(|previous| trailing_parenthetical(value, previous, &anchor))
        {
            continue;
        }
        kept.push(anchor);
    }
    kept
}

/// A secondary hit that opens with the parenthetical right after another
/// core (`410 U.S. 113, 153 (1973). Roe at 240`) belongs to that citation.
fn trailing_parenthetical(value: &str, previous: &Anchor, anchor: &Anchor) -> bool {
    anchor.family.is_some()
        && value[anchor.span.clone()].starts_with('(')
        && previous.span.end <= anchor.span.start
        && value[previous.span.end..anchor.span.start]
            .chars()
            .all(|character| javascript_whitespace(character) || character == ',' || character.is_ascii_digit() || character == '-')
}

pub(crate) fn citation_hits(value: &str, extended_us: bool) -> Vec<Hit> {
    primary_anchors(value, extended_us).into_iter().map(|anchor| anchor.span).collect()
}

fn primary_anchors(value: &str, extended_us: bool) -> Vec<Anchor> {
    let mut found = CITATION_PATTERN
        .find_iter(value)
        .filter(|matched| {
            !matches!(
                matched.as_str().split_whitespace().nth(1),
                Some(
                    "January"
                        | "February"
                        | "March"
                        | "April"
                        | "May"
                        | "June"
                        | "July"
                        | "August"
                        | "September"
                        | "October"
                        | "November"
                        | "December"
                )
            )
        })
        .map(|matched| matched.start()..matched.end())
        .collect::<Vec<_>>();
    // Multiword Canadian reporters may be absent from the legacy in-text
    // alternation. A verified registry match permits a new anchor; the
    // permissive splitter alone must never turn "23 and 25" into an authority.
    for matched in REPORTER_PATTERN.find_iter(value).flatten() {
        if let Some(hit) = found.iter_mut().find(|hit| hit.start == matched.start()) {
            hit.end = hit.end.max(matched.end());
        } else if REPORTER_PARTS.captures(matched.as_str()).ok().flatten()
            .and_then(|captures| captures.name("reporter").map(|surface| {
                crate::registry::registry().reporters_by_surface(surface.as_str())
                    .iter().any(|(reporter, _)| reporter.verified)
            })).unwrap_or(false)
        {
            found.push(matched.start()..matched.end());
        }
    }
    let captured = crate::us::find(value, extended_us);
    // Pinned source tokens own their core. Standard Commonwealth forms keep
    // a leading year; their captured reporter fields do not shorten that core.
    found.retain(|span| {
        let next = captured.partition_point(|matched| matched.span.end <= span.start);
        captured.get(next).is_none_or(|matched| matched.span.start >= span.end
            || (span.start < matched.span.start && matched.span.end == span.end
                && matched.fields.source_groups.get("reporter").and_then(|value| value.as_deref()).is_some_and(|surface|
                    crate::registry::registry().reporters_by_surface(surface).iter()
                        .any(|(reporter, _)| reporter.verified && reporter.source != "reporters-db"))))
    });
    let mut found: Vec<_> = found.into_iter()
        .map(|span| Anchor { span, ..Anchor::default() }).collect();
    found.extend(captured.into_iter().map(|matched| {
        let reading = crate::classify::extracted(&value[matched.span.clone()], matched.fields, matched.short_at);
        Anchor { span: matched.span, family: Some(reading.family()), reading: Some(reading), ..Anchor::default() }
    }));
    resolve(found)
}

/// Trim a candidate styled start: drop any leading signal ("See also", "Cf")
/// and reject a span that opens inside a parenthetical.
fn style_span_start(text: &str, mut start: usize, core_start: usize) -> Option<usize> {
    for _ in 0..4 {
        let window = &text[start..core_start];
        let Some(signal) = SIGNAL_PREFIX
            .find(window)
            .or_else(|| INTRODUCTORY_PREFIX.find(window))
        else {
            break;
        };
        start += signal.end();
    }
    // A style of cause must have balanced parentheses: the digit-tolerant
    // name grammar may otherwise start mid-parenthetical ("1998) v. Smith").
    let mut depth = 0i32;
    for character in text[start..core_start].chars() {
        if character == '(' {
            depth += 1;
        } else if character == ')' {
            depth -= 1;
            if depth < 0 {
                return None;
            }
        }
    }
    (depth == 0).then_some(start)
}

/// For every byte of `window`, whether it sits outside quotes, parentheses
/// and brackets (and outside a URL, whose `;` is not a delimiter).
pub(crate) fn top_level(window: &str) -> Vec<bool> {
    let masked = ONLINE_SOURCE
        .find_iter(window)
        .map(|matched| matched.start()..matched.start() + online_source_end(matched.as_str()))
        .collect::<Vec<_>>();
    let mut positions = vec![false; window.len() + 1];
    let (mut round, mut square, mut curly, mut smart, mut straight) = (0u32, 0u32, 0u32, false, false);
    let mut masked_index = 0;
    for (index, character) in window.char_indices() {
        while masked_index < masked.len() && index >= masked[masked_index].end { masked_index += 1; }
        if masked.get(masked_index).is_some_and(|range| range.contains(&index)) {
            continue;
        }
        positions[index] = !smart && !straight && round == 0 && square == 0 && curly == 0;
        let quoted = smart || straight;
        match character {
            '\u{201c}' => smart = true,
            '\u{201d}' => smart = false,
            '"' => straight = !straight,
            '(' if !quoted => round += 1,
            ')' if !quoted => round = round.saturating_sub(1),
            '[' if !quoted => square += 1,
            ']' if !quoted => square = square.saturating_sub(1),
            '{' if !quoted => curly += 1,
            '}' if !quoted => curly = curly.saturating_sub(1),
            _ => {}
        }
    }
    positions[window.len()] = !smart && !straight && round == 0 && square == 0 && curly == 0;
    positions
}

/// Raise `floor` past the last top-level semicolon or sentence end before the
/// anchor, so a styled span never reaches back over the delimiter that
/// separates it from the authority in front of it.
fn styled_floor(text: &str, floor: usize, core_start: usize) -> usize {
    let window = &text[floor..core_start];
    let top = top_level(window);
    STYLED_FLOOR
        .find_iter(window)
        .filter(|matched| top[matched.start()])
        .filter(|matched| {
            let period = window[..matched.end()].trim_end_matches(javascript_whitespace);
            !(period.ends_with('.')
                && CORPORATE_SUFFIX.is_match(period).unwrap_or(false))
        })
        .last()
        .map_or(floor, |matched| floor + matched.end())
}

fn scope_floor(scopes: &[usize], at: usize) -> usize {
    scopes[..scopes.partition_point(|&start| start <= at)]
        .last()
        .copied()
        .unwrap_or(0)
}

fn matched_style(
    pattern: &Regex,
    group: &str,
    text: &str,
    core_start: usize,
    floor: usize,
) -> usize {
    let floor = styled_floor(text, floor, core_start);
    pattern
        .captures(&text[floor..core_start])
        .and_then(|captures| captures.name(group))
        .and_then(|matched| style_span_start(text, floor + matched.start(), core_start))
        .unwrap_or(core_start)
}

/// The Act title a statute citation is styled with ("Criminal Code, RSC 1985").
fn statute_style_start(text: &str, core_start: usize, floor: usize) -> usize {
    matched_style(&STATUTE_TITLE, "title", text, core_start, floor)
}

/// A treaty's title, read past its signature date ("Convention ..., 4 November
/// 1950, 213 UNTS 221").
fn treaty_style_start(text: &str, core_start: usize, floor: usize) -> usize {
    let title_end = TRAILING_DATE
        .find(&text[floor..core_start])
        .map_or(core_start, |date| floor + date.start());
    let start = statute_style_start(text, title_end, floor);
    if start < title_end {
        start
    } else {
        core_start
    }
}

/// The author and work title a secondary source is styled with.
fn work_style_start(text: &str, core_start: usize, floor: usize) -> usize {
    let quoted = matched_style(&QUOTED_WORK, "work", text, core_start, floor);
    if quoted < core_start {
        return quoted;
    }
    matched_style(&PLAIN_WORK, "work", text, core_start, floor)
}

fn case_style_start(text: &str, core_start: usize, floor: usize) -> usize {
    let prefix = text[floor..core_start]
        .trim_end_matches(|character: char| javascript_whitespace(character) || character == ',');
    let Some(versus) = CASE_VERSUS.find_iter(prefix).flatten().last() else {
        // A style of cause with one party ("Reference re Secession of Quebec")
        // has no versus token to anchor on.
        return matched_style(&CASE_RE_STYLE, "name", text, core_start, floor);
    };
    if !prefix[versus.end()..]
        .trim_start_matches(javascript_whitespace)
        .chars()
        .next()
        .is_some_and(|character| character.is_uppercase() || character.is_numeric())
    {
        return core_start;
    }
    let Some(left) = CASE_LEFT
        .captures(&prefix[..versus.start()])
        .and_then(|captures| captures.name("left"))
    else {
        return core_start;
    };
    let Some(start) = style_span_start(text, floor + left.start(), core_start) else {
        return core_start;
    };
    let lead = CASE_LEAD_IN.find(&text[start..core_start]).map_or(0, |matched| matched.end());
    // Only a label or "In" that still leaves a party name ahead of the versus token.
    if lead > 0 && text[start + lead..].starts_with(|character: char| character.is_uppercase()) {
        start + lead
    } else {
        start
    }
}

/// The case name or author written in front of a `supra`, `(n 4)` or U.S.
/// short form ("Jordan, supra note 4", "Roe, 410 U.S. at 153").
fn antecedent_name(text: &str, core_start: usize, floor: usize) -> Option<Hit> {
    let floor = styled_floor(text, floor, core_start);
    let captures = ANTECEDENT_NAME.captures(&text[floor..core_start])?;
    let name = captures.name("name")?;
    let mut start = style_span_start(text, floor + name.start(), core_start)?;
    let lead = CASE_LEAD_IN.find(&text[start..core_start]).map_or(0, |matched| matched.end());
    if lead > 0 && text[start + lead..].starts_with(|character: char| character.is_uppercase()) {
        start += lead;
    }
    let end = floor + name.end();
    // Removing a signal can consume the entire captured name.
    if start >= end { return None; }
    let end = trim_style_end(text, start, end);
    (start < end).then_some(start..end)
}

/// A style span ends before the comma and spaces that join it to the core.
fn trim_style_end(text: &str, start: usize, end: usize) -> usize {
    start
        + text[start..end]
            .trim_end_matches(|character: char| javascript_whitespace(character) || character == ',')
            .len()
}

/// Eyecite's case-name scan skips placeholder citation tokens before a report.
/// Keep their original text in the full extent, outside the party-name span.
fn case_style_end(text: &str, start: usize, end: usize) -> usize {
    let mut end = trim_style_end(text, start, end);
    while let Some(placeholder) = PLACEHOLDER.find_iter(&text[start..end]).flatten().last() {
        if start + placeholder.end() != end { break; }
        end = trim_style_end(text, start, start + placeholder.start());
    }
    end
}

pub(crate) fn span(text: &str, range: Hit) -> Span {
    Span {
        text: text[range.clone()].to_owned(),
        start: range.start,
        end: range.end,
    }
}

/// The authority family the discovery grammar named. Later stages refine it
/// from the grammar's named groups.
fn authority(kind: &str) -> Authority {
    match kind {
        "case" => Authority::Case,
        "statute" => Authority::Statute,
        "journal" => Authority::Journal,
        "book" => Authority::Book,
        "parliamentary" => Authority::ParliamentaryPaper,
        "treaty" => Authority::Treaty,
        _ => Authority::Unknown,
    }
}

/// The introductory signal immediately in front of `start`, normalized.
fn signal(text: &str, floor: usize, start: usize) -> Option<Span> {
    let captures = INTRODUCTORY_SIGNAL.captures(text.get(floor..start)?)?;
    let matched = captures.name("signal")?;
    let normalized = matched
        .as_str()
        .to_lowercase()
        .chars()
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("e g", "eg");
    Some(Span {
        start: floor + matched.start(),
        end: floor + matched.end(),
        text: normalized,
    })
}

enum CoreKind {
    Full(Anchor),
    Back {
        form: Form,
        note: Option<u32>,
        oscola: bool,
        french: bool,
    },
    Unknown(String),
}

struct Core {
    span: Hit,
    kind: CoreKind,
}

fn blank_citation(text: &str, form: Form, authority: Authority, core: Hit, reason: &str) -> Citation {
    Citation {
        index: 0,
        form,
        authority,
        format: None,
        span: span(text, core.clone()),
        signal: None,
        full_span: span(text, core),
        style: None,
        parties: None,
        fields: Fields::default(),
        court: None,
        jurisdiction: None,
        language: None,
        pinpoints: Vec::new(),
        parentheticals: Vec::new(),
        history: Vec::new(),
        short_name: None,
        explicit_short_name: None,
        parallel_group: None,
        antecedent: None,
        key: None,
        alias: None,
        interpretations: Vec::new(),
        reasons: vec![reason.to_owned()],
    }
}

fn overlaps(range: &Hit, others: &[Hit]) -> bool {
    others
        .iter()
        .any(|other| range.start < other.end && other.start < range.end)
}

/// `Ibid`, `Id.`, `supra`, `above n`, `(n 4)`, `précité` and `op. cit.`
/// tokens outside every full citation core.
fn back_reference_cores(text: &str, taken: &[Hit]) -> Vec<Core> {
    let mut cores = Vec::new();
    for captures in BACK_REFERENCE.captures_iter(text).flatten() {
        let whole = captures.get(0).unwrap();
        let range = whole.start()..whole.end();
        if overlaps(&range, taken) {
            continue;
        }
        let note = ["supra_note", "above_note", "oscola_note", "precite_note", "opcit_note"]
            .iter()
            .find_map(|name| captures.name(name))
            .and_then(|value| value.as_str().parse().ok());
        cores.push(Core {
            span: range,
            kind: CoreKind::Back {
                form: if captures.name("ibid").is_some() {
                    Form::Ibid
                } else {
                    Form::Supra
                },
                note,
                oscola: captures.name("oscola").is_some(),
                french: captures.name("precite").is_some(),
            },
        });
    }
    cores
}

/// eyecite's unknown citation: a bare `§ 1983` that is neither inside a
/// citation nor the pinpoint of the reference right in front of it.
fn unknown_cores(text: &str, cores: &[Core]) -> Vec<Core> {
    let taken = cores.iter().map(|core| core.span.clone()).collect::<Vec<_>>();
    SECTION_SYMBOL
        .captures_iter(text)
        .filter_map(|captures| {
            let whole = captures.get(0).unwrap();
            let range = whole.start()..whole.end();
            if overlaps(&range, &taken) {
                return None;
            }
            let previous_end = taken
                .iter()
                .filter(|core| core.end <= range.start)
                .map(|core| core.end)
                .max();
            if previous_end.is_some_and(|end| {
                let gap = text[end..range.start].trim_start_matches(|character: char| {
                    javascript_whitespace(character) || character == ','
                });
                gap.is_empty() || gap.trim_end_matches(javascript_whitespace) == "at"
            }) {
                return None;
            }
            Some(Core {
                span: range,
                kind: CoreKind::Unknown(captures["section"].to_owned()),
            })
        })
        .collect()
}

fn full_citation(
    text: &str,
    anchor: &Anchor,
    previous_end: usize,
    limit: usize,
    paragraph_end: usize,
    source_name: Option<&crate::SourceCaseName>,
    options: &Options,
) -> Citation {
    let mut core = anchor.span.clone();
    let (kind, kind_reason) = anchor.family.unwrap_or(("other", "citation_grammar"));
    let has_section = anchor.reading.as_ref().is_some_and(|reading| reading.has_section());
    // Parentheses inside a section identifier (1.401(a)-1) are not a
    // subdivision. Extend a truncated law anchor before reading its pinpoints.
    if has_section {
        if let Some(suffix) = LAW_SUBDIVISION.captures(&text[core.end..limit])
            .and_then(|captures| captures.name("section_suffix"))
        {
            core.end += suffix.end();
        }
    }
    // Read actual subdivisions here so `(a)` is never a parenthetical.
    let subdivision = has_section
        .then(|| LAW_SUBDIVISION.find(&text[core.end..limit]))
        .flatten()
        .map(|matched| core.end..core.end + matched.end());
    let core_text = &text[core.clone()];
    let styled_start = anchor.style_start.unwrap_or_else(|| match kind {
        "case" => case_style_start(text, core.start, previous_end),
        "statute" => statute_style_start(text, core.start, previous_end),
        "treaty" => treaty_style_start(text, core.start, previous_end),
        "journal" | "book" | "parliamentary" => work_style_start(text, core.start, previous_end),
        // An online-only source is styled with the publisher and title in
        // front of the link; every other unclassified span carries no
        // styled prefix.
        _ if matches!(kind_reason, "online_grammar" | "quoted_source_grammar") =>
            work_style_start(text, core.start, previous_end),
        _ => core.start,
    });
    let short_pin = anchor.reading.as_ref().and_then(|reading| reading.short_at).map(|at| core.start + at);
    let short_form = short_pin.is_some();
    // A Bluebook pinpoint follows a comma with no keyword ("410 U.S. 113, 153").
    let bare_page = matches!(kind, "case" | "journal")
        && core_text.contains('.')
        && anchor.reading.as_ref().is_some_and(|reading| reading.reported());
    let mut tail = metadata::tail(
        text,
        short_pin.unwrap_or_else(|| subdivision.as_ref().map_or(core.end, |range| range.end)),
        limit,
        TailRules {
            post_citation: anchor.reading.as_ref().filter(|reading| reading.source_captured()).map(|_| {
                let source = if short_form { metadata::PostCitation::Short } else {
                    match kind {
                        "case" => metadata::PostCitation::Case,
                        "journal" => metadata::PostCitation::Journal,
                        _ => metadata::PostCitation::Law,
                    }
                };
                (source, paragraph_end)
            }),
            bare_page,
            oscola: false,
            inner: anchor.inner.or_else(|| source_name
                .filter(|name| name.pre_citation.is_some()).and_then(|name| name.pin_cite.as_ref())
                .map(|pin| (pin.start, pin.end))),
        },
    );
    if let Some(range) = subdivision {
        tail.pinpoints.insert(
            0,
            Pinpoint {
                kind: PinpointKind::Subsection,
                first: text[range.clone()].to_owned(),
                span: span(text, range),
                last: None,
            },
        );
    }
    let style_end = if kind == "case" { case_style_end(text, styled_start, core.start) }
        else { trim_style_end(text, styled_start, core.start) };
    let observed_name = text[styled_start..style_end].trim_matches(|character: char| {
        javascript_whitespace(character) || ",;:.".contains(character)
    });
    let observed_name = anchor
        .inner
        .map_or(observed_name, |(pin_start, _)| text[styled_start..pin_start].trim());
    let observed_name = if kind == "treaty" {
        TRAILING_DATE
            .find(observed_name)
            .map_or(observed_name, |date| observed_name[..date.start()].trim())
    } else {
        observed_name
    };
    let short_name = if observed_name.is_empty() {
        tail.short.clone()
    } else {
        Some(observed_name.to_owned())
    };
    let authority = if kind_reason == "online_grammar" {
        Authority::Webpage
    } else {
        authority(kind)
    };
    let mut citation = blank_citation(
        text,
        if short_form { Form::Short } else { Form::Full },
        authority,
        core.clone(),
        kind_reason,
    );
    if styled_start < core.start {
        citation.reasons.push("same_text_style".to_owned());
        let mut style_end = anchor.inner.map_or(style_end, |(pin_start, _)| pin_start);
        // A treaty's signature date sits between its title and its series.
        if kind == "treaty" {
            if let Some(date) = TRAILING_DATE.find(&text[styled_start..core.start]) {
                style_end = trim_style_end(text, styled_start, styled_start + date.start());
            }
        }
        citation.style = Some(span(text, styled_start..style_end));
    }
    if !tail.pinpoints.is_empty() {
        citation.reasons.push("pinpoint_grammar".to_owned());
    }
    if tail.short.is_some() {
        citation.reasons.push("short_form_suffix".to_owned());
    }
    if authority == Authority::Unknown && kind_reason == "citation_grammar" {
        citation.reasons.push("kind_unclassified".to_owned());
    }
    if kind_reason == "online_grammar" {
        citation.reasons.push("webpage".to_owned());
    }
    if short_form {
        citation.reasons.push("short_form".to_owned());
        if let Some(last) = tail.pinpoints.last() {
            citation.span = span(text, core.start..last.span.end);
        }
        if let Some(name) = antecedent_name(text, core.start, previous_end) {
            citation.short_name = Some(text[name.clone()].to_owned());
            citation.style = Some(span(text, name.clone()));
            citation.full_span = span(text, name.start..tail.end);
        } else {
            citation.full_span = span(text, core.start..tail.end);
        }
    } else {
        citation.short_name = short_name;
        citation.full_span = span(text, styled_start..tail.end);
    }
    citation.explicit_short_name = tail.short;
    citation.pinpoints = tail.pinpoints;
    citation.parentheticals = tail.parentheticals;
    if let Some(date) = tail.court_date {
        citation.fields.court_text = date.court;
        citation.fields.year = date.year;
        citation.fields.month = date.month;
        citation.fields.day = date.day;
    }
    if let Some(mut reading) = anchor.reading.clone() {
        if kind == "case" {
            reading.fields.source_case_name = source_name.cloned();
            if let Some(year) = source_name.and_then(|name| name.year.clone()) {
                reading.fields.year = Some(year);
            }
        } else if reading.source_captured() {
            reading.fields.source_case_name = Some(crate::SourceCaseName {
                full_span_start: core.start, ..Default::default()
            });
        }
        crate::classify::apply(&mut citation, reading, options);
        if core.end > anchor.span.end {
            if let Some(section) = &mut citation.fields.section {
                section.push_str(&text[anchor.span.end..core.end]);
            }
        }
    }
    citation.fields.pin_cite = tail.pin_cite;
    citation.fields.extra = tail.extra;
    citation.fields.publisher = citation.fields.publisher.or(tail.publisher);
    if let Some(name) = citation.fields.source_case_name.as_mut().filter(|_| !short_form) {
        name.full_span_end = tail.source_end;
        name.parenthetical = tail.source_parenthetical;
        name.pin_cite_span_end = tail.source_pin_end;
        if name.pre_citation.is_none() { name.pin_cite = tail.source_pin; }
    }
    if let Some(name) = &citation.fields.source_case_name {
        if name.pre_citation.is_some() {
            citation.full_span = span(text, citation.full_span.start.min(name.full_span_start)..citation.full_span.end);
            citation.fields.pin_cite = name.pin_cite.clone();
        }
    }
    if citation.style.is_none() && !short_form {
        if let Some(name) = citation.fields.source_case_name.as_ref().filter(|name| name.defendant.is_some()) {
            let end = trim_style_end(text, name.full_span_start, core.start);
            if name.full_span_start < end {
                citation.style = Some(span(text, name.full_span_start..end));
                citation.short_name = Some(text[name.full_span_start..end].to_owned());
                citation.full_span = span(text, name.full_span_start..citation.full_span.end);
                citation.reasons.push("same_text_style".to_owned());
            }
        }
    }
    citation.parties = if let Some(name) = citation.fields.source_case_name.as_mut().filter(|_| kind == "case") {
        Some(crate::Parties { plaintiff: name.plaintiff.take(), defendant: name.defendant.take() })
    } else if citation.authority == Authority::Case {
        citation.style.as_ref().and_then(|style| metadata::parties(&style.text))
    } else { None };
    citation
}

fn back_citation(
    text: &str,
    core: &Hit,
    (form, note, oscola, french): (Form, Option<u32>, bool, bool),
    source_name: Option<&crate::SourceCaseName>,
    previous_end: usize,
    limit: usize,
) -> Citation {
    let tail = metadata::tail(
        text,
        core.end,
        limit,
        TailRules {
            oscola,
            ..TailRules::default()
        },
    );
    let mut citation = blank_citation(text, form, Authority::Unknown, core.clone(), "reference_grammar");
    citation.fields.note = note;
    citation.fields.source_case_name = source_name.cloned();
    citation.fields.volume = citation.fields.source_case_name.as_mut().and_then(|name| name.volume.take());
    if french {
        citation.language = Some("fr".to_owned());
    }
    let name = (form == Form::Supra)
        .then(|| {
            if citation.fields.volume.is_some() {
                source_name.and_then(|source| source.antecedent_guess.as_ref()
                    .map(|name| source.full_span_start..source.full_span_start + name.len()))
            } else {
                antecedent_name(text, core.start, previous_end)
            }
        })
        .flatten();
    let start = name.as_ref().map_or(core.start, |name| name.start);
    if let Some(name) = name {
        citation.reasons.push("same_text_style".to_owned());
        citation.short_name = Some(text[name.clone()].to_owned());
        citation.style = Some(span(text, name));
    }
    if !tail.pinpoints.is_empty() {
        citation.reasons.push("pinpoint_grammar".to_owned());
    }
    let source_pin = source_name.and_then(|name| name.pin_cite.as_ref());
    citation.full_span = span(text, start..source_pin.map_or(tail.end, |pin| tail.end.max(pin.end)));
    citation.pinpoints = tail.pinpoints;
    citation.parentheticals = tail.parentheticals;
    citation.fields.pin_cite = source_pin.filter(|pin|
        tail.pin_cite.as_ref().is_none_or(|native| pin.end > native.end)).cloned().or(tail.pin_cite);
    citation
}

/// Every core in document order: full anchors, back references, then bare
/// section symbols, never overlapping.
fn cores(text: &str, options: &Options, scopes: &[usize]) -> Vec<Core> {
    let mut cores = citation_anchors(text, options.extended_us, scopes)
        .into_iter()
        .map(|anchor| Core {
            span: anchor.span.clone(),
            kind: CoreKind::Full(anchor),
        })
        .collect::<Vec<_>>();
    let taken = cores
        .iter()
        .map(|core| match &core.kind {
            CoreKind::Full(anchor) => anchor.style_start.unwrap_or(core.span.start)..core.span.end,
            _ => core.span.clone(),
        })
        .collect::<Vec<_>>();
    cores.extend(back_reference_cores(text, &taken));
    cores.sort_by_key(|core| core.span.start);
    let unknown = unknown_cores(text, &cores);
    cores.extend(unknown);
    cores.sort_by_key(|core| core.span.start);
    cores
}

/// Case names later text can refer back to: an explicit short form, the
/// style, and each party that is not the Crown or a state.
fn reference_names(citation: &Citation) -> Vec<String> {
    let mut names = crate::resolve::candidate_names(citation);
    names.retain(|name| {
        let lower = name.trim().to_lowercase();
        let source_name = citation.fields.source_case_name.is_some() && citation.parties.as_ref().is_some_and(|parties|
            parties.plaintiff.as_deref() == Some(name.as_str()) || parties.defendant.as_deref() == Some(name.as_str()));
        name.chars().count() >= 3
            && name.chars().next().is_some_and(char::is_uppercase)
            && if source_name {
                !name.ends_with('.') && !SOURCE_NAME_EXCLUDED.is_match(&name.to_lowercase()).expect("source name exclusion")
            } else { !GENERIC_PARTIES.contains(&lower.as_str()) }
    });
    names
}

fn word_boundary(text: &str, start: usize, end: usize) -> bool {
    !text[..start].chars().next_back().is_some_and(char::is_alphanumeric)
        && !text[end..].chars().next().is_some_and(char::is_alphanumeric)
}

static MARKUP_REFERENCE_FOLLOWING: LazyLock<CompiledGrammar> = LazyLock::new(|| {
    legal_grammar::compile_python_table_entry("ref.markup.following")
        .expect("pinned markup reference exclusion")
});
static MARKUP_REFERENCE_NAME: LazyLock<String> = LazyLock::new(|| {
    legal_grammar::load_tables().expect("grammar corpus")["ref.markup.source-name"].entry.pattern.clone()
});
static SOURCE_REFERENCE: LazyLock<String> = LazyLock::new(|| {
    legal_grammar::load_tables().expect("grammar corpus")["ref.us.name-pincite"].entry.pattern.clone()
});
static SOURCE_NAME_EXCLUDED: LazyLock<CompiledGrammar> = LazyLock::new(|| {
    legal_grammar::compile_python_table_entry("ref.us.excluded-name").expect("pinned reference names")
});

/// Bare case-name references (`Jordan at para 12`, `Roe at 240`) to a full
/// citation earlier in the text, and a name conjoined to the citation in
/// front of it (`...; see Oakes, supra note 4 and Jordan.`).
fn case_name_references(text: &str, citations: &[Citation], source_markup: Option<&crate::clean::Markup<'_>>) -> Vec<Citation> {
    let mut names = Vec::new();
    for citation in citations.iter().filter(|citation| citation.form == Form::Full) {
        for name in reference_names(citation) {
            let source_field = citation.fields.source_case_name.as_ref().and(citation.parties.as_ref()).and_then(|parties| {
                if parties.plaintiff.as_deref() == Some(&name) { Some(true) }
                else if parties.defendant.as_deref() == Some(&name) { Some(false) }
                else { None }
            });
            names.push((name, citation.span.end, citation.authority, source_field));
        }
    }
    names.sort_by_key(|(name, _, _, _)| std::cmp::Reverse(name.len()));
    names.dedup_by(|left, right| left.0 == right.0);
    let mut taken = citations
        .iter()
        .map(|citation| {
            citation.signal.as_ref().map_or(citation.full_span.start, |signal| signal.start)
                ..citation.full_span.end
        })
        .collect::<Vec<_>>();
    let mut found: Vec<Citation> = Vec::new();
    for (name, after, authority, source_field) in names {
        let source_pattern = source_field.map(|_| legal_grammar::compile_python_pattern(
            &SOURCE_REFERENCE.replace("{{name}}", &regex::escape(&name)), "").expect("escaped reference name"));
        let source_pins = source_pattern.as_ref().map(|pattern| pattern.captures_iter(&text[after..])
            .map(|captures| {
                let captures = captures.expect("source reference match");
                let pin = captures.name("pin_cite").expect("source reference pinpoint");
                (after + captures.get(0).unwrap().start(), span(text, after + pin.start()..after + pin.end()))
            }).collect::<Vec<_>>()).unwrap_or_default();
        // Eyecite permits variable whitespace within an emphasized name and
        // only punctuation/whitespace between that name and the closing tag.
        let name_pattern = source_markup.map(|_| legal_grammar::compile_python_pattern(&MARKUP_REFERENCE_NAME.replace("{{name}}",
            &name.split(crate::text::python_whitespace).filter(|value| !value.is_empty())
                .map(regex::escape).collect::<Vec<_>>().join(r"\s+")), "")
            .expect("escaped markup name"));
        let styled = source_markup.map(|markup| {
            let offset = markup.source_offset(after);
            name_pattern.as_ref().unwrap().captures_iter(&markup.source[offset..]).filter_map(|captures| {
                let captures = captures.expect("markup name match");
                let matched = captures.get(0)?;
                let name = captures.name("name")?;
                let name = markup.text_range(offset + name.start()..offset + name.end());
                let full = markup.text_range(offset + matched.start()..offset + matched.end());
                (name.start >= after
                    && !MARKUP_REFERENCE_FOLLOWING.is_match(&text[full.end..]).expect("markup reference exclusion"))
                    .then_some((name, full))
            }).collect::<Vec<_>>()
        }).unwrap_or_default();
        let mut matches: Vec<_> = text[after..].match_indices(name.as_str())
            .map(|(at, _)| after + at..after + at + name.len())
            .chain(styled.iter().map(|(range, _)| range.clone())).collect();
        matches.sort_by_key(|range| (range.start, range.end));
        matches.dedup();
        for matched in matches {
            let start = matched.start;
            let end = matched.end;
            let markup = styled.iter().find(|(range, _)| *range == matched).map(|(_, range)| range);
            let existing = source_field.and_then(|_| found.iter().position(|citation|
                citation.span.start <= start && end <= citation.span.end
                    && citation.fields.source_case_name.is_none()));
            if existing.is_none() && overlaps(&(start..end), &taken) {
                continue;
            }
            let previous_end = taken
                .iter()
                .filter(|range| range.end <= start)
                .map(|range| range.end)
                .max()
                .unwrap_or(0);
            let limit = taken
                .iter()
                .filter(|range| range.start >= end)
                .map(|range| range.start)
                .min()
                .unwrap_or(text.len());
            let tail = metadata::tail(text, end, limit, TailRules::default());
            let source_pin = source_pins.binary_search_by_key(&start, |(at, _)| *at).ok()
                .map(|index| &source_pins[index].1).filter(|pin| pin.end <= limit).cloned();
            if markup.is_none() && source_pin.is_none() && !word_boundary(text, start, end) { continue; }
            let conjoined = previous_end > 0 && {
                let gap = text[previous_end..start].trim_matches(javascript_whitespace);
                let gap = gap.strip_prefix(',').unwrap_or(gap).trim_start();
                matches!(gap, "and" | "&" | "et")
                    && text[end..limit]
                        .trim_start_matches(javascript_whitespace)
                        .chars()
                        .next()
                        .is_none_or(|character| ".;".contains(character))
            };
            if tail.pinpoints.is_empty() && source_pin.is_none() && !conjoined && markup.is_none() {
                continue;
            }
            let mut citation =
                blank_citation(text, Form::Reference, authority, start..end, "case_name_reference");
            citation.style = Some(span(text, start..end));
            citation.short_name = Some(name.clone());
            citation.full_span = span(text, markup.map_or(start, |range| range.start)
                ..markup.map_or(tail.end, |range| tail.end.max(range.end)));
            if !tail.pinpoints.is_empty() {
                citation.reasons.push("pinpoint_grammar".to_owned());
            }
            citation.pinpoints = tail.pinpoints;
            citation.parentheticals = tail.parentheticals;
            citation.fields.pin_cite = tail.pin_cite;
            if let Some(plaintiff) = source_field.filter(|_| source_pin.is_some() || markup.is_some()) {
                let reference_end = source_pin.as_ref().map_or(end, |pin| pin.end);
                let source_start = if source_pin.is_some() { start } else { markup.map_or(start, |range| range.start) };
                let source_end = if source_pin.is_some() { reference_end } else { markup.map_or(tail.end, |range| range.end) };
                citation.parties = Some(crate::Parties {
                    plaintiff: plaintiff.then(|| name.clone()), defendant: (!plaintiff).then(|| name.clone()),
                });
                citation.fields.source_case_name = Some(crate::SourceCaseName {
                    full_span_start: source_start, full_span_end: Some(source_end),
                    reference_span: Some(span(text, start..reference_end)), pin_cite: source_pin.clone(),
                    ..Default::default()
                });
                if let Some(pin) = source_pin {
                    citation.full_span.end = citation.full_span.end.max(pin.end);
                    citation.full_span.text = text[citation.full_span.start..citation.full_span.end].to_owned();
                    citation.fields.pin_cite = Some(pin);
                }
            }
            citation.signal = signal(text, previous_end, start);
            if let Some(index) = existing {
                if citation.fields.source_case_name.is_some() {
                    let original = &mut found[index];
                    original.parties = citation.parties;
                    original.fields.source_case_name = citation.fields.source_case_name;
                    original.fields.pin_cite = citation.fields.pin_cite.or(original.fields.pin_cite.take());
                    original.full_span = span(text, original.full_span.start..original.full_span.end.max(citation.full_span.end));
                }
                continue;
            }
            taken.push(citation.signal.as_ref().map_or(start, |signal| signal.start)..citation.full_span.end);
            found.push(citation);
        }
    }
    found
}

/// Every citation in document order, numbered by position.
pub fn find(text: &str, options: &Options) -> Vec<Citation> {
    find_styled(text, options, None)
}

pub(crate) fn find_styled(text: &str, options: &Options, markup: Option<&crate::clean::Markup<'_>>) -> Vec<Citation> {
    let mut scopes = options.notes.as_ref().map_or_else(
        || text.match_indices("\n\n").map(|(at, _)| at + 2).collect::<Vec<_>>(),
        |notes| notes.iter().map(|note| note.start).collect::<Vec<_>>(),
    );
    scopes.sort_unstable();
    let cores = cores(text, options, &scopes);
    if cores.is_empty() { return Vec::new(); }
    let source_names = crate::us::case_names(text, &cores.iter().filter_map(|core| match &core.kind {
        CoreKind::Full(anchor) if anchor.reading.as_ref().is_some_and(|reading| reading.source_captured()) =>
            Some((core.span.clone(), anchor.reading.as_ref().is_some_and(|reading| reading.short_at.is_some()))),
        _ => None,
    }).collect::<Vec<_>>(), markup);
    // Paragraph tokens inside a recognized citation are suppressed by the
    // source tokenizer's overlap rule; keep those newlines inside its token.
    let source_spans: Vec<_> = cores.iter().filter_map(|core| match &core.kind {
        CoreKind::Full(anchor) if anchor.reading.as_ref().is_some_and(|r| r.source_captured()) => Some(&core.span),
        _ => None,
    }).collect();
    let paragraphs: Vec<_> = text.match_indices('\n').map(|(at, _)| at).filter(|at| {
        let next = source_spans.partition_point(|span| span.end <= *at);
        source_spans.get(next).is_none_or(|span| *at < span.start)
    }).collect();
    let mut citations = Vec::with_capacity(cores.len());
    let mut previous_end = 0;
    for (index, core) in cores.iter().enumerate() {
        let limit = cores
            .get(index + 1)
            .map_or(text.len(), |next| match &next.kind {
                CoreKind::Full(anchor) => anchor.style_start.unwrap_or(next.span.start),
                _ => next.span.start,
            });
        let floor = previous_end.min(core.span.start).max(scope_floor(&scopes, core.span.start));
        let source_name = source_names.get(&core.span.start)
            .filter(|name| name.full_span_start >= floor);
        let mut citation = match &core.kind {
            CoreKind::Full(anchor) => full_citation(text, anchor, floor, limit,
                paragraphs.get(paragraphs.partition_point(|at| *at < core.span.end)).copied().unwrap_or(text.len()),
                source_name, options),
            CoreKind::Back {
                form,
                note,
                oscola,
                french,
            } => back_citation(text, &core.span, (*form, *note, *oscola, *french), source_name, floor, limit),
            CoreKind::Unknown(section) => {
                let mut citation =
                    blank_citation(text, Form::Unknown, Authority::Unknown, core.span.clone(), "section_symbol");
                citation.fields.section = Some(section.clone());
                citation
            }
        };
        citation.signal = signal(text, floor, citation.full_span.start);
        // Keep an enclosing citation's full parenthetical without making it
        // the style boundary for the citation contained inside it.
        previous_end = if citation.full_span.end > limit {
            core.span.end
        } else {
            citation.full_span.end
        };
        citations.push(citation);
    }
    let references = case_name_references(text, &citations, markup);
    citations.extend(references);
    citations.sort_by_key(|citation| citation.span.start);
    for (index, citation) in citations.iter_mut().enumerate() {
        citation.index = index;
    }
    citations
}

/// Note cross-references (`supra note 4`, `infra note 12`, `see footnote 7`,
/// `above n 4`, `(n 4)`, `note 4 ci-dessus`) with the direction they point.
pub fn note_references(text: &str) -> Vec<NoteReference> {
    NOTE_CROSS_REFERENCE
        .captures_iter(text)
        .flatten()
        .filter_map(|captures| {
            let whole = captures.get(0)?;
            let (note, direction) = if let Some(note) = captures.name("note") {
                let direction = if captures.name("back").is_some() {
                    NoteDirection::Back
                } else if captures.name("forward").is_some() {
                    NoteDirection::Forward
                } else {
                    NoteDirection::Unspecified
                };
                (note, direction)
            } else if let Some(note) = captures.name("oscola_note") {
                (note, NoteDirection::Back)
            } else {
                let direction = if captures.name("fr_back").is_some() {
                    NoteDirection::Back
                } else {
                    NoteDirection::Forward
                };
                (captures.name("fr_note")?, direction)
            };
            Some(NoteReference {
                span: span(text, whole.start()..whole.end()),
                note: note.as_str().parse().ok()?,
                direction,
            })
        })
        .collect()
}

/// Whether `text` holds a citation core (no case-name fallback).
pub(crate) fn has_core_citation(text: &str) -> bool {
    !citation_hits(text, true).is_empty()
}

/// Whether `text` holds a citation core or a two-party case name.
pub fn has_citation(text: &str) -> bool {
    has_core_citation(text) || CASE_NAME.is_match(text)
}
