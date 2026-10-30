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
static DATED: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.neutral.dated"));
static CANLII: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.canlii"));
static STATUTE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.ca.statute.first"));
static STATUTE_TITLED: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.statute.titled"));
static GAZETTE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.ca.gazette"));
pub(crate) static ROUTING: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.provider-routing"));
static BOOK: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.book.imprint"));
static UNREPORTED: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.case.unreported"));
static NUMBERLESS: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.neutral.numberless"));
static COURT_FILE: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.ca.court-file"));
static PARLIAMENTARY: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.parliamentary.paper"));
static FRENCH_REPORTER: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("reporter.language.fr"));
static INSTRUMENT_KIND: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("title.instrument-kind"));
static BOOK_CHAPTER: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("title.book-chapter"));
/// The French series names of the shared series defs (`LRC`, `LC`, `RLRQ`, `DORS`, `TR`).
static FRENCH_SERIES: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| {
    let tables = legal_grammar::load_tables().expect("grammar corpus");
    let defs = &tables["cite.ca.statute.first"].defs;
    legal_grammar::compile_ecmascript_pattern(
        "classify.fr-series",
        &format!("^(?:{}|{}|R[èe]gl(?:\\s.*)?)$", defs["fr_statute_series"], defs["fr_instrument_series"]),
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

/// (unnamed, precedence, form contradicted, year outside, variant, form unmatched, unverified): the lowest wins.
type Score = (bool, usize, bool, bool, bool, bool, bool);

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

/// The reason a citation the registry places only in jurisdictions the reading leaves out carries; it is not read
/// ([`crate::Options::jurisdictions`]).
pub(crate) const NOT_READ: &str = "jurisdiction_not_read";

/// Marks a citation [`NOT_READ`] when the registry rows that could identify it are all of jurisdictions left out.
fn mark_not_read<'a>(citation: &mut Citation, options: &crate::Options, mut jurisdictions: impl Iterator<Item = Option<&'a str>>) {
    let mut any = false;
    if jurisdictions.all(|jurisdiction| { any = true; !options.reads_jurisdiction(jurisdiction) }) && any {
        citation.reasons.push(NOT_READ.to_owned());
    }
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
            else if scores[at].is_some_and(|score| !score.0) && scores.iter().flatten().any(|other| other.0) { "citation_form" }
            else if scores[at].is_some_and(|score|
                scores.iter().flatten().any(|other| other.1 > score.1)) { "jurisdiction_priority" }
            else if let Some(context) = context { context }
            else if count == 1 { "unique_match" }
            else if scores[at].is_some_and(|score| !score.2) && scores.iter().flatten().any(|other| other.1 == scores[at].unwrap().1 && other.2) { "citation_form" }
            else if scores[at].is_some_and(|(_, _, _, outside, _, _, _)| !outside)
                && scores.iter().flatten().any(|(_, _, _, outside, _, _, _)| *outside) {
                if reading.kind == "court" { "court_period" } else { "reporter_period" } }
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
    if !options.reads_jurisdiction(Some(&court.jurisdiction)) { return None; }
    if explicit.is_some_and(|written| !written.iter().any(|candidate| candidate.id == court.id)) { return None; }
    if reporter.map_or_else(|| reading.jurisdiction.as_deref().is_some_and(|jurisdiction|
        !within(&court.jurisdiction, jurisdiction) && !within(jurisdiction, &court.jurisdiction)),
        |reporter| !reporter_supports_court(reporter, court)) { return None; }
    // A neutral citation is written as its court's jurisdiction writes them: a Canadian one never brackets its year
    // ("2020 FCA 5"), a Commonwealth one does ("[2020] FCA 5"). Where the identifier is several courts' and one
    // of them writes the form, a court that does not is no reading of it, whatever the jurisdictions' order.
    let identifies = |court: &Court| court.neutral.iter().any(|code| fold(code) == fold(surface));
    let writes = |court: &Court| within(&court.jurisdiction, "ca") != reading.bracketed;
    if neutral && identifies(court) && !writes(court) && registry().courts_by_surface(surface).into_iter()
        .any(|other| other.id != court.id && identifies(other) && writes(other)) { return None; }
    // Where courts share an identifier ("NBBR" is the Court of Queen's Bench and, from 2022, the Court of King's
    // Bench), the one that sat in the citation's year reads it before one that did not.
    let year = reading.fields.year.as_deref().and_then(|year| year.parse::<u16>().ok());
    let outside = year.is_some_and(|year| court.start.is_some_and(|start| year < start) || court.end.is_some_and(|end| year > end));
    Some((false, priority(Some(&court.jurisdiction), options), false, outside, neutral && !identifies(court),
        neutral && !writes(court), false))
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
    if neutral { mark_not_read(citation, options, candidates.iter().map(|court| Some(court.jurisdiction.as_str()))); }
    let readings = candidates.iter().map(|court| crate::Interpretation {
        kind: "court".into(), id: court.id.clone(), canonical: court.neutral.first().cloned().unwrap_or_else(|| surface.to_owned()),
        jurisdiction: Some(court.jurisdiction.clone()), selected: false, reason: String::new(),
    }).collect();
    let mut scores = candidates.iter().map(|court| court_score(court, surface, reading, neutral,
        explicit.as_deref(), reporter, options)).collect::<Vec<_>>();
    // A neutral citation names its court. A parenthetical after it that names none of its courts belongs to
    // another part of the reference ("274 NR 47, 2001 FCA 93 at para 43 (CA)") or names the case ("2015 FCA 95
    // (Martin)"); it does not unname the court.
    let mut context = explicit.as_ref().map(|_| "explicit_court");
    if neutral && explicit.is_some() && scores.iter().all(Option::is_none) {
        scores = candidates.iter().map(|court| court_score(court, surface, reading, neutral, None, reporter, options)).collect();
        context = None;
    }
    // Nor does one after a report that names none of the courts its reporter reports ("(C.A.)", read only as US
    // courts, after "79 B.C.L.R. (2d) 360"): the court is unknown, not in doubt.
    if !neutral && reporter.is_some() && scores.iter().all(Option::is_none) { return None; }
    select(citation, readings, scores, context).map(|at| candidates[at])
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

/// `surface` without a trailing ordinal series ("SCLR (2d)" -> "SCLR"), or all of it where it has none.
fn journal_base(surface: &str) -> &str {
    static SERIES: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let series = SERIES.get_or_init(|| regex::Regex::new(r"\s*\(\d+(?:st|nd|rd|th|d)\)\s*$").unwrap());
    series.find(surface).map_or(surface, |found| &surface[..found.start()])
}

/// The one journal the registry knows by `surface` without its ordinal series, where it has a series.
fn series_journal(surface: &str) -> Option<&'static Journal> {
    let base = journal_base(surface);
    let journals = (base.len() < surface.len()).then(|| registry().journals_by_surface(base)).unwrap_or_default();
    (journals.len() == 1).then(|| journals[0])
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

/// Whether a reporter ran in `year`: from its first edition's start, where every edition gives one, to its last
/// edition's end, where every edition gives one.
pub(crate) fn holds_year(reporter: &Reporter, year: u16) -> bool {
    let editions = &reporter.editions;
    let start = editions.iter().map(|edition| edition.start).collect::<Option<Vec<_>>>().and_then(|starts| starts.into_iter().min());
    let end = editions.iter().map(|edition| edition.end).collect::<Option<Vec<_>>>().and_then(|ends| ends.into_iter().max());
    start.is_none_or(|start| start <= year) && end.is_none_or(|end| year <= end)
}

/// Whether the citation's year rules out `canonical`, an edition of `reporter` ([`crate::Options::year_runs`]): the
/// edition had ended by that year, or, where the year is the volume, had not begun. A decision can be reported a year or
/// more after it was decided, never before, so a decision's year before an edition began rules nothing out. Only an
/// edition whose series the citation writes ("C.C.C. (3d)", "W.W.R. (N.S.)") speaks for itself: writers leave the series
/// out ("[1968] 1 C.C.C. 14" is the New Series, "(2004), 30 B.C.L.R. 103" the fourth) and keep an older name for a later
/// series ("[2007] 4 F.C. 3"), so otherwise, and where the edition gives no years, the reporter's run (all its
/// editions) does.
fn year_contradicts(reporter: &Reporter, canonical: &str, fields: &Fields) -> bool {
    let Some(year) = fields.year.as_deref().and_then(|year| year.parse::<u16>().ok()) else { return false };
    let is_volume = fields.volume.as_deref() == fields.year.as_deref();
    // A reporters-db reporter's series is always written ("F." is the first, "F.2d" the second), as the US grammars'
    // exact editions say.
    let written_series = reporter.source == "reporters-db" && !fields.exact_editions.is_empty()
        || fields.reporter.as_deref().is_some_and(|written| {
        let written = fold(written);
        written.ends_with("ns") || written.chars().last().is_some_and(|last| last.is_ascii_digit())
            || ["2d", "3d", "4th", "5th", "6th", "7th", "8th", "2nd", "3rd"].iter().any(|series| written.ends_with(series))
    });
    let edition = reporter.editions.iter().find(|edition| edition.abbreviation == canonical)
        .filter(|edition| written_series && (edition.start.is_some() || edition.end.is_some()));
    let (start, end) = match edition {
        Some(edition) => (edition.start, edition.end),
        None => {
            let editions = &reporter.editions;
            (editions.iter().map(|edition| edition.start).collect::<Option<Vec<_>>>().and_then(|starts| starts.into_iter().min()),
                editions.iter().map(|edition| edition.end).collect::<Option<Vec<_>>>().and_then(|ends| ends.into_iter().max()))
        }
    };
    end.is_some_and(|end| year > end) || is_volume && start.is_some_and(|start| year < start)
}

/// Whether the citation's form rules out `canonical` as numbered by year: such a series is cited by its year in square
/// brackets before the volume ("[1950] P. 190"), so one without them (`bracketed`, as its key reads it) whose volume is
/// not its year ("465 P. 50 (1970)", "(1953), 253 P. 675", the Pacific Reporter) reads, among reporters the setting
/// ranks alike, another first. Writers put a decision's year in parentheses before their own series ("(1974), 1 S.C.R.
/// 729"), so it ranks after the setting.
fn year_form_contradicts(reporter: &Reporter, canonical: &str, bracketed: bool, fields: &Fields) -> bool {
    // A run of years is read by its first ("(1877-78) 3 A.C. 193").
    let Some(year) = fields.year.as_deref().and_then(|year| year.get(..4)).and_then(|year| year.parse::<u16>().ok())
        else { return false };
    if bracketed || fields.volume.as_deref().is_none_or(|volume| volume == year.to_string()) { return false; }
    let periods: Vec<bool> = reporter.editions.iter().filter(|edition| edition.abbreviation == canonical)
        .flat_map(|edition| edition.numbering.iter())
        .filter(|period| period.start.is_none_or(|start| start <= year) && period.end.is_none_or(|end| year <= end))
        .map(|period| period.year_volume).collect();
    if periods.is_empty() { reporter.year_volume } else { periods.iter().all(|year_volume| *year_volume) }
}

/// Whether the reporters-db editions the US grammars name exactly speak for the citation: unless its year falls outside
/// every one of them ("(1926), 41 BR 412" is not the Bankruptcy Reporter, begun in 1979; "(1994), 122 A.L.R. 41" not
/// the American Law Reports' first series, ended in 1948).
fn named_editions_hold(candidates: &[(&Reporter, &str)], fields: &Fields) -> bool {
    let Some(year) = fields.year.as_deref().and_then(|year| year.parse::<u16>().ok()) else { return true };
    let mut named = candidates.iter().filter(|(reporter, canonical)| reporter.source == "reporters-db"
        && source_match(canonical, &reporter.name.en, &fields.exact_editions)).peekable();
    if named.peek().is_none() { return true; }
    !named.all(|(reporter, canonical)| reporter.editions.iter().find(|edition| edition.abbreviation == *canonical)
        .is_some_and(|edition| edition.start.is_some_and(|start| year < start) || edition.end.is_some_and(|end| year > end)))
}

/// `mirrored`: a reporters-db row carries this reporter's identity, so the editions the US grammars read speak for it.
/// `editions_hold`: [`named_editions_hold`].
#[allow(clippy::too_many_arguments)]
fn reporter_score(reporter: &Reporter, canonical: &str, surface: &str, open: Option<&str>, fields: &Fields,
    jurisdiction: Option<&str>, explicit: Option<&[&Court]>, observed: Option<&Court>, mirrored: bool, editions_hold: bool,
    bracketed: bool, options: &crate::Options) -> Option<Score> {
    let source_exact = !fields.exact_editions.is_empty();
    let source_captured = source_exact || !fields.variation_editions.is_empty();
    let exact = source_match(canonical, &reporter.name.en, &fields.exact_editions);
    // The US grammars' editions come from reporters-db and so name only its reporters: they rule out the reporters-db
    // rows they do not name, and an authored reporter only where a reporters-db row carries its identity. An authored
    // reporter they cannot know ("[1961] B.R. 771", Quebec's King's Bench reports) is read by the setting, after the
    // edition they name while that edition holds the citation's year ("337 Or. 433" is the Oregon Reports, "465 P. 50
    // (1970)" the Pacific Reporter); a court the citation names, or a year, decides between them first.
    if source_exact && !exact && (reporter.source == "reporters-db" || mirrored) { return None; }
    let unnamed = source_exact && !exact && editions_hold;
    if !source_exact && source_captured && reporter.source == "reporters-db"
        && !source_match(canonical, &reporter.name.en, &fields.variation_editions) { return None; }
    if jurisdiction.zip(reporter.jurisdiction.as_deref()).is_some_and(|(written, candidate)|
        !within(written, candidate) && !within(candidate, written)) { return None; }
    if explicit.is_some_and(|courts| !courts.iter().any(|court| reporter_supports_court(reporter, court))) { return None; }
    if observed.is_some_and(|court| !reporter_supports_court(reporter, court)) { return None; }
    if !options.reads_jurisdiction(reporter.jurisdiction.as_deref()) { return None; }
    if options.year_runs && year_contradicts(reporter, canonical, fields) { return None; }
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
    Some((unnamed, priority(reporter.jurisdiction.as_deref(), options), year_form_contradicts(reporter, canonical, bracketed, fields),
        outside_edition, variant, !form_matches, !reporter.verified))
}

fn reporter_for(surface: &str, open: Option<&str>, fields: &Fields, jurisdiction: Option<&str>, citation: &mut Citation, options: &crate::Options) -> Option<(&'static Reporter, String)> {
    // Custom source grammars can recognize spellings beyond a literal alias.
    // Their edition records identify candidates without parsing the text again.
    let candidates = reporter_candidates(surface, fields);
    mark_not_read(citation, options, candidates.iter().map(|(reporter, _)| reporter.jurisdiction.as_deref()));
    let source_captured = !fields.exact_editions.is_empty() || !fields.variation_editions.is_empty();
    // A written court rules reporters out only where it is one of theirs: one the registry places with none of them
    // ("(C.A.)", read only as US courts, after "79 B.C.L.R. (2d) 360") says nothing of which it is.
    let explicit = explicit_courts(citation).filter(|courts| candidates.iter()
        .any(|(reporter, _)| courts.iter().any(|court| reporter_supports_court(reporter, court))));
    let observed = crate::aliases::observed_court(citation).filter(|court| {
        let supported = candidates.iter().any(|(reporter, _)| reporter_supports_court(reporter, court));
        if !supported { citation.reasons.push("observed_alias_conflict".to_owned()); }
        supported
    });
    let readings = candidates.iter().map(|(reporter, canonical)| crate::Interpretation {
        kind: "reporter".into(), id: reporter.id.clone(), canonical: (*canonical).to_owned(),
        jurisdiction: reporter.jurisdiction.clone(), selected: false, reason: String::new(),
    }).collect();
    let editions_hold = named_editions_hold(&candidates, fields);
    let bracketed = open == Some("[") || citation.span.text.trim_start().starts_with('[');
    let mut scores = candidates.iter().map(|(reporter, canonical)| {
        let mirrored = candidates.iter().any(|(other, _)| other.source == "reporters-db" && other.id == reporter.id);
        reporter_score(reporter, canonical, surface, open, fields, jurisdiction, explicit.as_deref(), observed, mirrored,
            editions_hold, bracketed, options)
    }).collect::<Vec<_>>();
    // The citation's year is the text's own evidence and outranks the setting: a reporter whose run (all its editions)
    // does not hold it is no reading where another's does ("12 B.R. 45 (1981)" is not Quebec's King's Bench reports,
    // which ended in 1966).
    if let Some(year) = fields.year.as_deref().and_then(|year| year.parse::<u16>().ok()) {
        let held: Vec<bool> = candidates.iter().map(|(reporter, _)| holds_year(reporter, year)).collect();
        if scores.iter().zip(&held).any(|(score, held)| score.is_some() && *held) {
            for (score, held) in scores.iter_mut().zip(&held) { if !held { *score = None; } }
        }
    }
    let by_year: Vec<String> = candidates.iter().filter(|(reporter, canonical)| options.year_runs
        && year_contradicts(reporter, canonical, fields)).map(|(reporter, _)| reporter.id.clone()).collect();
    let chosen = select(citation, readings, scores, explicit.as_ref().map(|_| "explicit_court")
        .or(observed.map(|_| "observed_alias"))
        .or(source_captured.then_some("citation_grammar"))).map(|at| (candidates[at].0, candidates[at].1.to_owned()));
    for reading in citation.interpretations.iter_mut().filter(|reading| reading.kind == "reporter" && by_year.contains(&reading.id)) {
        reading.reason = "year_outside_run".to_owned();
    }
    chosen
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
    if !options.reads_jurisdiction(entry.jurisdiction.as_deref()) { return None; }
    // An article's year is its volume's ([`crate::Options::year_runs`]).
    if options.year_runs && fields.year.as_deref().and_then(|year| year.parse::<u16>().ok()).is_some_and(|year|
        !entry.holds(fields.volume.as_deref().and_then(|volume| volume.parse().ok()), year)) { return None; }
    Some((false, priority(entry.jurisdiction.as_deref(), options), false, false,
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
    reading.court = group(&captures, "court").or_else(|| group(&captures, "court_after"));
    Some(reading)
}

/// A neutral citation written without its decision number: a registered court's year, an unreported decision.
fn numberless(core: &str) -> Option<Reading> {
    let captures = whole(&NUMBERLESS, core)?;
    let surface = group(&captures, "court")?;
    let registry = registry();
    if registry.courts_by_surface(&surface).is_empty()
        || registry.reporters_by_surface(&surface).iter().any(|(reporter, _)| reporter.verified)
        || registry.journals_by_surface(&surface).iter().any(|journal| journal.verified) {
        return None;
    }
    let mut reading = Reading::new(Authority::Case, Some(Format::Docket), "unreported_grammar");
    reading.fields.year = group(&captures, "year");
    reading.court = Some(surface);
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
    Some(reported_fields(fields, journal, false, fancy_group(&captures, "open"),
        captures.name("at").map(|matched| matched.start())))
}

/// Classify fields captured by a reporter grammar without parsing its core again. `issued`: the citation writes an
/// issue after its volume ("(2016) 54:2 Alta L R 383"), which a journal has and a report does not.
pub(crate) fn reported_fields(
    fields: Fields, journal: bool, issued: bool, open: Option<String>, short_at: Option<usize>,
) -> Reading {
    let surface = fields.reporter.as_deref().expect("reporter grammar capture");
    let known_reporter = registry()
        .reporters_by_surface(&surface)
        .iter().any(|(reporter, _)| reporter.verified);
    let known_journal = registry()
        .journals_by_surface(&surface)
        .iter().any(|journal| journal.verified);
    let is_journal = issued || (journal && (!known_reporter || known_journal))
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
    // A volume of four digits is the year where every reporter of the name running that year is numbered by year
    // ("1932 AC 562": the Appeal Cases, also "AC", ended in 1890), not where one counts by volume ("2015 WL 2375225");
    // where none ran then ("1953 K.B. 792"), every reporter of the name decides.
    let as_year = fields.volume.as_deref().and_then(|volume| volume.parse::<u16>().ok());
    let named = registry().reporters_by_surface(surface);
    let running: Vec<_> = named.iter().filter(|(reporter, _)| as_year.is_none_or(|year| holds_year(reporter, year))).collect();
    let running = if running.is_empty() { named.iter().collect() } else { running };
    let year_volume = !running.is_empty() && running.iter().all(|(reporter, _)| reporter.year_volume);
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
#[cfg(feature = "us")]
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
            reading.alternatives.push(reported_fields(fields.clone(), journal, false, None, short_at));
            reading.fields.source_groups = fields.source_groups;
            reading.fields.exact_editions = fields.exact_editions;
            reading.fields.variation_editions = fields.variation_editions;
            return reading;
        }
    }
    reported_fields(fields, journal, false, None, short_at)
}

fn journal_article(core: &str) -> Option<Reading> {
    // A block written straight after the article's quoted title is read where it was found, after that title's
    // closing quotation mark.
    fn ending<'t>(text: &'t str, core: &str) -> Option<legal_grammar::GrammarCaptures<'t>> {
        crate::find::JOURNAL_ARTICLE.captures(text).ok().flatten().filter(|captures| captures.get(0)
            .is_some_and(|whole| whole.end() == text.len() && whole.start() == text.len() - core.len()))
    }
    let quoted = format!("\u{201d} {core}");
    let captures = ending(core, core).or_else(|| ending(&quoted, core))?;
    let mut reading = Reading::new(Authority::Journal, Some(Format::Publication), "article_grammar");
    let fields = &mut reading.fields;
    let first = |names: &[&str]| names.iter().find_map(|name| fancy_group(&captures, name));
    fields.year = first(&["year", "unpaged_year", "open_year", "late_year"]);
    fields.volume = first(&["volume", "unpaged_volume", "open_volume", "bare_volume", "late_volume"]);
    fields.reporter = first(&["journal", "unpaged_journal", "open_journal", "bare_journal", "late_journal"]);
    fields.page = first(&["page", "bare_page", "late_page"]);
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
            // The volume as written, where it says more than its year: `1980-81`, `1975(2)`, `1986 (Supp)`.
            fields.volume = group(&captures, "volume").filter(|volume| fields.year.as_ref() != Some(volume));
            fields.chapter = group(&captures, "chapter");
            // A schedule with no number of its own (`SO 2000, c 36, Sch`) is the chapter's one schedule.
            fields.schedule = group(&captures, "schedule")
                .or_else(|| captures.name("schedule_label").map(|_| String::new()));
        } else if group(&captures, "rcs_series").is_some() {
            // The Revised Statutes of Canada written with the Supreme Court Reports' French abbreviation.
            fields.series = Some("RSC".to_owned());
            fields.year = group(&captures, "rcs_year");
            fields.chapter = group(&captures, "rcs_chapter");
        } else if let Some(series) = group(&captures, "compiled_series") {
            fields.series = Some(series);
            fields.chapter = group(&captures, "compiled_chapter");
            // A chapter's regulation by its number (`RRS c E-9.1 Reg 2`).
            if let Some(regulation) = group(&captures, "compiled_regulation") {
                fields.regulation = Some(regulation);
                reading.authority = Some(Authority::Regulation);
                reading.format = Some(Format::RegulationSeries);
            }
        } else if let Some(regnal) = group(&captures, "regnal") {
            fields.regnal = Some(regnal);
            fields.chapter = group(&captures, "regnal_chapter");
        } else {
            reading.authority = Some(Authority::Regulation);
            reading.format = Some(Format::RegulationSeries);
            if let Some(instrument) = group(&captures, "instrument") {
                let (series, number) = instrument.split_once('/')?;
                // A federal regulation a territory kept is its own (`SOR/78-68 NT`): the series it is filed under.
                match group(&captures, "instrument_territory") {
                    Some(territory) => {
                        fields.series = Some(format!("{series} ({territory})"));
                        fields.regulation = Some(format!("{number} {territory}"));
                    }
                    None => {
                        fields.series = Some(series.to_owned());
                        fields.regulation = Some(number.to_owned());
                        reading.jurisdiction = Some("ca".to_owned());
                    }
                }
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
                // The number follows the instrument's word (`PEI Reg EC342/06`, `NWT Reg R-071-2022`, `YOIC 2020/68`);
                // a series that names none (`NLR 54/02`) ends at the first digit.
                let split = regulation.rfind("Reg").map(|at| at + 3).or_else(|| regulation.rfind("OIC").map(|at| at + 3))
                    .map(|at| {
                        let rest = regulation[at..].trim_start_matches('.').trim_start();
                        let rest = rest.strip_prefix("(Nu)").unwrap_or(rest);
                        regulation.len() - rest.len()
                    })
                    .unwrap_or_else(|| {
                        // Else at the space before the number (`CR Nu R-011-2005`), or its first digit (`NLR54/02`).
                        let digit = regulation.find(|character: char| character.is_ascii_digit()).unwrap_or(0);
                        regulation[..digit].rfind(char::is_whitespace).filter(|_| digit > 0).map_or(digit, |at| at + 1)
                    });
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
    // A neutral or CanLII citation after its decision's year in parentheses ("Hamilton v Open Window Bakery Ltd (2003),
    // 2004 SCC 9") is that citation, not a report whose volume is a year: when its identifier is CanLII's or a court's.
    if let Some(rest) = whole(&DATED, core).and_then(|captures| group(&captures, "citation")) {
        let registered = |reading: &Reading| reading.court.as_deref()
            .is_some_and(|surface| !registry().courts_by_surface(surface).is_empty());
        if let Some(reading) = canlii(&rest).or_else(|| neutral(&rest).filter(registered)) {
            return Some(reading);
        }
    }
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
    // A notice or regulation by its gazette, part and page: `(1991) A Gaz I, 1609`, `1991 GOQ 2, 1447 [2]`.
    if reason == "securities_grammar" {
        if let Some(captures) = whole(&GAZETTE, core) {
            let either = |mcgill: &str, canlii: &str| group(&captures, mcgill).or_else(|| group(&captures, canlii));
            let mut reading = Reading::new(Authority::Regulation, Some(Format::RegulationSeries), "securities_grammar");
            reading.fields.series = either("gazette", "canlii_gazette");
            reading.fields.year = either("year", "canlii_year");
            let part = either("part", "canlii_part")?;
            let part = match part.as_str() { "I" => "1", "II" => "2", "III" => "3", part => part }.to_owned();
            let sequence = group(&captures, "canlii_sequence").map(|sequence| format!(" [{sequence}]")).unwrap_or_default();
            reading.fields.regulation = Some(format!("{part}, {}{sequence}", either("page", "canlii_page")?));
            return Some(reading);
        }
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
        "unreported_grammar" => return unreported(core).or_else(|| numberless(core)),
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
        .or_else(|| us_journal(core).then(|| publication(true)).flatten())
        .or_else(|| publication(reason == "journal_grammar"))
        .or_else(|| book(core, style))
}

/// Whether `core` is a US journal citation, by the US extractors (none without the `us` feature).
#[cfg_attr(not(feature = "us"), allow(unused_variables))]
fn us_journal(core: &str) -> bool {
    #[cfg(feature = "us")]
    return crate::us::is_journal(core);
    #[cfg(not(feature = "us"))]
    false
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
        mark_not_read(citation, options, candidates.iter().map(|entry| entry.jurisdiction.as_deref()));
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
            // A journal printed with its series ("(2012), 57 SCLR (2d) 111") that no reporter's spelling holds: the
            // journal the registry knows without the series, the series kept in its name.
            if candidates.is_empty() && reading.authority == Some(Authority::Case) {
                if let Some(journal) = series_journal(&surface) {
                    fields.reporter_canonical = Some(format!("{} {}", journal.abbreviation, &surface[journal_base(&surface).len()..].trim()));
                    reading.authority = Some(Authority::Journal);
                    reading.format = Some(Format::Publication);
                    reading.reason = "journal_grammar";
                    if let Some(jurisdiction) = &journal.jurisdiction {
                        reading.jurisdiction.get_or_insert_with(|| jurisdiction.clone());
                    }
                }
            }
        }
    }
    if matches!(
        reading.format,
        Some(Format::StatuteVolume | Format::RegulationSeries | Format::Code)
    ) {
        let candidates = fields.series.as_deref().map(|surface| registry.series_candidates(surface)).unwrap_or_default();
        mark_not_read(citation, options, candidates.iter().map(|entry| Some(entry.jurisdiction.as_str())));
        let readings = candidates.iter().map(|entry| crate::Interpretation {
            kind: "series".into(), id: entry.id.clone(), canonical: entry.abbreviation.clone(),
            jurisdiction: Some(entry.jurisdiction.clone()), selected: false, reason: String::new(),
        }).collect();
        let scores = candidates.iter().map(|entry| {
            if !options.reads_jurisdiction(Some(&entry.jurisdiction)) { return None; }
            if reading.jurisdiction.as_deref().is_some_and(|written| {
                !within(written, &entry.jurisdiction) && !within(&entry.jurisdiction, written)
            }) { return None; }
            Some((false, priority(Some(&entry.jurisdiction), options), false, false, false, false, false))
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
    let editions_hold = named_editions_hold(&reporters, &publication_fields);
    let reporter_rank = reporters.iter().filter_map(|(reporter, canonical)|
        reporter_score(reporter, canonical, &publication_surface, reported.open.as_deref(),
            &publication_fields, written_jurisdiction, explicit.as_deref(), observed,
            reporters.iter().any(|(other, _)| other.source == "reporters-db" && other.id == reporter.id), editions_hold,
            reported.open.as_deref() == Some("[") || citation.span.text.trim_start().starts_with('['), options)
            .map(|(unnamed, priority, contradicted, period, variant, form, verified)|
                (unnamed, priority, contradicted, period, variant, form || publication_form, verified))).min();
    let journal_rank = if explicit.is_some() { None } else {
        journals.iter().filter_map(|journal| journal_score(journal, &publication_fields,
            written_jurisdiction, options)
            .map(|(unnamed, priority, contradicted, period, variant, form, verified)|
                (unnamed, priority, contradicted, period, variant, form || publication_form, verified))).min()
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
    // The Charter's title styles its citation, so its language is read here, where the style is known.
    if reading.reason == "charter_grammar" && style.starts_with("Charte") {
        reading.language = Some("fr");
    }
    if matches!(
        reading.authority,
        Some(Authority::Statute | Authority::Regulation)
    ) {
        let title = if style.is_empty() { citation.span.text.as_str() } else { style.as_str() };
        instrument_kind(title, &mut reading);    }
    // A chapter-numbered Quebec regulation carries its number as a rule
    // (`RLRQ, c C-25.01, r 0.1`; `RRQ 1981, c M-20, r 1`): part of its identity, not a pinpoint.
    if reading.format == Some(Format::StatuteVolume)
        && reading.fields.series.as_deref().is_some_and(|series| reading.fields.year.is_none()
            || crate::registry::registry().series_by_surface(series)
                .is_some_and(|series| series.kind == crate::registry::SeriesKind::Regulations))
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

/// Build this module's lazily built statics now ([`crate::warm`]).
pub(crate) fn warm() {
    crate::warm_statics!(
        NEUTRAL, TRIBUNAL, BRACKETED, PARENTHESIZED, DATED, CANLII, STATUTE, STATUTE_TITLED, ROUTING,
        BOOK, UNREPORTED, COURT_FILE, PARLIAMENTARY, FRENCH_REPORTER,
        INSTRUMENT_KIND, BOOK_CHAPTER, FRENCH_SERIES, GAZETTE
    );
}
