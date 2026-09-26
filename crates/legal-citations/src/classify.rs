//! Decide [`Authority`](crate::Authority) and [`Format`](crate::Format) and parse
//! [`Fields`](crate::Fields) from the grammar's named groups.
//!
//! Each reader re-applies the grammar that describes the core to the whole
//! core span and reads its named groups; the registry then supplies canonical
//! reporters, court ids, jurisdictions and languages. A surface the registry
//! does not know keeps its grammar reading, so an empty or partial registry
//! degrades to fewer ids, never to a different classification.

use crate::find::{us_journal, DATABASE, JOURNAL_CUE, PARLIAMENTARY_COMMONWEALTH, REPORTER_PARTS, TREATY};
use crate::model::{Authority, Citation, CourtRef, Fields, Form, Format, PinpointKind};
use crate::registry::{fold, registry, Court, Reporter, ReporterKind, SeriesKind};
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
static CANLII: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.canlii"));
static STATUTE: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.ca.statute.first"));
static STATUTE_TITLED: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.statute.titled"));
static ROUTING: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.provider-routing"));
static JOURNAL_ARTICLE: LazyLock<CompiledGrammar> =
    LazyLock::new(|| backtracking("cite.journal.article"));
static BOOK: LazyLock<CompiledEcmascriptGrammar> = LazyLock::new(|| linear("cite.book.imprint"));
static PARLIAMENTARY: LazyLock<CompiledEcmascriptGrammar> =
    LazyLock::new(|| linear("cite.parliamentary.paper"));
static US_CODE: LazyLock<CompiledGrammar> = LazyLock::new(|| backtracking("cite.us.code.parts"));
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

/// Canadian neutral citations print a bare year; the Commonwealth courts that
/// share their identifiers bracket it (`2020 FCA 5` vs `[2020] FCA 5`).
fn court_for(surface: &str, bracketed: bool) -> Option<&'static Court> {
    let candidates = registry().courts_by_surface(surface);
    candidates
        .iter()
        .find(|court| court.jurisdiction.starts_with("ca") != bracketed)
        .or(candidates.first())
        .copied()
}

/// A report series by surface. Where several share one, the citation's shape
/// decides: Australian series take a parenthesized year (`(2001) 110 FCR 1`,
/// `(1992) 175 CLR 1`), Canadian and English ones a bracketed year
/// (`[2005] 1 FCR 123`), and only U.S. reporters are dotted with no year
/// (`123 Or. 456` is Oregon, `71 OR (2d) 725` Ontario).
fn reporter_for(surface: &str, open: Option<&str>) -> Option<(&'static Reporter, String)> {
    let candidates = registry().reporters_by_surface(surface);
    let jurisdiction = |reporter: &Reporter| reporter.jurisdiction.clone().unwrap_or_default();
    let preferred = |reporter: &&(&Reporter, &str)| {
        let jurisdiction = jurisdiction(reporter.0);
        match open {
            Some("(") => jurisdiction.starts_with("au"),
            Some(_) => !jurisdiction.starts_with("au") && !jurisdiction.starts_with("us"),
            None if surface.contains('.') => jurisdiction.starts_with("us"),
            None => !jurisdiction.starts_with("us"),
        }
    };
    let chosen = candidates.iter().find(preferred).or(candidates.first())?;
    Some((chosen.0, chosen.1.to_owned()))
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
#[derive(Default)]
struct Reading {
    authority: Option<Authority>,
    format: Option<Format>,
    fields: Fields,
    /// A court surface with the grammar's canonical id when the registry lacks it.
    court: Option<(String, Option<String>)>,
    bracketed: bool,
    open: Option<String>,
    language: Option<&'static str>,
    jurisdiction: Option<String>,
    reason: &'static str,
}

impl Reading {
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
    ] {
        let Some(captures) = whole(pattern, core) else {
            continue;
        };
        let surface = group(&captures, "court")?;
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
        reading.court = Some((surface, canonical.map(|id| id.replace(' ', "-"))));
        reading.bracketed = bracketed;
        return Some(reading);
    }
    None
}

fn canlii(core: &str) -> Option<Reading> {
    let captures = whole(&CANLII, core)?;
    let mut reading = Reading::new(Authority::Case, Some(Format::CanLii), "canlii_grammar");
    reading.fields.year = group(&captures, "year");
    reading.fields.number = group(&captures, "number");
    reading.fields.reporter = Some("CanLII".to_owned());
    Some(reading)
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

/// `42 U.S.C. § 1983`, `26 C.F.R. § 1.1`, `Pub. L. No. 111-148`.
fn us_code(core: &str) -> Option<Reading> {
    let captures = US_CODE.captures(core).ok()??;
    let mut reading = Reading::new(Authority::Statute, Some(Format::Code), "code_grammar");
    let fields = &mut reading.fields;
    if let Some(public_law) = fancy_group(&captures, "public_law") {
        fields.series = Some("Pub. L.".to_owned());
        fields.number = Some(public_law);
    } else {
        fields.volume = fancy_group(&captures, "title").or_else(|| fancy_group(&captures, "plain_title"));
        fields.series = fancy_group(&captures, "code").or_else(|| fancy_group(&captures, "plain_code"));
        fields.section = fancy_group(&captures, "section").or_else(|| fancy_group(&captures, "plain_section"));
    }
    let code = fields.series.as_deref().map(fold).unwrap_or_default();
    if code == "cfr" || code.ends_with("reg") || code.ends_with("regs") || code.contains("admincode") {
        reading.authority = Some(Authority::Regulation);
    }
    if let Some(series) = fields.series.as_deref().and_then(|series| registry().series_by_surface(series)) {
        reading.jurisdiction = Some(series.jurisdiction.clone());
    } else if matches!(code.as_str(), "usc" | "usca" | "uscs" | "cfr" | "publ" | "fedreg") {
        reading.jurisdiction = Some("us".to_owned());
    }
    Some(reading)
}

/// `[2016] 1 SCR 631`, `(1992) 175 CLR 1`, `410 U.S. 113`, `123 F.3d at 456`,
/// and the journals written the same way (`(2019) 97 Can Bar Rev 1`).
fn reported(core: &str, journal: bool) -> Option<Reading> {
    let captures = REPORTER_PARTS.captures(core).ok()??;
    let surface = fancy_group(&captures, "reporter")?;
    let known_reporter = registry()
        .reporter_by_surface(&surface)
        .is_some_and(|(reporter, _)| reporter.verified);
    let known_journal = registry()
        .journal_by_surface(&surface)
        .is_some_and(|journal| journal.verified);
    let is_journal = journal
        || (known_journal && !known_reporter)
        || (!known_reporter && JOURNAL_CUE.is_match(&surface));
    let mut reading = if is_journal {
        Reading::new(Authority::Journal, Some(Format::Publication), "journal_grammar")
    } else {
        Reading::new(Authority::Case, Some(Format::Reporter), "reporter_grammar")
    };
    reading.open = fancy_group(&captures, "open");
    reading.bracketed = reading.open.as_deref() == Some("[");
    reading.fields.year = fancy_group(&captures, "year");
    reading.fields.volume = fancy_group(&captures, "volume");
    reading.fields.page = fancy_group(&captures, "page");
    if FRENCH_REPORTER.is_match(&surface) {
        reading.language = Some("fr");
    }
    reading.fields.reporter = Some(surface);
    Some(reading)
}

fn journal_article(core: &str) -> Option<Reading> {
    let captures = JOURNAL_ARTICLE.captures(core).ok()??;
    let matched = captures.get(0)?;
    if matched.start() != 0 || matched.end() != core.len() {
        return None;
    }
    let mut reading = Reading::new(Authority::Journal, Some(Format::Publication), "article_grammar");
    let fields = &mut reading.fields;
    fields.year = fancy_group(&captures, "year").or_else(|| fancy_group(&captures, "unpaged_year"));
    fields.volume = fancy_group(&captures, "volume").or_else(|| fancy_group(&captures, "unpaged_volume"));
    fields.reporter = fancy_group(&captures, "journal").or_else(|| fancy_group(&captures, "unpaged_journal"));
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
    let imprint = group(&captures, "imprint").or_else(|| group(&captures, "edition_imprint"));
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

fn read(citation: &Citation, style: &str) -> Option<Reading> {
    let core = citation.span.text.as_str();
    let reason = citation.reasons.first().map_or("", String::as_str);
    match reason {
        "online_grammar" => {
            let mut reading = Reading::new(Authority::Webpage, Some(Format::Url), "online_grammar");
            reading.fields.url = Some(core.to_owned());
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
        "article_grammar" => return journal_article(core).or_else(|| reported(core, true)),
        "us_journal_grammar" => {
            return reported(core, true).map(|mut reading| {
                reading.jurisdiction = Some("us".to_owned());
                reading
            })
        }
        "statute_grammar" if !core.contains(',') || us_code(core).is_some() => {
            if let Some(reading) = us_code(core) {
                return Some(reading);
            }
        }
        _ => {}
    }
    database(core)
        .or_else(|| neutral(core))
        .or_else(|| canlii(core))
        .or_else(|| treaty(core))
        .or_else(|| parliamentary(core))
        .or_else(|| legislation(core))
        .or_else(|| us_code(core))
        .or_else(|| (us_journal(core)).then(|| reported(core, true)).flatten())
        .or_else(|| reported(core, reason == "journal_grammar"))
        .or_else(|| book(core, style))
}

/// Complete a reading from the registry: court ids, canonical reporters,
/// series jurisdictions and languages.
fn resolve(reading: &mut Reading) -> Option<CourtRef> {
    let registry = registry();
    let mut court = None;
    if let Some((surface, canonical)) = reading.court.take() {
        if let Some(found) = court_for(&surface, reading.bracketed) {
            reading.jurisdiction.get_or_insert_with(|| found.jurisdiction.clone());
            court = Some(CourtRef {
                id: found.id.clone(),
                text: surface,
            });
        } else if let Some(id) = canonical {
            court = Some(CourtRef { id, text: surface });
        }
    }
    let fields = &mut reading.fields;
    if reading.authority == Some(Authority::Journal) {
        if let Some(journal) = fields.reporter.as_deref().and_then(|surface| registry.journal_by_surface(surface)) {
            fields.reporter_canonical = Some(journal.abbreviation.clone());
            if let Some(jurisdiction) = &journal.jurisdiction {
                reading.jurisdiction.get_or_insert_with(|| jurisdiction.clone());
            }
        }
    } else if let Some(surface) = fields.reporter.clone() {
        // Quicklaw is catalogued with its number marker (`OJ No`).
        let found = reporter_for(&surface, reading.open.as_deref()).or_else(|| {
            (reading.format == Some(Format::Database))
                .then(|| reporter_for(&format!("{surface} No"), None))
                .flatten()
        });
        if let Some((reporter, canonical)) = found {
            fields.reporter_canonical = Some(canonical);
            if let Some(jurisdiction) = &reporter.jurisdiction {
                reading.jurisdiction.get_or_insert_with(|| jurisdiction.clone());
            }
            if reporter.kind == ReporterKind::Database && reading.format == Some(Format::Reporter) {
                reading.format = Some(Format::Database);
            }
            if let [only] = reporter.courts.as_slice() {
                if court.is_none() && reading.authority == Some(Authority::Case) {
                    if let Some(found) = registry.court(only) {
                        court = Some(CourtRef {
                            id: found.id.clone(),
                            text: surface,
                        });
                    }
                }
            }
        }
    }
    if matches!(
        reading.format,
        Some(Format::StatuteVolume | Format::RegulationSeries)
    ) {
        if let Some(series) = fields.series.as_deref().and_then(|series| registry.series_by_surface(series)) {
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
    court
}

pub fn classify(_text: &str, citation: &mut Citation) {
    if !matches!(citation.form, Form::Full | Form::Short) {
        return;
    }
    let style = citation.style.as_ref().map_or("", |style| style.text.as_str()).to_owned();
    let Some(mut reading) = read(citation, &style) else {
        return;
    };
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
    let court = resolve(&mut reading);
    if let Some(authority) = reading.authority {
        citation.authority = authority;
    }
    citation.format = reading.format;
    let note = citation.fields.note;
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
