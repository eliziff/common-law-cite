//! Discover citation cores, their styled extent, pinpoints and `ibid`/`supra`
//! references. Classification and metadata happen in later stages.

use crate::model::{Authority, Citation, Fields, Form, Pinpoint, PinpointKind, Span};
use crate::text::javascript_whitespace;
use crate::Options;
use legal_grammar::{AsciiBoundedGrammar, CompiledEcmascriptGrammar};
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

fn ecmascript(pattern: &str, flags: &str) -> CompiledEcmascriptGrammar {
    legal_grammar::compile_ecmascript_pattern("citator", pattern, flags)
        .expect("frozen citator regex")
}

static CITATION_PATTERN: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| legal_grammar::compile_ecmascript_table_entry("cite.in-text").unwrap());
static CASE_NAME: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
    Regex::new(
        r"(?m)(?:^|[^\p{L}])(?:R\.|[A-Z][\p{L}\p{M}'’.&-]*(?:\s+(?:of|the|and|&|[A-Z][\p{L}\p{M}'’.&-]*)){0,6})\s+v(?:\.|ersus)?\s+[A-Z][\p{L}\p{M}'’.&-]*",
    )
});
static ROUTING_PATTERN: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| {
    legal_grammar::compile_ecmascript_table_entry("cite.provider-routing").unwrap()
});
static SIGNAL_PREFIX: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| {
    legal_grammar::compile_ecmascript_table_entry("signal.prefix.toa").unwrap()
});
static SHORT_FORM_SUFFIX: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| {
    legal_grammar::compile_ecmascript_table_entry("shortform.splitter").unwrap()
});
static AUTHORITY_REFERENCE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| {
    legal_grammar::compile_ecmascript_table_entry("ref.inline.toa").unwrap()
});
static SUPRA_NOTE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| {
    legal_grammar::compile_ecmascript_table_entry("ref.supra-note.linking").unwrap()
});
// Classify spans already found by the citation grammar. The splitter is
// intentionally permissive: do not use it to discover new prose spans.
static REPORTER_PATTERN: LazyLock<legal_grammar::CompiledGrammar> =
    LazyLock::new(|| legal_grammar::compile_table_entry("cite.reporter.splitter").unwrap());
static JOURNAL_PATTERN: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| {
    legal_grammar::compile_ecmascript_table_entry("cite.journal.toa").unwrap()
});
// Secondary-source first references. Case law reaches the citation grammar
// through a reporter, a neutral citation or a docket; every other authority
// announces itself with a publication block instead, one family per entry.
// These only ever add anchors: a secondary hit that touches a case-law hit is
// dropped, so the case lane is byte-identical with and without them.
static SECONDARY_ECMASCRIPT: LazyLock<[(&'static str, &'static str, CompiledEcmascriptGrammar); 4]> =
    LazyLock::new(|| {
        [
            ("statute", "ca_statute_grammar", "cite.ca.statute.first"),
            ("statute", "titled_statute_grammar", "cite.statute.titled"),
            ("book", "book_grammar", "cite.book.imprint"),
            ("parliamentary", "parliamentary_grammar", "cite.parliamentary.paper"),
        ]
        .map(|(kind, reason, id)| {
            (
                kind,
                reason,
                legal_grammar::compile_ecmascript_table_entry(id).unwrap(),
            )
        })
    });
// Article without a first page ("… (2020) The Journal of Value Inquiry at 1")
// needs a lookahead so the pinpoint stays outside the core, which is the
// backtracking dialect rather than the linear one.
static JOURNAL_ARTICLE: LazyLock<legal_grammar::CompiledGrammar> =
    LazyLock::new(|| legal_grammar::compile_table_entry("cite.journal.article").unwrap());
static ONLINE_SOURCE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| legal_grammar::compile_ecmascript_table_entry("cite.url").unwrap());
static PINPOINT_PATTERNS: LazyLock<[(&'static str, CompiledEcmascriptGrammar); 3]> =
    LazyLock::new(|| {
        [
            (
                "paragraph",
                legal_grammar::compile_ecmascript_table_entry("pinpoint.para.toa").unwrap(),
            ),
            (
                "section",
                legal_grammar::compile_ecmascript_table_entry("pinpoint.section.toa")
                    .unwrap(),
            ),
            (
                "page",
                legal_grammar::compile_ecmascript_table_entry("pinpoint.page.toa").unwrap(),
            ),
        ]
    });
static PINPOINT_ITEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d+(?:\.\d+)*(?:\([A-Za-z0-9]+\))*").unwrap());
static PINPOINT_BRIDGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^[\s,]*(?:at\s+)?$").unwrap());
static CASE_VERSUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:v|c)\.?\s+").unwrap());
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
// boundary the next authority is split on.
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
// "Reference re Secession of Quebec" / "Re Residential Tenancies Act": a style
// of cause with one party instead of two.
static CASE_RE_STYLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)(?<name>(?:Reference\s+re|Renvoi\s+relatif|In\s+re|In\s+the\s+[Mm]atter\s+of|Re)\s+[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&()-]*(?:\s+(?:[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&()-]*|\([\p{Lu}][^()\n]{0,80}\)|of|the|and|for|de|la|du|des|aux|en)){0,12})(?:,\s*(?:1[6-9]|20)\d{2})?\s*,?\s*$",
    )
    .unwrap()
});
// The title a statute citation is styled with, ending in the instrument word
// and optionally carrying its own regnal year and jurisdiction.
static STATUTE_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        // The instrument word ends the title ("Criminal Code") or opens it
        // ("Charter of the French language", "Loi sur la protection").
        r"(?m)(?<title>[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&()-]*(?:\s+(?:[\p{Lu}\p{N}][\p{L}\p{M}\p{N}.'\u{2019}&()-]*|of|the|and|to|for|in|on|de|la|du|des|et)){0,12}\s+(?:Acts?|Codes?|Rules?|Regulations?|Charter|Convention|Treaty|Protocol|Declaration)|(?:Acts?|Codes?|Rules?|Regulations?|Charte?r?|Loi|R\u{e8}glement|Convention|Treaty|Protocol|Declaration)\s+(?:of|on|respecting|concerning|sur|de|du|des|pour)(?:\s+(?:[\p{L}\p{M}\p{N}.'\u{2019}&()-]+)){1,12})(?:,\s*(?:1[6-9]|20)\d{2})?(?:\s*\([\p{Lu}][^()\n]{0,20}\))?\s*,?\s*$",
    )
    .unwrap()
});
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
static RESIDUAL_CUE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| {
    legal_grammar::compile_ecmascript_table_entry("cite.us.fallback-cue").unwrap()
});
static COMMON_US_LAW: LazyLock<AsciiBoundedGrammar> = LazyLock::new(|| {
    legal_grammar::compile_ascii_bounded_table_entry("cite.us.law.common")
        .expect("common US law grammar")
});
static STANDARD_CANDIDATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:\A|[^A-Za-z0-9])(?<citation>(?<volume>[1-9][0-9]*) (?<reporter>[^;\r\n]{1,180}?),? (?:at\s?(?:p(?:\.|age)?)? )?(?<page>[0-9]+|_+))(?:\z|[^A-Za-z0-9])",
    )
    .unwrap()
});
static EXTENDED_CANDIDATE: LazyLock<Regex> = LazyLock::new(extended_standard_candidate);
static STANDARD_SURFACES: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let tables = legal_grammar::load_tables().expect("shared US citation grammar");
    let entry = tables
        .get("cite.us.reporter.standard.full")
        .expect("shared reporter grammar");
    ["us_reporters", "us_journals"]
        .into_iter()
        .flat_map(|name| split_literal_alternation(&entry.defs[name]))
        .map(decode_surface)
        .collect()
});
static EXTENDED_US_PATTERNS: LazyLock<[AsciiBoundedGrammar; 4]> = LazyLock::new(|| {
    EXTENDED_US_CITATION_IDS.map(|id| {
        legal_grammar::compile_ascii_bounded_table_entry(id).expect("extended US grammar")
    })
});

fn extended_standard_candidate() -> Regex {
    let tables = legal_grammar::load_tables().expect("shared US citation grammar");
    let page = &tables["cite.us.reporter.full"].defs["us_page"];
    Regex::new(&format!(
        r"(?:\A|[^A-Za-z0-9])(?<citation>(?<volume>[1-9][0-9]*) (?<reporter>[^;\r\n]{{1,180}}?),? (?:at\s?(?:p(?:\.|age)?)? )?(?<page>{page}))(?:\z|[^A-Za-z0-9])"
    ))
    .unwrap()
}

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

fn standard_us_matches(value: &str, pattern: &Regex) -> Vec<Hit> {
    let mut found = Vec::new();
    let mut cursor = 0;
    while let Some(captures) = pattern.captures_at(value, cursor) {
        let citation = captures.name("citation").expect("standard citation");
        let reporter = captures.name("reporter").expect("standard reporter");
        let reporter = reporter.as_str();
        let known = if reporter.chars().any(javascript_whitespace) {
            let compact = reporter
                .chars()
                .filter(|character| !javascript_whitespace(*character))
                .collect::<String>();
            STANDARD_SURFACES.contains(&compact)
        } else {
            STANDARD_SURFACES.contains(reporter)
        };
        if known {
            found.push(citation.start()..citation.end());
        }
        cursor = citation.start() + 1;
    }
    found
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
/// text by [`citation_kind`].
struct Anchor {
    span: Hit,
    family: Option<(&'static str, &'static str)>,
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

/// First references to the authorities that never carry a reporter: statutes
/// and regulations, journal articles, monographs and edited collections,
/// parliamentary papers, and online-only sources.
fn secondary_hits(value: &str) -> Vec<Anchor> {
    let mut found = Vec::new();
    for (kind, reason, pattern) in SECONDARY_ECMASCRIPT.iter() {
        found.extend(
            pattern
                .find_iter(value)
                .map(|matched| (matched.start()..matched.end(), (*kind, *reason))),
        );
    }
    found.extend(
        JOURNAL_ARTICLE
            .find_iter(value)
            .flatten()
            .map(|matched| (matched.start()..matched.end(), ("journal", "article_grammar"))),
    );
    found.extend(
        ONLINE_SOURCE
            .find_iter(value)
            .map(|matched| (matched.start()..matched.end(), ("other", "online_grammar"))),
    );
    found.sort_by(|left, right| {
        left.0
            .start
            .cmp(&right.0.start)
            .then_with(|| right.0.end.cmp(&left.0.end))
    });
    let mut resolved: Vec<Anchor> = Vec::new();
    for (span, family) in found {
        if resolved
            .last()
            .is_some_and(|previous| span.start < previous.span.end)
        {
            continue;
        }
        resolved.push(Anchor {
            span,
            family: Some(family),
        });
    }
    resolved
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
    let mut anchors = secondary_hits(value)
        .into_iter()
        .filter(|anchor| {
            !claimed
                .iter()
                .any(|hit| anchor.span.start < hit.end && hit.start < anchor.span.end)
        })
        .collect::<Vec<_>>();
    anchors.extend(primary.into_iter().map(|span| Anchor { span, family: None }));
    anchors.sort_by_key(|anchor| anchor.span.start);
    anchors
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


fn citation_kind(core: &str) -> (&'static str, &'static str) {
    let is_whole = |span: &Hit| span.start == 0 && span.end == core.len();
    if JOURNAL_PATTERN
        .find(core)
        .is_some_and(|matched| is_whole(&(matched.start()..matched.end())))
    {
        return ("journal", "journal_grammar");
    }
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
        let Some(signal) = SIGNAL_PREFIX.find(&text[start..core_start]) else {
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

/// Raise `floor` past the last semicolon or sentence end before the anchor, so
/// a styled span never reaches back over the delimiter that separates it from
/// the authority in front of it.
fn styled_floor(text: &str, floor: usize, core_start: usize) -> usize {
    STYLED_FLOOR
        .find_iter(&text[floor..core_start])
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
    let Some(versus) = CASE_VERSUS.find_iter(prefix).last() else {
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

fn pinpoint_hits(text: &str, core_end: usize, limit: usize) -> (Vec<(Hit, &'static str)>, usize) {
    let tail = &text[core_end..limit];
    let mut selected = None::<(&'static str, usize, usize, usize, usize)>;
    for (kind, pattern) in PINPOINT_PATTERNS.iter() {
        let Some(captures) = pattern.captures(tail) else {
            continue;
        };
        let matched = captures.get(0).unwrap();
        if !PINPOINT_BRIDGE.is_match(&tail[..matched.start()]) {
            continue;
        }
        let sequence = captures.get(1).unwrap();
        let candidate = (
            *kind,
            matched.start(),
            matched.end(),
            sequence.start(),
            sequence.end(),
        );
        if selected.is_none_or(|current| candidate.1 < current.1) {
            selected = Some(candidate);
        }
    }
    let Some((kind, _, matched_end, sequence_start, sequence_end)) = selected else {
        return (Vec::new(), core_end);
    };
    let sequence = core_end + sequence_start..core_end + sequence_end;
    let pinpoints = PINPOINT_ITEM
        .find_iter(&text[sequence.clone()])
        .map(|matched| {
            (
                sequence.start + matched.start()..sequence.start + matched.end(),
                kind,
            )
        })
        .collect();
    (pinpoints, core_end + matched_end)
}

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

pub(crate) fn span(text: &str, range: Hit) -> Span {
    Span {
        text: text[range.clone()].to_owned(),
        start: range.start,
        end: range.end,
    }
}

fn pinpoint_kind(kind: &str) -> PinpointKind {
    match kind {
        "paragraph" => PinpointKind::Paragraph,
        "section" => PinpointKind::Section,
        _ => PinpointKind::Page,
    }
}

fn pinpoints(text: &str, hits: Vec<(Hit, &'static str)>) -> Vec<Pinpoint> {
    hits.into_iter()
        .map(|(range, kind)| Pinpoint {
            kind: pinpoint_kind(kind),
            first: text[range.clone()].to_owned(),
            last: None,
            span: span(text, range),
        })
        .collect()
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
        _ => Authority::Unknown,
    }
}

fn full_citations(text: &str, options: &Options) -> Vec<Citation> {
    let anchors = citation_anchors(text, options.extended_us);
    let mut citations = Vec::with_capacity(anchors.len());
    let mut previous_end = 0;
    for (index, anchor) in anchors.iter().enumerate() {
        let core = &anchor.span;
        let limit = anchors
            .get(index + 1)
            .map_or(text.len(), |next| next.span.start);
        let core_text = &text[core.clone()];
        let (kind, kind_reason) = anchor.family.unwrap_or_else(|| citation_kind(core_text));
        let styled_start = match kind {
            "case" => case_style_start(text, core.start, previous_end),
            "statute" => statute_style_start(text, core.start, previous_end),
            "journal" | "book" | "parliamentary" => {
                work_style_start(text, core.start, previous_end)
            }
            // An online-only source is styled with the publisher and title in
            // front of the link; every other unclassified span carries no
            // styled prefix.
            _ if kind_reason == "online_grammar" => {
                work_style_start(text, core.start, previous_end)
            }
            _ => core.start,
        };
        let (pinpoint_hits, pinpoint_end) = pinpoint_hits(text, core.end, limit);
        let explicit_short = explicit_short_form(text, pinpoint_end, limit);
        let end = explicit_short
            .as_ref()
            .map_or(pinpoint_end, |(_, end)| *end);
        let observed_name = text[styled_start..core.start].trim_matches(|character: char| {
            javascript_whitespace(character) || ",;:.".contains(character)
        });
        let short_name = if observed_name.is_empty() {
            explicit_short.as_ref().map(|(short, _)| short.clone())
        } else {
            Some(observed_name.to_owned())
        };
        let mut reasons = vec![kind_reason.to_owned()];
        if styled_start < core.start {
            reasons.push("same_text_style".to_owned());
        }
        if !pinpoint_hits.is_empty() {
            reasons.push("pinpoint_grammar".to_owned());
        }
        if explicit_short.is_some() {
            reasons.push("short_form_suffix".to_owned());
        }
        let authority = authority(kind);
        if authority == Authority::Unknown && kind_reason == "citation_grammar" {
            reasons.push("kind_unclassified".to_owned());
        }
        if kind_reason == "online_grammar" {
            reasons.push("webpage".to_owned());
        }
        citations.push(Citation {
            index: 0,
            form: Form::Full,
            authority: if kind_reason == "online_grammar" {
                Authority::Webpage
            } else {
                authority
            },
            format: None,
            span: span(text, core.clone()),
            full_span: span(text, styled_start..end),
            style: (styled_start < core.start).then(|| span(text, styled_start..core.start)),
            parties: None,
            fields: Fields::default(),
            court: None,
            jurisdiction: None,
            language: None,
            pinpoints: pinpoints(text, pinpoint_hits),
            parentheticals: Vec::new(),
            history: Vec::new(),
            short_name,
            explicit_short_name: explicit_short.map(|(short, _)| short),
            parallel_group: None,
            antecedent: None,
            key: None,
            reasons,
        });
        previous_end = end;
    }
    citations
}

/// `ibid`, `supra` and note references that point back to an earlier authority.
fn back_references(text: &str, full: &[Citation]) -> Vec<Citation> {
    let cores = full
        .iter()
        .map(|citation| citation.span.start..citation.span.end)
        .collect::<Vec<_>>();
    let tokens = AUTHORITY_REFERENCE
        .find_iter(text)
        .map(|matched| matched.start()..matched.end())
        .filter(|token| {
            !cores
                .iter()
                .any(|core| token.start < core.end && core.start < token.end)
        })
        .collect::<Vec<_>>();
    tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            let limit = tokens
                .get(index + 1)
                .map_or(text.len(), |next| next.start)
                .min(
                    cores
                        .iter()
                        .find(|core| core.start >= token.end)
                        .map_or(text.len(), |core| core.start),
                );
            let (hits, end) = pinpoint_hits(text, token.end, limit);
            let token_text = &text[token.clone()];
            let ibid = token_text
                .get(..4)
                .is_some_and(|value| value.eq_ignore_ascii_case("ibid"));
            let note = (!ibid)
                .then(|| SUPRA_NOTE.captures(token_text))
                .flatten()
                .and_then(|captures| captures.name("note"))
                .and_then(|value| value.as_str().parse().ok());
            Citation {
                index: 0,
                form: if ibid { Form::Ibid } else { Form::Supra },
                authority: Authority::Unknown,
                format: None,
                span: span(text, token.clone()),
                full_span: span(text, token.start..end),
                style: None,
                parties: None,
                fields: Fields {
                    note,
                    ..Fields::default()
                },
                court: None,
                jurisdiction: None,
                language: None,
                pinpoints: pinpoints(text, hits),
                parentheticals: Vec::new(),
                history: Vec::new(),
                short_name: None,
                explicit_short_name: None,
                parallel_group: None,
                antecedent: None,
                key: None,
                reasons: vec!["reference_grammar".to_owned()],
            }
        })
        .collect()
}

/// Every citation in document order, numbered by position.
pub fn find(text: &str, options: &Options) -> Vec<Citation> {
    let mut citations = full_citations(text, options);
    let references = back_references(text, &citations);
    citations.extend(references);
    citations.sort_by_key(|citation| citation.span.start);
    for (index, citation) in citations.iter_mut().enumerate() {
        citation.index = index;
    }
    citations
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
