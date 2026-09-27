//! Render citations, pinpoints and short forms in McGill style (Canadian Guide
//! to Uniform Legal Citation, 10th ed), English and French.
//!
//! What the apps rely on, in one place:
//!
//! * Pinpoints ([`pinpoint`], [`pinpoints`]): `at para 12`, `at paras 12-14`,
//!   `at paras 20, 23, 25`, `at 353`, `at 553, 559`, `s 7(2)`, `ss 7(2)-(4)`,
//!   `r 3`, `art 1457`, `n 4`. Pages are `at 353`, never `at p 353`.
//!   Consecutive locators collapse into ranges and a range or list states a
//!   shared section number once (legal-pinpointer `formatPinpoint`). French:
//!   `au para 12`, `aux paras 12-14`, `à la p 353`, `aux pp 353, 359`,
//!   `art 7(2)` for a section.
//! * Back references: [`ibid`] (`Ibid`, `Ibid at para 5`, `Ibid, s 7`) and
//!   [`supra`] (`Jordan, supra note 4 at para 5`).
//! * Styles of cause ([`case_name`], [`party`]): the Crown is `R`, the
//!   Attorney General is `(AG)` (`(PG)` in French), abbreviations lose their
//!   periods, `X, Re` is `Re X`, French uses `c` for `v`.
//! * Core citations ([`normalize_citation`], [`database_citation`]),
//!   whole first references ([`full`]) and journal articles ([`article`]).
//! * Tables of authorities: [`short_label`], [`toa_sort_key`], [`heading`]
//!   and the Word `TA \c` category ([`ta_category`]).

use crate::model::{Authority, Citation, Format, Pinpoint, PinpointKind};
use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

/// Eyecite's class-specific corrected-citation operations. The facade supplies
/// its mutable groups and metadata; no parsing or formatting occurs in Python.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub enum CorrectionKind { Plain, Resource, Case, ShortCase, Law, Journal }

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CorrectionMetadata {
    pub pin_cite: Option<String>,
    pub plaintiff: Option<String>,
    pub defendant: Option<String>,
    pub extra: Option<String>,
    pub court: Option<String>,
    pub year: Option<String>,
    pub month: Option<String>,
    pub day: Option<String>,
    pub publisher: Option<String>,
    pub parenthetical: Option<String>,
    pub antecedent_guess: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CitationCorrection {
    pub text: String,
    pub kind: CorrectionKind,
    pub style: Option<String>,
    pub jurisdiction: Option<String>,
    pub source_layout: Option<CorrectionLayout>,
    pub reporter: Option<String>,
    pub corrected_reporter: Option<String>,
    pub page: Option<String>,
    #[serde(default)]
    pub metadata: CorrectionMetadata,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CorrectionLayout {
    pub prefix: String,
    pub suffix: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CorrectedCitation {
    pub citation: String,
    pub full: String,
    pub page: Option<String>,
}

pub fn correct_citation(request: &CitationCorrection) -> CorrectedCitation {
    static PAGE_REPORTER: LazyLock<Regex> = LazyLock::new(||
        legal_grammar::compile_ecmascript_table_entry("format.eyecite.page-correction-reporter").unwrap());
    let mut citation = request.text.clone();
    let mut page = request.page.clone();
    if !matches!(request.kind, CorrectionKind::Plain) {
        if let (Some(written), Some(corrected)) = (&request.reporter, &request.corrected_reporter) {
            citation = citation.replace(written, corrected);
        }
        if request.reporter.as_deref().is_some_and(|reporter| !reporter.is_empty())
            && [&request.reporter, &request.corrected_reporter].into_iter()
                .flatten().any(|reporter| PAGE_REPORTER.is_match(reporter)) {
            page = page.map(|page| page.replace("[U]", "(U)").replace("[A]", "(A)"));
        }
        if let (Some(written), Some(corrected)) = (&request.page, &page) {
            if !corrected.is_empty() && corrected != written { citation = citation.replace(written, corrected); }
        }
    }
    // The facade also represents Commonwealth citation families. Their
    // existing written court/date layout must not acquire U.S. metadata
    // parentheticals (e.g. a redundant year after a neutral citation).
    if request.jurisdiction.as_deref().is_none_or(|place| place != "us" && !place.starts_with("us-")) {
        if let Some(layout) = &request.source_layout {
            let full = format!("{}{}{}", layout.prefix, citation, layout.suffix);
            return CorrectedCitation { citation, full, page };
        }
    }
    let m = &request.metadata;
    fn present(value: &Option<String>) -> Option<&str> {
        value.as_deref().filter(|value| !value.is_empty())
    }
    let mut full = String::new();
    if matches!(request.kind, CorrectionKind::Case) {
        if let Some(plaintiff) = present(&m.plaintiff) { full.push_str(&format!("{plaintiff} v. ")); }
        if let Some(defendant) = present(&m.defendant) { full.push_str(&format!("{defendant}, ")); }
        // The Commonwealth model retains Re/Reference/Ex parte styles without
        // inventing two-party metadata. Preserve that existing representation.
        if full.is_empty() {
            if let Some(style) = request.style.as_deref().filter(|style| crate::metadata::single_party(style)) {
                full.push_str(style);
                full.push_str(", ");
            }
        }
    } else if matches!(request.kind, CorrectionKind::ShortCase) {
        if let Some(name) = present(&m.antecedent_guess) { full.push_str(&format!("{name}, ")); }
    }
    full.push_str(&citation);
    if matches!(request.kind, CorrectionKind::Case | CorrectionKind::Law | CorrectionKind::Journal) {
        if let Some(pin) = present(&m.pin_cite) {
            if !matches!(request.kind, CorrectionKind::Law) { full.push_str(", "); }
            full.push_str(pin);
        }
        if matches!(request.kind, CorrectionKind::Case) {
            if let Some(extra) = present(&m.extra) { full.push_str(extra); }
        }
        let fields: &[&Option<String>] = match request.kind {
            CorrectionKind::Case => &[&m.court, &m.year],
            CorrectionKind::Law => &[&m.publisher, &m.month, &m.day, &m.year],
            _ => &[&m.year],
        };
        let date = fields.iter().filter_map(|value| present(value)).collect::<Vec<_>>().join(" ");
        if !date.is_empty() { full.push_str(&format!(" ({date})")); }
        if let Some(parenthetical) = present(&m.parenthetical) { full.push_str(&format!(" ({parenthetical})")); }
    }
    CorrectedCitation { citation, full, page }
}

/// Output language.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Language {
    #[default]
    En,
    Fr,
}

impl Language {
    /// `fr`, `fr-CA`, `FR` → French; anything else English.
    pub fn from_code(code: &str) -> Self {
        if code.trim().to_lowercase().starts_with("fr") {
            Self::Fr
        } else {
            Self::En
        }
    }
}

/// Rendering options.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Style {
    pub language: Language,
    /// Joins range endpoints: `-` by default; Beaver renders `\u{2013}`.
    pub range_dash: &'static str,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            language: Language::En,
            range_dash: "-",
        }
    }
}

impl Style {
    pub fn french() -> Self {
        Self {
            language: Language::Fr,
            ..Self::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Locators

struct Locator {
    root: String,
    root_parts: Vec<u64>,
    suffixes: Vec<String>,
}

/// `7(2)(a)` → root `7`, suffixes `2`, `a`. Page and paragraph numbers are
/// roots without suffixes; `xii` does not parse.
fn parse_locator(value: &str) -> Option<Locator> {
    let raw = value.trim();
    let root_end = raw
        .char_indices()
        .find(|&(position, character)| {
            !(character.is_ascii_digit()
                || (character == '.'
                    && position > 0
                    && raw[position + 1..].starts_with(|next: char| next.is_ascii_digit())))
        })
        .map_or(raw.len(), |(position, _)| position);
    let root = &raw[..root_end];
    if root.is_empty() || !root.starts_with(|character: char| character.is_ascii_digit()) {
        return None;
    }
    let mut suffixes = Vec::new();
    let mut tail = &raw[root_end..];
    while !tail.is_empty() {
        let inner = tail.strip_prefix('(')?;
        let close = inner.find(')')?;
        if close == 0 || inner[..close].contains('(') {
            return None;
        }
        suffixes.push(inner[..close].to_owned());
        tail = &inner[close + 1..];
    }
    Some(Locator {
        root: root.to_owned(),
        root_parts: root.split('.').map(|part| part.parse().unwrap_or(0)).collect(),
        suffixes,
    })
}

fn roman_number(value: &str) -> Option<u32> {
    let lower = value.to_lowercase();
    if lower.is_empty() || !lower.chars().all(|character| "ivxlc".contains(character)) {
        return None;
    }
    let digit = |character: char| match character {
        'i' => 1,
        'v' => 5,
        'x' => 10,
        'l' => 50,
        _ => 100,
    };
    let characters = lower.chars().collect::<Vec<_>>();
    let mut total: i64 = 0;
    for (position, &character) in characters.iter().enumerate() {
        let current = digit(character);
        let next = characters.get(position + 1).map_or(0, |&next| digit(next));
        total += if current < next { -current } else { current };
    }
    u32::try_from(total).ok()
}

fn alphabet_number(value: &str) -> Option<u32> {
    let lower = value.to_lowercase();
    if lower.is_empty() || !lower.chars().all(|character| character.is_ascii_lowercase()) {
        return None;
    }
    lower
        .bytes()
        .try_fold(0u32, |total, byte| total.checked_mul(26)?.checked_add(u32::from(byte - b'a' + 1)))
}

fn consecutive_atomic(left: &str, right: &str) -> bool {
    if let (Ok(left), Ok(right)) = (left.parse::<u64>(), right.parse::<u64>()) {
        return right == left + 1;
    }
    let both_roman = roman_number(left).is_some() && roman_number(right).is_some();
    if both_roman && (left.len() > 1 || right.len() > 1) {
        return roman_number(right) == roman_number(left).map(|value| value + 1);
    }
    match (alphabet_number(left), alphabet_number(right)) {
        (Some(left), Some(right)) => right == left + 1,
        _ => false,
    }
}

fn is_consecutive(left: &str, right: &str) -> bool {
    let (Some(left), Some(right)) = (parse_locator(left), parse_locator(right)) else {
        return false;
    };
    if left.suffixes.is_empty() && right.suffixes.is_empty() {
        if left.root_parts.len() != right.root_parts.len() {
            return false;
        }
        let prefix = left.root_parts.len() - 1;
        return left.root_parts[..prefix] == right.root_parts[..prefix]
            && right.root_parts[prefix] == left.root_parts[prefix] + 1;
    }
    if left.root != right.root || left.suffixes.len() != right.suffixes.len() {
        return false;
    }
    let prefix = left.suffixes.len() - 1;
    left.suffixes[..prefix] == right.suffixes[..prefix]
        && consecutive_atomic(&left.suffixes[prefix], &right.suffixes[prefix])
}

/// The end of a range with the shared root dropped: `7(2)`..`7(4)` → `(4)`.
fn shortened_range_end(left: &str, right: &str) -> String {
    let (Some(parsed_left), Some(parsed_right)) = (parse_locator(left), parse_locator(right)) else {
        return right.to_owned();
    };
    if parsed_left.root != parsed_right.root || parsed_right.suffixes.is_empty() {
        return right.to_owned();
    }
    let prefix = parsed_right.suffixes.len() - 1;
    if parsed_left.suffixes.len() < prefix || parsed_left.suffixes[..prefix] != parsed_right.suffixes[..prefix] {
        return right.to_owned();
    }
    format!("({})", parsed_right.suffixes[prefix])
}

/// A list states a shared section number once: `20(a), (b)(i)`.
fn shared_root_display(previous: &str, locator: &str) -> String {
    match (parse_locator(previous), parse_locator(locator)) {
        (Some(left), Some(right))
            if !left.suffixes.is_empty() && !right.suffixes.is_empty() && left.root == right.root =>
        {
            locator.trim()[right.root.len()..].to_owned()
        }
        _ => locator.to_owned(),
    }
}

fn normalize_space(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Collapse locators into McGill ranges and lists: `12, 13, 14, 20` →
/// `12-14, 20`; `7(2), 7(3), 7(4)` → `7(2)-(4)`. Each `(first, last)` item
/// is a single locator (`last` = `None`) or an explicit range.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct LocatorGroup {
    pub start: usize,
    pub end: usize,
    pub first: String,
    pub first_display: String,
    pub last: String,
    pub last_display: String,
}

fn locator_groups(items: &[(String, String)]) -> Vec<LocatorGroup> {
    let mut groups = Vec::new();
    let mut start = 0;
    while start < items.len() {
        let mut end = start;
        while end + 1 < items.len() && is_consecutive(&items[end].1, &items[end + 1].0) { end += 1; }
        let (first, last) = (&items[start].0, &items[end].1);
        groups.push(LocatorGroup {
            start, end, first: first.clone(), last: last.clone(),
            first_display: if start > 0 { shared_root_display(&items[start - 1].1, first) } else { first.clone() },
            last_display: if first != last { shortened_range_end(first, last) } else { last.clone() },
        });
        start = end + 1;
    }
    groups
}

fn display_groups(groups: &[LocatorGroup], dash: &str) -> String {
    groups.iter().map(|group| if group.first == group.last { group.first_display.clone() }
        else { format!("{}{dash}{}", group.first_display, group.last_display) }).collect::<Vec<_>>().join(", ")
}

pub fn collapse(items: &[(&str, Option<&str>)], dash: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    let items = items.iter().filter_map(|(first, last)| {
        let first = normalize_space(first);
        let last = last.map(normalize_space).filter(|last| !last.is_empty() && *last != first);
        if first.is_empty() || !seen.insert((first.clone(), last.clone())) { return None; }
        Some((first.clone(), last.unwrap_or(first)))
    }).collect::<Vec<_>>();
    display_groups(&locator_groups(&items), dash)
}

/// Pinpointer's formatter layout: links belong to the caller, while range
/// endpoints, shared roots and wording belong to the citation engine.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct PinpointLayout {
    pub plain: String,
    pub prefix: String,
    pub locators: String,
    pub groups: Vec<LocatorGroup>,
}

pub fn pinpoint_layout(kind: &str, values: &[String], full: bool) -> PinpointLayout {
    let mut seen = std::collections::HashSet::new();
    let items = values.iter().map(|value| normalize_space(value)).filter(|value| !value.is_empty() && seen.insert(value.clone()))
        .map(|value| (value.clone(), value)).collect::<Vec<_>>();
    let groups = locator_groups(&items);
    let locators = display_groups(&groups, "-");
    let prefix = if items.is_empty() { "" } else {
        match kind {
            "pilcrow" => "\u{b6} ", "silcrow" => "\u{a7} ",
            _ if !full => "",
            _ => prefix(match kind {
                "page" => PinpointKind::Page, "section" => PinpointKind::Section,
                "rule" => PinpointKind::Rule, "article" => PinpointKind::Article,
                _ => PinpointKind::Paragraph,
            }, items.len() != 1, Language::En),
        }
    }.to_owned();
    PinpointLayout { plain: format!("{prefix}{locators}"), prefix, locators, groups }
}

fn prefix(kind: PinpointKind, plural: bool, language: Language) -> &'static str {
    use PinpointKind::*;
    match (language, kind, plural) {
        (Language::En, Paragraph, false) => "at para ",
        (Language::En, Paragraph, true) => "at paras ",
        (Language::En, Page, _) => "at ",
        (Language::En, Section | Subsection, false) => "s ",
        (Language::En, Section | Subsection, true) => "ss ",
        (Language::Fr, Paragraph, false) => "au para ",
        (Language::Fr, Paragraph, true) => "aux paras ",
        (Language::Fr, Page, false) => "\u{e0} la p ",
        (Language::Fr, Page, true) => "aux pp ",
        (Language::Fr, Section | Subsection | Article, false) => "art ",
        (Language::Fr, Section | Subsection | Article, true) => "arts ",
        (_, Rule, false) => "r ",
        (_, Rule, true) => "rr ",
        (Language::En, Article, false) => "art ",
        (Language::En, Article, true) => "arts ",
        (Language::En, Schedule, false) => "Schedule ",
        (Language::En, Schedule, true) => "Schedules ",
        (Language::Fr, Schedule, false) => "annexe ",
        (Language::Fr, Schedule, true) => "annexes ",
        (_, Footnote, false) => "n ",
        (_, Footnote, true) => "nn ",
        (Language::En, Clause, false) => "cl ",
        (Language::En, Clause, true) => "cls ",
        (Language::Fr, Clause, false) => "al ",
        (Language::Fr, Clause, true) => "als ",
    }
}

/// The label for locators of one kind: `pinpoint(Paragraph, &[("12", Some("14"))])`
/// → `at paras 12-14`. Empty when nothing is left after trimming.
pub fn pinpoint(kind: PinpointKind, items: &[(&str, Option<&str>)], style: Style) -> String {
    let collapsed = collapse(items, style.range_dash);
    if collapsed.is_empty() {
        return String::new();
    }
    let distinct = items
        .iter()
        .filter(|(first, _)| !first.trim().is_empty())
        .map(|(first, _)| normalize_space(first))
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let plural = distinct > 1 || items.iter().any(|(first, last)| last.is_some_and(|last| normalize_space(last) != normalize_space(first) && !last.trim().is_empty()));
    format!("{}{collapsed}", prefix(kind, plural, style.language))
}

/// Labels for a citation's pinpoints, one label per run of the same kind,
/// joined by `, ` (`at para 12`, `s 7(2)`).
pub fn pinpoints(pins: &[Pinpoint], style: Style) -> String {
    let mut labels = Vec::new();
    let mut start = 0;
    while start < pins.len() {
        let kind = pins[start].kind;
        let mut end = start + 1;
        while end < pins.len() && pins[end].kind == kind {
            end += 1;
        }
        let items = pins[start..end]
            .iter()
            .map(|pin| (pin.first.as_str(), pin.last.as_deref()))
            .collect::<Vec<_>>();
        let label = pinpoint(kind, &items, style);
        if !label.is_empty() {
            labels.push(label);
        }
        start = end;
    }
    labels.join(", ")
}

/// Attach a pinpoint label to an authority: `X at para 5`, `X, s 7`.
pub fn attach(authority: &str, label: &str) -> String {
    let label = label.trim();
    if label.is_empty() {
        return authority.to_owned();
    }
    if authority.trim().is_empty() {
        return label.to_owned();
    }
    let connective = ["at ", "au ", "aux ", "\u{e0} la "]
        .iter()
        .any(|prefix| label.starts_with(prefix));
    if connective {
        format!("{} {label}", authority.trim_end())
    } else {
        format!("{}, {label}", authority.trim_end())
    }
}

/// `Ibid`, `Ibid at para 5`, `Ibid, s 7` (the same in French).
pub fn ibid(pinpoint_label: Option<&str>) -> String {
    attach("Ibid", pinpoint_label.unwrap_or_default())
}

/// `Jordan, supra note 4`, `Jordan, supra note 4 at para 5`.
pub fn supra(short: &str, note: u32, pinpoint_label: Option<&str>) -> String {
    attach(
        &format!("{}, supra note {note}", short.trim()),
        pinpoint_label.unwrap_or_default(),
    )
}

// ---------------------------------------------------------------------------
// Case names

static CROWN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:R\.?|Rex|Regina|Reginam|The (?:King|Queen)|(?:Her|His) Majesty(?: the)? (?:King|Queen)(?: in right of [^,]+)?|Sa Majest\u{e9} (?:la Reine|le Roi))$").unwrap()
});
static ET_AL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i),?\s+et al\.?$").unwrap());
static AG_BARE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\((?:the )?Attorney General(?: of)?\)").unwrap());
static AG_OF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\((?:the )?Attorney General (?:of|for) ([^)]+)\)").unwrap());
static AG_LEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:The )?Attorney General (?:of|for) ((?:the )?[A-Z][A-Za-z ]*?)(\s*\(.*)?$").unwrap()
});
static PG_PAREN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\(Procureur(?:e)? g\u{e9}n\u{e9}ral(?:e)?(?: (du|de la|de l'|des|de) ([^)]+))?\)").unwrap()
});
static PG_LEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:(?:Le|La) )?Procureur(?:e)? g\u{e9}n\u{e9}ral(?:e)? (?:du |de la |de l'|des |de )(.+?)(\s*\(.*)?$").unwrap()
});
static UPPER_INITIALS: LazyLock<legal_grammar::CompiledGrammar> =
    LazyLock::new(|| format_pattern("format.upper-initials"));
static SINGLE_INITIAL: LazyLock<legal_grammar::CompiledGrammar> =
    LazyLock::new(|| format_pattern("format.single-initial"));
static ABBREVIATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(Ltd|Lt\u{e9}e|Inc|Co|Corp|Cie|Assn|Assoc|Bros|Dept|Govt|Mfg|Intl|Ins|Mun|Twp|Ry|No|St|Ste|Mr|Mrs|Ms|Dr|Jr|Sr|Comm|Commn|Admin|Bd|Cty|Gen|Hosp|Prov|Reg|Soc|Univ|Ass)\.").unwrap()
});
static LOWER_INITIALS: LazyLock<legal_grammar::CompiledGrammar> =
    LazyLock::new(|| format_pattern("format.lower-initials"));
static VERSUS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+(?:v|vs|c|versus)\.?\s+").unwrap());
static MATTER_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.+?)(?:,\s*Re|\s+\(Re\))$").unwrap());
static MATTER_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:In\s+re|In\s+the\s+matter\s+of|Re:?)\s+(.+)$").unwrap());
static COURT_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s+\((?:S\.?C\.?C\.?|C\.?S\.?C\.?)\)\s*$").unwrap());

/// One party in McGill form (legal-pinpointer `mcgillParty`).
pub fn party(value: &str, language: Language) -> String {
    let party = normalize_space(value);
    if CROWN.is_match(&party) {
        return "R".into();
    }
    let ag = if language == Language::Fr { "PG" } else { "AG" };
    let mut party = ET_AL.replace(&party, "").into_owned();
    party = AG_BARE.replace_all(&party, format!("({ag})").as_str()).into_owned();
    party = AG_OF
        .replace_all(&party, |captures: &regex::Captures| format!("({ag} {})", &captures[1]))
        .into_owned();
    party = AG_LEAD
        .replace(&party, |captures: &regex::Captures| {
            format!(
                "{} ({ag}){}",
                captures[1].trim(),
                captures.get(2).map_or("", |rest| rest.as_str())
            )
        })
        .into_owned();
    party = PG_PAREN
        .replace_all(&party, |captures: &regex::Captures| match (captures.get(1), captures.get(2)) {
            (Some(article), Some(place)) => format!("(PG {} {})", article.as_str(), place.as_str()),
            _ => "(PG)".into(),
        })
        .into_owned();
    party = PG_LEAD
        .replace(&party, |captures: &regex::Captures| {
            format!(
                "{} (PG){}",
                captures[1].trim(),
                captures.get(2).map_or("", |rest| rest.as_str())
            )
        })
        .into_owned();
    party = UPPER_INITIALS
        .replace_all(&party, |captures: &legal_grammar::GrammarCaptures| captures[1].replace('.', ""))
        .into_owned();
    party = SINGLE_INITIAL.replace_all(&party, "${1}").into_owned();
    party = ABBREVIATION.replace_all(&party, "$1").into_owned();
    party = LOWER_INITIALS
        .replace_all(&party, |captures: &legal_grammar::GrammarCaptures| captures[1].replace('.', ""))
        .into_owned();
    normalize_space(&party)
}

/// A style of cause in McGill form (legal-pinpointer `cleanCaseName`):
/// `R. v. Jordan` → `R v Jordan`; `Her Majesty the Queen v. Smith` →
/// `R v Smith`; `Attorney General of Canada v. Bedford` → `Canada (AG) v
/// Bedford`; `Eurig Estate, Re` → `Re Eurig Estate`. French output uses `c`
/// and `(PG)`.
pub fn case_name(style: &str, language: Language) -> String {
    case_name_language(style, Some(language))
}

fn case_name_language(style: &str, language: Option<Language>) -> String {
    let name = normalize_space(style.trim().trim_end_matches([',', ';']));
    let name = COURT_SUFFIX.replace(&name, "").into_owned();
    let has_versus = VERSUS.is_match(&name);
    if !has_versus {
        if let Some(captures) = MATTER_SUFFIX.captures(&name) {
            return format!("Re {}", party(&captures[1], language.unwrap_or_default()));
        }
        if let Some(captures) = MATTER_PREFIX.captures(&name) {
            return format!("Re {}", party(&captures[1], language.unwrap_or_default()));
        }
        return party(&name, language.unwrap_or_default());
    }
    let mut output = String::new();
    let mut start = 0;
    for separator in VERSUS.find_iter(&name) {
        output.push_str(&party(&name[start..separator.start()], language.unwrap_or_default()));
        let versus = match language {
            Some(Language::Fr) => "c",
            Some(Language::En) => "v",
            None => separator.as_str().trim().trim_end_matches('.'),
        };
        output.push_str(&format!(" {versus} "));
        start = separator.end();
    }
    output.push_str(&party(&name[start..], language.unwrap_or_default()));
    output
}

fn is_crown_party(value: &str) -> bool {
    CROWN.is_match(value.trim())
}

/// The short label a later reference or a table uses: the explicit short
/// name, else for a case the non-Crown party (`R v Jordan` → `Jordan`) or the
/// first party, else the observed name, else the citation core.
pub fn short_label(citation: &Citation, language: Language) -> String {
    if let Some(explicit) = citation.explicit_short_name.as_deref().filter(|value| !value.trim().is_empty()) {
        return explicit.trim().to_owned();
    }
    if citation.authority == Authority::Case {
        if let Some(parties) = &citation.parties {
            let plaintiff = parties.plaintiff.as_deref().filter(|name| !name.is_empty());
            let defendant = parties.defendant.as_deref().filter(|name| !name.is_empty());
            let chosen = if plaintiff.is_some_and(is_crown_party) { defendant } else { plaintiff.or(defendant) };
            if let Some(chosen) = chosen { return party(chosen, language); }
        }
        if let Some(style) = citation.style.as_ref().map(|style| style.text.as_str()) {
            let name = case_name(style, language);
            let mut sides = name.split([' ']).collect::<Vec<_>>();
            if let Some(split) = sides.iter().position(|word| *word == "v" || *word == "c") {
                let (left, right) = sides.split_at_mut(split);
                let left = left.join(" ");
                let right = right[1..].join(" ");
                return if left == "R" { right } else { left };
            }
            return name;
        }
    }
    citation
        .short_name
        .as_deref()
        .or(citation.style.as_ref().map(|style| style.text.as_str()))
        .map(|value| value.trim().trim_end_matches(',').trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| normalize_citation(&citation.span.text))
}

fn format_pattern(id: &str) -> legal_grammar::CompiledGrammar {
    legal_grammar::compile_ecmascript_backtracking_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CaseHeading {
    pub name: String,
    pub citation: String,
}

/// Pinpointer's heading splitter; the provider removes its platform suffix first.
pub fn case_heading(heading: &str) -> CaseHeading {
    static START: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(|| format_pattern("format.heading-case-start"));
    static SEPARATOR: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(|| format_pattern("format.heading-case-separator"));
    static PREFIX: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(|| format_pattern("format.heading-case-prefix"));
    for separator in SEPARATOR.find_iter(heading).flatten() {
        let rest = &heading[separator.end()..];
        if START.find(rest).ok().flatten().is_some_and(|found| found.start() == 0) {
            return CaseHeading { name: heading[..separator.start()].trim().into(), citation: rest.into() };
        }
    }
    if let Some(parts) = PREFIX.captures(heading).ok().flatten() {
        return CaseHeading { name: parts[2].trim().into(), citation: parts[1].trim().into() };
    }
    CaseHeading { name: heading.into(), citation: String::new() }
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct DocumentCitation {
    pub document_type: String,
    pub title: String,
    pub citation: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct FormattedDocument {
    pub title: String,
    pub citation: String,
    pub plain: String,
}

/// Pinpointer's makeCitation. HTML escaping and italics stay in the presentation adapter.
pub fn document(input: &DocumentCitation) -> FormattedDocument {
    static LEGISLATION: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(|| format_pattern("format.heading-legislation-start"));
    static LOCATOR: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(|| format_pattern("format.legislation-locator-tail"));
    let mut title = if input.document_type == "case" {
        case_name_language(&case_heading(&input.title).name, None)
    } else { input.title.clone() };
    let mut citation = normalize_citation(&input.citation);
    if input.document_type == "legislation" {
        for (position, _) in title.match_indices(',') {
            let suffix = title[position + 1..].trim_start();
            if LEGISLATION.is_match(suffix).unwrap_or(false) {
                if citation.is_empty() { citation = normalize_citation(suffix); }
                title = title[..position].trim().into();
                break;
            }
        }
        citation = LOCATOR.replace(&citation, "").trim().into();
    }
    if !citation.is_empty() && normalize_citation(&title) == citation { title.clear(); }
    let plain = match (title.is_empty(), citation.is_empty()) {
        (_, true) => title.clone(), (true, _) => citation.clone(),
        _ => format!("{title}, {citation}"),
    };
    FormattedDocument { title, citation, plain }
}

// ---------------------------------------------------------------------------
// Citations

/// McGill abbreviations carry no periods: `[1964] S.C.R. 642` → `[1964] SCR
/// 642`, `(CanLII)` is dropped (legal-pinpointer `normalizeCitation`).
pub fn normalize_citation(value: &str) -> String {
    static LABEL: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(|| format_pattern("format.citation-canlii-label"));
    static PERIOD: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(|| format_pattern("format.citation-period"));
    let spaced = crate::text::normalize_javascript_whitespace(value);
    let without_label = LABEL.replace_all(&spaced, " ");
    crate::text::normalize_javascript_whitespace(&PERIOD.replace_all(&without_label, "${1}"))
}

/// A database citation names its service: `2019 CarswellOnt 123 (WL Can)`,
/// `[2019] OJ No 45 (QL)` (legal-pinpointer `databaseCitation`).
pub fn database_citation(value: &str) -> String {
    static CARSWELL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bCarswell[A-Za-z]+\s+\d+$").unwrap());
    static QUICKLAW: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\[(?:18|19|20)\d{2}\]\s+[A-Z][A-Za-z ]*\s+No\s+\d+$").unwrap());
    let citation = normalize_citation(value);
    if CARSWELL.is_match(&citation) {
        format!("{citation} (WL Can)")
    } else if QUICKLAW.is_match(&citation) {
        format!("{citation} (QL)")
    } else {
        citation
    }
}

/// A first reference in McGill form: normalized style, core and pinpoints
/// (`R v Jordan, 2016 SCC 27 at para 5`; `Criminal Code, RSC 1985, c C-46, s
/// 7`). Citations that are not cases or legislation keep their written text.
pub fn full(citation: &Citation, style: Style) -> String {
    let core = if citation.format == Some(Format::Database) {
        database_citation(&citation.span.text)
    } else {
        normalize_citation(&citation.span.text)
    };
    let name = citation
        .style
        .as_ref()
        .map(|span| span.text.trim().trim_end_matches([',', ';']).trim().to_owned())
        .filter(|name| !name.is_empty());
    let authority = match citation.authority {
        Authority::Case => match name {
            Some(name) => format!("{}, {core}", case_name(&name, style.language)),
            None => core,
        },
        authority if authority.is_legislation() => match name {
            Some(name) => format!("{}, {core}", normalize_space(&name)),
            None => core,
        },
        _ => return normalize_space(&citation.full_span.text),
    };
    attach(&authority, &pinpoints(&citation.pinpoints, style))
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CaseCitationRequest {
    pub texts: Vec<String>,
    pub language: Option<String>,
    pub fallback: String,
    pub provider: Option<String>,
}

/// Pinpointer's selection order over shared discovery records. Source segment
/// exclusions and stable ties are retained; no second citation parser runs here.
pub fn choose_case_citation(request: &CaseCitationRequest) -> String {
    static SEPARATOR: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(|| format_pattern("format.reporter-separator"));
    static EXCLUDED: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(|| format_pattern("format.not-reporter"));
    static PREFERENCE: LazyLock<Vec<legal_grammar::CompiledGrammar>> = LazyLock::new(||
        ["official", "fr", "en"].iter().map(|name| format_pattern(&format!("format.reporter-preference.{name}"))).collect());
    static TRANSLATIONS: LazyLock<std::collections::HashMap<String, std::collections::HashMap<String, String>>> = LazyLock::new(||
        serde_json::from_str(include_str!("../registry/neutral-translations.json")).expect("Pinpointer neutral translation preferences"));
    let language = Language::from_code(request.language.as_deref().unwrap_or("en"));
    let mut neutrals = Vec::new();
    let mut reporters = Vec::new();
    let mut canlii = Vec::new();
    let mut in_house = Vec::new();
    let options = crate::Options { resolve: false, parallel: false, ..crate::Options::default() };
    for text in request.texts.iter().chain(std::iter::once(&request.fallback)) {
        let citations = crate::extract(text, &options);
        let cases = citations.iter().filter(|citation| citation.form == crate::Form::Full && citation.authority == Authority::Case).collect::<Vec<_>>();
        for citation in &cases {
            let core = normalize_citation(&citation.span.text);
            match citation.format {
                Some(Format::Neutral) if citation.fields.series.as_deref().is_some_and(|surface|
                    crate::registry::registry().courts_by_surface(surface).iter().any(|court| court.canlii.is_some() || court.canlii_fr.is_some())) => neutrals.push(core),
                Some(Format::CanLii) => {
                    let mut core = format!("{} CanLII {}", citation.fields.year.as_deref().unwrap_or(""), citation.fields.number.as_deref().unwrap_or(""));
                    if let Some(court) = citation.parentheticals.iter().find(|part| part.kind == crate::ParentheticalKind::Court) {
                        core.push_str(&format!(" ({})", normalize_citation(&court.content)));
                    }
                    canlii.push(core);
                }
                Some(Format::Database) if core.to_lowercase().contains("carswell") || core.to_lowercase().contains(" no ") => in_house.push(core),
                _ => {}
            }
        }
        let mut start = 0;
        for end in SEPARATOR.find_iter(text).flatten().map(|found| (found.start(), found.end()))
            .chain(std::iter::once((text.len(), text.len()))) {
            let segment = normalize_citation(&text[start..end.0]);
            if !EXCLUDED.is_match(&segment).unwrap_or(false) {
                if let Some(citation) = cases.iter().filter(|citation| citation.format == Some(Format::Reporter)
                    && citation.span.start >= start && citation.span.start < end.0)
                    .min_by_key(|citation| (!citation.span.text.starts_with('['), citation.span.start)) {
                    // The source's numbered-reporter branch omits a preceding
                    // parenthesized decision year; the parsed core stays intact.
                    let core = &citation.span.text;
                    let displayed = if core.starts_with('(') {
                        core.split_once(')').map_or(core.as_str(), |(_, rest)| rest.trim_start_matches([',', ' ']))
                    } else { core.as_str() };
                    reporters.push(normalize_citation(displayed));
                }
            }
            start = end.1;
        }
    }
    if let Some(first) = neutrals.first() {
        let parts = first.split_whitespace().collect::<Vec<_>>();
        if let [year, code, number] = parts.as_slice() {
            let lang = if language == Language::Fr { "fr" } else { "en" };
            if let Some(equivalent) = TRANSLATIONS[lang].get(*code) {
                let translated = format!("{year} {equivalent} {number}");
                if neutrals.contains(&translated) { return translated; }
            }
        }
        return first.clone();
    }
    if let Some((_, reporter)) = reporters.iter().enumerate().min_by_key(|(index, reporter)| {
        let upper = reporter.to_uppercase();
        let score = 30 * i32::from(PREFERENCE[0].is_match(&upper).unwrap_or(false))
            + 5 * i32::from(PREFERENCE[if language == Language::Fr { 1 } else { 2 }].is_match(&upper).unwrap_or(false));
        (-score, *index)
    }) { return reporter.clone(); }
    if let Some(first) = canlii.first() { return first.clone(); }
    if let Some(first) = in_house.first() {
        let preferred = in_house.iter().find(|citation| match request.provider.as_deref() {
            Some("westlaw") => citation.to_lowercase().contains("carswell"),
            Some("lexis") => citation.to_lowercase().contains("no "),
            _ => false,
        }).unwrap_or(first);
        return database_citation(preferred);
    }
    database_citation(&request.fallback)
}

/// A journal article's parts.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct Article {
    pub authors: Vec<String>,
    pub title: String,
    pub year: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub journal: Option<String>,
    pub first_page: Option<String>,
    /// An online paper's own document citation, used when there is no journal.
    #[serde(rename = "docCitation")]
    pub document: Option<String>,
}

/// McGill 6.1: `Author, "Title" (Year) Volume:Issue Journal FirstPage`; more
/// than three authors become `First et al`; two or three are joined
/// `A, B & C` (legal-pinpointer `articleCitation`).
pub fn article(article: &Article) -> String {
    let authors = article
        .authors
        .iter()
        .map(|author| normalize_space(author))
        .filter(|author| !author.is_empty())
        .collect::<Vec<_>>();
    let byline = match authors.len() {
        0 => String::new(),
        1 => authors[0].clone(),
        2 | 3 => format!("{} & {}", authors[..authors.len() - 1].join(", "), authors[authors.len() - 1]),
        _ => format!("{} et al", authors[0]),
    };
    let clean = |value: &Option<String>| value.as_deref().map(normalize_space).filter(|value| !value.is_empty());
    let year = clean(&article.year).map(|year| year.chars().take(4).collect::<String>());
    let volume = [clean(&article.volume), clean(&article.issue)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(":");
    let tail = if article.journal.as_ref().is_some_and(|journal| !journal.is_empty()) {
        [Some(volume).filter(|value| !value.is_empty()), clean(&article.journal), clean(&article.first_page)]
            .into_iter().flatten().collect::<Vec<_>>().join(" ")
    } else { String::new() };
    let located = if !tail.is_empty() {
        format!("({}) {tail}", year.as_deref().unwrap_or_default())
    } else {
        [year.map(|year| format!("({year})")), article.document.clone().filter(|value| !value.is_empty())]
            .into_iter().flatten().collect::<Vec<_>>().join(", ")
    };
    let title = normalize_space(&article.title);
    let work = format!("\u{201c}{title}\u{201d} {located}").trim().to_owned();
    [byline, work]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------
// Tables of authorities

/// A sort key for a table of authorities: leading `R v`, `Re`, `Reference
/// re`, `The` (and French `R c`, `Renvoi relatif à`) are ignored; accents and
/// case fold away; digit runs compare numerically.
pub fn toa_sort_key(name: &str) -> String {
    static LEAD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:R\.?\s+(?:v|c)\.?\s+|Reference\s+re:?\s+|Renvoi\s+relatif\s+(?:\u{e0}|au|aux)\s+|In\s+re\s+|Re:?\s+|The\s+)").unwrap()
    });
    let mut value = normalize_space(name);
    loop {
        let stripped = LEAD.replace(&value, "").into_owned();
        if stripped == value || stripped.is_empty() {
            break;
        }
        value = stripped;
    }
    let folded = value
        .nfkd()
        .filter(|character| !unicode_normalization::char::is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect::<String>();
    let mut output = String::with_capacity(folded.len() + 16);
    for word in folded.split_whitespace() {
        if !output.is_empty() {
            output.push(' ');
        }
        let mut digits = String::new();
        for character in word.chars() {
            if character.is_ascii_digit() {
                digits.push(character);
            } else {
                flush_digits(&mut output, &mut digits);
                output.push(character);
            }
        }
        flush_digits(&mut output, &mut digits);
    }
    output
}

fn flush_digits(output: &mut String, digits: &mut String) {
    if digits.is_empty() {
        return;
    }
    let trimmed = digits.trim_start_matches('0');
    let trimmed = if trimmed.is_empty() { "0" } else { trimmed };
    // Length-prefixed so `9` < `10` < `100` under plain string comparison.
    output.push_str(&format!("{:02}{trimmed}", trimmed.len()));
    digits.clear();
}

/// Table-of-authorities headings.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Heading {
    Cases,
    Legislation,
    Regulations,
    Constitutional,
    Rules,
    Secondary,
    Government,
    Other,
}

impl Heading {
    pub fn label(self, language: Language) -> &'static str {
        match (self, language) {
            (Self::Cases, Language::En) => "Cases",
            (Self::Cases, Language::Fr) => "Jurisprudence",
            (Self::Legislation, Language::En) => "Legislation",
            (Self::Legislation, Language::Fr) => "L\u{e9}gislation",
            (Self::Regulations, Language::En) => "Regulations",
            (Self::Regulations, Language::Fr) => "R\u{e8}glements",
            (Self::Constitutional, Language::En) => "Constitutional documents",
            (Self::Constitutional, Language::Fr) => "Documents constitutionnels",
            (Self::Rules, Language::En) => "Rules",
            (Self::Rules, Language::Fr) => "R\u{e8}gles",
            (Self::Secondary, Language::En) => "Secondary sources",
            (Self::Secondary, Language::Fr) => "Doctrine",
            (Self::Government, Language::En) => "Parliamentary and government documents",
            (Self::Government, Language::Fr) => "Documents parlementaires et gouvernementaux",
            (Self::Other, Language::En) => "Other",
            (Self::Other, Language::Fr) => "Autres",
        }
    }

    /// The Word `TA \c` category: 1 cases, 2 statutes, 3 other, 4 rules,
    /// 5 treatises (secondary sources), 6 regulations, 7 constitutional.
    pub fn ta_category(self) -> u8 {
        match self {
            Self::Cases => 1,
            Self::Legislation => 2,
            Self::Other | Self::Government => 3,
            Self::Rules => 4,
            Self::Secondary => 5,
            Self::Regulations => 6,
            Self::Constitutional => 7,
        }
    }
}

/// The table heading an authority is listed under.
pub fn heading(authority: Authority) -> Heading {
    match authority {
        Authority::Case => Heading::Cases,
        Authority::Statute => Heading::Legislation,
        Authority::Regulation => Heading::Regulations,
        Authority::Constitution => Heading::Constitutional,
        Authority::CourtRule => Heading::Rules,
        Authority::Journal | Authority::Book | Authority::BookChapter | Authority::Webpage => Heading::Secondary,
        Authority::Bill | Authority::Debate | Authority::ParliamentaryPaper | Authority::GovernmentDocument => {
            Heading::Government
        }
        Authority::Treaty | Authority::Unknown => Heading::Other,
    }
}

/// The Word `TA \c` category for an authority (case 1, statute 2, other 3,
/// rules 4, secondary 5, regulations 6, constitutional 7).
pub fn ta_category(authority: Authority) -> u8 {
    heading(authority).ta_category()
}
