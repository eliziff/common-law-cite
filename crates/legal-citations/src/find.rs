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
use crate::{CitationStyle, Options};
use legal_grammar::{CompiledEcmascriptGrammar, CompiledGrammar};
use regex::Regex;
use std::ops::Range;
use std::sync::LazyLock;

#[derive(serde::Serialize)]
pub struct ProviderCitationMatch<'a> {
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
    pub family: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub court: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporter: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<&'a str>,
}

/// Original LSP provider routing; byte offsets are converted by the host adapter.
pub fn provider_citations(text: &str) -> Vec<ProviderCitationMatch<'_>> {
    crate::classify::ROUTING
        .captures_iter(text)
        .map(|captures| {
            let matched = captures.get(0).unwrap();
            let has = |name| captures.name(name).is_some();
            let (family, jurisdiction) = if has("uk_neutral") {
                ("neutral", Some("uk"))
            } else if has("ca_statute") {
                ("statute", Some("ca"))
            } else if has("ca_reporter") {
                ("reporter", Some("ca"))
            } else if has("ca_neutral") {
                ("neutral", Some("ca"))
            } else if has("us_reporter") {
                ("reporter", Some("us"))
            } else if has("neutral") {
                ("neutral", None)
            } else {
                ("reporter", None)
            };
            let group = |names: &[&str]| {
                names
                    .iter()
                    .find_map(|name| captures.name(name).map(|capture| capture.as_str()))
            };
            ProviderCitationMatch {
                text: matched.as_str(),
                start: matched.start(),
                end: matched.end(),
                family,
                jurisdiction,
                year: group(&["uk_year", "ca_year", "year"]),
                court: group(&["uk_court", "ca_court", "court"]),
                number: group(&["uk_num", "ca_num", "num"]),
                volume: group(&["us_volume", "volume"]),
                reporter: group(&["us_reporter_name", "reporter"]),
                page: group(&["us_page", "page"]),
            }
        })
        .collect()
}

pub(crate) type Hit = Range<usize>;

fn linear(id: &str) -> CompiledEcmascriptGrammar {
    legal_grammar::compile_ecmascript_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

fn backtracking(id: &str) -> CompiledGrammar {
    legal_grammar::compile_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

static CITATION_PATTERN: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.in-text"));
static REPORTER_MONTH: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.reporter.month"));
static REPORTER_WORD: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.reporter.word"));
// These source-owned styles retain LSP's native Unicode regex semantics.
static CASE_NAME: LazyLock<Regex> = LazyLock::new(|| linear("style.case-name"));
static SIGNAL_PREFIX: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("signal.prefix.toa"));
static INTRODUCTORY_SIGNAL: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("signal.introductory"));
// Précité ends in a non-ASCII letter, so its word boundary needs the Unicode
// (backtracking) dialect.
static BACK_REFERENCE: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("ref.back"));
static INLINE_REFERENCE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("ref.inline.toa"));
static INLINE_NOTE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("ref.supra-note.linking"));
static NOTE_CROSS_REFERENCE: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("ref.note-cross"));
static ANTECEDENT_NAME: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("ref.antecedent-name"));
static SECTION_SYMBOL: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.section-symbol"));
// Complete existing cores. The permissive splitter does not discover anchors.
static REPORTER_PATTERN: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("cite.reporter.splitter"));
static JOURNAL_PATTERN: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.journal.toa"));
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
static LAW_SUBDIVISION: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.us.law.subdivision"));
// Secondary-source first references. Case law reaches the citation grammar
// through a reporter, a neutral citation or a docket; every other authority
// announces itself with a publication block instead, one family per entry.
// These only ever add anchors: a secondary hit that touches a case-law hit is
// dropped, so the case lane is byte-identical with and without them.
static SECONDARY_ECMASCRIPT: LazyLock<[(&'static str, &'static str, CompiledEcmascriptGrammar); 10]> =
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
            ("case", "unreported_grammar", "cite.case.unreported"),
        ]
        .map(|(kind, reason, id)| (kind, reason, linear(id)))
    });
// Article without a first page ("… (2020) The Journal of Value Inquiry at 1")
// needs a lookahead so the pinpoint stays outside the core, which is the
// backtracking dialect rather than the linear one.
static JOURNAL_ARTICLE: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("cite.journal.article"));
static ONLINE_SOURCE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.url"));
// Forms that carry no reporter, journal or imprint the grammars above read, with the guides that
// prescribe them: a guide's own forms run only when the caller asks for that guide or for none.
type Guides = &'static [CitationStyle];
const EVERY: Guides = &[];
const CANADA: Guides = &[CitationStyle::Mcgill, CitationStyle::Coal];
const MCGILL: Guides = &[CitationStyle::Mcgill];
const COMMONWEALTH: Guides = &[CitationStyle::Mcgill, CitationStyle::Oscola, CitationStyle::Aglc, CitationStyle::Nzlsg];
const TITLE_YEAR: Guides = &[CitationStyle::Oscola, CitationStyle::Aglc, CitationStyle::Nzlsg];
const EUROPE: Guides = &[CitationStyle::Mcgill, CitationStyle::Oscola];
const UNITED_STATES: Guides = &[CitationStyle::Mcgill, CitationStyle::Bluebook];
const BLUEBOOK: Guides = &[CitationStyle::Bluebook];
const AGLC: Guides = &[CitationStyle::Aglc];
static SECONDARY_SOURCES: LazyLock<Vec<(&'static str, &'static str, CompiledGrammar, Guides)>> = LazyLock::new(|| [
    ("journal", "manuscript_grammar", "cite.secondary.manuscript", EVERY),
    ("book", "thesis_grammar", "cite.secondary.thesis", EVERY),
    ("journal", "paper_grammar", "cite.secondary.paper", EVERY),
    ("journal", "dated_work_grammar", "cite.secondary.dated", EVERY),
    ("journal", "news_grammar", "cite.secondary.news", EVERY),
    ("journal", "news_grammar", "cite.secondary.periodical", EVERY),
    ("journal", "news_grammar", "cite.secondary.letter-to-editor", EVERY),
    ("journal", "article_grammar", "cite.journal.bracket-year", EVERY),
    ("government", "international_grammar", "cite.secondary.international", EVERY),
    ("government", "international_grammar", "cite.international.organization", EVERY),
    ("case", "international_case_grammar", "cite.case.international", EVERY),
    ("book", "encyclopedia_grammar", "cite.secondary.encyclopedia", EVERY),
    ("book", "encyclopedia_grammar", "cite.secondary.encyclopedia-ced", CANADA),
    ("book", "encyclopedia_fascicle_grammar", "cite.secondary.jurisclasseur", CANADA),
    ("book", "dictionary_grammar", "cite.secondary.dictionary", EVERY),
    ("book", "coursepack_grammar", "cite.secondary.coursepack", EVERY),
    ("government", "intellectual_property_grammar", "cite.secondary.intellectual-property", EVERY),
    ("case", "court_file_grammar", "cite.ca.court-file", CANADA),
    ("statute", "code_grammar", "cite.ca.code", CANADA),
    ("statute", "bylaw_grammar", "cite.ca.bylaw", CANADA),
    ("court_rule", "court_rules_grammar", "cite.ca.court-rules", CANADA),
    ("parliamentary", "parliamentary_grammar", "cite.parliamentary.record", CANADA),
    ("parliamentary", "parliamentary_grammar", "cite.parliamentary.petition", CANADA),
    ("parliamentary", "parliamentary_grammar", "cite.secondary.committee-report", CANADA),
    ("statute", "foreign_statute_grammar", "cite.foreign.titled-statute", MCGILL),
    ("statute", "foreign_statute_grammar", "cite.foreign.uk-instrument", COMMONWEALTH),
    ("constitution", "constitution_grammar", "cite.foreign.us-constitution", UNITED_STATES),
    ("statute", "foreign_statute_grammar", "cite.foreign.us-code", UNITED_STATES),
    ("parliamentary", "foreign_parliamentary_grammar", "cite.foreign.us-legislative", UNITED_STATES),
    ("case", "foreign_reporter_grammar", "cite.foreign.us-reporter", MCGILL),
    ("statute", "foreign_statute_grammar", "cite.foreign.civil-code", MCGILL),
    ("case", "foreign_court_grammar", "cite.foreign.civil-court", MCGILL),
    ("statute", "foreign_statute_grammar", "cite.foreign.french-instrument", MCGILL),
    ("journal", "foreign_doctrine_grammar", "cite.foreign.french-doctrine", MCGILL),
    ("statute", "foreign_statute_grammar", "cite.foreign.eu", EUROPE),
    ("case", "foreign_court_grammar", "cite.foreign.eu-case", EUROPE),
    ("statute", "title_year_statute_grammar", "cite.statute.title-year", TITLE_YEAR),
    ("case", "unreported_grammar", "cite.case.unreported-commonwealth", TITLE_YEAR),
    ("government", "report_grammar", "cite.secondary.law-reform", EVERY),
    ("statute", "foreign_statute_grammar", "cite.us.code-section", BLUEBOOK),
    ("court_rule", "court_rules_grammar", "cite.us.court-rule", BLUEBOOK),
    ("government", "report_grammar", "cite.us.opinion", BLUEBOOK),
    ("book", "book_edition_grammar", "cite.us.book-edition", BLUEBOOK),
    ("journal", "news_grammar", "cite.us.periodical", BLUEBOOK),
    ("book", "encyclopedia_grammar", "cite.aglc.online-at", AGLC),
    ("parliamentary", "foreign_parliamentary_grammar", "cite.foreign.commonwealth-parliament", MCGILL),
    ("parliamentary", "foreign_parliamentary_grammar", "cite.foreign.french-parliament", MCGILL),
    ("statute", "securities_grammar", "cite.ca.securities", CANADA),
    ("statute", "securities_grammar", "cite.ca.gazette", CANADA),
    ("government", "report_grammar", "cite.secondary.report", EVERY),
    ("document", "correspondence_grammar", "cite.secondary.correspondence", EVERY),
    ("document", "archival_grammar", "cite.secondary.archival", EVERY),
    ("book", "religious_text_grammar", "cite.secondary.religious", EVERY),
].into_iter().map(|(kind, reason, id, guides)| (kind, reason, backtracking(id), guides)).collect());
static CHARTER: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("cite.ca.charter"));
static CASE_FLOOR: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("style.case-floor"));
// A document's own list or table of authorities: its heading, and the heading that ends it.
static AUTHORITY_LIST: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("section.authority-list"));
static AUTHORITY_LIST_END: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("section.authority-list.end"));
// "In the Matter of ...": a proceeding a court file names when no style of cause does.
static MATTER_STYLE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("style.matter"));
static ACCESS_DATE: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("attach.access-date"));
static SUPRA_AFTER: LazyLock<Regex> = LazyLock::new(|| linear("ref.supra-follows"));
static GOVERNMENT_AUTHOR: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("style.government-author"));
static LEGISLATIVE_TITLE: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("style.legislative-title"));
static CASE_VERSUS: LazyLock<Regex> = LazyLock::new(|| linear("style.case-versus"));
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
// boundary the next authority is split on.
static STYLED_FLOOR: LazyLock<Regex> = LazyLock::new(|| linear("style.floor"));
// "Reference re Secession of Quebec" / "Re Residential Tenancies Act" /
// "Renvoi relatif à la sécession du Québec" / "Moore (Re)": a style of cause
// with one party instead of two.
static CASE_RE_STYLE: LazyLock<Regex> = LazyLock::new(|| linear("style.case-single-party"));
// The title a statute or treaty citation is styled with, ending in the
// instrument word and optionally carrying its own regnal year and jurisdiction.
static STATUTE_TITLE: LazyLock<Regex> = LazyLock::new(|| linear("style.statute-title"));
static TRAILING_DATE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("style.trailing-date"));
// "Author, \u{201c}Article Title\u{201d}" (and any "in Editor, ed," lead-in):
// the styled part of a secondary source sitting in front of its publication
// block.
static QUOTED_WORK: LazyLock<Regex> = LazyLock::new(|| linear("style.quoted-work"));
// An unsigned work that opens with its quoted title.
static QUOTED_TITLE: LazyLock<Regex> = LazyLock::new(|| linear("style.quoted-title"));
// "Author, Title" when the title is written in sentence case or holds a
// parenthesis or an identifier.
static AUTHORED_WORK: LazyLock<Regex> = LazyLock::new(|| linear("style.authored-work"));
// The same styled part when the work carries no quoted title: a monograph, a
// debate record, a dictionary.
static PLAIN_WORK: LazyLock<Regex> = LazyLock::new(|| linear("style.plain-work"));
// "…", Case Comment on <decision>: the words that make a citation the subject
// of the commentary cited after it.
static COMMENTED_SUBJECT: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("attach.commented-subject"));
static REPRINT: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("ref.reprint"));
static REPRINT_LOCATION: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("ref.reprint.location"));
// The words that introduce a work's link: "(June 2006), online: <".
static LINK_LEAD: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("attach.link"));
/// Names that open a style of cause but never identify a case on their own.
const GENERIC_PARTIES: [&str; 18] = [
    "r", "r.", "rex", "regina", "the queen", "her majesty the queen", "his majesty the king",
    "the king", "queen", "king", "crown", "canada", "united states", "state", "people",
    "commonwealth", "the state", "attorney general",
];

/// A discovered authority anchor carries the family and fields recognized by
/// its grammar. Unclassified spans are read when the anchor is materialized.
#[derive(Default)]
struct Anchor {
    span: Hit,
    source_span: Option<Hit>,
    preceding_text_end: Option<usize>,
    native: bool,
    family: Option<(&'static str, &'static str)>,
    reading: Option<crate::classify::Reading>,
}

impl Anchor {
    fn new(span: Hit, family: (&'static str, &'static str)) -> Self {
        Self { span, family: Some(family), native: true, ..Self::default() }
    }

    fn read(&mut self, text: &str) {
        self.reading = crate::classify::read(&text[self.span.clone()], self.family.map_or("", |(_, reason)| reason), "");
    }
}

fn resolve(mut found: Vec<Anchor>) -> Vec<Anchor> {
    found.sort_by(|left, right| {
        left.span.start
            .cmp(&right.span.start)
            .then_with(|| right.span.end.cmp(&left.span.end))
    });
    let mut resolved: Vec<Anchor> = Vec::new();
    for hit in found {
        if resolved.last().is_some_and(|previous| hit.span.start < previous.span.end) {
            continue;
        }
        resolved.push(hit);
    }
    resolved
}

/// First references to the authorities that never carry a reporter: statutes
/// and regulations, journal articles, monographs and edited collections,
/// parliamentary papers, treaties, database identifiers and online-only sources.
fn secondary_hits(value: &str, primary: &[Hit], styles: Option<&[CitationStyle]>) -> Vec<Anchor> {
    let mut found = Vec::new();
    for (kind, reason, pattern) in SECONDARY_ECMASCRIPT.iter() {
        for matched in pattern.find_iter(value) {
            let start = matched.start();
            if *reason == "unreported_grammar" && case_style_start(value, start, 0) == start {
                continue;
            }
            // A revised volume and year without a chapter ("RSA 2000") cites nothing unless an Act's
            // title leads it ("Companies' Creditors Arrangement Act, R.S.C. 1985") or it is cited for
            // its appendix ("RSC 1985, App II, No 5").
            if *reason == "ca_statute_grammar" && pattern.captures(matched.as_str()).is_some_and(|capture|
                capture.name("series").is_some() && capture.name("chapter").is_none() && capture.name("schedule").is_none())
                && !value[matched.end()..].trim_start_matches([',', ' ']).starts_with("App")
                && statute_style_start(value, start, 0) == start {
                continue;
            }
            if *reason == "book_grammar" && pattern.captures(matched.as_str()).is_some_and(|capture|
                ["publisher_imprint", "oscola_imprint", "aglc_imprint"].iter().any(|group| capture.name(group).is_some())) {
                let authored = matched_style(&AUTHORED_WORK, "work", value, start, 0);
                // A court/date tail is not a publisher imprint. An authored
                // work cannot claim a recognised case core as its title.
                if authored == start || primary.iter().any(|core|
                    authored <= core.start && core.end <= start
                        && occurrence_family(&value[core.clone()]).0 == "case") {
                    continue;
                }
            }
            found.push(Anchor::new(start..matched.end(), (*kind, *reason)));
        }
    }
    for (kind, reason, pattern, guides) in SECONDARY_SOURCES.iter() {
        if guides.is_empty() || styles.is_none_or(|styles| styles.iter().any(|style| guides.contains(style))) {
            found.extend(pattern.find_iter(value).flatten()
            .map(|matched| Anchor::new(matched.start()..matched.end(), (*kind, *reason))));
        }
    }
    // The Charter's enacting instrument is its core; the title before it styles it.
    if styles.is_none_or(|styles| styles.iter().any(|style| CANADA.contains(style))) {
        found.extend(CHARTER.captures_iter(value).flatten().filter_map(|captures| captures.name("source"))
            .map(|source| Anchor::new(source.start()..source.end(), ("statute", "charter_grammar"))));
    }
    found.extend(JOURNAL_ARTICLE.find_iter(value).flatten().map(|matched| {
        Anchor::new(matched.start()..matched.end(), ("journal", "article_grammar"))
    }));
    let mut link_end = None;
    for matched in ONLINE_SOURCE.find_iter(value) {
        let link = link_core(value, matched.start()..matched.end());
        // An alternate or archived link written right after another one is
        // that source's, not a source of its own.
        let alternate = link_end.is_some_and(|end| alternate_link(value, end, link.start));
        link_end = Some(link_end_with_closer(value, &link));
        if !alternate {
            found.push(Anchor::new(link, ("other", "online_grammar")));
        }
    }
    resolve(found)
}

/// A link without the sentence punctuation after it, or a closing bracket
/// that encloses it rather than belonging to it ("<https://example.org/a>.",
/// "[https://example.org/a]").
fn link_core(text: &str, mut link: Hit) -> Hit {
    // A link written without its scheme is found with the bracket that opens it.
    link.start = link.end - text[link.clone()].trim_start_matches(['<', ' ']).len();
    while let Some(last) = text[link.clone()].chars().next_back() {
        let unbalanced = |open: char| text[link.clone()].matches(open).count()
            < text[link.clone()].matches(last).count();
        let trailing = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | '\u{2019}' | '\u{201d}' => true,
            ')' => unbalanced('('),
            ']' => unbalanced('['),
            '}' => unbalanced('{'),
            _ => false,
        };
        if !trailing || link.end - last.len_utf8() <= link.start { break; }
        link.end -= last.len_utf8();
    }
    link
}

/// The end of a link and of the bracket that encloses it ("<url>", "[url]").
fn link_end_with_closer(text: &str, link: &Hit) -> usize {
    let closer = match text[..link.start].chars().next_back() {
        Some('<') => '>',
        Some('[') => ']',
        _ => return link.end,
    };
    if text[link.end..].starts_with(closer) { link.end + closer.len_utf8() } else { link.end }
}

/// Only punctuation, brackets and spaces between one link and the next.
fn alternate_link(text: &str, end: usize, start: usize) -> bool {
    end <= start && text[end..start].chars().all(|character| javascript_whitespace(character)
        || ".,;:\\[<(".contains(character))
}

/// Where the words that introduce a link begin ("(June 2006), online: <"),
/// so the work they follow styles it; the link itself when none do.
fn link_lead_start(text: &str, link_start: usize, floor: usize) -> usize {
    let end = link_start - text[..link_start].strip_suffix(['<', '[']).map_or(0, |_| 1);
    let from = floor.max(end.saturating_sub(200)).min(end);
    (from..end).filter(|at| text.is_char_boundary(*at)).find(|&at| LINK_LEAD.find(&text[at..end]).ok()
        .flatten().is_some_and(|found| found.start() == 0 && found.end() == end - at)).unwrap_or(link_start)
}

/// A link citation's extent: its own enclosing bracket and the alternate or
/// archived links written right after it, each with its bracket.
fn online_extent(text: &str, link: &Hit, limit: usize) -> usize {
    let mut end = link_end_with_closer(text, link);
    while let Some(next) = ONLINE_SOURCE.find_iter(&text[end..limit.max(end)]).next() {
        let next = link_core(text, end + next.start()..end + next.end());
        if !alternate_link(text, end, next.start) { break; }
        end = link_end_with_closer(text, &next);
    }
    // The date it was visited closes the link: "<...> (last visited 1 July 2024)", and an
    // explanation in parentheses after it belongs to the citation.
    let end = end + ACCESS_DATE.find(&text[end..limit.max(end)]).ok().flatten().map_or(0, |found| found.end());
    metadata::citation_parenthetical(text, end, limit.max(end)).map_or(end, |range| range.end)
}

/// Frozen LSP citation_kind: occurrence family, not registry identity.
fn occurrence_family(core: &str) -> (&'static str, &'static str) {
    let whole = |span: Hit| span.start == 0 && span.end == core.len();
    if JOURNAL_PATTERN.find(core).is_some_and(|hit| whole(hit.range())) {
        return ("journal", "journal_grammar");
    }
    if crate::us::is_law(core) { return ("statute", "statute_grammar"); }
    if let Some(captures) = crate::classify::ROUTING.captures(core) {
        if whole(captures.get(0).unwrap().range()) {
            return if captures.name("ca_statute").is_some() {
                ("statute", "provider_statute_routing")
            } else { ("case", "provider_routing") };
        }
    }
    if REPORTER_PATTERN.find(core).ok().flatten().is_some_and(|hit| whole(hit.start()..hit.end())) {
        return ("case", "reporter_grammar");
    }
    (if core.contains("CanLII") { "case" } else { "other" }, "citation_grammar")
}

fn citation_anchors(value: &str, extended_us: bool, styles: Option<&[CitationStyle]>, scopes: &[usize], enrich: bool) -> Vec<Anchor> {
    let (mut found, reporters) = primary_ranges(value);
    let captured = if enrich {
        crate::us::find(value, extended_us, Some(&mut found))
    } else {
        found.extend(crate::us::citation_spans(value, extended_us));
        Vec::new()
    };
    // A (year) volume publication page block the citation grammar read only
    // part of is one citation: "(2023) 57:3 RJT 487" is not the issue "3 RJT
    // 487", and "(2021), 25 Can. Crim L Rev 255" is not "(2021), 25 Can.".
    // The block keeps the family its publication names ("(1879), 5 Ex D 264"
    // is a case).
    let mut secondary = secondary_hits(value, &found, styles);
    let mut primary = Vec::new();
    for anchor in resolve(found.into_iter().map(|span| Anchor { span, ..Anchor::default() }).collect()) {
        let hit = &anchor.span;
        // A treaty series block ("Can TS 1994 No 2") holds the number the in-text grammar reads.
        match secondary.iter_mut().find(|whole| whole.family.is_some_and(|(_, reason)| reason == "article_grammar" || reason == "treaty_grammar")
            && whole.span.start <= hit.start && hit.end <= whole.span.end && whole.span != *hit) {
            Some(whole) => if whole.reading.is_none() && whole.family.is_some_and(|(_, reason)| reason == "article_grammar") {
                whole.read(value);
                if let Some(kind) = whole.reading.as_ref().map(|reading| reading.family().0).filter(|kind| *kind != "other") {
                    whole.family = Some((kind, "article_grammar"));
                }
            },
            // A report written right before ", supra" is the short title a supra names ("Nortel
            // 2014, supra note 15"), never a citation of its own.
            None if SUPRA_AFTER.is_match(&value[anchor.span.end..]) => {}
            None => primary.push(Anchor::new(anchor.span.clone(), occurrence_family(&value[anchor.span]))),
        }
    }
    // A case claims its style of cause, and a style of cause can read as a
    // statute title ("Re Residential Tenancies Act, 1979, [1981] 1 SCR 714")
    // or as a work title; nothing inside that prefix is a second authority.
    let mut claimed = Vec::with_capacity(primary.len());
    let mut floor = 0;
    for hit in &primary {
        // A style of cause never swallows a statute the secondary grammars found before it.
        let statute_end = secondary.iter().filter(|anchor| anchor.family.is_some_and(|(kind, _)| kind == "statute")
            && anchor.span.end <= hit.span.start).map(|anchor| anchor.span.end).max().unwrap_or(0);
        let start = if hit.family.is_some_and(|(kind, _)| kind == "case") {
            case_style_start(value, hit.span.start, floor.max(statute_end).max(scope_floor(scopes, hit.span.start)))
        } else {
            hit.span.start
        };
        claimed.push(start..hit.span.end);
        floor = hit.span.end;
    }
    // LSP citation_anchors keeps the primary hit when a secondary span overlaps.
    let mut anchors = secondary
        .into_iter()
        .filter(|anchor| {
            !claimed
                .iter()
                .any(|hit| anchor.span.start < hit.end && hit.start < anchor.span.end)
        })
        .collect::<Vec<_>>();
    anchors.extend(primary);
    anchors.sort_by_key(|anchor| anchor.span.start);
    // This is LSP citation_anchors' complete result. Added source discoveries
    // must not truncate its occurrence pinpoints or suppress its references.
    if !enrich { return anchors; }
    // LSP selects primary and secondary occurrences before Eyecite attaches
    // source tokens. Source-only discoveries cannot displace native extents.
    let mut additional = Vec::new();
    for matched in captured {
        let index = anchors.partition_point(|anchor| anchor.span.end <= matched.span.start);
        let native = anchors.get_mut(index).filter(|anchor| anchor.span.start < matched.span.end);
        let mut reading = crate::classify::extracted(&value[matched.span.clone()], matched.fields, matched.short_at);
        if let Some(anchor) = native {
            if anchor.span.start < matched.span.start && reading.fields.year.is_none() {
                if anchor.reading.is_none() { anchor.read(value); }
                reading.fields.year = anchor.reading.as_mut().and_then(|native| native.fields.year.take());
            }
            anchor.source_span = (anchor.span != matched.span).then_some(matched.span);
            anchor.preceding_text_end = matched.preceding_text_end;
            anchor.reading = Some(reading);
        } else {
            additional.push(Anchor { span: matched.span, preceding_text_end: matched.preceding_text_end, family: Some(reading.family()),
                reading: Some(reading), ..Anchor::default() });
        }
    }
    anchors.extend(additional);
    anchors.sort_by_key(|anchor| anchor.span.start);
    // Eyecite's captured fields already supply their reading. Parse only the
    // remaining native families, instead of parsing then replacing those fields.
    for anchor in anchors.iter_mut().filter(|anchor| anchor.reading.is_none()) { anchor.read(value); }
    // Carry ALR's recognized reporter fields without replacing native extents
    // or pinned Eyecite readings. Its permissive boundary pattern alone does
    // not establish a citation; the registry or direct alias evidence does.
    let mut additional = Vec::new();
    for span in reporters {
        let next = anchors.partition_point(|anchor| anchor.span.end <= span.start);
        let overlap = anchors.get_mut(next).filter(|anchor| anchor.span.start < span.end);
        if overlap.as_ref().is_some_and(|anchor| span.start >= anchor.span.start || anchor.span.end > span.end
            || anchor.reading.as_ref().is_some_and(|reading| reading.source_captured())) { continue; }
        let mut source = Anchor::new(span.clone(), ("case", "reporter_grammar"));
        source.read(value);
        let registered = source.reading.as_ref().and_then(|reading| reading.fields.reporter.as_deref())
            .is_some_and(|surface| !crate::registry::registry().reporters_by_surface(surface).is_empty()
                || !crate::registry::registry().journals_by_surface(surface).is_empty());
        if !registered && !crate::aliases::observed_form(&value[span.clone()]) { continue; }
        if let Some(anchor) = overlap {
            anchor.source_span = Some(span);
            anchor.reading = source.reading;
        } else {
            source.native = false;
            additional.push(source);
        }
    }
    anchors.extend(additional);
    anchors.sort_by_key(|anchor| anchor.span.start);
    // A (year) volume publication page block keeps the family its publication
    // names, as one holding a citation core does above, so it is styled as that
    // family is: a law report or a court's identifier by the parties ("Lark v
    // Lark (2003) 2 SCR 118", "Doe (AB) v Roe (CD) (2029) ABQB 812"), a
    // periodical by the author and title in front of it ("…, “Fen Law” (2027),
    // 71 Crim LQ 212").
    for anchor in &mut anchors {
        let read = anchor.reading.as_ref().map(|reading| reading.family().0);
        let family = match (anchor.family, read) {
            (Some(("journal", reason)), Some("case")) => Some(("case", reason)),
            (Some(("case", reason)), Some("journal")) => Some(("journal", reason)),
            _ => continue,
        };
        let core = &value[anchor.span.clone()];
        if JOURNAL_ARTICLE.find(core).ok().flatten().is_some_and(|block| block.start() == 0 && block.end() == core.len()) {
            anchor.family = family;
        }
    }
    anchors
}

pub(crate) fn citation_hits(value: &str, extended_us: bool) -> Vec<Hit> {
    let (mut spans, _) = primary_ranges(value);
    spans.extend(crate::us::citation_spans(value, extended_us));
    resolve(spans.into_iter().map(|span| Anchor { span, ..Anchor::default() }).collect())
        .into_iter().map(|anchor| anchor.span).collect()
}

fn primary_ranges(value: &str) -> (Vec<Hit>, Vec<Hit>) {
    let mut found = CITATION_PATTERN
        .find_iter(value)
        .filter(|matched| !month_reporter(matched.as_str()) && !word_reporter(matched.as_str()))
        .map(|matched| matched.start()..matched.end())
        .collect::<Vec<_>>();
    // LSP citation_hits completes reports only at an already recognized start.
    let reporters = REPORTER_PATTERN.find_iter(value).flatten()
        .map(|matched| matched.start()..matched.end()).collect::<Vec<_>>();
    for matched in &reporters {
        if let Some(hit) = found.iter_mut().find(|hit| hit.start == matched.start) {
            hit.end = hit.end.max(matched.end);
        }
    }
    (found, reporters)
}

/// A date the in-text grammar read as volume, reporter and page ("12 Feb 2030",
/// "[2030] May 5"): a month name is never an unregistered reporter, though one
/// written exactly as a registered reporter is ("12 APR 345").
fn month_reporter(hit: &str) -> bool {
    REPORTER_MONTH.captures(hit).and_then(|captures| captures.name("month")).is_some_and(|month| {
        let month = month.as_str();
        !crate::registry::registry().reporters_by_surface(month).iter().any(|(reporter, _)|
            reporter.editions.iter().any(|edition| edition.abbreviation == month)
                || reporter.variations.contains_key(month))
    })
}

/// A paragraph, heading, telephone number or clerk's stamp read as volume, reporter and page
/// around one plain word ("34 Conclusion 35", "59 ARTICLE 9", "2921 Facsimile 403", a stamp's
/// "2025", "EDMONTON" and "12" on lines of their own): an ordinary word is never an unregistered
/// reporter, and an all-capital word after a year is a neutral citation's court unless it names
/// none and reads as a stamp or a date.
fn word_reporter(hit: &str) -> bool {
    REPORTER_WORD.captures(hit).is_some_and(|captures| {
        let word = captures.name("word").unwrap().as_str();
        let year = captures.name("volume").is_some_and(|volume| volume.as_str().len() == 4
            && (volume.as_str().starts_with("19") || volume.as_str().starts_with("20")));
        let registry = crate::registry::registry();
        // An unregistered court on a line of its own, of two letters, or numbering its decision
        // like a year ("2023 AT 3" from "2023 AT 3:00 PM", "2021 CBC 2021") is no court.
        let page_year = captures.name("page").is_some_and(|page| page.as_str().len() == 4
            && (page.as_str().starts_with("19") || page.as_str().starts_with("20")));
        let unnamed = registry.courts_by_surface(word).is_empty()
            && (hit.contains('\n') || word.len() <= 2 || page_year);
        !(year && word.chars().all(|character| character.is_ascii_uppercase()) && !unnamed)
            && !registry.reporters_by_surface(word).iter().any(|(reporter, _)|
                reporter.editions.iter().any(|edition| edition.abbreviation == word)
                    || reporter.variations.contains_key(word))
    })
}

/// Trim a candidate styled start: drop any leading signal ("See also", "Cf")
/// and reject a span that opens inside a parenthetical.
fn style_span_start(text: &str, mut start: usize, core_start: usize) -> Option<usize> {
    // Extracted PDF notes can put the footnote number on its own line before
    // the authority. A work-style match may otherwise consume that number.
    if start == 0 || text[..start].ends_with('\n') {
        if let Some(line_end) = text[start..core_start].find('\n') {
            let line = text[start..start + line_end].trim_end_matches('\r');
            if !line.is_empty() && line.bytes().all(|byte| byte.is_ascii_digit()) {
                start += line_end + 1;
            }
        }
    }
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

/// For every byte of `window`, whether it sits outside quotes, parentheses
/// and brackets (and outside a URL, whose `;` is not a delimiter).
pub(crate) fn top_level(window: &str) -> Vec<bool> {
    let masked = ONLINE_SOURCE
        .find_iter(window)
        .map(|matched| matched.start()..matched.end())
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

/// Raise `floor` past the last semicolon or sentence end before the
/// anchor, so a styled span never reaches back over the delimiter that
/// separates it from the authority in front of it. A title's own punctuation
/// is no such delimiter: one inside a quotation the window holds whole ("Why
/// Ferns? Notes on …", "Reeds; or, …") is passed over.
fn styled_floor(text: &str, floor: usize, core_start: usize) -> usize {
    let window = &text[floor..core_start];
    let quoted = quotations(window);
    STYLED_FLOOR
        .find_iter(window)
        .filter(|matched| !quoted.iter().any(|quote| quote.start < matched.start() && matched.end() <= quote.end))
        // An abbreviation's period before a parenthesis or a lowercase word ends no sentence:
        // "Tacora Resources Inc. (Re)", "Fen Mills Ltd. v Ash".
        .filter(|matched| !(matched.as_str().trim_end().ends_with('.')
            && window[matched.end()..].starts_with(|character: char| character == '(' || character.is_lowercase())))
        .last()
        .map_or(floor, |matched| floor + matched.end())
}

/// The quotations `window` holds whole, each from its opening mark to the mark
/// that closes it. Curly quotations nest; straight ones pair in turn.
fn quotations(window: &str) -> Vec<Hit> {
    let mut found = Vec::new();
    let (mut depth, mut opened, mut straight) = (0usize, 0, None);
    for (at, character) in window.char_indices() {
        match character {
            '\u{201c}' => {
                if depth == 0 { opened = at; }
                depth += 1;
            }
            '\u{201d}' if depth > 0 => {
                depth -= 1;
                if depth == 0 { found.push(opened..at); }
            }
            '"' if depth == 0 => match straight.take() {
                Some(start) => found.push(start..at),
                None => straight = Some(at),
            },
            _ => {}
        }
    }
    found
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
    // An unsigned work opens with its quoted title where the work begins: past
    // the floor, a note number and any signal, never in running text.
    let floor = styled_floor(text, floor, core_start);
    let blank = text[floor..core_start].len() - text[floor..core_start].trim_start_matches(javascript_whitespace).len();
    if let Some(opening) = style_span_start(text, floor + blank, core_start) {
        if let Some(work) = QUOTED_TITLE.captures(&text[opening..core_start]).and_then(|captures| captures.name("work")) {
            return opening + work.start();
        }
    }
    let authored = matched_style(&AUTHORED_WORK, "work", text, core_start, floor);
    if authored < core_start {
        return authored;
    }
    matched_style(&PLAIN_WORK, "work", text, core_start, floor)
}

/// Past the label of a numbered paragraph and a leading "In" that open the
/// sentence a case is cited in ("In R v Oak-Elm, 2030 BCPC 512", "12. Smith
/// v Jones"): neither is part of its style of cause.
fn past_lead_in(text: &str, start: usize, core_start: usize) -> usize {
    let lead = CASE_LEAD_IN.find(&text[start..core_start]).map_or(0, |matched| matched.end());
    if lead > 0 && text[start + lead..].starts_with(|character: char| character.is_uppercase()) {
        start + lead
    } else {
        start
    }
}

fn case_style_start(text: &str, core_start: usize, floor: usize) -> usize {
    // A style of cause starts after a quotation and within its own paragraph.
    let floor = CASE_FLOOR.find_iter(&text[floor..core_start]).last().map_or(floor, |found| {
        let matched = found.as_str();
        if matched.starts_with('\n') { floor + found.start() + matched.len() - matched.trim_start().len() }
        else { floor + found.end() }
    });
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
    style_span_start(text, floor + left.start(), core_start)
        .map_or(core_start, |start| past_lead_in(text, start, core_start))
}

/// The case name or author written in front of a `supra`, `(n 4)` or U.S.
/// short form ("Jordan, supra note 4", "Roe, 410 U.S. at 153").
fn antecedent_name(text: &str, core_start: usize, floor: usize) -> Option<Hit> {
    let floor = styled_floor(text, floor, core_start);
    let captures = ANTECEDENT_NAME.captures(&text[floor..core_start])?;
    let name = captures.name("name")?;
    let start = past_lead_in(text, style_span_start(text, floor + name.start(), core_start)?, core_start);
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
        "government" => Authority::GovernmentDocument,
        "court_rule" => Authority::CourtRule,
        "constitution" => Authority::Constitution,
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
    // Eyecite's tokenizer suppresses a section token inside a citation token,
    // even when LSP's occurrence extent ends earlier than that source token.
    let taken = cores.iter().flat_map(|core| {
        std::iter::once(core.span.clone()).chain(match &core.kind {
            CoreKind::Full(anchor) => anchor.source_span.clone(),
            _ => None,
        })
    }).collect::<Vec<_>>();
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
    reach: usize,
    source_name: Option<&crate::SourceCaseName>,
    options: Option<&Options>,
) -> Citation {
    let core = anchor.span.clone();
    let source_core = anchor.source_span.as_ref().unwrap_or(&anchor.span);
    let (kind, kind_reason) = anchor.family.unwrap_or(("other", "citation_grammar"));
    // A treaty series the in-text grammar found first is still styled by the treaty's title.
    let kind = if anchor.reading.as_ref().is_some_and(|reading| reading.family().0 == "treaty") { "treaty" } else { kind };
    let has_section = anchor.reading.as_ref().is_some_and(|reading| reading.has_section());
    // Parentheses inside a section identifier (1.401(a)-1) are not a
    // subdivision. Retain the section suffix in fields without widening the
    // original native occurrence.
    let section_suffix = if has_section {
        LAW_SUBDIVISION.captures(&text[source_core.end..limit])
            .and_then(|captures| captures.name("section_suffix"))
            .map(|suffix| source_core.end..source_core.end + suffix.end())
    } else { None };
    // Read actual subdivisions here so `(a)` is never a parenthetical.
    let subdivision_start = section_suffix.as_ref().map_or(source_core.end, |suffix| suffix.end);
    let subdivision = has_section
        .then(|| LAW_SUBDIVISION.find(&text[subdivision_start..limit]))
        .flatten()
        .map(|matched| subdivision_start..subdivision_start + matched.end());
    let core_text = &text[core.clone()];
    // The Charter's title and the pinpoint written between it and its enacting instrument.
    let charter = (kind_reason == "charter_grammar").then(|| CHARTER.captures_iter(&text[previous_end..core.end]).flatten()
        .filter(|captures| captures.name("source").is_some_and(|source| previous_end + source.end() == core.end))
        .last().and_then(|captures| captures.name("title").map(|title| (previous_end + title.start(), previous_end + title.end(),
            captures.name("pin").map(|pin| previous_end + pin.start()..previous_end + pin.end()))))).flatten();
    // An unreported order's style of cause written before its date, place and court: "Fen Mills
    // Inc. (Re) (4 May 2036), Toronto, Ont Sup Ct J [Commercial List] CV-36-00712345-00CL".
    let dated_style = (kind_reason == "court_file_grammar").then(|| {
        let window = &text[previous_end..core.start];
        let at = previous_end + window.match_indices(" (").map(|(at, _)| at)
            .find(|at| window[at + 2..].starts_with(|character: char| character.is_ascii_digit()))?;
        Some((case_style_start(text, at, previous_end), at)).filter(|(start, _)| *start < at)
    }).flatten();
    let styled_start = match kind {
        _ if charter.is_some() => charter.as_ref().unwrap().0,
        // An order or endorsement known by its file number is named by the matter its clause
        // opens with when it has no style of cause ("In the Matter of the Compromise or
        // Arrangement of Ash Ltd, Court File No 2601-04417").
        "case" if kind_reason == "court_file_grammar" => Some(case_style_start(text, core.start, previous_end))
            .filter(|start| *start < core.start)
            .or_else(|| dated_style.map(|(start, _)| start)).or_else(|| {
                let floor = styled_floor(text, previous_end, core.start);
                let blank = text[floor..core.start].len() - text[floor..core.start].trim_start_matches(javascript_whitespace).len();
                style_span_start(text, floor + blank, core.start)
                    .filter(|start| MATTER_STYLE.is_match(&text[*start..core.start]))
            }).unwrap_or(core.start),
        "case" => case_style_start(text, core.start, previous_end),
        "statute" => statute_style_start(text, core.start, previous_end),
        "treaty" => treaty_style_start(text, core.start, previous_end),
        // A dictionary or encyclopedia entry opens with its own title.
        _ if matches!(kind_reason, "dictionary_grammar" | "encyclopedia_grammar") => core.start,
        "journal" | "book" | "parliamentary" | "government" | "document" => work_style_start(text, core.start, previous_end),
        // An online-only source is styled with the publisher and title in
        // front of the link; every other unclassified span carries no
        // styled prefix.
        _ if kind_reason == "online_grammar" => {
            let lead = link_lead_start(text, core.start, previous_end);
            Some(work_style_start(text, lead, previous_end)).filter(|start| *start < lead).unwrap_or(core.start)
        }
        _ => core.start,
    };
    // A name runs over at most three lines of a page and a few hundred characters: one longer
    // took in a letterhead, an index or a record's prose ("Canada (AG) v. Northrop Grumman
    // Overseas ⏎ ss. 50, 18.2 Rule 398 ⏎ 13, 14, 214, …"), and the citation has no name.
    // A bill's citation holds its own title ("Bill C-9, An Act to …"): what comes before it is
    // no name of it.
    let styled_start = if charter.is_none() && (text[styled_start..core.start].matches("\n\n").count() > 2
        || text[styled_start..core.start].chars().count() > 250
        || core_text.starts_with("Bill ") && core_text.contains(',')) { core.start } else { styled_start };
    let short_pin = anchor.reading.as_ref().and_then(|reading| reading.short_at).map(|at| source_core.start + at);
    let short_form = short_pin.is_some();
    // A Bluebook pinpoint follows a comma with no keyword ("410 U.S. 113, 153").
    let bare_page = matches!(kind, "case" | "journal")
        && core_text.contains('.')
        && anchor.reading.as_ref().is_some_and(|reading| reading.reported());
    let mut tail = if options.is_none() {
        metadata::native_tail(text, core.end, limit)
    } else { metadata::tail(
        text,
        short_pin.unwrap_or_else(|| subdivision.as_ref().map_or(core.end, |range| range.end)),
        limit,
        TailRules {
            core_end: Some(core.end),
            post_citation: anchor.reading.as_ref().filter(|reading| options.is_some() && reading.source_captured() && !short_form).map(|reading| {
                let source = match reading.family().0 {
                        "case" => metadata::PostCitation::Case,
                        "journal" => metadata::PostCitation::Journal,
                        _ => metadata::PostCitation::Law,
                };
                (source, source_core.end, reach)
            }),
            bare_page,
            oscola: false,
            inner: source_name
                .filter(|name| name.pre_citation.is_some()).and_then(|name| name.pin_cite.as_ref())
                .map(|pin| (pin.start, pin.end))
                // An online work's pinpoint precedes its link: "(2022) at 2, 13 online: <...>".
                .or_else(|| (kind_reason == "online_grammar").then(|| text[styled_start..core.start]
                    .rfind(" at ").map(|at| (styled_start + at + 1, core.start))).flatten()),
        },
    ) };
    if let Some(pin) = charter.as_ref().and_then(|(_, _, pin)| pin.clone()) {
        let written = text[pin.clone()].trim_start_matches(|character: char| character == ',' || javascript_whitespace(character));
        let start = pin.end - written.len();
        let first = written.trim_start_matches(|character: char| !character.is_ascii_digit());
        let first = first.split(|character: char| !(character.is_ascii_alphanumeric() || character == '.')).next().unwrap_or("");
        tail.pinpoints.insert(0, Pinpoint { kind: PinpointKind::Section, first: first.to_owned(),
            span: span(text, start..pin.end), last: None });
    }
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
    // An online work's name and title end where the words that introduce its link, or its
    // pinpoint, begin: “Title” (22 October 2024) online: [...], “Title” (2022) at 2, 13 online: <...>.
    let name_end = if let Some((_, title_end, _)) = &charter { *title_end }
        else if let Some(end) = dated_style.filter(|(start, _)| *start == styled_start).map(|(_, end)| end) { end }
        else if kind_reason == "online_grammar" {
        let lead = link_lead_start(text, core.start, styled_start);
        text[styled_start..lead].rfind(" at ").filter(|at| text[styled_start + at + 4..].starts_with(|c: char| c.is_ascii_digit()))
            .map_or(lead, |at| styled_start + at)
    } else { core.start };
    let style_end = trim_style_end(text, styled_start, name_end);
    let observed_name = text[styled_start..style_end].trim_matches(|character: char| {
        javascript_whitespace(character) || ",;:.".contains(character)
    });
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
        let mut style_end = style_end;
        // A treaty's signature date sits between its title and its series.
        if kind == "treaty" {
            if let Some(date) = TRAILING_DATE.find(&text[styled_start..core.start]) {
                style_end = trim_style_end(text, styled_start, styled_start + date.start());
            }
        }
        citation.style = Some(span(text, styled_start..style_end));
    }
    if !tail.pinpoints.is_empty() || tail.pin_cite_kind.is_some() {
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
    }
    citation.short_name = short_name;
    let mut full_start = styled_start;
    if kind_reason == "online_grammar" {
        tail.end = tail.end.max(online_extent(text, &core, limit));
        // An unstyled link keeps the bracket that encloses it whole.
        if styled_start == core.start && link_end_with_closer(text, &core) > core.end {
            full_start -= 1;
        }
    }
    citation.full_span = span(text, full_start..tail.end);
    let source_span = (!short_form && *source_core != core).then(|| source_core.clone());
    citation.explicit_short_name = tail.short;
    citation.pinpoints = tail.pinpoints;
    citation.parentheticals = tail.parentheticals;
    if let Some(date) = tail.court_date {
        citation.fields.court_text = date.court;
        citation.fields.year = date.year;
        citation.fields.month = date.month;
        citation.fields.day = date.day;
    }
    if let Some((options, mut reading)) = options.and_then(|options| anchor.reading.clone().map(|reading| (options, reading))) {
        if reading.family().0 == "case" {
            reading.fields.source_case_name = source_name.cloned();
            if let Some(year) = source_name.and_then(|name| name.year.clone()) {
                reading.fields.year = Some(year);
            }
        } else if reading.source_captured() {
            reading.fields.source_case_name = Some(crate::SourceCaseName {
                full_span_start: source_core.start, ..Default::default()
            });
        }
        if let Some(source_span) = source_span {
            if let Some(name) = &mut reading.fields.source_case_name {
                name.reference_span = Some(span(text, source_span));
            }
        }
        crate::classify::apply(&mut citation, reading, options);
        // A work a government body wrote is a government document, and a link whose title
        // names an enactment or a treaty cites that enactment or treaty.
        let style = citation.style.as_ref().map_or("", |style| style.text.as_str());
        citation.authority = match citation.authority {
            Authority::Book | Authority::Journal if GOVERNMENT_AUTHOR.is_match(style).unwrap_or(false) =>
                Authority::GovernmentDocument,
            Authority::Webpage if LEGISLATIVE_TITLE.is_match(style).unwrap_or(false) =>
                if ["Treaty", "Agreement", "Convention", "Accord", "Covenant", "Protocol"].iter().any(|word| style.contains(word)) {
                    Authority::Treaty
                } else { Authority::Statute },
            authority => authority,
        };
        if let Some(suffix) = section_suffix {
            if let Some(section) = &mut citation.fields.section {
                section.push_str(&text[suffix]);
            }
        }
    }
    citation.fields.pin_cite = tail.pin_cite;
    citation.fields.pin_cite_kind = tail.pin_cite_kind;
    citation.fields.explicit_short_span = tail.short_span;
    // LSP citation_occurrences_in_text stops at the native occurrence fields.
    // Registry identity and source metadata belong to the full citation API.
    if options.is_none() { return citation; }
    citation.fields.extra = tail.extra;
    citation.fields.publisher = citation.fields.publisher.or(tail.publisher);
    if let Some(name) = citation.fields.source_case_name.as_mut().filter(|_| !short_form) {
        name.full_span_end = tail.source_end;
        name.parenthetical = tail.source_parenthetical;
        name.pin_cite_span_end = tail.source_pin_end;
        if name.pre_citation.is_none() { name.pin_cite = tail.source_pin; }
    }
    citation.parties = if let Some(name) = citation.fields.source_case_name.as_mut().filter(|_| citation.authority == Authority::Case) {
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
    let inline_reference = INLINE_REFERENCE.find(&text[core.clone()]).map(|matched| crate::InlineReference {
        span: span(text, core.start + matched.start()..core.start + matched.end()),
        note: INLINE_NOTE.captures(matched.as_str()).and_then(|captures|
            captures.name("note").and_then(|note| note.as_str().parse().ok())),
    });
    let tail = metadata::tail(
        text,
        core.end,
        limit,
        TailRules {
            core_end: inline_reference.as_ref().map(|reference| reference.span.end),
            oscola,
            ..TailRules::default()
        },
    );
    let mut citation = blank_citation(text, form, Authority::Unknown, core.clone(), "reference_grammar");
    citation.fields.note = note;
    citation.fields.inline_reference = inline_reference;
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
    if !tail.pinpoints.is_empty() || tail.pin_cite_kind.is_some() {
        citation.reasons.push("pinpoint_grammar".to_owned());
    }
    let source_pin = source_name.and_then(|name| name.pin_cite.as_ref());
    citation.full_span = span(text, start..source_pin.map_or(tail.end, |pin| tail.end.max(pin.end)));
    citation.pinpoints = tail.pinpoints;
    citation.parentheticals = tail.parentheticals;
    citation.fields.pin_cite = tail.pin_cite;
    citation.fields.pin_cite_kind = tail.pin_cite_kind;
    citation
}

/// Every core in document order: full anchors, back references, then bare
/// section symbols, never overlapping.
fn cores(text: &str, options: &Options, scopes: &[usize], enrich: bool) -> Vec<Core> {
    let Some(notes) = &options.notes else { return cores_in(text, options, scopes, enrich); };
    // ALR receives each note separately; a joined document must preserve those
    // input boundaries before competing discovery matches suppress each other.
    let mut boundaries = notes.iter().flat_map(|note| [note.start, note.end])
        .chain([0, text.len()]).collect::<Vec<_>>();
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries.windows(2).flat_map(|range| {
        let start = range[0];
        cores_in(&text[start..range[1]], options, &[], enrich).into_iter().map(move |mut core| {
            core.span = core.span.start + start..core.span.end + start;
            if let CoreKind::Full(anchor) = &mut core.kind {
                anchor.span = core.span.clone();
                anchor.preceding_text_end = anchor.preceding_text_end.map(|end| start + end);
                anchor.source_span = anchor.source_span.take().map(|span| span.start + start..span.end + start);
            }
            core
        })
    }).collect()
}

fn cores_in(text: &str, options: &Options, scopes: &[usize], enrich: bool) -> Vec<Core> {
    let mut cores = citation_anchors(text, options.extended_us, options.styles.as_deref(), scopes, enrich)
        .into_iter()
        .map(|anchor| Core {
            span: anchor.span.clone(),
            kind: CoreKind::Full(anchor),
        })
        .collect::<Vec<_>>();
    if !enrich { return cores; }
    let taken = cores
        .iter()
        .map(|core| core.span.clone())
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

/// Native citation occurrences, without source-only discoveries or identity and
/// name-reference metadata that the occurrence projection does not consume.
pub fn find_occurrences(text: &str, options: &Options) -> Vec<Citation> {
    discover(text, options, None, false)
}

/// Reference occurrences use the same discovery, written names and pinpoint
/// extents as citation occurrences. The inline marker remains a separate span.
pub fn find_references(text: &str) -> Vec<Citation> {
    find_styled(text, &Options::default(), None).into_iter()
        .filter(|citation| citation.fields.inline_reference.is_some()).collect()
}

pub(crate) fn find_styled(text: &str, options: &Options, markup: Option<&crate::clean::Markup<'_>>) -> Vec<Citation> {
    discover(text, options, markup, true)
}

fn discover(text: &str, options: &Options, markup: Option<&crate::clean::Markup<'_>>, enrich: bool) -> Vec<Citation> {
    let mut scopes = options.notes.iter().flatten()
        .flat_map(|note| [note.start, note.end]).collect::<Vec<_>>();
    scopes.sort_unstable();
    let cores = cores(text, options, &scopes, enrich);
    if cores.is_empty() { return Vec::new(); }
    let source_names = if enrich { crate::us::case_names(text, &cores.iter().filter_map(|core| match &core.kind {
        CoreKind::Full(anchor) if anchor.reading.as_ref().is_some_and(|reading| reading.source_captured()) =>
            Some((anchor.source_span.as_ref().unwrap_or(&core.span).clone(),
                anchor.reading.as_ref().filter(|reading| reading.short_at.is_some())
                    .map(|reading| reading.fields.source_groups.get("page").and_then(Option::as_deref).unwrap_or("")),
                anchor.preceding_text_end)),
        _ => None,
    }).collect::<Vec<_>>(), markup) } else { Default::default() };
    // Paragraph tokens inside a recognized citation are suppressed by the
    // source tokenizer's overlap rule; keep those newlines inside its token.
    let source_spans: Vec<_> = cores.iter().filter_map(|core| match &core.kind {
        CoreKind::Full(anchor) if anchor.reading.as_ref().is_some_and(|r| r.source_captured()) =>
            Some(anchor.source_span.as_ref().unwrap_or(&core.span)),
        _ => None,
    }).collect();
    let paragraphs: Vec<_> = text.match_indices('\n').map(|(at, _)| at).filter(|at| {
        let next = source_spans.partition_point(|span| span.end <= *at);
        source_spans.get(next).is_none_or(|span| *at < span.start)
    }).collect();
    let mut citations = Vec::with_capacity(cores.len());
    let mut floors = Vec::with_capacity(cores.len());
    let mut previous_end = 0;
    for (index, core) in cores.iter().enumerate() {
        let scope_end = if options.notes.is_some() {
            scopes.get(scopes.partition_point(|at| *at <= core.span.start)).copied().unwrap_or(text.len())
        } else { text.len() };
        let limit = cores.get(index + 1).map_or(text.len(), |next| next.span.start).min(scope_end);
        let floor = previous_end.min(core.span.start).max(scope_floor(&scopes, core.span.start));
        let source_start = match &core.kind {
            CoreKind::Full(anchor) => anchor.source_span.as_ref().map_or(core.span.start, |span| span.start),
            _ => core.span.start,
        };
        let source_name = source_names.get(&source_start);
        // A full case's trailing court/date may follow its parallels, never
        // a later citation with other text in between ("..., at para 162.
        // The ... R v Fontaine, [2004] 1 SCR 702": that bracketed year is
        // Fontaine's, not Stone's).
        let paragraph_end = paragraphs.get(paragraphs.partition_point(|at| *at < core.span.end)).copied()
            .unwrap_or(text.len()).min(scope_end);
        let reach = cores[index + 1..].iter().zip(&cores[index..])
            .find(|(next, previous)| next.span.start >= paragraph_end || !metadata::parallel_gap(text,
                previous.span.end.min(next.span.start), next.span.start))
            .map_or(paragraph_end, |(next, _)| next.span.start.min(paragraph_end));
        let mut citation = match &core.kind {
            CoreKind::Full(anchor) => full_citation(text, anchor, floor, limit, reach,
                source_name, enrich.then_some(options)),
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
                citation.fields.source_case_name = source_name.cloned();
                citation
            }
        };
        if enrich {
            if let Some(subject) = commented_work(text, &mut citation, &citations, &floors) {
                citations.truncate(subject);
                floors.truncate(subject);
            }
            citation.signal = signal(text, floor, citation.full_span.start);
        }
        // LSP's style floor follows the native pinpoint/short-form suffix;
        // source parentheticals retain their independent full extent. Every
        // citation is a floor, so a style never reaches back into a preceding
        // supra/ibid pinpoint ("supra note 3 at para 43. R v Lukacs").
        previous_end = citation.fields.explicit_short_span.as_ref().or(citation.fields.pin_cite.as_ref())
            .map_or(core.span.end, |suffix| suffix.end.max(core.span.end));
        citations.push(citation);
        floors.push(floor);
    }
    // Eyecite's IdToken/SupraToken discovery is independent of LSP's longer
    // note-reference core: a source token can survive an overlapping native
    // reporter extent (for example, a note number read as reporter volume).
    for (&start, source) in &source_names {
        let Some(form) = source.reference_form else { continue; };
        if cores.iter().any(|core| core.span.start == start) { continue; }
        let token = source.token_span.as_ref().expect("source reference token");
        let floor = scope_floor(&scopes, start);
        let limit = scopes.get(scopes.partition_point(|at| *at <= start)).copied().unwrap_or(text.len());
        let mut citation = back_citation(text, &(token.start..token.end),
            (form, None, false, false), Some(source), floor, limit);
        citation.fields.inline_reference = None;
        citations.push(citation);
    }
    let mut citations = join_links(text, join_legislation(text, citations));
    if enrich {
        let references = case_name_references(text, &citations, markup);
        citations.extend(references);
    }
    citations.sort_by_key(|citation| citation.span.start);
    for (index, citation) in citations.iter_mut().enumerate() {
        citation.index = index;
    }
    citations
}

/// A commentary on a decision or another work ("Kim Day, “A Missed Turn”, Case
/// Comment on R v Fir, 2030 BCCA 431 (2032) 48:2 Imag Law Rev 233") is one
/// citation: the commented decision, written between the commentary's title
/// and its publication block, is its subject, not an authority of its own. The
/// commentary is styled from its author and title; the position of its subject
/// (and any parallel citation of it) among `previous` is returned, so they can
/// be dropped.
fn commented_work(text: &str, citation: &mut Citation, previous: &[Citation], floors: &[usize]) -> Option<usize> {
    if citation.form != Form::Full || citation.style.is_some()
        || !matches!(citation.authority, Authority::Journal | Authority::Book | Authority::BookChapter) {
        return None;
    }
    let mut end = citation.span.start;
    for (at, subject) in previous.iter().enumerate().rev() {
        if subject.form != Form::Full || subject.full_span.end > end
            || !text[subject.full_span.end..end].chars().all(|character| character == ',' || javascript_whitespace(character)) {
            return None;
        }
        let floor = floors[at].min(subject.full_span.start);
        if let Some(lead) = COMMENTED_SUBJECT.find(&text[floor..subject.full_span.start]) {
            let lead = floor + lead.start();
            let start = work_style_start(text, lead, floor);
            if start >= lead {
                return None;
            }
            let style = span(text, start..trim_style_end(text, start, citation.span.start));
            citation.short_name = Some(style.text.trim_matches(|character: char|
                javascript_whitespace(character) || ",;:.".contains(character)).to_owned());
            citation.style = Some(style);
            citation.full_span = span(text, start..citation.full_span.end);
            citation.reasons.push("same_text_style".to_owned());
            return Some(at);
        }
        end = subject.full_span.start;
    }
    None
}

/// The citations a document's own list or table of authorities holds: after its heading, each
/// one written close after the last (its entries follow one another), up to the next heading of
/// the document's body ("PART I – FACTS", "I. INTRODUCTION") or a schedule after it. The list
/// ends where text runs on without a citation, as the body's prose does, so a line of a table of
/// contents naming the list holds none. A note is never part of the list, and the first note after
/// the heading ends it (a PDF's notes follow its page's text).
pub(crate) fn authority_list_citations(text: &str, citations: &[Citation], notes: &[crate::NoteRange]) -> Vec<usize> {
    const ENTRY_GAP: usize = 320;
    let mut listed = Vec::new();
    for heading in AUTHORITY_LIST.find_iter(text).flatten() {
        if notes.iter().any(|note| note.start <= heading.start() && heading.start() < note.end) { continue; }
        let note_start = notes.iter().map(|note| note.start).filter(|&start| start >= heading.end()).min();
        let limit = AUTHORITY_LIST_END.find_iter(&text[heading.end()..]).flatten().next()
            .map_or(text.len(), |found| heading.end() + found.start()).min(note_start.unwrap_or(text.len()));
        let mut inside = citations.iter().filter(|citation| heading.end() <= citation.full_span.start
            && citation.full_span.start < limit).collect::<Vec<_>>();
        inside.sort_by_key(|citation| citation.full_span.start);
        let mut cursor = heading.end();
        for citation in inside {
            if citation.full_span.start > cursor + ENTRY_GAP { break; }
            listed.push(citation.index);
            cursor = cursor.max(citation.full_span.end);
        }
    }
    listed
}

/// A link written after a citation and its pinpoint and introduced as where the
/// work is read ("…, s 4, online (pdf): [perma.cc/…]", "… at 12, online:
/// <https://…>") belongs to that citation, not to a source of its own.
fn join_links(text: &str, citations: Vec<Citation>) -> Vec<Citation> {
    let mut joined: Vec<Citation> = Vec::with_capacity(citations.len());
    for citation in citations {
        let link = citation.form == Form::Full && citation.authority == Authority::Webpage && citation.style.is_none();
        // A link written right after a citation, with only a comma between, is where it is read
        // ("Siler (Re), 2018 ABQB 465 (CanLII), <https://canlii.ca/t/hsjtk>").
        let bare = |gap: &str| gap.trim_matches(|character: char| javascript_whitespace(character) || character == ',').is_empty();
        let Some(previous) = joined.last_mut().filter(|previous| link && previous.form == Form::Full
            && previous.full_span.end <= citation.full_span.start
            && (bare(&text[previous.full_span.end..citation.full_span.start])
                || LINK_LEAD.find(&text[previous.full_span.end..citation.full_span.start]).ok().flatten()
                .is_some_and(|lead| lead.start() == 0 && previous.full_span.end + lead.end() == citation.full_span.start))) else {
            joined.push(citation);
            continue;
        };
        previous.full_span = span(text, previous.full_span.start..citation.full_span.end);
        previous.fields.url = previous.fields.url.take().or(citation.fields.url);
    }
    joined
}

/// A statute's title and the chapter citation written right after it are one
/// citation ("Constitution Act, 1867 (UK), 30 & 31 Vict, c 3"), and a reprint
/// written after a statute is part of it ("reprinted in RSC 1985, App II,
/// No 5"), not an authority of its own.
fn join_legislation(text: &str, citations: Vec<Citation>) -> Vec<Citation> {
    let mut joined: Vec<Citation> = Vec::with_capacity(citations.len());
    for mut citation in citations {
        let legislation = |citation: &Citation| citation.form == Form::Full && citation.authority.is_legislation();
        let Some(previous) = joined.last_mut()
            .filter(|previous| legislation(previous) && legislation(&citation)
                && previous.full_span.end <= citation.full_span.start) else {
            joined.push(citation);
            continue;
        };
        let gap = &text[previous.full_span.end..citation.full_span.start];
        if REPRINT.find(gap).ok().flatten().is_some() {
            let end = citation.full_span.end + REPRINT_LOCATION.find(&text[citation.full_span.end..])
                .ok().flatten().map_or(0, |found| found.end());
            previous.full_span = span(text, previous.full_span.start..end);
            continue;
        }
        let title_only = previous.fields.chapter.is_none() && previous.fields.series.is_none()
            && previous.pinpoints.is_empty();
        if title_only && citation.style.is_none() && citation.fields.chapter.is_some()
            && gap.trim_matches(javascript_whitespace) == "," {
            citation.style = Some(previous.style.clone().unwrap_or_else(|| previous.span.clone()));
            citation.full_span = span(text, previous.full_span.start..citation.full_span.end);
            citation.signal = citation.signal.take().or_else(|| previous.signal.take());
            let mut parentheticals = std::mem::take(&mut previous.parentheticals);
            parentheticals.append(&mut citation.parentheticals);
            citation.parentheticals = parentheticals;
            citation.short_name = citation.short_name.take().or_else(|| previous.short_name.take());
            if !citation.reasons.iter().any(|reason| reason == "same_text_style") {
                citation.reasons.push("same_text_style".to_owned());
            }
            *previous = citation;
            continue;
        }
        joined.push(citation);
    }
    joined
}

/// Eyecite helpers.filter_citations: source extents order references and
/// suppress a reference overlapping a citation, or supra overlapping a short.
/// Native occurrence adapters consume `find` directly, before this projection.
pub(crate) fn filter_citations(citations: Vec<Citation>) -> Vec<Citation> {
    let extent = |citation: &Citation, full| {
        let name = citation.fields.source_case_name.as_ref();
        if full {
            (name.map_or(citation.full_span.start, |name| name.full_span_start),
             name.and_then(|name| name.full_span_end).unwrap_or(citation.full_span.end))
        } else {
            let span = name.and_then(|name| name.reference_span.as_ref()).unwrap_or(&citation.span);
            (span.start, span.end)
        }
    };
    let mut by_span = std::collections::HashMap::new();
    let mut ordered = Vec::new();
    for citation in citations {
        let next = ordered.len();
        let index = *by_span.entry(extent(&citation, false)).or_insert(next);
        if index == next { ordered.push(citation); } else { ordered[index] = citation; }
    }
    ordered.sort_by_key(|citation| extent(citation, true));
    let mut filtered: Vec<Citation> = Vec::new();
    for citation in ordered {
        if let Some(previous) = filtered.last() {
            let (start, end) = extent(&citation, true);
            let (prior_start, prior_end) = extent(previous, true);
            if start.max(prior_start) < end.min(prior_end) {
                if previous.form == Form::Reference { filtered.pop(); }
                else if citation.form == Form::Reference
                    || (citation.form == Form::Supra && previous.form == Form::Short) { continue; }
            }
        }
        filtered.push(citation);
    }
    for (index, citation) in filtered.iter_mut().enumerate() { citation.index = index; }
    filtered
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
