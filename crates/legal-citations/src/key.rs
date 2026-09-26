//! Versioned identity keys for authorities.
//!
//! # Lookup key v2 (durable contract)
//!
//! A key identifies an *authority*, never a particular way of writing it: the
//! same decision, statute or regulation written in any supported variant
//! (English or French series, dotted or undotted abbreviations, reporter
//! variations known to the registry) yields the same key, and two different
//! authorities never share one. Pinpoints, parentheticals, styles of cause,
//! signals and history never enter a key. Databases are keyed on this string,
//! so its grammar below only ever changes together with [`KEY_VERSION`].
//!
//! ```text
//! key        = "2:" kind ":" component *( ":" component )
//! component  = 1*( a-z / 0-9 / "-" / "." )  |  "-"   ; "-" alone = absent
//! ```
//!
//! Component normalizers:
//!
//! * **fold** — NFKC, lowercase, keep only alphanumerics (`S.C.R.`, `S C R`
//!   and `SCR` → `scr`; `D.L.R. (4th)` → `dlr4th`). This is exactly
//!   [`crate::registry::fold`], the registry's own surface-matching form.
//! * **ident** — NFKC, lowercase; every dash (`‐‑‒–—―−`) becomes `-`; a `.`
//!   between two digits is kept; alphanumerics are kept; every other run of
//!   characters becomes a single `-`; leading and trailing `-` are removed
//!   (`C-46` → `c-46`, `S-22.6` → `s-22.6`, `1 (2nd Supp)` → `1-2nd-supp`).
//! * **num** — ident, then leading zeros are stripped from an all-digit value
//!   (`005` → `5`, `0` stays `0`).
//! * **year** — the value's digits; a two-digit regulation year is expanded
//!   with a fixed pivot (`00`–`49` → `20xx`, `50`–`99` → `19xx`).
//!
//! Registry ids (court, series) are used verbatim; they are stable by the
//! registry's own contract. When the registry does not know a court, reporter
//! or series, the written surface is folded instead.
//!
//! Kinds, decided by [`Citation::format`] (and [`Authority::Journal`]):
//!
//! | kind | shape | example |
//! |---|---|---|
//! | `neutral` | `2:neutral:{year}:{court}:{num}` | `2015 SCC 5`, `2015 CSC 5` → `2:neutral:2015:scc:5` |
//! | `reporter` | `2:reporter:{rep}:{num volume}:{num page}` | `26 DLR (4th) 200` → `2:reporter:dlr4th:26:200`; `410 U.S. 113` → `2:reporter:us:410:113` |
//! | `reporter` (year as volume) | `2:reporter:{rep}:{year}[:{num volume}]:{num page}` | `[2015] 1 SCR 331`, `[2015] 1 R.C.S. 331` → `2:reporter:scr:2015:1:331`; `[1932] AC 562` → `2:reporter:ac:1932:562` |
//! | `canlii` | `2:canlii:{year}:{num}` | `2004 CanLII 12345 (ON CA)` → `2:canlii:2004:12345` |
//! | `database` | `2:database:{db}:{year}:{num}` | `2019 CarswellOnt 123` → `2:database:carswellont:2019:123`; `[2019] OJ No 45` → `2:database:oj:2019:45` |
//! | `docket` | `2:docket:{court}:{ident docket}` | needs a registry court |
//! | `statute` | `2:statute:{jur}:{series}:{ident year}:{ident chapter}` | `RSC 1985, c C-46`, `LRC 1985, ch C-46` → `2:statute:ca:rsc:1985:c-46` |
//! | `regulation` | `2:regulation:{jur}:{series}:{year}:{ident number}` | `SOR/2002-227` → `2:regulation:ca:sor:2002:227`; `O Reg 191/11` → `2:regulation:ca-on:oreg:2011:191`; `CRC, c 870` → `2:regulation:ca:crc:-:870` |
//! | `code` | `2:code:{series}:{ident title}:{ident section}` | `42 U.S.C. § 1983` → `2:code:usc:42:1983` |
//! | `journal` | `2:journal:{journal}:{num volume}:{num page}` | `(2010) 55:3 McGill LJ 1` → `2:journal:mcgilllj:55:1` |
//!
//! Details:
//!
//! * `{court}` is the registry court id of [`Citation::court`], else of the
//!   neutral identifier ([`Fields::series`]) looked up in the registry, else
//!   the fold of that identifier.
//! * `{rep}` and `{db}` are the fold of the registry's canonical edition
//!   abbreviation for [`Fields::reporter_canonical`] or [`Fields::reporter`],
//!   else the fold of the written abbreviation. `{journal}` is the fold of the
//!   registry journal's abbreviation, else as `{rep}`. A
//!   database abbreviation drops a trailing `No` (`OJ No` → `oj`).
//! * The year is part of a reporter key exactly when the registry marks the
//!   reporter `year_volume`; for a reporter the registry does not know, when
//!   the citation opens with a bracketed year (`[1999] 2 XYZ 5`); and always
//!   when there is no volume number (`[1932] AC 562`). Otherwise a volume is
//!   required.
//! * A journal volume drops its issue (`55:3` and `55(3)` → `55`); a journal
//!   without a volume uses the year in its place.
//! * `{series}` is the registry series id for [`Fields::series`], else its
//!   fold, else `-` (titled acts cited by year and chapter only). `{jur}` is
//!   the registry series jurisdiction, else [`Citation::jurisdiction`], else
//!   `-`. A statute chapter drops a leading `c`/`ch`/`chap` label.
//! * A regulation number comes from [`Fields::regulation`] (else
//!   [`Fields::number`]), which may carry its year (`191/11`, `2002-227`). A
//!   chapter-numbered regulation (`CRC, c 870`; `RLRQ, c C-12, r 1`) is keyed
//!   by chapter (`870`; `c-12-r-1`) and its year component is always `-`,
//!   because consolidations are identified by chapter alone.
//! * A code section drops subdivisions (`1983(a)` → `1983`); a missing title is `-`.
//! * No key: non-[`Form::Full`] citations, books, webpages, bills, debates,
//!   papers, citations without a [`Format`], and any citation missing a
//!   required component. Keys are never guessed from free text.
//!
//! [`v1`] reproduces the retired v1 normalizer so stores can be migrated.

use crate::model::{Authority, Citation, Form, Format};
use crate::registry::{self, fold, Court, Journal, Registry, Reporter, Series};
use std::fmt;
use unicode_normalization::UnicodeNormalization;

pub const KEY_VERSION: &str = "2";

/// The v2 key of a full citation against the embedded registry.
pub fn key(citation: &Citation) -> Option<String> {
    key_in(citation, registry::registry())
}

/// The v2 key of a full citation against `registry`.
pub fn key_in(citation: &Citation, registry: &Registry) -> Option<String> {
    if citation.form != Form::Full {
        return None;
    }
    let parts = match citation.format? {
        Format::Neutral => neutral(citation, registry)?,
        Format::Reporter if citation.authority == Authority::Journal => journal(citation, registry)?,
        Format::Reporter => reporter(citation, registry)?,
        Format::Publication if citation.authority == Authority::Journal => {
            journal(citation, registry)?
        }
        Format::CanLii => vec![
            "canlii".into(),
            year(citation.fields.year.as_deref()?)?,
            num(citation.fields.number.as_deref().or(citation.fields.page.as_deref())?)?,
        ],
        Format::Database => database(citation, registry)?,
        Format::Docket => docket(citation, registry)?,
        Format::StatuteVolume => statute(citation, registry)?,
        Format::RegulationSeries => regulation(citation, registry)?,
        Format::Code => code(citation, registry)?,
        Format::Publication | Format::Url => return None,
    };
    Some(format!("{KEY_VERSION}:{}", parts.join(":")))
}

/// Why [`key_for_text`] could not produce a key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyError {
    /// No full citation was found.
    NoCitation,
    /// More than one full citation was found (a parallel pair counts as two).
    Multiple(usize),
    /// One citation was found but it has no stable identity (a book, a URL).
    NoIdentity,
}

impl fmt::Display for KeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCitation => formatter.write_str("no citation was found"),
            Self::Multiple(count) => write!(
                formatter,
                "citation must identify one citation form; {count} citations were found"
            ),
            Self::NoIdentity => formatter.write_str("the citation has no stable identity"),
        }
    }
}

impl std::error::Error for KeyError {}

/// The key of the single full citation in `text`. Replaces
/// `caselaw_citation_lookup_key`, which fell back to normalizing arbitrary text.
pub fn key_for_text(text: &str) -> Result<String, KeyError> {
    let options = crate::Options {
        resolve: false,
        parallel: false,
        extended_us: true,
        notes: None,
    };
    let citations = crate::extract(text, &options);
    let full = citations
        .iter()
        .filter(|citation| citation.form == Form::Full)
        .collect::<Vec<_>>();
    match full.as_slice() {
        [] => Err(KeyError::NoCitation),
        [only] => only
            .key
            .clone()
            .or_else(|| key(only))
            .ok_or(KeyError::NoIdentity),
        many => Err(KeyError::Multiple(many.len())),
    }
}

/// The retired v1 lookup key: NFKC, lowercase ASCII alphanumerics, with `.`,
/// `-` and `/` between digits spelled `dot`, `dash`, `slash`.
pub fn v1(value: &str) -> String {
    let mut characters = value.nfkc().peekable();
    let mut key = String::with_capacity(value.len());
    let mut previous_digit = false;
    while let Some(character) = characters.next() {
        let character = if matches!(character, '\u{2013}' | '\u{2014}') {
            '-'
        } else {
            character
        };
        if previous_digit
            && matches!(character, '.' | '-' | '/')
            && characters.peek().is_some_and(char::is_ascii_digit)
        {
            key.push_str(match character {
                '.' => "dot",
                '-' => "dash",
                _ => "slash",
            });
        } else {
            for character in character.to_lowercase() {
                if character == '\u{df}' {
                    key.push_str("ss");
                } else if character.is_ascii_alphanumeric() {
                    key.push(character);
                }
            }
        }
        previous_digit = character.is_ascii_digit();
    }
    key
}

// ---------------------------------------------------------------------------
// Kinds

fn neutral(citation: &Citation, registry: &Registry) -> Option<Vec<String>> {
    let fields = &citation.fields;
    let court = court_id(citation, registry)?;
    Some(vec![
        "neutral".into(),
        year(fields.year.as_deref()?)?,
        court,
        num(fields.number.as_deref().or(fields.page.as_deref())?)?,
    ])
}

fn court_id(citation: &Citation, registry: &Registry) -> Option<String> {
    if let Some(court) = &citation.court {
        if !court.id.is_empty() {
            return Some(court.id.clone());
        }
    }
    let series = citation.fields.series.as_deref()?;
    Some(match court_by_surface(registry, series) {
        Some(court) => court.id.clone(),
        None => nonempty(fold(series))?,
    })
}

/// The folded reporter component and whether the registry marks it year-as-volume
/// (`None` when the registry does not know the reporter).
pub(crate) fn reporter_component(
    citation: &Citation,
    registry: &Registry,
) -> Option<(String, Option<bool>)> {
    let fields = &citation.fields;
    let surfaces = [fields.reporter_canonical.as_deref(), fields.reporter.as_deref()];
    if citation.authority == Authority::Journal {
        if let Some(journal) = surfaces
            .iter()
            .flatten()
            .find_map(|surface| journal_by_surface(registry, surface))
        {
            return Some((nonempty(fold(&journal.abbreviation))?, None));
        }
    }
    let known = surfaces
        .iter()
        .flatten()
        .find_map(|surface| reporter_by_surface(registry, surface));
    match known {
        Some((reporter, canonical)) => Some((nonempty(fold(canonical))?, Some(reporter.year_volume))),
        None => {
            let written = fields.reporter_canonical.as_deref().or(fields.reporter.as_deref())?;
            Some((nonempty(fold(written))?, None))
        }
    }
}

fn reporter(citation: &Citation, registry: &Registry) -> Option<Vec<String>> {
    let fields = &citation.fields;
    let (component, year_volume) = reporter_component(citation, registry)?;
    let page = num(fields.page.as_deref()?)?;
    let volume = fields.volume.as_deref().and_then(num);
    let bracketed = citation.span.text.trim_start().starts_with('[');
    let with_year = year_volume.unwrap_or(bracketed) || volume.is_none();
    let mut parts = vec!["reporter".to_owned(), component];
    if with_year {
        parts.push(year(fields.year.as_deref()?)?);
        parts.extend(volume);
    } else {
        parts.push(volume?);
    }
    parts.push(page);
    Some(parts)
}

fn journal(citation: &Citation, registry: &Registry) -> Option<Vec<String>> {
    let fields = &citation.fields;
    let (component, _) = reporter_component(citation, registry)?;
    let page = num(fields.page.as_deref()?)?;
    let volume = fields
        .volume
        .as_deref()
        .map(|volume| volume.split([':', '(']).next().unwrap_or(volume))
        .and_then(num);
    let volume = match volume {
        Some(volume) => volume,
        None => year(fields.year.as_deref()?)?,
    };
    Some(vec!["journal".into(), component, volume, page])
}

fn database(citation: &Citation, registry: &Registry) -> Option<Vec<String>> {
    let fields = &citation.fields;
    let surface = fields
        .reporter_canonical
        .as_deref()
        .or(fields.reporter.as_deref())
        .or(fields.series.as_deref())?;
    let canonical = reporter_by_surface(registry, surface).map_or(surface, |(_, canonical)| canonical);
    let trimmed = canonical.trim().trim_end_matches('.');
    let trimmed = trimmed
        .strip_suffix(" No")
        .or_else(|| trimmed.strip_suffix(" no"))
        .or_else(|| trimmed.strip_suffix(" NO"))
        .unwrap_or(trimmed);
    Some(vec![
        "database".into(),
        nonempty(fold(trimmed))?,
        year(fields.year.as_deref()?)?,
        num(fields.number.as_deref().or(fields.page.as_deref())?)?,
    ])
}

fn docket(citation: &Citation, registry: &Registry) -> Option<Vec<String>> {
    let fields = &citation.fields;
    let court = citation
        .court
        .as_ref()
        .filter(|court| !court.id.is_empty())
        .map(|court| court.id.clone())
        .or_else(|| {
            fields
                .series
                .as_deref()
                .and_then(|series| court_by_surface(registry, series))
                .map(|court| court.id.clone())
        })?;
    let docket = fields.docket.as_deref().or(fields.number.as_deref())?;
    Some(vec!["docket".into(), court, nonempty(ident(docket))?])
}

fn series_parts(citation: &Citation, registry: &Registry) -> (String, String) {
    let written = citation.fields.series.as_deref();
    let known = written.and_then(|series| series_by_surface(registry, series));
    let series = match (known, written) {
        (Some(series), _) => series.id.clone(),
        (None, Some(written)) => nonempty(fold(written)).unwrap_or_else(|| "-".into()),
        (None, None) => "-".into(),
    };
    let jurisdiction = known
        .map(|series| series.jurisdiction.clone())
        .or_else(|| citation.jurisdiction.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "-".into());
    (jurisdiction, series)
}

fn strip_chapter_label(chapter: &str) -> &str {
    let trimmed = chapter.trim();
    let lower = trimmed.to_lowercase();
    for label in ["chapitre", "chapter", "chap", "ch", "c"] {
        if lower.starts_with(label) {
            let rest = &trimmed[label.len()..];
            // The label must be followed by a separator: `c C-46`, `ch. 5`.
            if rest.starts_with(['.', ' ', '\u{a0}']) {
                let rest = rest.trim_start_matches(['.', ' ', '\u{a0}']);
                if !rest.is_empty() {
                    return rest;
                }
            }
        }
    }
    trimmed
}

fn statute(citation: &Citation, registry: &Registry) -> Option<Vec<String>> {
    let fields = &citation.fields;
    let chapter = nonempty(ident(strip_chapter_label(fields.chapter.as_deref()?)))?;
    let (jurisdiction, series) = series_parts(citation, registry);
    let year = fields
        .year
        .as_deref()
        .or(fields.regnal.as_deref())
        .and_then(|value| nonempty(ident(value)))
        .unwrap_or_else(|| "-".into());
    Some(vec!["statute".into(), jurisdiction, series, year, chapter])
}

fn regulation(citation: &Citation, registry: &Registry) -> Option<Vec<String>> {
    let fields = &citation.fields;
    let (jurisdiction, series) = series_parts(citation, registry);
    let written = fields.regulation.as_deref().or(fields.number.as_deref());
    let (year_part, number) = if let Some(chapter) = fields.chapter.as_deref() {
        let mut number = nonempty(ident(strip_chapter_label(chapter)))?;
        if let Some(regulation) = written {
            let regulation = regulation
                .trim()
                .trim_start_matches(['r', 'R'])
                .trim_start_matches(['.', ' ']);
            number = format!("{number}-r-{}", nonempty(ident(regulation))?);
        }
        ("-".to_owned(), number)
    } else {
        let written = written?.trim();
        let written = written
            .strip_prefix("Reg")
            .or_else(|| written.strip_prefix("reg"))
            .map_or(written, |rest| rest.trim_start_matches(['.', ' ']));
        let (embedded_year, number) = split_regulation_number(written);
        let year = embedded_year
            .or(fields.year.as_deref())
            .and_then(regulation_year)
            .unwrap_or_else(|| "-".into());
        (year, num(number)?)
    };
    Some(vec!["regulation".into(), jurisdiction, series, year_part, number])
}

/// `191/11` → (`11`, `191`); `2002-227` → (`2002`, `227`); `227` → (None, `227`).
fn split_regulation_number(value: &str) -> (Option<&str>, &str) {
    let digits = |part: &str| !part.is_empty() && part.chars().all(|character| character.is_ascii_digit());
    if let Some((number, year)) = value.split_once('/') {
        let (number, year) = (number.trim(), year.trim());
        if digits(number) && digits(year) && matches!(year.len(), 2 | 4) {
            return (Some(year), number);
        }
    }
    for dash in ['-', '\u{2013}', '\u{2010}', '\u{2011}'] {
        if let Some((year, number)) = value.split_once(dash) {
            let (year, number) = (year.trim(), number.trim());
            if digits(number) && digits(year) && matches!(year.len(), 2 | 4) {
                return (Some(year), number);
            }
        }
    }
    (None, value)
}

fn regulation_year(value: &str) -> Option<String> {
    let digits = value.chars().filter(char::is_ascii_digit).collect::<String>();
    match digits.len() {
        4 => Some(digits),
        2 => {
            let two: u32 = digits.parse().ok()?;
            Some(format!("{}{digits}", if two < 50 { "20" } else { "19" }))
        }
        _ => None,
    }
}

fn code(citation: &Citation, registry: &Registry) -> Option<Vec<String>> {
    let fields = &citation.fields;
    let surface = fields.series.as_deref().or(fields.reporter.as_deref())?;
    let series = match series_by_surface(registry, surface) {
        Some(series) => series.id.clone(),
        None => nonempty(fold(surface))?,
    };
    let section = fields
        .section
        .as_deref()
        .or(fields.page.as_deref())
        .or_else(|| citation.pinpoints.first().map(|pinpoint| pinpoint.first.as_str()))?;
    let section = section
        .trim()
        .trim_start_matches(['§', ' ', '\u{a0}'])
        .split('(')
        .next()
        .unwrap_or_default();
    let title = fields
        .volume
        .as_deref()
        .and_then(|value| nonempty(ident(value)))
        .unwrap_or_else(|| "-".into());
    Some(vec!["code".into(), series, title, nonempty(ident(section))?])
}

// ---------------------------------------------------------------------------
// Normalizers

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

/// See the module documentation (`ident`).
pub fn ident(value: &str) -> String {
    let characters = value.nfkc().flat_map(char::to_lowercase).collect::<Vec<_>>();
    let mut output = String::with_capacity(characters.len());
    let mut pending_separator = false;
    for (position, &character) in characters.iter().enumerate() {
        let keep_dot = character == '.'
            && position > 0
            && characters[position - 1].is_ascii_digit()
            && characters.get(position + 1).is_some_and(char::is_ascii_digit);
        if character.is_alphanumeric() || keep_dot {
            if pending_separator && !output.is_empty() {
                output.push('-');
            }
            pending_separator = false;
            output.push(character);
        } else {
            // Dashes, spaces, parentheses and other punctuation all separate.
            pending_separator = true;
        }
    }
    output
}

fn num(value: &str) -> Option<String> {
    // A blank page (`___`) folds to nothing and is not an identity.
    let value = ident(value);
    if value.is_empty() {
        return None;
    }
    if value.chars().all(|character| character.is_ascii_digit()) {
        let trimmed = value.trim_start_matches('0');
        return Some(if trimmed.is_empty() { "0".into() } else { trimmed.into() });
    }
    Some(value)
}

fn year(value: &str) -> Option<String> {
    let digits = value.chars().filter(char::is_ascii_digit).collect::<String>();
    (digits.len() == 4).then_some(digits)
}

// ---------------------------------------------------------------------------
// Registry access that works on the embedded (indexed) registry and on a
// registry assembled by hand (tests, callers with their own tables).

fn indexed(registry: &Registry) -> bool {
    std::ptr::eq(registry, registry::registry())
}

pub(crate) fn court<'r>(registry: &'r Registry, id: &str) -> Option<&'r Court> {
    if indexed(registry) {
        registry.court(id)
    } else {
        registry.courts.iter().find(|court| court.id == id)
    }
}

pub(crate) fn court_by_surface<'r>(registry: &'r Registry, surface: &str) -> Option<&'r Court> {
    if indexed(registry) {
        return registry.court_by_surface(surface);
    }
    let folded = fold(surface);
    if folded.is_empty() {
        return None;
    }
    registry
        .courts
        .iter()
        .find(|court| court.neutral.iter().chain(&court.aliases).any(|value| fold(value) == folded))
}

pub(crate) fn reporter_by_surface<'r>(
    registry: &'r Registry,
    surface: &str,
) -> Option<(&'r Reporter, &'r str)> {
    if indexed(registry) {
        return registry.reporter_by_surface(surface);
    }
    let folded = fold(surface);
    if folded.is_empty() {
        return None;
    }
    for reporter in &registry.reporters {
        if let Some(edition) = reporter
            .editions
            .iter()
            .find(|edition| fold(&edition.abbreviation) == folded)
        {
            return Some((reporter, edition.abbreviation.as_str()));
        }
    }
    registry.reporters.iter().find_map(|reporter| {
        reporter
            .variations
            .iter()
            .find(|(written, _)| fold(written) == folded)
            .map(|(_, canonical)| (reporter, canonical.as_str()))
    })
}

pub(crate) fn journal_by_surface<'r>(registry: &'r Registry, surface: &str) -> Option<&'r Journal> {
    if indexed(registry) {
        return registry.journal_by_surface(surface);
    }
    let folded = fold(surface);
    if folded.is_empty() {
        return None;
    }
    registry.journals.iter().find(|journal| {
        std::iter::once(&journal.abbreviation)
            .chain(&journal.variations)
            .any(|value| fold(value) == folded)
    })
}

pub(crate) fn series_by_surface<'r>(registry: &'r Registry, surface: &str) -> Option<&'r Series> {
    if indexed(registry) {
        return registry.series_by_surface(surface);
    }
    let folded = fold(surface);
    if folded.is_empty() {
        return None;
    }
    registry.series.iter().find(|series| {
        std::iter::once(&series.abbreviation)
            .chain(&series.variations)
            .any(|value| fold(value) == folded)
    })
}
