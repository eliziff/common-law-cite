//! Courts, reporters, statute and regulation series, and jurisdictions.
//!
//! The registry is data, not code: every table lives as JSON under
//! `crates/legal-citations/registry/` so non-Rust consumers can read the same
//! files. Canadian, Commonwealth and international entries are authored here;
//! US entries (courts, reporters, code and session-law series, journals) are
//! generated into `registry/upstream/` from Free Law Project's reporters-db and
//! courts-db by `tools/sync-upstream.py` at the versions pinned in
//! `upstream.lock`. `journals.json` holds the McGill Guide abbreviation
//! inventory that is not a reporter.
//!
//! # Verified and unverified entries
//!
//! Every [`Reporter`] and [`Journal`] carries `verified` (JSON default `true`;
//! only `"verified": false` is ever written). An unverified entry is an
//! abbreviation from an inventory that does not say what it abbreviates: the
//! McGill list entries that neither a name cue (`Rep`, `Cas`, `LJ`, `Rev`, ...)
//! nor a manual identification classified. They are kept so a surface is
//! recognised at all, but they are not evidence of kind:
//!
//! * Lookups prefer a verified entry over an unverified one sharing a surface.
//! * Consumers (the classify stage) must ignore an unverified journal or
//!   reporter whenever it conflicts with grammar evidence, e.g. a
//!   `(1979) 2 EHRR 245` or `[1990] 1 Xyz 12` shape is a case report even if
//!   `Xyz` is only known as an unverified journal, and an unverified entry
//!   alone never turns a citation into a journal article.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Names {
    pub en: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fr: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Jurisdiction {
    /// `ca`, `ca-on`, `uk`, `uk-ew`, `au`, `au-nsw`, `nz`, `us`, `us-ca`, `int`.
    pub id: String,
    pub name: Names,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CourtLevel {
    /// Final court of appeal: SCC, UKSC, HCA, JCPC, SCOTUS.
    Apex,
    Appellate,
    SuperiorTrial,
    InferiorTrial,
    Tribunal,
}

impl CourtLevel {
    /// 5 apex, 4 appellate, 3 superior trial, 2 inferior trial, 1 tribunal.
    pub fn rank(self) -> u8 {
        match self {
            Self::Apex => 5,
            Self::Appellate => 4,
            Self::SuperiorTrial => 3,
            Self::InferiorTrial => 2,
            Self::Tribunal => 1,
        }
    }
}

/// Where a public source publishes a court's decisions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CanLiiRoute {
    /// CanLII jurisdiction path segment (`ca`, `on`, `yk`).
    pub jurisdiction: String,
    /// CanLII database id with its exact casing (`scc`, `fct`, `NBQB`).
    pub database: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Court {
    /// Stable registry id: lowercase neutral code where one exists (`scc`,
    /// `onca`, `ewca-civ`, `hca`), else courts-db's id for US courts.
    pub id: String,
    pub name: Names,
    pub jurisdiction: String,
    pub level: CourtLevel,
    /// Neutral-citation court identifiers, English first (`SCC`, `CSC`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub neutral: Vec<String>,
    /// Other surface forms read in parentheticals and headings (`ON CA`,
    /// `Ont CA`, `C.A. Ont.`, `2d Cir.`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Original courts-db citation string, distinct from other aliases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citation_string: Option<String>,
    /// Contradictory source dates retained without inventing corrected bounds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_dates: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canlii: Option<CanLiiRoute>,
    /// The route CanLII uses for decisions cited by the court's French neutral
    /// identifier (`2019 CSC 5` lives under `fr/ca/csc`, not `ca/scc`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canlii_fr: Option<CanLiiRoute>,
    /// First and last year the court issued decisions under this identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<u16>,
    /// The court this one succeeded (`onsc` after `oncj-gen`) or sits within.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReporterKind {
    /// Official reports: SCR, FCR, AC, CLR, U.S.
    Official,
    /// Unofficial general or regional reports: DLR, OR, WWR, All ER, F.3d.
    General,
    /// Subject reports: CCC, CR, CPR, BLR.
    Specialty,
    /// Commercial database identifiers: CarswellOnt, OJ No, WL.
    Database,
    /// Case digests that do not report full text: ACWS, WCB. Never the
    /// preferred citation of a parallel group.
    Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Edition {
    /// Canonical abbreviation of the edition (`DLR (4th)`, `F.3d`).
    pub abbreviation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<u16>,
    /// Numbering regimes when this edition changed between continuous and
    /// annually reset volumes. Missing/ambiguous years cannot be guessed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub numbering: Vec<Numbering>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Numbering {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<u16>,
    pub year_volume: bool,
    /// A two-digit report-year locator in this period, e.g. 82 DTC = 1982.
    /// The expanded year must lie within the period's explicit bounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_year_century: Option<u16>,
    /// Edition implied by this period when the citation omits its series label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
    pub evidence: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Reporter {
    /// Stable registry id (`scr`, `dlr`, `ac`, `us`, `f`).
    pub id: String,
    pub name: Names,
    pub kind: ReporterKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    /// Courts whose decisions the reporter publishes; empty for general reporters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub courts: Vec<String>,
    /// Reporter-based inference used only when the citation does not name a court.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_court: Option<String>,
    pub editions: Vec<Edition>,
    /// Surface form -> canonical edition abbreviation (`S.C.R.` -> `SCR`,
    /// `R.C.S.` -> `SCR`, `D.L.R. (4th)` -> `DLR (4th)`).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub variations: HashMap<String, String>,
    /// Commonwealth year-as-volume series: `[1932] AC 562`, `[2016] 1 SCR 631`.
    #[serde(default)]
    pub year_volume: bool,
    /// Where the data came from: `authored`, `reporters-db`, `mcgill`.
    pub source: String,
    /// `false` when the source does not establish that this is a law report
    /// (see the module documentation).
    #[serde(default = "verified_default", skip_serializing_if = "is_verified")]
    pub verified: bool,
}

fn verified_default() -> bool {
    true
}

fn is_verified(verified: &bool) -> bool {
    *verified
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesKind {
    RevisedStatutes,
    AnnualStatutes,
    Regulations,
    Code,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Series {
    /// `rsc`, `sc`, `rso`, `so`, `sor`, `oreg`, `rlrq`, `usc`.
    pub id: String,
    pub name: Names,
    pub kind: SeriesKind,
    pub jurisdiction: String,
    /// Canonical McGill abbreviation (`RSC`, `SOR`, `O Reg`).
    pub abbreviation: String,
    /// Other surface forms, including French and dotted forms (`R.S.C.`, `LRC`, `L.R.C.`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// CanLII legislation database id for the series, when CanLII publishes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canlii: Option<CanLiiRoute>,
}

/// A law journal or other periodical: `McGill LJ`, `Harv. L. Rev.`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Journal {
    pub id: String,
    /// Full title, when the source gives one (the McGill inventory does not).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub abbreviation: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    /// Where the data came from: `mcgill`, `reporters-db`.
    pub source: String,
    /// `false` when the source does not establish that this is a periodical
    /// (see the module documentation).
    #[serde(default = "verified_default", skip_serializing_if = "is_verified")]
    pub verified: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Registry {
    pub jurisdictions: Vec<Jurisdiction>,
    pub courts: Vec<Court>,
    pub reporters: Vec<Reporter>,
    pub series: Vec<Series>,
    #[serde(default)]
    pub journals: Vec<Journal>,
    #[serde(skip)]
    index: Index,
}

/// Surface lookups are single-valued and first-listed-wins, except that a
/// verified reporter or journal always outranks an unverified one: authored
/// tables load before upstream ones, and within a table a Canadian entry is listed
/// before a colliding Commonwealth one (`FCA`, `CLR`, `FCR`, `SI`). The
/// `*s_by_surface` accessors return every entry sharing a surface, in the same
/// order, for callers that disambiguate by context (`[2020] FCA 5` is
/// Australian, `2020 FCA 5` Canadian).
#[derive(Clone, Debug, Default)]
struct Index {
    court_by_id: HashMap<String, usize>,
    court_by_surface: HashMap<String, Vec<usize>>,
    court_by_parenthetical: BTreeMap<String, Vec<usize>>,
    reporter_by_surface: HashMap<String, Vec<(usize, String)>>,
    series_by_surface: HashMap<String, Vec<usize>>,
    journal_by_surface: HashMap<String, Vec<usize>>,
}

/// Case- and punctuation-insensitive lookup form of a surface string:
/// `S.C.R.`, `S C R` and `scr` all fold to `scr`; `D.L.R. (4th)` to `dlr4th`.
pub fn fold(surface: &str) -> String {
    surface
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Eyecite helpers.get_court_by_paren uses Unicode regex word characters.
fn parenthetical_fold(surface: &str) -> String {
    static NONWORD: LazyLock<regex::Regex> = LazyLock::new(|| {
        let tables = legal_grammar::load_tables().expect("grammar corpus");
        regex::Regex::new(&tables["court.parenthetical.nonword"].entry.pattern)
            .expect("court parenthetical normalization")
    });
    NONWORD.replace_all(surface, "").to_lowercase()
}

impl Registry {
    fn indexed(mut self) -> Self {
        let mut index = Index::default();
        for (position, court) in self.courts.iter().enumerate() {
            index.court_by_id.insert(court.id.clone(), position);
            if let Some(surface) = &court.citation_string {
                index.court_by_parenthetical.entry(parenthetical_fold(surface)).or_default().push(position);
            }
            for surface in court.neutral.iter().chain(&court.aliases) {
                let courts = index.court_by_surface.entry(fold(surface)).or_default();
                if !courts.contains(&position) {
                    courts.push(position);
                }
            }
        }
        for (position, reporter) in self.reporters.iter().enumerate() {
            // Variations are a HashMap: visit them in a fixed order.
            let mut variations: Vec<_> = reporter.variations.iter().collect();
            variations.sort();
            let surfaces = reporter
                .editions
                .iter()
                .map(|edition| (&edition.abbreviation, &edition.abbreviation))
                .chain(variations);
            for (surface, canonical) in surfaces {
                let reporters = index.reporter_by_surface.entry(fold(surface)).or_default();
                if !reporters.iter().any(|(at, _)| *at == position) {
                    reporters.push((position, canonical.clone()));
                }
            }
        }
        for found in index.reporter_by_surface.values_mut() {
            found.sort_by_key(|(at, _)| !self.reporters[*at].verified);
        }
        for (position, series) in self.series.iter().enumerate() {
            for surface in std::iter::once(&series.abbreviation).chain(&series.variations) {
                let entries = index.series_by_surface.entry(fold(surface)).or_default();
                if !entries.contains(&position) { entries.push(position); }
            }
        }
        for (position, journal) in self.journals.iter().enumerate() {
            for surface in std::iter::once(&journal.abbreviation).chain(&journal.variations) {
                let entries = index.journal_by_surface.entry(fold(surface)).or_default();
                if !entries.contains(&position) { entries.push(position); }
            }
        }
        for entries in index.journal_by_surface.values_mut() {
            entries.sort_by_key(|&at| !self.journals[at].verified);
        }
        self.index = index;
        self
    }

    pub fn court(&self, id: &str) -> Option<&Court> {
        self.index.court_by_id.get(id).map(|&at| &self.courts[at])
    }

    /// The court a neutral identifier or alias names (`SCC`, `CSC`, `ON CA`).
    /// A surface several courts share resolves to the first listed (the
    /// Canadian `fca` for `FCA`); see [`Registry::courts_by_surface`].
    pub fn court_by_surface(&self, surface: &str) -> Option<&Court> {
        self.courts_by_surface(surface).into_iter().next()
    }

    /// Every court a surface names, preferred first (`FCA` -> `fca`, `fca-au`).
    pub fn courts_by_surface(&self, surface: &str) -> Vec<&Court> {
        self.index
            .court_by_surface
            .get(&fold(surface))
            .map(|positions| positions.iter().map(|&at| &self.courts[at]).collect())
            .unwrap_or_default()
    }

    /// Exact court aliases precede the pinned source's citation-string prefix
    /// readings. Return every candidate so context can resolve a unique court.
    pub fn courts_by_parenthetical(&self, surface: &str) -> Vec<&Court> {
        let exact = self.courts_by_surface(surface);
        if !exact.is_empty() { return exact; }
        let normalized = parenthetical_fold(surface);
        if normalized.is_empty() { return Vec::new(); }
        if let Some(positions) = self.index.court_by_parenthetical.get(&normalized) {
            return positions.iter().map(|&at| &self.courts[at]).collect();
        }
        self.index.court_by_parenthetical.range(normalized.clone()..)
            .take_while(|(prefix, _)| prefix.starts_with(&normalized))
            .flat_map(|(_, positions)| positions.iter().map(|&at| &self.courts[at])).collect()
    }

    /// The reporter and canonical edition abbreviation a surface form names.
    /// Check [`Reporter::verified`] before treating the match as evidence of
    /// kind. A surface several reporters share resolves to the first verified
    /// one listed; see
    /// [`Registry::reporters_by_surface`].
    pub fn reporter_by_surface(&self, surface: &str) -> Option<(&Reporter, &str)> {
        self.reporters_by_surface(surface).into_iter().next()
    }

    /// Every reporter a surface names, verified entries first, then in listed
    /// order (`CLR` -> `clr`,
    /// `clr-au`; `OR` -> Ontario Reports, then reporters-db's Oregon Reports).
    pub fn reporters_by_surface(&self, surface: &str) -> Vec<(&Reporter, &str)> {
        self.index
            .reporter_by_surface
            .get(&fold(surface))
            .map(|found| {
                found
                    .iter()
                    .map(|(at, canonical)| (&self.reporters[*at], canonical.as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The journal an abbreviation names (`McGill LJ`, `Harv. L. Rev.`). Check
    /// [`Journal::verified`] before treating the match as evidence of kind.
    pub fn journal_by_surface(&self, surface: &str) -> Option<&Journal> {
        self.journals_by_surface(surface).into_iter().next()
    }

    pub fn journals_by_surface(&self, surface: &str) -> Vec<&Journal> {
        self.index
            .journal_by_surface
            .get(&fold(surface))
            .map(|entries| entries.iter().map(|&at| &self.journals[at]).collect())
            .unwrap_or_default()
    }

    pub fn series_by_surface(&self, surface: &str) -> Option<&Series> {
        self.series_candidates(surface).into_iter().next()
    }

    pub fn series_candidates(&self, surface: &str) -> Vec<&Series> {
        self.index
            .series_by_surface
            .get(&fold(surface))
            .map(|entries| entries.iter().map(|&at| &self.series[at]).collect())
            .unwrap_or_default()
    }

    pub fn jurisdiction(&self, id: &str) -> Option<&Jurisdiction> {
        self.jurisdictions.iter().find(|jurisdiction| jurisdiction.id == id)
    }
}

fn load() -> Registry {
    fn table<T: for<'de> Deserialize<'de>>(name: &str, json: &str) -> Vec<T> {
        serde_json::from_str(json).unwrap_or_else(|error| panic!("registry/{name}: {error}"))
    }
    let mut reporters: Vec<Reporter> = table("reporters.json", include_str!("../registry/reporters.json"));
    reporters.extend(table::<Reporter>(
        "upstream/reporters.json",
        include_str!("../registry/upstream/reporters.json"),
    ));
    let mut courts: Vec<Court> = table("courts.json", include_str!("../registry/courts.json"));
    courts.extend(table::<Court>(
        "upstream/courts.json",
        include_str!("../registry/upstream/courts.json"),
    ));
    let mut series: Vec<Series> = table("series.json", include_str!("../registry/series.json"));
    series.extend(table::<Series>(
        "upstream/series.json",
        include_str!("../registry/upstream/series.json"),
    ));
    let mut journals: Vec<Journal> = table("journals.json", include_str!("../registry/journals.json"));
    journals.extend(table::<Journal>(
        "upstream/journals.json",
        include_str!("../registry/upstream/journals.json"),
    ));
    Registry {
        jurisdictions: table("jurisdictions.json", include_str!("../registry/jurisdictions.json")),
        courts,
        reporters,
        series,
        journals,
        index: Index::default(),
    }
    .indexed()
}

/// The embedded registry. Its JSON is validated by this crate's tests, so the
/// load cannot fail at runtime for a published build.
pub fn registry() -> &'static Registry {
    static REGISTRY: LazyLock<Registry> = LazyLock::new(load);
    &REGISTRY
}
