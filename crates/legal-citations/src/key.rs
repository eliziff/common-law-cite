//! Versioned identity keys for authorities.
//!
//! # Lookup key v3 (durable contract)
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
//! key        = "3:" kind ":" component *( ":" component )
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
//! | `neutral` | `3:neutral:{year}:{court}:{num}` | `2015 SCC 5`, `2015 CSC 5` → `3:neutral:2015:scc:5` |
//! | `reporter` | `3:reporter:{reporter-id}:{rep}:{num volume}:{num page}` | `26 DLR (4th) 200` → `3:reporter:dlr:dlr4th:26:200`; `410 U.S. 113` → `3:reporter:us:us:410:113` |
//! | `reporter` (year as volume) | `3:reporter:{reporter-id}:{rep}:{year}[:{num volume}]:{num page}` | `[2015] 1 SCR 331`, `[2015] 1 R.C.S. 331` → `3:reporter:scr:scr:2015:1:331`; `[1932] AC 562` → `3:reporter:ac:ac:1932:562` |
//! | `canlii` | `3:canlii:{year}:{num}` | `2004 CanLII 12345 (ON CA)` → `3:canlii:2004:12345` |
//! | `database` | `3:database:{db}:{year}:{num}` | `2019 CarswellOnt 123` → `3:database:carswellont:2019:123`; `[2019] OJ No 45` → `3:database:oj:2019:45` |
//! | `docket` | `3:docket:{court}:{ident docket}` | needs a registry court |
//! | `statute` | `3:statute:{jur}:{series}:{ident year}:{ident chapter}` | `RSC 1985, c C-46`, `LRC 1985, ch C-46` → `3:statute:ca:rsc:1985:c-46` |
//! | `regulation` | `3:regulation:{jur}:{series}:{year}:{ident number}` | `SOR/2002-227` → `3:regulation:ca:sor:2002:227`; `O Reg 191/11` → `3:regulation:ca-on:oreg:2011:191`; `CRC, c 870` → `3:regulation:ca:crc:-:870` |
//! | `code` | `3:code:{series}:{ident title}:{ident section}` | `42 U.S.C. § 1983` → `3:code:usc:42:1983` |
//! | `journal` | `3:journal:{journal}:{num volume}:{num page}` | `(2010) 55:3 McGill LJ 1` → `3:journal:mcgilllj:55:1` |
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
//! * Reporter keys include the selected registry id and edition so identical
//!   abbreviations from different jurisdictions never share an identity.
//! * The year is part of a reporter key when its edition's numbering regime
//!   marks `year_volume`, falling back to the reporter's default; an ambiguous
//!   or missing year across numbering regimes produces no key. For a reporter
//!   the registry does not know, the year is used when
//!   the citation opens with a bracketed year (`[1999] 2 XYZ 5`). A
//!   continuously numbered reporter requires its volume.
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
//!   Additional captured identity fields (chapter, subject, act, issue, etc.)
//!   follow as named components. These are source groups, not trailing metadata.
//!   Resources identified by other fields use `-` for the absent section.
//! * No key: non-[`Form::Full`] citations, books, webpages, bills, debates,
//!   papers, citations without a [`Format`], and any citation missing a
//!   required component, or unresolved registry interpretations. Keys are never
//!   guessed from free text.

use crate::model::{Authority, Citation, Form, Format};
use crate::registry::{self, fold, Court, Journal, Registry, Reporter, Series};
use std::fmt;
use unicode_normalization::UnicodeNormalization;
use std::sync::LazyLock;

static CODE_SECTION: LazyLock<legal_grammar::CompiledEcmascriptGrammar> = LazyLock::new(|| {
    legal_grammar::compile_ecmascript_table_entry("cite.us.code.section").expect("code section grammar")
});

pub const KEY_VERSION: &str = "3";

/// The v3 key of a full citation against the embedded registry.
pub fn key(citation: &Citation) -> Option<String> {
    crate::aliases::resolve(citation).map(|target| target.key.clone())
        .or_else(|| key_in(citation, registry::registry()))
}

/// The structural v3 key against `registry`, before evidence-backed parallels.
pub fn key_in(citation: &Citation, registry: &Registry) -> Option<String> {
    if citation.form != Form::Full {
        return None;
    }
    if citation.is_ambiguous() {
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
        jurisdiction_priority: Vec::new(),
        ..Default::default()
    };
    let citations = crate::extract(text, &options);
    single_key(&citations)
}

pub(crate) fn single_key(citations: &[Citation]) -> Result<String, KeyError> {
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

/// The reporter id/edition component and whether its numbering uses the year
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
            .find_map(|surface| selected_journal(citation, registry, surface))
        {
            return Some((nonempty(fold(&journal.abbreviation))?, None));
        }
    }
    let known = selected_reporter(citation, registry);
    match known {
        Some((reporter, canonical)) => {
            let edition = reporter.editions.iter().find(|edition| edition.abbreviation == canonical)?;
            let year = fields.year.as_deref().and_then(|year| year.parse::<u16>().ok());
            let (year_volume, canonical) = if edition.numbering.is_empty() {
                (reporter.year_volume, canonical)
            } else {
                let policies: Vec<_> = edition.numbering.iter().filter(|period| {
                    year.is_none_or(|year| period.start.is_none_or(|start| year >= start)
                        && period.end.is_none_or(|end| year <= end))
                }).map(|period| (period.year_volume, period.edition.as_deref().unwrap_or(canonical))).collect();
                let first = *policies.first()?;
                if policies.iter().any(|policy| *policy != first) { return None; }
                first
            };
            Some((format!("{}:{}", reporter.id, nonempty(fold(canonical))?), Some(year_volume)))
        }
        None => {
            if fields.reporter_id.is_some() { return None; }
            let written = fields.reporter_canonical.as_deref().or(fields.reporter.as_deref())?;
            if !registry.reporters_by_surface(written).is_empty() { return None; }
            if citation.authority == Authority::Journal {
                return Some((nonempty(fold(written))?, None));
            }
            Some((format!("unknown:{}", nonempty(fold(written))?), None))
        }
    }
}

fn reporter(citation: &Citation, registry: &Registry) -> Option<Vec<String>> {
    let fields = &citation.fields;
    let (component, year_volume) = reporter_component(citation, registry)?;
    let page = num(fields.page.as_deref()?)?;
    let volume = fields.volume.as_deref().and_then(num);
    let bracketed = citation.span.text.trim_start().starts_with('[');
    let with_year = year_volume.unwrap_or(bracketed);
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
        // Source reporter grammars capture a database's citation year as
        // its volume. A parenthetical decision date is separate metadata.
        year(fields.volume.as_deref().or(fields.year.as_deref())?)?,
        num(fields.number.as_deref().or(fields.page.as_deref())?)?,
    ])
}

fn docket(citation: &Citation, registry: &Registry) -> Option<Vec<String>> {
    // A dated unreported decision is not the file itself: several decisions
    // can share a docket. Keep a document-local identity until its decision
    // identity can be represented without conflating those decisions.
    if citation.reasons.iter().any(|reason| reason == "unreported_grammar") { return None; }
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
    let known = written.and_then(|series| selected_series(citation, registry, series));
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
    let series = match selected_series(citation, registry, surface) {
        Some(series) => series.id.clone(),
        None => nonempty(fold(fields.source_edition.as_ref()
            .map_or(surface, |edition| edition.short_name.as_str())))?,
    };
    let section = fields
        .section
        .as_deref()
        .or(fields.page.as_deref())
        .or_else(|| fields.source_groups.is_empty().then(||
            citation.pinpoints.first().map(|pinpoint| pinpoint.first.as_str())).flatten());
    let section = match section {
        Some(section) => {
            let captures = CODE_SECTION.captures(section.trim().trim_start_matches(['§', ' ', '\u{a0}']))?;
            nonempty(ident(captures.name("section")?.as_str()))?
        }
        None if !fields.source_groups.is_empty() => "-".into(),
        None => return None,
    };
    let title = fields
        .volume
        .as_deref()
        .and_then(|value| nonempty(ident(value)))
        .unwrap_or_else(|| "-".into());
    let mut parts = vec!["code".into(), series, title, section];
    // Eyecite ResourceCitation identity retains the complete source groups.
    // Keep the shared title/section normalization, but do not discard the
    // remaining hierarchy or publication identifiers supplied by that grammar.
    for (name, value) in &fields.source_groups {
        if matches!(name.as_str(), "reporter" | "section")
            || matches!(name.as_str(), "title" | "volume") && fields.volume.is_some()
            || name == "page" && fields.section.is_none()
            // CFR's source `chapter` is already projected into the title slot.
            || name == "chapter" && fields.chapter.is_none()
        {
            continue;
        }
        if let Some(value) = value.as_deref().and_then(|value| nonempty(ident(value))) {
            parts.extend([ident(name), value]);
        }
    }
    (parts[2] != "-" || parts[3] != "-" || parts.len() > 4).then_some(parts)
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

/// Reuse classification's selected identity in keys, ranking and URLs. Never
/// perform a new first-match lookup that changes a jurisdictional choice.
pub(crate) fn selected_reporter<'r>(citation: &Citation, registry: &'r Registry) -> Option<(&'r Reporter, &'r str)> {
    let fields = &citation.fields;
    if let Some(id) = &fields.reporter_id {
        let reporter = registry.reporters.iter().find(|reporter| &reporter.id == id)?;
        let canonical = fields.reporter_canonical.as_deref()?;
        let edition = reporter.editions.iter().find(|edition| edition.abbreviation == canonical)?;
        return Some((reporter, &edition.abbreviation));
    }
    if citation.interpretations.iter().any(|reading| reading.kind == "reporter") { return None; }
    let surface = fields.reporter_canonical.as_deref().or(fields.reporter.as_deref())?;
    if indexed(registry) {
        let candidates = registry.reporters_by_surface(surface);
        let compatible: Vec<_> = candidates.into_iter().filter(|(reporter, _)| {
            citation.jurisdiction.as_deref().is_none_or(|jurisdiction| reporter.jurisdiction.as_deref().is_some_and(|candidate| {
                jurisdiction == candidate || jurisdiction.strip_prefix(candidate).is_some_and(|rest| rest.starts_with('-'))
            }))
        }).collect();
        return if let [only] = compatible.as_slice() { Some(*only) } else { None };
    }
    reporter_by_surface(registry, surface)
}

pub(crate) fn selected_journal<'r>(citation: &Citation, registry: &'r Registry, surface: &str) -> Option<&'r Journal> {
    if citation.is_ambiguous() { return None; }
    if let Some(chosen) = citation.interpretations.iter().find(|entry| entry.kind == "journal" && entry.selected) {
        return registry.journals.iter().find(|journal| journal.id == chosen.id);
    }
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

pub(crate) fn selected_series<'r>(citation: &Citation, registry: &'r Registry, surface: &str) -> Option<&'r Series> {
    if citation.is_ambiguous() { return None; }
    if let Some(chosen) = citation.interpretations.iter().find(|entry| entry.kind == "series" && entry.selected) {
        return registry.series.iter().find(|series| series.id == chosen.id);
    }
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
