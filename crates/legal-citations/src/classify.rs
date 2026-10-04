//! Decide [`Authority`](crate::Authority) and [`Format`](crate::Format) and parse
//! [`Fields`](crate::Fields) from the grammar's named groups.
//!
//! Discovery carries the recognized core and its parsed groups into classification;
//! the registry then supplies canonical
//! reporters, court ids, jurisdictions and languages. A surface the registry
//! does not know keeps its grammar reading, so an empty or partial registry
//! degrades to fewer ids, never to a different classification.

use crate::find::{DATABASE, JOURNAL_CUE, PARLIAMENTARY_COMMONWEALTH, TREATY};
use crate::model::{Authority, Citation, CourtRef, Fields, Format, PinpointKind};
use crate::registry::{fold, registry, Court, Journal, Reporter, ReporterKind, SeriesKind};
use legal_grammar::{CompiledEcmascriptGrammar, CompiledGrammar};
use regex::Captures;
use std::sync::LazyLock;

fn linear(id: &str) -> CompiledEcmascriptGrammar {
    legal_grammar::compile_ecmascript_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

fn backtracking(id: &str) -> CompiledGrammar {
    legal_grammar::compile_table_entry(id).unwrap_or_else(|error| panic!("{error}"))
}

static NEUTRAL: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.neutral"));
static TRIBUNAL: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.neutral.tribunal"));
static BRACKETED: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.neutral.bracketed"));
static PARENTHESIZED: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.neutral.parenthesized"));
static CANLII: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.canlii"));
static STATUTE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.ca.statute.first"));
static STATUTE_TITLED: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.statute.titled"));
pub(crate) static ROUTING: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.provider-routing"));
static JOURNAL_ARTICLE: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("cite.journal.article"));
static BOOK: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.book.imprint"));
static UNREPORTED: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.case.unreported"));
static COURT_FILE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.ca.court-file"));
static PARLIAMENTARY: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.parliamentary.paper"));
static FRENCH_REPORTER: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("reporter.language.fr"));
static INSTRUMENT_KIND: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("title.instrument-kind"));
static BOOK_CHAPTER: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("title.book-chapter"));
/// The French statute series of the shared series defs (`LRC`, `LC`, `RLRQ`).
static FRENCH_SERIES: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| {
    let tables = legal_grammar::load_tables().expect("grammar corpus");
    let defs = &tables["cite.ca.statute.first"].defs;
    legal_grammar::compile_ecmascript_pattern(
        "classify.fr-series",
        &format!("^(?:{}|DORS|TR|R[èe]gl(?:\\s.*)?)$", defs["fr_statute_series"]),
        "",
    )
    .expect("French series grammar")
});

/// A country preference also covers its provinces/states; unrelated ids never
/// match by an accidental string prefix.
fn within(jurisdiction: &str, parent: &str) -> bool {
    jurisdiction == parent || jurisdiction.strip_prefix(parent).is_some_and(|rest| rest.starts_with('-'))
}

fn priority(jurisdiction: Option<&str>, options: &crate::Options) -> usize {
    options.jurisdiction_priority.iter().position(|preferred| {
        jurisdiction.is_some_and(|jurisdiction| within(jurisdiction, preferred))
    }).unwrap_or(options.jurisdiction_priority.len())
}

/// Intersect written court readings. An empty intersection is contradictory
/// evidence, not absence of evidence; an ambiguous surface retains every court.
fn written_courts(citation: &Citation) -> impl Iterator<Item = String> + '_ {
    citation.fields.court_text.iter().cloned().chain(citation.parentheticals.iter()
        .filter(|part| part.kind == crate::ParentheticalKind::Court)
        .filter_map(|part| crate::metadata::read_court(&part.content).and_then(|reading| reading.court)))
}

type Score = (usize, bool, bool, bool, bool);

pub(crate) fn explicit_courts(citation: &Citation) -> Option<Vec<&'static Court>> {
    let mut found: Option<Vec<&Court>> = None;
    for surface in written_courts(citation) {
        let candidates = registry().courts_by_parenthetical(&surface);
        if candidates.is_empty() { continue; }
        if let Some(found) = &mut found {
            found.retain(|court| candidates.iter().any(|candidate| candidate.id == court.id));
        } else {
            found = Some(candidates);
        }
    }
    found
}

pub(crate) fn reporter_supports_court(reporter: &Reporter, court: &Court) -> bool {
    if !reporter.courts.is_empty() { return reporter.courts.contains(&court.id); }
    reporter.jurisdiction.as_deref().is_none_or(|jurisdiction| {
        within(&court.jurisdiction, jurisdiction) || within(jurisdiction, &court.jurisdiction)
    })
}

/// Select only a unique best reading. Equal-ranked meanings stay unresolved.
/// Scores exclude contradictory written courts before applying preferences.
fn select(
    citation: &mut Citation,
    mut readings: Vec<crate::Interpretation>,
    scores: Vec<Option<Score>>,
    context: Option<&str>,
) -> Option<usize> {
    let best = scores.iter().flatten().min().copied();
    let winners = scores.iter().enumerate().filter(|(_, score)| score.is_some() && **score == best)
        .map(|(at, _)| at).collect::<Vec<_>>();
    let chosen = if let [only] = winners.as_slice() { Some(*only) } else { None };
    let count = readings.len();
    for (at, reading) in readings.iter_mut().enumerate() {
        reading.selected = chosen == Some(at);
        reading.reason = if scores[at].is_none() { match context {
            Some("explicit_court") => "court_conflict",
            Some("observed_alias") => "observed_alias_conflict",
            Some("citation_grammar") => "citation_form_conflict",
            _ => "jurisdiction_conflict",
        } }
            else if !reading.selected { "alternative" }
            else if scores[at].is_some_and(|score|
                scores.iter().flatten().any(|other| other.0 > score.0)) { "jurisdiction_priority" }
            else if let Some(context) = context { context }
            else if count == 1 { "unique_match" }
            else if scores[at].is_some_and(|(_, outside, _, _, _)| !outside)
                && scores.iter().flatten().any(|(_, outside, _, _, _)| *outside) { "reporter_period" }
            else { "citation_form" }.to_owned();
    }
    if count > 1 || chosen.is_none() {
        citation.interpretations.extend(readings);
    }
    if count > 0 && chosen.is_none() {
        citation.reasons.push("ambiguous_registry".to_owned());
    }
    chosen
}

fn court_score(court: &Court, surface: &str, reading: &Reading, neutral: bool,
    explicit: Option<&[&Court]>, reporter: Option<&Reporter>, options: &crate::Options) -> Option<Score> {
    if explicit.is_some_and(|written| !written.iter().any(|candidate| candidate.id == court.id)) { return None; }
    if reporter.map_or_else(|| reading.jurisdiction.as_deref().is_some_and(|jurisdiction|
        !within(&court.jurisdiction, jurisdiction) && !within(jurisdiction, &court.jurisdiction)),
        |reporter| !reporter_supports_court(reporter, court)) { return None; }
    Some((priority(Some(&court.jurisdiction), options), false,
        neutral && !court.neutral.iter().any(|code| fold(code) == fold(surface)),
        neutral && within(&court.jurisdiction, "ca") == reading.bracketed, false))
}

fn court_for(surface: &str, reading: &Reading, neutral: bool, citation: &mut Citation, options: &crate::Options) -> Option<&'static Court> {
    let candidates = if neutral { registry().courts_by_surface(surface) } else {
        let mut candidates = Vec::new();
        for written in written_courts(citation) {
            for court in registry().courts_by_parenthetical(&written) {
                if !candidates.iter().any(|other: &&Court| other.id == court.id) { candidates.push(court); }
            }
        }
        candidates
    };
    let reporter = reading.fields.reporter_id.as_deref()
        .and_then(|id| registry().reporters.iter().find(|reporter| reporter.id == id));
    let explicit = explicit_courts(citation);
    let readings = candidates.iter().map(|court| crate::Interpretation {
        kind: "court".into(), id: court.id.clone(), canonical: court.neutral.first().cloned().unwrap_or_else(|| surface.to_owned()),
        jurisdiction: Some(court.jurisdiction.clone()), selected: false, reason: String::new(),
    }).collect();
    let scores = candidates.iter().map(|court| court_score(court, surface, reading, neutral,
        explicit.as_deref(), reporter, options)).collect();
    select(citation, readings, scores, explicit.as_ref().map(|_| "explicit_court")).map(|at| candidates[at])
}

fn source_match(canonical: &str, name: &str, editions: &[crate::SourceEdition]) -> bool {
    editions.iter().any(|edition| edition.short_name == canonical && edition.reporter.name == name)
}

fn reporter_candidates(surface: &str, fields: &Fields) -> Vec<(&'static Reporter, &'static str)> {
    let mut candidates = registry().reporters_by_surface(surface);
    for edition in fields.exact_editions.iter().chain(&fields.variation_editions) {
        for candidate in registry().reporters_by_surface(&edition.short_name) {
            if candidate.0.name.en == edition.reporter.name && !candidates.iter().any(|other|
                other.0.id == candidate.0.id && other.1 == candidate.1) {
                candidates.push(candidate);
            }
        }
    }
    candidates
}

fn journal_candidates(surface: &str, fields: &Fields) -> Vec<&'static Journal> {
    let mut candidates = registry().journals_by_surface(surface);
    for edition in fields.exact_editions.iter().chain(&fields.variation_editions) {
        for journal in registry().journals_by_surface(&edition.short_name) {
            if journal.name.as_deref() == Some(&edition.reporter.name)
                && !candidates.iter().any(|other| other.id == journal.id) {
                candidates.push(journal);
            }
        }
    }
    candidates
}

fn reporter_score(reporter: &Reporter, canonical: &str, surface: &str, open: Option<&str>, fields: &Fields,
    jurisdiction: Option<&str>, explicit: Option<&[&Court]>, observed: Option<&Court>, options: &crate::Options) -> Option<Score> {
    let source_exact = !fields.exact_editions.is_empty();
    let source_captured = source_exact || !fields.variation_editions.is_empty();
    let exact = source_match(canonical, &reporter.name.en, &fields.exact_editions);
    if source_exact && !exact { return None; }
    if !source_exact && source_captured && reporter.source == "reporters-db"
        && !source_match(canonical, &reporter.name.en, &fields.variation_editions) { return None; }
    if jurisdiction.zip(reporter.jurisdiction.as_deref()).is_some_and(|(written, candidate)|
        !within(written, candidate) && !within(candidate, written)) { return None; }
    if explicit.is_some_and(|courts| !courts.iter().any(|court| reporter_supports_court(reporter, court))) { return None; }
    if observed.is_some_and(|court| !reporter_supports_court(reporter, court)) { return None; }
    let jurisdiction = reporter.jurisdiction.as_deref().unwrap_or_default();
    let form_matches = match open {
        Some("(") => within(jurisdiction, "au"),
        Some(_) => !within(jurisdiction, "au") && !within(jurisdiction, "us"),
        None if surface.contains('.') => within(jurisdiction, "us"),
        None => !within(jurisdiction, "us"),
    };
    let outside_edition = fields.year.as_deref().and_then(|year| year.parse::<u16>().ok()).is_some_and(|year| {
        reporter.editions.iter().find(|edition| edition.abbreviation == canonical)
            .is_some_and(|edition| edition.start.is_some_and(|start| year < start)
                || edition.end.is_some_and(|end| year > end))
    });
    let variant = if !source_exact && reporter.source == "reporters-db" && source_captured { true }
        else { fold(canonical) != fold(surface) };
    Some((priority(reporter.jurisdiction.as_deref(), options), outside_edition, variant, !form_matches, !reporter.verified))
}

fn reporter_for(surface: &str, open: Option<&str>, fields: &Fields, jurisdiction: Option<&str>, citation: &mut Citation, options: &crate::Options) -> Option<(&'static Reporter, String)> {
    // Custom source grammars can recognize spellings beyond a literal alias.
    // Their edition records identify candidates without parsing the text again.
    let candidates = reporter_candidates(surface, fields);
    let source_captured = !fields.exact_editions.is_empty() || !fields.variation_editions.is_empty();
    let explicit = explicit_courts(citation);
    let observed = crate::aliases::observed_court(citation).filter(|court| {
        let supported = candidates.iter().any(|(reporter, _)| reporter_supports_court(reporter, court));
        if !supported { citation.reasons.push("observed_alias_conflict".to_owned()); }
        supported
    });
    let readings = candidates.iter().map(|(reporter, canonical)| crate::Interpretation {
        kind: "reporter".into(), id: reporter.id.clone(), canonical: (*canonical).to_owned(),
        jurisdiction: reporter.jurisdiction.clone(), selected: false, reason: String::new(),
    }).collect();
    let scores = candidates.iter().map(|(reporter, canonical)| reporter_score(reporter, canonical,
        surface, open, fields, jurisdiction, explicit.as_deref(), observed, options)).collect();
    select(citation, readings, scores, explicit.as_ref().map(|_| "explicit_court")
        .or(observed.map(|_| "observed_alias"))
        .or(source_captured.then_some("citation_grammar"))).map(|at| (candidates[at].0, candidates[at].1.to_owned()))
}

fn journal_score(entry: &Journal, fields: &Fields, jurisdiction: Option<&str>, options: &crate::Options) -> Option<Score> {
    let source_exact = !fields.exact_editions.is_empty();
    let source_captured = source_exact || !fields.variation_editions.is_empty();
    let name = entry.name.as_deref().unwrap_or("");
    let exact = source_match(&entry.abbreviation, name, &fields.exact_editions);
    if source_exact && !exact { return None; }
    if !source_exact && source_captured && entry.source == "reporters-db"
        && !source_match(&entry.abbreviation, name, &fields.variation_editions) { return None; }
    if jurisdiction.zip(entry.jurisdiction.as_deref()).is_some_and(|(written, candidate)|
        !within(written, candidate) && !within(candidate, written)) { return None; }
    Some((priority(entry.jurisdiction.as_deref(), options), false,
        !source_exact && source_captured && entry.source == "reporters-db", false, !entry.verified))
}

fn whole<'t>(pattern: &CompiledEcmascriptGrammar, core: &'t str) -> Option<Captures<'t>> {
    pattern
        .captures(core)
        .filter(|captures| captures.get(0).is_some_and(|matched| matched.start() == 0 && matched.end() == core.len()))
}

fn group(captures: &Captures<'_>, name: &str) -> Option<String> {
    captures
        .name(name)
        .map(|value| value.as_str().trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn fancy_group(captures: &legal_grammar::GrammarCaptures<'_>, name: &str) -> Option<String> {
    captures
        .name(name)
        .map(|value| value.as_str().trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// The last four-digit year in `value`.
fn last_year(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    (0..bytes.len().saturating_sub(3))
        .rev()
        .find(|&at| {
            bytes[at..at + 4].iter().all(u8::is_ascii_digit)
                && !bytes.get(at + 4).is_some_and(u8::is_ascii_digit)
                && (at == 0 || !bytes[at - 1].is_ascii_digit())
        })
        .map(|at| value[at..at + 4].to_owned())
}

/// What a core reads as.
#[derive(Clone, Debug, Default)]
pub(crate) struct Reading {
    authority: Option<Authority>,
    format: Option<Format>,
    pub(crate) fields: Fields,
    /// A written court surface; the registry establishes its identity.
    court: Option<String>,
    bracketed: bool,
    open: Option<String>,
    pub(crate) short_at: Option<usize>,
    language: Option<&'static str>,
    jurisdiction: Option<String>,
    reason: &'static str,
    /// Other families parsed from the same core during discovery.
    alternatives: Vec<Reading>,
}

impl Reading {
    pub(crate) fn family(&self) -> (&'static str, &'static str) {
        let kind = match self.authority {
            Some(Authority::Case) => "case", Some(Authority::Journal) => "journal",
            Some(Authority::Book | Authority::BookChapter) => "book",
            Some(Authority::Treaty) => "treaty",
            Some(Authority::ParliamentaryPaper | Authority::Debate) => "parliamentary",
            Some(authority) if authority.is_legislation() => "statute", _ => "other",
        };
        (kind, self.reason)
    }
    pub(crate) fn reported(&self) -> bool { self.fields.reporter.is_some() && self.fields.volume.is_some() }
    pub(crate) fn has_section(&self) -> bool { self.format == Some(Format::Code) && self.fields.section.is_some() }
    pub(crate) fn source_captured(&self) -> bool { !self.fields.source_groups.is_empty() }
    fn new(authority: Authority, format: Option<Format>, reason: &'static str) -> Self {
        Self {
            authority: Some(authority),
            format,
            reason,
            ..Self::default()
        }
    }
}

fn neutral(core: &str) -> Option<Reading> {
    for (entry, pattern, bracketed) in [
        ("cite.neutral.tribunal", &*TRIBUNAL, false),
        ("cite.neutral.bracketed", &*BRACKETED, true),
        ("cite.neutral", &*NEUTRAL, false),
        ("cite.neutral.parenthesized", &*PARENTHESIZED, false),
    ] {
        let Some(captures) = whole(pattern, core) else {
            continue;
        };
        let mut surface = group(&captures, "court")?;
        // A year written in parentheses, as a report's is, leaves only CanLII's
        // or a registered court's identifier a decision ("(2029) ABQB 812").
        if entry == "cite.neutral.parenthesized" {
            if surface == "CanLII" {
                return Some(canlii_reading(group(&captures, "year"), group(&captures, "num")));
            }
            if registry().courts_by_surface(&surface).is_empty() {
                return None;
            }
        }
        if let Some(division) = group(&captures, "division") {
            let divided = format!("{surface} {division}");
            if !registry().courts_by_surface(&divided).is_empty() {
                surface = divided;
            }
        }
        // A year followed by a known reporter (2001 DTC 5123) is not an
        // invented court identifier. Actual court/reporter collisions retain
        // the neutral grammar and are handled by their contextual readings.
        if registry().courts_by_surface(&surface).is_empty()
            && (registry().reporters_by_surface(&surface).iter().any(|(reporter, _)| reporter.verified)
                || registry().journals_by_surface(&surface).iter().any(|journal| journal.verified))
        {
            return None;
        }
        let canonical = legal_grammar::canonical_group(entry, "court", &surface);
        let written = legal_grammar::canonical_group(entry, "court", "")
            .map(|_| surface.to_lowercase().replace('.', ""));
        let mut reading = Reading::new(Authority::Case, Some(Format::Neutral), "neutral_grammar");
        // The grammar maps only the French identifiers onto their English
        // counterparts (CSC -> scc, Trib conc -> comp trib).
        if canonical.is_some() && canonical != written && entry != "cite.neutral.bracketed" {
            reading.language = Some("fr");
        }
        reading.fields.year = group(&captures, "year");
        reading.fields.number = group(&captures, "num");
        reading.fields.series = Some(surface.clone());
        reading.court = Some(surface);
        reading.bracketed = bracketed;
        return Some(reading);
    }
    None
}

fn canlii(core: &str) -> Option<Reading> {
    let captures = whole(&CANLII, core)?;
    Some(canlii_reading(group(&captures, "year"), group(&captures, "number")))
}

fn canlii_reading(year: Option<String>, number: Option<String>) -> Reading {
    let mut reading = Reading::new(Authority::Case, Some(Format::CanLii), "canlii_grammar");
    reading.fields.year = year;
    reading.fields.number = number;
    reading.fields.reporter = Some("CanLII".to_owned());
    reading
}

fn database(core: &str) -> Option<Reading> {
    let captures = whole(&DATABASE, core)?;
    let mut reading = Reading::new(Authority::Case, Some(Format::Database), "database_grammar");
    let fields = &mut reading.fields;
    if let Some(service) = group(&captures, "service") {
        fields.year = group(&captures, "year");
        fields.reporter = Some(service);
        fields.number = group(&captures, "number");
    } else if let Some(service) = group(&captures, "ql_service") {
        fields.year = group(&captures, "ql_year");
        fields.reporter = Some(service);
        fields.number = group(&captures, "ql_number");
        reading.bracketed = true;
    } else if let Some(service) = group(&captures, "soquij") {
        let number = group(&captures, "soquij_number")?;
        fields.year = number.get(..4).map(str::to_owned);
        fields.reporter = Some(service);
        fields.number = Some(number);
        reading.language = Some("fr");
    } else {
        fields.reporter = Some("AZ".to_owned());
        fields.number = group(&captures, "azimut_number");
        reading.language = Some("fr");
    }
    Some(reading)
}

fn treaty(core: &str) -> Option<Reading> {
    let captures = whole(&TREATY, core)?;
    let mut reading = Reading::new(Authority::Treaty, Some(Format::Publication), "treaty_grammar");
    let fields = &mut reading.fields;
    if let Some(series) = group(&captures, "series") {
        fields.volume = group(&captures, "volume");
        fields.page = group(&captures, "page");
        fields.series = Some(series);
    } else if let Some(series) = group(&captures, "ts_series") {
        fields.year = group(&captures, "ts_year");
        fields.number = group(&captures, "ts_number");
        fields.series = Some(series);
    } else {
        fields.series = group(&captures, "ets");
        fields.number = group(&captures, "ets_number");
    }
    let series = fields.series.as_deref().map(fold).unwrap_or_default();
    if matches!(series.as_str(), "rtnu" | "rtcan" | "ste" | "stce") {
        reading.language = Some("fr");
    }
    Some(reading)
}

fn unreported(core: &str) -> Option<Reading> {
    let captures = whole(&UNREPORTED, core)?;
    let mut reading = Reading::new(Authority::Case, Some(Format::Docket), "unreported_grammar");
    reading.fields.year = group(&captures, "year");
    reading.fields.month = group(&captures, "month");
    reading.fields.day = group(&captures, "day");
    reading.fields.place = group(&captures, "place");
    reading.fields.docket = group(&captures, "docket");
    reading.court = group(&captures, "court");
    Some(reading)
}

fn parliamentary(core: &str) -> Option<Reading> {
    if let Some(captures) = whole(&PARLIAMENTARY_COMMONWEALTH, core) {
        let debate = ["house", "au_record", "nz_record"]
            .iter()
            .any(|name| captures.name(name).is_some());
        let mut reading = if debate {
            Reading::new(Authority::Debate, Some(Format::Publication), "westminster_grammar")
        } else {
            Reading::new(Authority::ParliamentaryPaper, Some(Format::Publication), "westminster_grammar")
        };
        let fields = &mut reading.fields;
        fields.body = group(&captures, "house").or_else(|| group(&captures, "chamber")).or_else(|| group(&captures, "paper_house"));
        fields.volume = group(&captures, "volume").or_else(|| group(&captures, "nz_volume"));
        fields.page = group(&captures, "column").or_else(|| group(&captures, "nz_page"));
        fields.series = group(&captures, "command_series").or_else(|| group(&captures, "nz_record"));
        fields.number = group(&captures, "command_number").or_else(|| group(&captures, "paper_number"));
        fields.session = group(&captures, "paper_session");
        fields.year = group(&captures, "command_year")
            .or_else(|| group(&captures, "sitting").as_deref().and_then(last_year))
            .or_else(|| group(&captures, "au_sitting").as_deref().and_then(last_year));
        return Some(reading);
    }
    let captures = whole(&PARLIAMENTARY, core)?;
    if let Some(bill) = group(&captures, "bill") {
        let mut reading = Reading::new(Authority::Bill, Some(Format::Publication), "parliamentary_grammar");
        let number = bill
            .split(',')
            .next()
            .and_then(|head| head.split_whitespace().nth(1))
            .map(str::to_owned);
        reading.fields.bill = number;
        return Some(reading);
    }
    let mut reading = Reading::new(Authority::Debate, Some(Format::Publication), "parliamentary_grammar");
    let body = format!(
        "{}{}",
        captures.name("body").map_or("", |value| value.as_str()),
        captures.name("record").map_or("", |value| value.as_str())
    );
    reading.fields.body = Some(body.trim().to_owned());
    reading.fields.session = group(&captures, "session");
    reading.fields.year = group(&captures, "sitting").as_deref().and_then(last_year);
    Some(reading)
}

/// `[2016] 1 SCR 631`, `(1992) 175 CLR 1`, `410 U.S. 113`, `123 F.3d at 456`,
/// and the journals written the same way (`(2019) 97 Can Bar Rev 1`).
fn reported_from_parts(captures: &legal_grammar::GrammarCaptures<'_>, journal: bool) -> Option<Reading> {
    let fields = Fields {
        reporter: Some(fancy_group(&captures, "reporter")?),
        year: fancy_group(&captures, "year"),
        volume: fancy_group(&captures, "volume"),
        page: fancy_group(&captures, "page"),
        ..Fields::default()
    };
    Some(reported_fields(fields, journal, fancy_group(&captures, "open"),
        captures.name("at").map(|matched| matched.start())))
}

/// Classify fields captured by a reporter grammar without parsing its core again.
pub(crate) fn reported_fields(
    fields: Fields, journal: bool, open: Option<String>, short_at: Option<usize>,
) -> Reading {
    let surface = fields.reporter.as_deref().expect("reporter grammar capture");
    let known_reporter = registry()
        .reporters_by_surface(&surface)
        .iter().any(|(reporter, _)| reporter.verified);
    let known_journal = registry()
        .journals_by_surface(&surface)
        .iter().any(|journal| journal.verified);
    let is_journal = (journal && (!known_reporter || known_journal))
        || (known_journal && !known_reporter)
        || (!known_reporter && JOURNAL_CUE.is_match(&surface));
    let mut reading = if is_journal {
        Reading::new(Authority::Journal, Some(Format::Publication), "journal_grammar")
    } else {
        Reading::new(Authority::Case, Some(Format::Reporter), "reporter_grammar")
    };
    reading.short_at = short_at;
    reading.open = open;
    reading.bracketed = reading.open.as_deref() == Some("[");
    if FRENCH_REPORTER.is_match(surface) {
        reading.language = Some("fr");
    }
    let year_volume = registry().reporters_by_surface(surface);
    let year_volume = !year_volume.is_empty() && year_volume.iter().all(|(reporter, _)| reporter.year_volume);
    reading.fields = fields;
    reading.fields.page = reading.fields.page.filter(|page| !page.chars().all(|c| c == '_'));
    if reading.fields.year.is_none()
        && reading.fields.volume.as_deref().is_some_and(|volume| volume.len() == 4)
    {
        if year_volume {
            reading.fields.year = reading.fields.volume.take();
        }
    }
    reading
}

/// Use the fields and source family captured by the pinned U.S. extractor.
pub(crate) fn extracted(core: &str, mut fields: Fields, short_at: Option<usize>) -> Reading {
    let group = |name: &str| fields.source_groups.get(name).cloned().flatten();
    fields.reporter = group("reporter");
    fields.year = group("year");
    fields.volume = group("volume").or_else(|| group("title"));
    fields.page = group("page");
    fields.section = group("section");
    fields.chapter = group("chapter");
    fields.month = group("month");
    fields.day = group("day");
    fields.number = group("number").or_else(|| group("law"));
    fields.docket = group("docket_number");
    fields.regulation = group("reg").or_else(|| group("rule"));
    if fields.page.as_deref().is_some_and(|page| !page.is_empty() && page.chars().all(|c| c == '_')) {
        fields.source_groups.insert("page".to_owned(), None);
    }
    let editions = if fields.exact_editions.is_empty() { &fields.variation_editions } else { &fields.exact_editions };
    // Eyecite _extract_full_citation: reporters, then laws, then journals.
    let reporter = editions.iter().any(|edition| edition.reporter.source == "reporters");
    let law = !reporter && editions.iter().any(|edition| edition.reporter.source == "laws");
    let journal = !reporter && !law;
    if law {
        let regulation = editions.iter().all(|edition| edition.reporter.cite_type.starts_with("admin_"));
        // reporters-db labels CFR's title number `chapter`. Preserve that
        // original group, while retaining the engine's existing title field.
        if editions.iter().all(|edition| edition.short_name == "C.F.R.") {
            fields.volume = fields.chapter.take();
        }
        if editions.iter().all(|edition| edition.short_name == "Pub. L.") {
            fields.number = fields.volume.take();
        }
        fields.series = fields.reporter.take();
        let mut reading = Reading::new(if regulation { Authority::Regulation } else { Authority::Statute },
            Some(Format::Code), "code_grammar");
        reading.fields = fields;
        reading.short_at = short_at;
        return reading;
    }
    if fields.reporter.as_deref().is_some_and(|surface| !registry().courts_by_surface(surface).is_empty()) {
        if let Some(mut reading) = neutral(core) {
            reading.alternatives.push(reported_fields(fields.clone(), journal, None, short_at));
            reading.fields.source_groups = fields.source_groups;
            reading.fields.exact_editions = fields.exact_editions;
            reading.fields.variation_editions = fields.variation_editions;
            return reading;
        }
    }
    reported_fields(fields, journal, None, short_at)
}

fn journal_article(core: &str) -> Option<Reading> {
    let captures = JOURNAL_ARTICLE.captures(core).ok()??;
    let matched = captures.get(0)?;
    if matched.start() != 0 || matched.end() != core.len() {
        return None;
    }
    let mut reading = Reading::new(Authority::Journal, Some(Format::Publication), "article_grammar");
    let fields = &mut reading.fields;
    let first = |names: [&str; 3]| names.into_iter().find_map(|name| fancy_group(&captures, name));
    fields.year = first(["year", "unpaged_year", "open_year"]);
    fields.volume = first(["volume", "unpaged_volume", "open_volume"]);
    fields.reporter = first(["journal", "unpaged_journal", "open_journal"]);
    fields.page = fancy_group(&captures, "page");
    Some(reading)
}

fn book(core: &str, style: &str) -> Option<Reading> {
    let captures = whole(&BOOK, core)?;
    let chapter = BOOK_CHAPTER.is_match(style).unwrap_or(false);
    let mut reading = if chapter {
        Reading::new(Authority::BookChapter, Some(Format::Publication), "book_grammar")
    } else {
        Reading::new(Authority::Book, Some(Format::Publication), "book_grammar")
    };
    let fields = &mut reading.fields;
    fields.edition = group(&captures, "edition");
    fields.page = group(&captures, "first_page");
    fields.place = group(&captures, "place");
    let imprint = group(&captures, "imprint").or_else(|| group(&captures, "edition_imprint"))
        .or_else(|| group(&captures, "publisher_imprint"));
    if let Some(imprint) = imprint {
        fields.year = last_year(&imprint);
        let publisher = imprint
            .split(|character| character == ',' || character == ';')
            .map(str::trim)
            .find(|part| !part.is_empty() && !part.chars().all(|character| character.is_ascii_digit()))
            .map(str::to_owned);
        if fields.place.is_some() {
            fields.publisher = publisher;
        } else if let Some((place, publisher)) = imprint.split_once(':') {
            fields.place = Some(place.trim().to_owned());
            fields.publisher = publisher.split(',').next().map(|value| value.trim().to_owned());
        } else {
            fields.publisher = publisher;
        }
    }
    Some(reading)
}

/// The Canadian statute and regulation citation shapes.
fn legislation(core: &str) -> Option<Reading> {
    if let Some(captures) = whole(&STATUTE, core) {
        let mut reading = Reading::new(Authority::Statute, Some(Format::StatuteVolume), "statute_grammar");
        let fields = &mut reading.fields;
        if let Some(series) = group(&captures, "series") {
            fields.series = Some(series);
            fields.year = group(&captures, "year");
            fields.chapter = group(&captures, "chapter");
            fields.schedule = group(&captures, "schedule");
        } else if let Some(series) = group(&captures, "compiled_series") {
            fields.series = Some(series);
            fields.chapter = group(&captures, "compiled_chapter");
        } else if let Some(regnal) = group(&captures, "regnal") {
            fields.regnal = Some(regnal);
            fields.chapter = group(&captures, "regnal_chapter");
        } else {
            reading.authority = Some(Authority::Regulation);
            reading.format = Some(Format::RegulationSeries);
            if let Some(instrument) = group(&captures, "instrument") {
                let (series, number) = instrument.split_once('/')?;
                fields.series = Some(series.to_owned());
                fields.regulation = Some(number.to_owned());
                reading.jurisdiction = Some("ca".to_owned());
            } else if let Some(chapter) = group(&captures, "consolidated_chapter") {
                fields.series = Some("CRC".to_owned());
                fields.chapter = Some(chapter);
            } else if let Some(series) = group(&captures, "rr_series") {
                fields.series = Some(series);
                fields.year = group(&captures, "rr_year");
                fields.regulation = group(&captures, "rr_number");
            } else if let Some(regulation) = group(&captures, "fr_regulation") {
                let split = regulation.rfind(char::is_whitespace)?;
                fields.series = Some(regulation[..split].trim().to_owned());
                fields.regulation = Some(regulation[split..].trim().to_owned());
            } else {
                let regulation = group(&captures, "regulation")?;
                let split = regulation
                    .char_indices()
                    .find(|(_, character)| character.is_ascii_digit())
                    .map_or(0, |(at, _)| at);
                fields.series = Some(regulation[..split].trim().trim_end_matches('.').to_owned());
                fields.regulation = Some(regulation[split..].trim().to_owned());
            }
        }
        if fields.series.as_deref().is_some_and(|series| FRENCH_SERIES.is_match(series)) {
            reading.language = Some("fr");
        }
        return Some(reading);
    }
    if let Some(captures) = whole(&STATUTE_TITLED, core) {
        let mut reading = Reading::new(Authority::Statute, None, "titled_statute_grammar");
        reading.fields.year = group(&captures, "year").or_else(|| group(&captures, "title_year"));
        return Some(reading);
    }
    let captures = whole(&ROUTING, core).filter(|captures| captures.name("ca_statute").is_some())?;
    let statute = group(&captures, "ca_statute")?;
    let (series, year) = statute.rsplit_once(char::is_whitespace)?;
    let mut reading = Reading::new(Authority::Statute, Some(Format::StatuteVolume), "statute_grammar");
    reading.fields.series = Some(series.trim().to_owned());
    reading.fields.year = Some(year.to_owned());
    Some(reading)
}

/// Constitution, court rules, treaty or regulation, as the title names it.
fn instrument_kind(title: &str, reading: &mut Reading) {
    let Some(captures) = INSTRUMENT_KIND.captures(title.trim()).ok().flatten() else {
        return;
    };
    let statute_volume = reading.format == Some(Format::StatuteVolume);
    if captures.name("constitution").is_some() {
        reading.authority = Some(Authority::Constitution);
    } else if captures.name("court_rule").is_some() {
        reading.authority = Some(Authority::CourtRule);
    } else if captures.name("treaty").is_some() && !statute_volume && reading.format.is_none() {
        reading.authority = Some(Authority::Treaty);
    } else if captures.name("regulation").is_some() && reading.format.is_none() {
        reading.authority = Some(Authority::Regulation);
    }
}

pub(crate) fn read(core: &str, reason: &str, style: &str) -> Option<Reading> {
    // Preserve verified reporter and journal forms before generic families.
    let parts = crate::find::reporter_parts(core);
    let verified = parts.as_ref().and_then(|captures| captures.name("reporter")).is_some_and(|surface| {
        registry().reporters_by_surface(surface.as_str()).iter().any(|(reporter, _)| reporter.verified)
            || registry().journals_by_surface(surface.as_str()).iter().any(|journal| journal.verified)
    });
    let publication = |journal| parts.as_ref().and_then(|captures| reported_from_parts(captures, journal));
    let neutral_with_publication = |mut reading: Reading| {
        if reading.court.as_deref().is_some_and(|surface|
            !registry().reporters_by_surface(surface).is_empty()
                || !registry().journals_by_surface(surface).is_empty()) {
            reading.alternatives.extend(publication(false));
        }
        reading
    };
    if verified {
        if let Some(reading) = neutral(core) { return Some(neutral_with_publication(reading)); }
        if let Some(reading) = publication(reason == "journal_grammar") { return Some(reading); }
    }
    // The Guide forms the finder's own grammars name keep the family their grammar read.
    const NAMED: [(&str, Authority); 24] = [
        ("manuscript_grammar", Authority::Journal), ("paper_grammar", Authority::Journal),
        ("dated_work_grammar", Authority::Journal), ("news_grammar", Authority::Journal),
        ("foreign_doctrine_grammar", Authority::Journal), ("thesis_grammar", Authority::Book),
        ("encyclopedia_grammar", Authority::Book), ("encyclopedia_fascicle_grammar", Authority::Book),
        ("dictionary_grammar", Authority::Book), ("coursepack_grammar", Authority::Book), ("book_edition_grammar", Authority::Book),
        ("religious_text_grammar", Authority::Book), ("international_grammar", Authority::GovernmentDocument),
        ("intellectual_property_grammar", Authority::GovernmentDocument), ("report_grammar", Authority::GovernmentDocument),
        ("international_case_grammar", Authority::Case), ("foreign_reporter_grammar", Authority::Case),
        ("foreign_court_grammar", Authority::Case), ("code_grammar", Authority::Statute),
        ("bylaw_grammar", Authority::Statute), ("foreign_statute_grammar", Authority::Statute),
        ("title_year_statute_grammar", Authority::Statute), ("securities_grammar", Authority::Regulation),
        ("court_rules_grammar", Authority::CourtRule),
    ];
    const MORE: [(&str, Authority); 4] = [("constitution_grammar", Authority::Constitution),
        ("foreign_parliamentary_grammar", Authority::ParliamentaryPaper),
        ("correspondence_grammar", Authority::Unknown), ("archival_grammar", Authority::Unknown)];
    if let Some((name, authority)) = NAMED.iter().chain(MORE.iter()).find(|(name, _)| *name == reason) {
        return Some(Reading::new(*authority, None, name));
    }
    match reason {
        "online_grammar" => {
            let mut reading = Reading::new(Authority::Webpage, Some(Format::Url), "online_grammar");
            reading.fields.url = Some(core.to_owned());
            return Some(reading);
        }
        "court_file_grammar" => {
            // An unreported order, endorsement or decision, known by the file it was made under.
            let captures = whole(&COURT_FILE, core)?;
            let mut reading = Reading::new(Authority::Case, Some(Format::Docket), "court_file_grammar");
            reading.fields.docket = group(&captures, "docket").or_else(|| group(&captures, "commercial_docket"))
                .or_else(|| group(&captures, "decision")).or_else(|| group(&captures, "order_docket"));
            reading.jurisdiction = Some("ca".to_owned());
            return Some(reading);
        }
        "charter_grammar" => {
            let mut reading = Reading::new(Authority::Constitution, Some(Format::StatuteVolume), "charter_grammar");
            reading.fields.year = Some("1982".to_owned());
            reading.fields.chapter = Some("11".to_owned());
            reading.jurisdiction = Some("ca".to_owned());
            if style.starts_with("Charte") {
                reading.language = Some("fr");
            }
            return Some(reading);
        }
        "book_grammar" => return book(core, style),
        "article_grammar" => return neutral(core).or_else(|| journal_article(core)).or_else(|| publication(true)),
        "us_journal_grammar" => {
            return publication(true).map(|mut reading| {
                reading.jurisdiction = Some("us".to_owned());
                reading
            })
        }
        _ => {}
    }
    if let Some(reading) = database(core) { return Some(reading); }
    if !verified {
        if let Some(reading) = neutral(core) { return Some(neutral_with_publication(reading)); }
    }
    canlii(core)
        .or_else(|| unreported(core))
        .or_else(|| treaty(core))
        .or_else(|| parliamentary(core))
        .or_else(|| legislation(core))
        .or_else(|| (crate::us::is_journal(core)).then(|| publication(true)).flatten())
        .or_else(|| publication(reason == "journal_grammar"))
        .or_else(|| book(core, style))
}

/// Complete a reading from the registry: court ids, canonical reporters,
/// series jurisdictions and languages.
fn resolve(reading: &mut Reading, citation: &mut Citation, options: &crate::Options) -> Option<CourtRef> {
    let registry = registry();
    let mut court = None;
    let core_court = reading.court.is_some();
    if let Some(surface) = reading.court.take() {
        if let Some(found) = court_for(&surface, reading, true, citation, options) {
            reading.jurisdiction.get_or_insert_with(|| found.jurisdiction.clone());
            court = Some(CourtRef {
                id: found.id.clone(),
                text: surface,
            });
        } else if registry.courts_by_surface(&surface).is_empty() {
            citation.reasons.push("unverified_court".to_owned());
        }
    }
    let fields = &mut reading.fields;
    if reading.authority == Some(Authority::Journal) {
        let candidates = journal_candidates(fields.reporter.as_deref().unwrap_or(""), fields);
        let source_captured = !fields.exact_editions.is_empty() || !fields.variation_editions.is_empty();
        let readings = candidates.iter().map(|entry| crate::Interpretation {
            kind: "journal".into(), id: entry.id.clone(), canonical: entry.abbreviation.clone(),
            jurisdiction: entry.jurisdiction.clone(), selected: false, reason: String::new(),
        }).collect();
        let scores = candidates.iter().map(|entry| journal_score(entry, fields,
            reading.jurisdiction.as_deref(), options)).collect();
        if let Some(journal) = select(citation, readings, scores,
            reading.jurisdiction.as_ref().map(|_| "explicit_jurisdiction")
                .or(source_captured.then_some("citation_grammar"))).map(|at| candidates[at]) {
            fields.reporter_canonical = Some(journal.abbreviation.clone());
            if let Some(jurisdiction) = &journal.jurisdiction {
                reading.jurisdiction.get_or_insert_with(|| jurisdiction.clone());
            }
        }
    } else if let Some(surface) = fields.reporter.clone() {
        // Quicklaw is catalogued with its number marker (`OJ No`).
        let lookup = if reading.format == Some(Format::Database)
            && registry.reporters_by_surface(&surface).is_empty()
        { format!("{surface} No") } else { surface.clone() };
        let found = reporter_for(&lookup, reading.open.as_deref(), fields,
            reading.jurisdiction.as_deref(), citation, options);
        if let Some((reporter, canonical)) = found {
            if let Some(volume) = fields.volume.as_deref().filter(|volume| volume.len() == 2) {
                if let Some(edition) = reporter.editions.iter().find(|edition| edition.abbreviation == canonical) {
                    let years: std::collections::BTreeSet<_> = edition.numbering.iter().filter_map(|period| {
                        let year = period.short_year_century? + volume.parse::<u16>().ok()?;
                        (period.start.is_some_and(|start| year >= start)
                            && period.end.is_some_and(|end| year <= end)).then_some(year)
                    }).collect();
                    if years.len() == 1 {
                        fields.year = years.first().map(u16::to_string);
                        fields.volume = None;
                    }
                }
            }
            fields.reporter_canonical = Some(canonical);
            fields.reporter_id = Some(reporter.id.clone());
            if let Some(jurisdiction) = &reporter.jurisdiction {
                reading.jurisdiction.get_or_insert_with(|| jurisdiction.clone());
            }
            if reporter.kind == ReporterKind::Database {
                if reading.format == Some(Format::Reporter) {
                    reading.format = Some(Format::Database);
                    fields.number = fields.number.take().or_else(|| fields.page.clone());
                }
                // The reporter grammar captures a database's written citation
                // year as its volume. Keep that year before a later decision-
                // date parenthetical (which may name a different year).
                if reading.format == Some(Format::Database)
                    && fields.source_groups.get("year").and_then(Option::as_ref).is_none() {
                    fields.year = fields.volume.as_deref()
                        .filter(|volume| volume.len() == 4 && volume.bytes().all(|byte| byte.is_ascii_digit()))
                        .map(str::to_owned).or(fields.year.take());
                }
            }
            if court.is_none() && reading.authority == Some(Authority::Case) && explicit_courts(citation).is_none() {
                let observed = crate::aliases::observed_court(citation)
                    .filter(|court| reporter_supports_court(reporter, court));
                let inferred = observed.map(|court| court.id.as_str()).or(reporter.default_court.as_deref()).or_else(|| match reporter.courts.as_slice() {
                    [only] => Some(only.as_str()), _ => None,
                });
                if let Some(found) = inferred.and_then(|id| registry.court(id)) {
                    court = Some(CourtRef { id: found.id.clone(), text: surface });
                    citation.reasons.push(if observed.is_some() { "observed_alias" } else { "reporter_court" }.to_owned());
                }
            }
        } else {
            // Shared spelling can be certain even when reporter identity is
            // not: Bankruptcy Reports and other B.R. reporters all print B.R.
            let candidates = registry.reporters_by_surface(&surface);
            if let Some((_, canonical)) = candidates.first() {
                if candidates.iter().all(|(_, other)| other == canonical) {
                    fields.reporter_canonical = Some((*canonical).to_owned());
                }
            }
        }
    }
    if matches!(
        reading.format,
        Some(Format::StatuteVolume | Format::RegulationSeries | Format::Code)
    ) {
        let candidates = fields.series.as_deref().map(|surface| registry.series_candidates(surface)).unwrap_or_default();
        let readings = candidates.iter().map(|entry| crate::Interpretation {
            kind: "series".into(), id: entry.id.clone(), canonical: entry.abbreviation.clone(),
            jurisdiction: Some(entry.jurisdiction.clone()), selected: false, reason: String::new(),
        }).collect();
        let scores = candidates.iter().map(|entry| {
            if reading.jurisdiction.as_deref().is_some_and(|written| {
                !within(written, &entry.jurisdiction) && !within(&entry.jurisdiction, written)
            }) { return None; }
            Some((priority(Some(&entry.jurisdiction), options), false, false, false, false))
        }).collect();
        if let Some(series) = select(citation, readings, scores, reading.jurisdiction.as_ref().map(|_| "explicit_jurisdiction")).map(|at| candidates[at]) {
            reading.jurisdiction.get_or_insert_with(|| series.jurisdiction.clone());
            if series.language.as_deref() == Some("fr") {
                reading.language = Some("fr");
            }
            if series.kind == SeriesKind::Regulations && reading.authority == Some(Authority::Statute) {
                reading.authority = Some(Authority::Regulation);
                reading.format = Some(Format::RegulationSeries);
            }
        }
    }
    if !core_court && explicit_courts(citation).is_some() {
        let surface = written_courts(citation).find(|surface| !registry.courts_by_parenthetical(surface).is_empty());
        if let Some(surface) = surface {
            if let Some(found) = court_for(&surface, reading, false, citation, options) {
                court = Some(CourtRef { id: found.id.clone(), text: surface });
                // The written court refines a series-wide jurisdiction.
                if reading.jurisdiction.as_deref().is_none_or(|current| within(&found.jurisdiction, current)) {
                    reading.jurisdiction = Some(found.jurisdiction.clone());
                }
                citation.reasons.push("court_parenthetical".to_owned());
            }
        }
    }
    court
}

/// A core can be both a neutral citation and a publication citation. Resolve
/// the family before its registry identity; the ordinary registry selectors
/// still decide among courts, reporters, or journals within that family.
fn competing_family(citation: &mut Citation, reading: Reading, options: &crate::Options) -> Reading {
    if reading.format != Some(Format::Neutral) { return reading; }
    let Some(mut reported) = reading.alternatives.first().cloned() else { return reading; };
    let mut neutral = reading.clone();
    neutral.alternatives.clear();
    reported.alternatives.clear();
    reported.authority = Some(Authority::Case);
    reported.format = Some(Format::Reporter);
    reported.reason = "reporter_grammar";
    let Some(court_surface) = neutral.court.clone() else { return reading; };
    let Some(publication_surface) = reported.fields.reporter.clone() else { return reading; };
    let source = &reading.fields;
    let written_jurisdiction = reading.jurisdiction.as_deref();
    neutral.jurisdiction.clone_from(&reading.jurisdiction);
    let mut publication_fields = reported.fields.clone();
    publication_fields.exact_editions.clone_from(&source.exact_editions);
    publication_fields.variation_editions.clone_from(&source.variation_editions);
    publication_fields.year = publication_fields.year
        .or_else(|| source.source_case_name.as_ref().and_then(|name| name.year.clone()))
        .or_else(|| citation.fields.year.clone());
    let courts = registry().courts_by_surface(&court_surface);
    let reporters = reporter_candidates(&publication_surface, &publication_fields);
    let journals = journal_candidates(&publication_surface, &publication_fields);
    if courts.is_empty() || (reporters.is_empty() && journals.is_empty()) { return reading; }

    let explicit = explicit_courts(citation);
    let observed = crate::aliases::observed_court(citation)
        .filter(|court| reporters.iter().any(|(reporter, _)| reporter_supports_court(reporter, court)));
    let court_rank = courts.iter().filter_map(|court| court_score(court, &court_surface, &neutral, true,
        explicit.as_deref(), None, options)).min();
    let publication_form = !neutral.bracketed;
    let reporter_rank = reporters.iter().filter_map(|(reporter, canonical)|
        reporter_score(reporter, canonical, &publication_surface, reported.open.as_deref(),
            &publication_fields, written_jurisdiction, explicit.as_deref(), observed, options)
            .map(|(priority, period, variant, form, verified)|
                (priority, period, variant, form || publication_form, verified))).min();
    let journal_rank = if explicit.is_some() { None } else {
        journals.iter().filter_map(|journal| journal_score(journal, &publication_fields,
            written_jurisdiction, options)
            .map(|(priority, period, variant, form, verified)|
                (priority, period, variant, form || publication_form, verified))).min()
    };
    let mut journal_reading = reported.clone();
    journal_reading.authority = Some(Authority::Journal);
    journal_reading.format = Some(Format::Publication);
    journal_reading.reason = "journal_grammar";
    let mut alternatives = Vec::new();
    let mut scores = Vec::new();
    let mut interpretations = Vec::new();
    for (available, next, score, kind, surface, jurisdiction) in [
        (true, neutral, court_rank, "court", &court_surface,
            (courts.len() == 1).then(|| courts[0].jurisdiction.clone())),
        (!reporters.is_empty(), reported, reporter_rank, "reporter", &publication_surface,
            (reporters.len() == 1).then(|| reporters[0].0.jurisdiction.clone()).flatten()),
        (!journals.is_empty(), journal_reading, journal_rank, "journal", &publication_surface,
            (journals.len() == 1).then(|| journals[0].jurisdiction.clone()).flatten()),
    ] {
        if !available { continue; }
        alternatives.push(next);
        scores.push(score);
        interpretations.push(crate::Interpretation {
            kind: "family".into(), id: kind.into(), canonical: surface.to_string(),
            jurisdiction, selected: false, reason: String::new(),
        });
    }
    let chosen = select(citation, interpretations, scores,
        explicit.as_ref().map(|_| "explicit_court")
            .or(written_jurisdiction.map(|_| "explicit_jurisdiction")));
    let Some(chosen) = chosen else { return reading; };
    let next = &alternatives[chosen];
    if next.authority == reading.authority && next.format == reading.format { return reading; }
    let mut next = next.clone();
    if let Some(year) = reading.fields.source_case_name.as_ref().and_then(|name| name.year.as_ref()) {
        next.fields.year.get_or_insert_with(|| year.clone());
    }
    next.fields.source_case_name = reading.fields.source_case_name;
    next.fields.source_groups = reading.fields.source_groups;
    next.fields.exact_editions = reading.fields.exact_editions;
    next.fields.variation_editions = reading.fields.variation_editions;
    next.short_at = reading.short_at;
    next.jurisdiction = next.jurisdiction.or(reading.jurisdiction);
    next
}

pub(crate) fn apply(citation: &mut Citation, mut reading: Reading, options: &crate::Options) {
    reading = competing_family(citation, reading, options);
    reading.fields.court_text = citation.fields.court_text.clone();
    reading.fields.year = reading.fields.year.or_else(|| citation.fields.year.clone());
    reading.fields.month = reading.fields.month.or_else(|| citation.fields.month.clone());
    reading.fields.day = reading.fields.day.or_else(|| citation.fields.day.clone());
    let style = citation.style.as_ref().map_or("", |style| style.text.as_str()).to_owned();
    if reading.authority == Some(Authority::Book) && BOOK_CHAPTER.is_match(&style).unwrap_or(false) {
        reading.authority = Some(Authority::BookChapter);
    }
    if matches!(
        reading.authority,
        Some(Authority::Statute | Authority::Regulation)
    ) {
        let title = if style.is_empty() { citation.span.text.as_str() } else { style.as_str() };
        instrument_kind(title, &mut reading);
    }
    // A chapter-numbered Quebec regulation carries its number as a rule
    // (`RLRQ, c C-25.01, r 0.1`): part of its identity, not a pinpoint.
    if reading.format == Some(Format::StatuteVolume)
        && reading.fields.year.is_none()
        && reading.fields.series.is_some()
        && citation.pinpoints.first().is_some_and(|pinpoint| pinpoint.kind == PinpointKind::Rule)
    {
        let rule = citation.pinpoints.remove(0);
        reading.fields.regulation = Some(rule.first);
        reading.authority = Some(Authority::Regulation);
        reading.format = Some(Format::RegulationSeries);
    }
    if let Some(date) = crate::metadata::parenthetical_date(citation) {
        reading.fields.year = reading.fields.year.or(date.year);
        reading.fields.month = reading.fields.month.or(date.month);
        reading.fields.day = reading.fields.day.or(date.day);
    }
    let court = resolve(&mut reading, citation, options);
    if let Some(authority) = reading.authority {
        citation.authority = authority;
    }
    citation.format = reading.format;
    let note = citation.fields.note;
    reading.fields.pin_cite = citation.fields.pin_cite.take();
    citation.fields = reading.fields;
    citation.fields.note = note;
    if court.is_some() {
        citation.court = court;
    }
    if reading.jurisdiction.is_some() {
        citation.jurisdiction = reading.jurisdiction;
    }
    if let Some(language) = reading.language {
        citation.language = Some(language.to_owned());
    }
    if !citation.reasons.iter().any(|reason| reason == reading.reason) {
        citation.reasons.push(reading.reason.to_owned());
    }
}
