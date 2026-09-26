//! Courts, reporters, statute and regulation series, and jurisdictions.
//!
//! The registry is data, not code: every table lives as JSON under
//! `crates/legal-citations/registry/` so non-Rust consumers can read the same
//! files. Canadian, Commonwealth and international entries are authored here;
//! US entries are generated from Free Law Project's reporters-db and courts-db
//! by `tools/sync-upstream.py` at the versions pinned in `upstream.lock`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canlii: Option<CanLiiRoute>,
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
    pub editions: Vec<Edition>,
    /// Surface form -> canonical edition abbreviation (`S.C.R.` -> `SCR`,
    /// `R.C.S.` -> `SCR`, `D.L.R. (4th)` -> `DLR (4th)`).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub variations: HashMap<String, String>,
    /// Commonwealth year-as-volume series: `[1932] AC 562`, `[2016] 1 SCR 631`.
    #[serde(default)]
    pub year_volume: bool,
    /// Where the data came from: `authored`, `reporters-db`.
    pub source: String,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Registry {
    pub jurisdictions: Vec<Jurisdiction>,
    pub courts: Vec<Court>,
    pub reporters: Vec<Reporter>,
    pub series: Vec<Series>,
    #[serde(skip)]
    index: Index,
}

#[derive(Clone, Debug, Default)]
struct Index {
    court_by_id: HashMap<String, usize>,
    court_by_surface: HashMap<String, usize>,
    reporter_by_surface: HashMap<String, (usize, String)>,
    series_by_surface: HashMap<String, usize>,
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

impl Registry {
    fn indexed(mut self) -> Self {
        let mut index = Index::default();
        for (position, court) in self.courts.iter().enumerate() {
            index.court_by_id.insert(court.id.clone(), position);
            for surface in court.neutral.iter().chain(&court.aliases) {
                index.court_by_surface.entry(fold(surface)).or_insert(position);
            }
        }
        for (position, reporter) in self.reporters.iter().enumerate() {
            for edition in &reporter.editions {
                index
                    .reporter_by_surface
                    .entry(fold(&edition.abbreviation))
                    .or_insert((position, edition.abbreviation.clone()));
            }
            for (surface, canonical) in &reporter.variations {
                index
                    .reporter_by_surface
                    .entry(fold(surface))
                    .or_insert((position, canonical.clone()));
            }
        }
        for (position, series) in self.series.iter().enumerate() {
            for surface in std::iter::once(&series.abbreviation).chain(&series.variations) {
                index.series_by_surface.entry(fold(surface)).or_insert(position);
            }
        }
        self.index = index;
        self
    }

    pub fn court(&self, id: &str) -> Option<&Court> {
        self.index.court_by_id.get(id).map(|&at| &self.courts[at])
    }

    /// The court a neutral identifier or alias names (`SCC`, `CSC`, `ON CA`).
    pub fn court_by_surface(&self, surface: &str) -> Option<&Court> {
        self.index
            .court_by_surface
            .get(&fold(surface))
            .map(|&at| &self.courts[at])
    }

    /// The reporter and canonical edition abbreviation a surface form names.
    pub fn reporter_by_surface(&self, surface: &str) -> Option<(&Reporter, &str)> {
        self.index
            .reporter_by_surface
            .get(&fold(surface))
            .map(|(at, canonical)| (&self.reporters[*at], canonical.as_str()))
    }

    pub fn series_by_surface(&self, surface: &str) -> Option<&Series> {
        self.index
            .series_by_surface
            .get(&fold(surface))
            .map(|&at| &self.series[at])
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
    Registry {
        jurisdictions: table("jurisdictions.json", include_str!("../registry/jurisdictions.json")),
        courts,
        reporters,
        series: table("series.json", include_str!("../registry/series.json")),
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
