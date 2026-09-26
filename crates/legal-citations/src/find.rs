//! Discover citation cores and their styled extent: full citations, U.S.
//! short forms, `Ibid`/`Id.`, `supra`/`above n`/`(n 4)`/`précité`, bare
//! case-name references, bare section symbols, and the introductory signal in
//! front of each. Classification and metadata happen in later stages.
//!
//! Full spans never overlap and never cross a top-level `;`, so a footnote is
//! rebuilt exactly from its citations' full spans and the gaps between them.

use crate::metadata::{self, TailRules};
use crate::model::{Authority, Citation, Fields, Form, NoteDirection, NoteReference, Span};
use crate::text::javascript_whitespace;
use crate::Options;
use legal_grammar::{AsciiBoundedGrammar, CompiledEcmascriptGrammar, CompiledGrammar};
use regex::Regex;
use std::collections::HashSet;
use std::ops::Range;
use std::sync::LazyLock;

const EXTENDED_US_CITATION_IDS: [&str; 4] = [
    "cite.us.reporter.custom.full",
    "cite.us.reporter.custom.short",
    "cite.us.law.full",
    "cite.us.law.short",
];

pub(crate) type Hit = Range<usize>;

fn linear(id: &str) -> CompiledEcmascriptGrammar {
    legal_grammar::compile_ecmascript_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

fn backtracking(id: &str) -> CompiledGrammar {
    legal_grammar::compile_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

static CITATION_PATTERN: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.in-text"));
// Unicode letter classes cannot be written portably (the corpus bans \p{}),
// so the case-name and style-of-cause shapes below stay Rust-only.
static CASE_NAME: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
    Regex::new(
        r"(?m)(?:^|[^\p{L}])(?:R\.|[A-Z][\p{L}\p{M}'’.&-]*(?:\s+(?:of|the|and|&|[A-Z][\p{L}\p{M}'’.&-]*)){0,6})\s+v(?:\.|ersus)?\s+[A-Z][\p{L}\p{M}'’.&-]*",
    )
});
static ROUTING_PATTERN: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.provider-routing"));
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
// Classify spans already found by the citation grammar. The splitter is
// intentionally permissive: do not use it to discover new prose spans.
static REPORTER_PATTERN: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("cite.reporter.splitter"));
pub(crate) static REPORTER_PARTS: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("cite.reporter.parts"));
pub(crate) static JOURNAL_CUE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.journal.title-cue"));
static JOURNAL_PATTERN: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.journal.toa"));
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
static SECONDARY_ECMASCRIPT: LazyLock<[(&'static str, &'static str, CompiledEcmascriptGrammar); 8]> =
    LazyLock::new(|| {
        [
            // Multi-word tribunal identifiers (2020 Comp Trib 6, 2000 CIRB LD
            // 213) are out of cite.in-text's reach.
            ("case", "neutral_grammar", "cite.neutral.tribunal"),
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
static CASE_VERSUS: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("party.versus"));
static CASE_LEFT: LazyLock<Regex> = LazyLock::new(|| {
    // A balanced, uppercase-first parenthetical ("Quebec (Attorney General)")
    // counts as one party token. Bounded (<=80 chars), non-nested, never
    // line-spanning, so "(1998)", "(2d)" and "(see below)" stay rejected.
    Regex::new(
        r"(?m)(?<left>[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&()-]*(?:\s+(?:[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&()-]*|\([\p{Lu}][^()\n]{0,80}\)|of|the|and|for|de|la|du)){0,12})\s*$",
    )
    .unwrap()
});
// What can open a numbered paragraph ahead of its first case: the paragraph's
// own label ("12.", "[12]", "(a)") and a leading "In". Neither is part of a
// party name, although the party grammar accepts numbers and capitals.
static CASE_LEAD_IN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(?:\d{1,4}\.|\[\d{1,4}\]|\((?:\d{1,4}|\p{Ll}{1,4})\))\s+)?(?:In\s+)?").unwrap()
});
// The hard delimiters between two authorities in one footnote: a semicolon,
// or a sentence period that is not an abbreviation or an initial. No styled
// span reaches back across one, so widening a span can never swallow the
// boundary the next authority is split on. Only top-level delimiters count
// (see `top_level`), and a corporate suffix's period is not a sentence end.
static STYLED_FLOOR: LazyLock<Regex> =
    LazyLock::new(|| {
        Regex::new(
            // A question or exclamation mark closes a sentence only when
            // nothing follows it: inside a closing quote it is part of an
            // article title ("What is Speciesism?").
            "(?s)(?:;|(?:[^\\s.][\\p{L}]{2,}|\\d)(?:[\u{201d}\u{2019}\"')\\]]*\\.[\u{201d}\u{2019}\"')\\]]*|[!?]))\\s",
        )
        .unwrap()
    });
static CORPORATE_SUFFIX: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("boundary.corporate-suffix"));
// "Reference re Secession of Quebec" / "Re Residential Tenancies Act" /
// "Renvoi relatif à la sécession du Québec" / "Moore (Re)": a style of cause
// with one party instead of two.
static CASE_RE_STYLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)(?<name>(?:Reference\s+re|In\s+re|In\s+the\s+[Mm]atter\s+of|Re|Ex\s+parte)\s+[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&()-]*(?:\s+(?:[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&()-]*|\([\p{Lu}][^()\n]{0,80}\)|of|the|and|for|de|la|du|des|aux|en)){0,12}|Renvoi\s+relatif\s+(?:à\s+la|à\s+l['\u{2019}]|à|au|aux)\s*[\p{L}\p{M}\p{N}'\u{2019} -]{1,120}?|[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&-]*(?:\s+(?:[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&-]*|of|the|and|de|la|du|des)){0,8}\s+\((?:Re|Renvoi)\))(?:,\s*(?:1[6-9]|20)\d{2})?\s*,?\s*$",
    )
    .unwrap()
});
// The title a statute or treaty citation is styled with, ending in the
// instrument word and optionally carrying its own regnal year and jurisdiction.
static STATUTE_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        // The instrument word ends the title ("Criminal Code") or opens it
        // ("Charter of the French language", "Loi sur la protection",
        // "International Covenant on Civil and Political Rights").
        r"(?m)(?<title>[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&()-]*(?:\s+(?:[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&()-]*|of|the|and|to|for|in|on|de|la|du|des|et)){0,12}\s+(?:Acts?|Codes?|Rules?|Regulations?|Charter|Convention|Treaty|Protocol|Declaration|Covenant|Agreement|Statute)|(?:[\p{Lu}][\p{L}\p{M}'\u{2019}-]*\s+){0,4}(?:Acts?|Codes?|Rules?|Regulations?|Charte?r?|Loi|R\u{e8}glement|Convention|Treaty|Protocol|Declaration|Covenant|Agreement|Pacte|Trait\u{e9}|Statute)(?:\s+\p{Ll}+)?\s+(?:of|on|respecting|concerning|sur|de|du|des|pour|relatif|relative|between|for|to)(?:\s+(?:[\p{L}\p{M}\p{N}.'\u{2019}&()-]+)){1,12})(?:,\s*(?:1[6-9]|20)\d{2})?(?:\s*\([\p{Lu}][^()\n]{0,20}\))?\s*,?\s*$",
    )
    .unwrap()
});
static TRAILING_DATE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("style.trailing-date"));
// "Author, \u{201c}Article Title\u{201d}" (and any "in Editor, ed," lead-in):
// the styled part of a secondary source sitting in front of its publication
// block.
static QUOTED_WORK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        // The lead-in is an author list, never arbitrary prose: a sentence
        // that happens to end in a quoted title is not a styled citation.
        "(?s)(?<work>[\\p{Lu}][\\p{L}\\p{M}\\p{N}.'\u{2019}&-]*(?:,?\\s+(?:[\\p{Lu}\\p{N}][\\p{L}\\p{M}\\p{N}.'\u{2019}&-]*|&|et|al|eds?|de|la|du|des|van|von|di|le|of|the|and|for|in|on)){0,15},?\\s*[\"\u{201c}][^\"\u{201c}\u{201d}\n]{1,300}[\"\u{201d}][^\"\u{201c}\u{201d}\n]{0,120})\\s*$",
    )
    .unwrap()
});
// The same styled part when the work carries no quoted title: a monograph, a
// debate record, a dictionary.
static PLAIN_WORK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)(?<work>[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.:'\u{2019}&()-]*(?:,?\s+(?:[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.:'\u{2019}&()-]*|&|of|the|and|to|for|in|on|de|la|du|des|et|al|eds?)){0,24})\s*,?\s*$",
    )
    .unwrap()
});
static RESIDUAL_CUE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.us.fallback-cue"));
static COMMON_US_LAW: LazyLock<AsciiBoundedGrammar> = LazyLock::new(|| {
    legal_grammar::compile_ascii_bounded_table_entry("cite.us.law.common")
        .expect("common US law grammar")
});
static STANDARD_CANDIDATE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.us.reporter.candidate"));
static EXTENDED_CANDIDATE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.us.reporter.candidate.extended"));
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
static STANDARD_SURFACES: LazyLock<HashSet<String>> =
    LazyLock::new(|| surfaces(&["us_reporters", "us_journals"]));
static US_JOURNAL_SURFACES: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let reporters = surfaces(&["us_reporters"]);
    surfaces(&["us_journals"])
        .into_iter()
        .filter(|surface| !reporters.contains(surface))
        .collect()
});
static EXTENDED_US_PATTERNS: LazyLock<[AsciiBoundedGrammar; 4]> = LazyLock::new(|| {
    EXTENDED_US_CITATION_IDS.map(|id| {
        legal_grammar::compile_ascii_bounded_table_entry(id).expect("extended US grammar")
    })
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

fn standard_us_matches(value: &str, pattern: &Regex) -> Vec<Hit> {
    let mut found = Vec::new();
    let mut cursor = 0;
    while let Some(captures) = pattern.captures_at(value, cursor) {
        let citation = captures.name("citation").expect("standard citation");
        let reporter = captures.name("reporter").expect("standard reporter");
        if STANDARD_SURFACES.contains(&compact_surface(reporter.as_str())) {
            found.push(citation.start()..citation.end());
        }
        cursor = citation.start() + 1;
    }
    found
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

fn us_fallback_ranges(value: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut seen = HashSet::new();
    for cue in RESIDUAL_CUE.find_iter(value) {
        let start = value[..cue.start()].rfind('\n').map_or(0, |at| at + 1);
        let end = value[cue.end()..]
            .find('\n')
            .map_or(value.len(), |at| cue.end() + at);
        if seen.insert((start, end)) {
            ranges.push((start, end));
        }
    }
    ranges
}

/// A discovered authority anchor. `family` is set when the grammar that found
/// it already names the family; otherwise the span is classified from its own
/// text by [`citation_kind`]. `style_start` and `inner` carry a styled part
/// and pinpoints the grammar itself located (the Charter's full form).
struct Anchor {
    span: Hit,
    family: Option<(&'static str, &'static str)>,
    style_start: Option<usize>,
    inner: Option<(usize, usize)>,
}

impl Anchor {
    fn new(span: Hit, family: Option<(&'static str, &'static str)>) -> Self {
        Self {
            span,
            family,
            style_start: None,
            inner: None,
        }
    }
}

fn resolve(mut found: Vec<Hit>) -> Vec<Hit> {
    found.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| right.end.cmp(&left.end))
    });
    let mut resolved: Vec<Hit> = Vec::new();
    for hit in found {
        if resolved
            .last()
            .is_some_and(|previous| hit.start < previous.end)
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
                .map(|matched| Anchor::new(matched.start()..matched.end(), Some((*kind, *reason)))),
        );
    }
    found.extend(JOURNAL_ARTICLE.find_iter(value).flatten().map(|matched| {
        Anchor::new(matched.start()..matched.end(), Some(("journal", "article_grammar")))
    }));
    found.extend(ONLINE_SOURCE.find_iter(value).map(|matched| {
        Anchor::new(matched.start()..matched.end(), Some(("other", "online_grammar")))
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

fn citation_anchors(value: &str, extended_us_fallback: bool) -> Vec<Anchor> {
    let primary = citation_hits(value, extended_us_fallback);
    // A case claims its style of cause, and a style of cause can read as a
    // statute title ("Re Residential Tenancies Act, 1979, [1981] 1 SCR 714")
    // or as a work title; nothing inside that prefix is a second authority.
    let mut claimed = Vec::with_capacity(primary.len());
    let mut floor = 0;
    for hit in &primary {
        let start = if citation_kind(&value[hit.clone()]).0 == "case" {
            case_style_start(value, hit.start, floor)
        } else {
            hit.start
        };
        claimed.push(start..hit.end);
        floor = hit.end;
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
                anchor.span.start <= hit.start
                    && hit.end <= anchor.span.end
                    && anchor.span.len() > hit.len()
            })
        });
    let primary = primary
        .into_iter()
        .filter(|hit| {
            !containing
                .iter()
                .any(|anchor| anchor.span.start <= hit.start && hit.end <= anchor.span.end)
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
    anchors.extend(primary.into_iter().map(|span| Anchor::new(span, None)));
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

pub(crate) fn citation_hits(value: &str, extended_us_fallback: bool) -> Vec<Hit> {
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
    // Complete recognized report citations (including a series and page) at
    // their existing start only. The permissive splitter must not turn
    // a pinpoint such as "23 and 25" into a new authority.
    for matched in REPORTER_PATTERN.find_iter(value).flatten() {
        if let Some(hit) = found.iter_mut().find(|hit| hit.start == matched.start()) {
            hit.end = hit.end.max(matched.end());
        }
    }
    found.extend(standard_us_matches(value, &STANDARD_CANDIDATE));
    found.extend(COMMON_US_LAW.find_spans(value));
    if extended_us_fallback {
        for (start, end) in us_fallback_ranges(value) {
            let candidate = &value[start..end];
            found.extend(
                standard_us_matches(candidate, &EXTENDED_CANDIDATE)
                    .into_iter()
                    .map(|matched| start + matched.start..start + matched.end),
            );
            for (id, pattern) in EXTENDED_US_CITATION_IDS
                .iter()
                .zip(EXTENDED_US_PATTERNS.iter())
            {
                if id.ends_with(".short") && !candidate.contains(" at") {
                    continue;
                }
                found.extend(
                    pattern
                        .find_spans(candidate)
                        .into_iter()
                        .map(|matched| start + matched.start..start + matched.end),
                );
            }
        }
    }
    resolve(found)
}

fn whole(pattern: &Regex, core: &str) -> bool {
    pattern
        .find(core)
        .is_some_and(|matched| matched.start() == 0 && matched.end() == core.len())
}

/// Whether a `(year) volume Title page` span names a periodical: its title
/// carries a journal word (`Rev`, `LJ`, `LQ`). `(1992) 175 CLR 1` does not.
pub(crate) fn journal_surface(core: &str) -> bool {
    let reporter = REPORTER_PARTS
        .captures(core)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("reporter").map(|value| value.as_str().to_owned()));
    JOURNAL_CUE.is_match(reporter.as_deref().unwrap_or(core))
}

fn citation_kind(core: &str) -> (&'static str, &'static str) {
    if us_journal(core) {
        return ("journal", "us_journal_grammar");
    }
    if whole(&JOURNAL_PATTERN, core) && journal_surface(core) {
        return ("journal", "journal_grammar");
    }
    let is_whole = |span: &Hit| span.start == 0 && span.end == core.len();
    if COMMON_US_LAW.find_spans(core).iter().any(is_whole)
        || EXTENDED_US_CITATION_IDS
            .iter()
            .zip(EXTENDED_US_PATTERNS.iter())
            .any(|(id, pattern)| {
                id.contains(".law.") && pattern.find_spans(core).iter().any(is_whole)
            })
    {
        return ("statute", "statute_grammar");
    }
    if whole(&TREATY, core) {
        return ("treaty", "treaty_grammar");
    }
    if whole(&PARLIAMENTARY_COMMONWEALTH, core) {
        return ("parliamentary", "westminster_grammar");
    }
    if whole(&DATABASE, core) {
        return ("case", "database_grammar");
    }
    if let Some(captures) = ROUTING_PATTERN.captures(core) {
        let matched = captures.get(0).unwrap();
        if matched.start() == 0 && matched.end() == core.len() {
            return if captures.name("ca_statute").is_some() {
                ("statute", "provider_routing")
            } else {
                ("case", "provider_routing")
            };
        }
    }
    if REPORTER_PATTERN
        .find(core)
        .ok()
        .flatten()
        .is_some_and(|matched| is_whole(&(matched.start()..matched.end())))
    {
        return ("case", "reporter_grammar");
    }
    if core.contains("CanLII") {
        ("case", "citation_grammar")
    } else {
        ("other", "citation_grammar")
    }
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
        .map(|matched| matched.start()..matched.end())
        .collect::<Vec<_>>();
    let mut positions = vec![false; window.len() + 1];
    let (mut round, mut square, mut smart, mut straight) = (0u32, 0u32, false, false);
    for (index, character) in window.char_indices() {
        if masked.iter().any(|range| range.contains(&index)) {
            continue;
        }
        positions[index] = !smart && !straight && round == 0 && square == 0;
        let quoted = smart || straight;
        match character {
            '\u{201c}' => smart = true,
            '\u{201d}' => smart = false,
            '"' => straight = !straight,
            '(' if !quoted => round += 1,
            ')' if !quoted => round = round.saturating_sub(1),
            '[' if !quoted => square += 1,
            ']' if !quoted => square = square.saturating_sub(1),
            _ => {}
        }
    }
    positions[window.len()] = !smart && !straight && round == 0 && square == 0;
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
    let end = trim_style_end(text, start, floor + name.end());
    (start < end).then_some(start..end)
}

/// A style span ends before the comma and spaces that join it to the core.
fn trim_style_end(text: &str, start: usize, end: usize) -> usize {
    start
        + text[start..end]
            .trim_end_matches(|character: char| javascript_whitespace(character) || character == ',')
            .len()
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
    let captures = INTRODUCTORY_SIGNAL.captures(&text[floor..start])?;
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
) -> Citation {
    let mut core = anchor.span.clone();
    let (kind, kind_reason) = anchor.family.unwrap_or_else(|| citation_kind(&text[core.clone()]));
    // eyecite's law extractors stop before written subdivisions (`§ 1.401(a)-1`).
    if kind_reason == "statute_grammar" {
        if let Some(subdivision) = LAW_SUBDIVISION.find(&text[core.end..limit]) {
            core.end += subdivision.end();
        }
    }
    let core = &core;
    let core_text = &text[core.clone()];
    let styled_start = anchor.style_start.unwrap_or_else(|| match kind {
        "case" => case_style_start(text, core.start, previous_end),
        "statute" => statute_style_start(text, core.start, previous_end),
        "treaty" => treaty_style_start(text, core.start, previous_end),
        "journal" | "book" | "parliamentary" => work_style_start(text, core.start, previous_end),
        // An online-only source is styled with the publisher and title in
        // front of the link; every other unclassified span carries no
        // styled prefix.
        _ if kind_reason == "online_grammar" => work_style_start(text, core.start, previous_end),
        _ => core.start,
    });
    let short_form = anchor.family.is_none() && kind == "case" && core_text.contains(" at ");
    // A Bluebook pinpoint follows a comma with no keyword ("410 U.S. 113, 153").
    let bare_page = matches!(kind, "case" | "journal")
        && core_text.contains('.')
        && REPORTER_PARTS.is_match(core_text).unwrap_or(false);
    let tail = metadata::tail(
        text,
        core.end,
        limit,
        TailRules {
            bare_page,
            oscola: false,
            inner: anchor.inner,
        },
    );
    let style_end = trim_style_end(text, styled_start, core.start);
    let observed_name = text[styled_start..core.start].trim_matches(|character: char| {
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
    citation
}

fn back_citation(
    text: &str,
    core: &Hit,
    (form, note, oscola, french): (Form, Option<u32>, bool, bool),
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
    if french {
        citation.language = Some("fr".to_owned());
    }
    let name = (form == Form::Supra)
        .then(|| antecedent_name(text, core.start, previous_end))
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
    citation.full_span = span(text, start..tail.end);
    citation.pinpoints = tail.pinpoints;
    citation.parentheticals = tail.parentheticals;
    citation
}

/// Every core in document order: full anchors, back references, then bare
/// section symbols, never overlapping.
fn cores(text: &str, options: &Options) -> Vec<Core> {
    let mut cores = citation_anchors(text, options.extended_us)
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
    let mut names = Vec::new();
    names.extend(citation.explicit_short_name.clone());
    if citation.authority != Authority::Case {
        return names;
    }
    if let Some(style) = &citation.style {
        names.push(style.text.clone());
        if let Some(parties) = metadata::parties(&style.text) {
            names.push(parties.plaintiff);
            names.push(parties.defendant);
        }
    }
    names.retain(|name| {
        let lower = name.trim().to_lowercase();
        name.chars().count() >= 3
            && name.chars().next().is_some_and(char::is_uppercase)
            && !GENERIC_PARTIES.contains(&lower.as_str())
    });
    names
}

fn word_boundary(text: &str, start: usize, end: usize) -> bool {
    !text[..start].chars().next_back().is_some_and(char::is_alphanumeric)
        && !text[end..].chars().next().is_some_and(char::is_alphanumeric)
}

/// Bare case-name references (`Jordan at para 12`, `Roe at 240`) to a full
/// citation earlier in the text, and a name conjoined to the citation in
/// front of it (`...; see Oakes, supra note 4 and Jordan.`).
fn case_name_references(text: &str, citations: &[Citation]) -> Vec<Citation> {
    let mut names = Vec::new();
    for citation in citations.iter().filter(|citation| citation.form == Form::Full) {
        for name in reference_names(citation) {
            names.push((name, citation.full_span.end, citation.authority));
        }
    }
    names.sort_by_key(|(name, _, _)| std::cmp::Reverse(name.len()));
    names.dedup_by(|left, right| left.0 == right.0);
    let mut taken = citations
        .iter()
        .map(|citation| {
            citation.signal.as_ref().map_or(citation.full_span.start, |signal| signal.start)
                ..citation.full_span.end
        })
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    for (name, after, authority) in names {
        for (at, _) in text[after..].match_indices(name.as_str()) {
            let start = after + at;
            let end = start + name.len();
            if !word_boundary(text, start, end) || overlaps(&(start..end), &taken) {
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
            if tail.pinpoints.is_empty() && !conjoined {
                continue;
            }
            let mut citation =
                blank_citation(text, Form::Reference, authority, start..end, "case_name_reference");
            citation.style = Some(span(text, start..end));
            citation.short_name = Some(name.clone());
            citation.full_span = span(text, start..tail.end);
            if !tail.pinpoints.is_empty() {
                citation.reasons.push("pinpoint_grammar".to_owned());
            }
            citation.pinpoints = tail.pinpoints;
            citation.parentheticals = tail.parentheticals;
            citation.signal = signal(text, previous_end, start);
            taken.push(citation.signal.as_ref().map_or(start, |signal| signal.start)..tail.end);
            found.push(citation);
        }
    }
    found
}

/// Every citation in document order, numbered by position.
pub fn find(text: &str, options: &Options) -> Vec<Citation> {
    let cores = cores(text, options);
    let mut citations = Vec::with_capacity(cores.len());
    let mut previous_end = 0;
    for (index, core) in cores.iter().enumerate() {
        let limit = cores
            .get(index + 1)
            .map_or(text.len(), |next| match &next.kind {
                CoreKind::Full(anchor) => anchor.style_start.unwrap_or(next.span.start),
                _ => next.span.start,
            });
        let floor = previous_end.min(core.span.start);
        let mut citation = match &core.kind {
            CoreKind::Full(anchor) => full_citation(text, anchor, floor, limit),
            CoreKind::Back {
                form,
                note,
                oscola,
                french,
            } => back_citation(text, &core.span, (*form, *note, *oscola, *french), floor, limit),
            CoreKind::Unknown(section) => {
                let mut citation =
                    blank_citation(text, Form::Unknown, Authority::Unknown, core.span.clone(), "section_symbol");
                citation.fields.section = Some(section.clone());
                citation
            }
        };
        citation.signal = signal(text, floor, citation.full_span.start);
        previous_end = citation.full_span.end;
        citations.push(citation);
    }
    let references = case_name_references(text, &citations);
    citations.extend(references);
    citations.sort_by_key(|citation| citation.full_span.start);
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
    has_core_citation(text)
        || CASE_NAME
            .as_ref()
            .is_ok_and(|pattern| pattern.is_match(text))
}
