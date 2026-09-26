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
pub fn collapse(items: &[(&str, Option<&str>)], dash: &str) -> String {
    // (first, last) runs in order, duplicates removed.
    let mut runs: Vec<(String, String)> = Vec::new();
    let mut seen = Vec::<(String, Option<String>)>::new();
    for (first, last) in items {
        let first = normalize_space(first);
        let last = last.map(normalize_space).filter(|last| !last.is_empty() && *last != first);
        if first.is_empty() || seen.contains(&(first.clone(), last.clone())) {
            continue;
        }
        seen.push((first.clone(), last.clone()));
        let end = last.clone().unwrap_or_else(|| first.clone());
        match runs.last_mut() {
            Some((_, current)) if is_consecutive(current, &first) => *current = end,
            _ => runs.push((first, end)),
        }
    }
    let mut output = Vec::with_capacity(runs.len());
    let mut previous: Option<&str> = None;
    for (first, last) in &runs {
        let first_display = previous.map_or_else(|| first.clone(), |previous| shared_root_display(previous, first));
        if first == last {
            output.push(first_display);
        } else {
            output.push(format!("{first_display}{dash}{}", shortened_range_end(first, last)));
        }
        previous = Some(last);
    }
    output.join(", ")
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
static UPPER_INITIALS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b((?:[A-Z]\.){2,})([\s,)]|$)").unwrap());
static SINGLE_INITIAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([A-Z])\.(\s|[,)]|$)").unwrap());
static ABBREVIATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(Ltd|Lt\u{e9}e|Inc|Co|Corp|Cie|Assn|Assoc|Bros|Dept|Govt|Mfg|Intl|Ins|Mun|Twp|Ry|No|St|Ste|Mr|Mrs|Ms|Dr|Jr|Sr|Comm|Commn|Admin|Bd|Cty|Gen|Hosp|Prov|Reg|Soc|Univ|Ass)\.").unwrap()
});
static LOWER_INITIALS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b((?:[a-z]\.){2,})([\s,)]|$)").unwrap());
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
        .replace_all(&party, |captures: &regex::Captures| {
            format!("{}{}", captures[1].replace('.', ""), &captures[2])
        })
        .into_owned();
    party = SINGLE_INITIAL.replace_all(&party, "$1$2").into_owned();
    party = ABBREVIATION.replace_all(&party, "$1").into_owned();
    party = LOWER_INITIALS
        .replace_all(&party, |captures: &regex::Captures| {
            format!("{}{}", captures[1].replace('.', ""), &captures[2])
        })
        .into_owned();
    normalize_space(&party)
}

/// A style of cause in McGill form (legal-pinpointer `cleanCaseName`):
/// `R. v. Jordan` → `R v Jordan`; `Her Majesty the Queen v. Smith` →
/// `R v Smith`; `Attorney General of Canada v. Bedford` → `Canada (AG) v
/// Bedford`; `Eurig Estate, Re` → `Re Eurig Estate`. French output uses `c`
/// and `(PG)`.
pub fn case_name(style: &str, language: Language) -> String {
    let name = normalize_space(style.trim().trim_end_matches([',', ';']));
    let name = COURT_SUFFIX.replace(&name, "").into_owned();
    let versus = if language == Language::Fr { "c" } else { "v" };
    let has_versus = VERSUS.is_match(&name);
    if !has_versus {
        if let Some(captures) = MATTER_SUFFIX.captures(&name) {
            return format!("Re {}", party(&captures[1], language));
        }
        if let Some(captures) = MATTER_PREFIX.captures(&name) {
            return format!("Re {}", party(&captures[1], language));
        }
        return party(&name, language);
    }
    VERSUS
        .split(&name)
        .map(|value| party(value, language))
        .collect::<Vec<_>>()
        .join(&format!(" {versus} "))
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
            let chosen = if is_crown_party(&parties.plaintiff) {
                &parties.defendant
            } else {
                &parties.plaintiff
            };
            return party(chosen, language);
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

// ---------------------------------------------------------------------------
// Citations

/// McGill abbreviations carry no periods: `[1964] S.C.R. 642` → `[1964] SCR
/// 642`, `(CanLII)` is dropped (legal-pinpointer `normalizeCitation`).
pub fn normalize_citation(value: &str) -> String {
    let spaced = normalize_space(value);
    let without_canlii = spaced
        .replace(" (CanLII)", " ")
        .replace(" (canlii)", " ")
        .replace("(CanLII)", " ");
    let characters = without_canlii.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(without_canlii.len());
    let letter = |character: char| character.is_ascii_alphabetic() || ('\u{c0}'..='\u{ff}').contains(&character);
    for (position, &character) in characters.iter().enumerate() {
        if character == '.' && position > 0 && letter(characters[position - 1]) {
            let mut next = position + 1;
            while next < characters.len() && characters[next].is_whitespace() {
                next += 1;
            }
            let spaced = next > position + 1;
            let drop = match characters.get(next) {
                None => true,
                Some(&following) => letter(following) || following == '(' || (spaced && following.is_ascii_digit()),
            };
            if drop {
                continue;
            }
        }
        output.push(character);
    }
    normalize_space(&output)
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

/// A journal article's parts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Article {
    pub authors: Vec<String>,
    pub title: String,
    pub year: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub journal: Option<String>,
    pub first_page: Option<String>,
    /// An online paper's own document citation, used when there is no journal.
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
    let located = match clean(&article.journal) {
        Some(journal) => {
            let tail = [Some(volume).filter(|value| !value.is_empty()), Some(journal), clean(&article.first_page)]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            match &year {
                Some(year) => format!("({year}) {tail}"),
                None => tail,
            }
        }
        None => [year.map(|year| format!("({year})")), clean(&article.document)]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", "),
    };
    let title = normalize_space(&article.title);
    let work = if title.is_empty() {
        located
    } else {
        format!("\u{201c}{title}\u{201d} {located}").trim().to_owned()
    };
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
