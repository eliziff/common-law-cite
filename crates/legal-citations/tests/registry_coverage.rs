//! The registry is a strict superset of every legacy citation table in the
//! owner's repos. `tests/fixtures/sources/*.json` freezes each table (see
//! `tools/extract-source-tables.py`); every key in every fixture must resolve
//! here, with the same CanLII route, level and aliasing the source gave it,
//! except for the corrections listed (with evidence) in
//! `tests/fixtures/sources/corrections.json`. Dropping a registry entry that a
//! fixture holds fails this test.

use legal_citations::registry::{registry, CanLiiRoute, Court, Registry};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn sources_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sources")
}

struct Fixture {
    name: String,
    kind: String,
    entries: Vec<Value>,
}

fn fixtures() -> Vec<Fixture> {
    let mut paths: Vec<_> = std::fs::read_dir(sources_dir())
        .expect("tests/fixtures/sources")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter(|path| path.file_name().is_some_and(|name| name != "corrections.json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let body: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            for field in ["repo", "path", "table", "commit", "extracted_by"] {
                assert!(body["source"][field].is_string(), "{name}: header lacks source.{field}");
            }
            Fixture {
                name,
                kind: body["kind"].as_str().expect("kind").to_owned(),
                entries: body["entries"].as_array().expect("entries").clone(),
            }
        })
        .collect()
}

/// A documented disagreement with a source: the source's value was wrong and
/// the registry holds the corrected one.
#[derive(Clone)]
struct Correction {
    field: String,
    key: String,
    source: String,
    registry: String,
}

fn corrections() -> Vec<Correction> {
    let body: Value =
        serde_json::from_str(&std::fs::read_to_string(sources_dir().join("corrections.json")).unwrap()).unwrap();
    body["corrections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            assert!(
                row["evidence"].as_str().is_some_and(|text| !text.trim().is_empty()),
                "correction without evidence: {row}"
            );
            let text = |field: &str| row[field].as_str().unwrap_or_else(|| panic!("{field}: {row}")).to_owned();
            Correction { field: text("field"), key: text("key"), source: text("source"), registry: text("registry") }
        })
        .collect()
}

fn route_text(route: &CanLiiRoute) -> String {
    format!("{}/{}", route.jurisdiction, route.database)
}

/// Every court a source key can name: by registry id, then by neutral
/// identifier or alias.
fn courts<'r>(registry: &'r Registry, key: &str) -> Vec<&'r Court> {
    let mut found: Vec<&Court> = registry.court(key).into_iter().collect();
    for court in registry.courts_by_surface(key) {
        if !found.iter().any(|seen| seen.id == court.id) {
            found.push(court);
        }
    }
    found
}

fn court_ids(courts: &[&Court]) -> BTreeSet<String> {
    courts.iter().map(|court| court.id.clone()).collect()
}

fn series_ids(registry: &Registry, key: &str) -> BTreeSet<String> {
    registry.series_by_surface(key).map(|series| series.id.clone()).into_iter().collect()
}

/// The registry jurisdiction a source's jurisdiction code names: `FED` is
/// federal Canada, a province or territory code is `ca-<code>`.
fn jurisdiction_id(registry: &Registry, key: &str) -> Option<String> {
    let code = key.to_lowercase();
    if code == "fed" {
        return Some("ca".to_owned());
    }
    [code.clone(), format!("ca-{code}")]
        .into_iter()
        .find(|id| registry.jurisdiction(id).is_some())
}

struct Checker<'r> {
    registry: &'r Registry,
    corrections: Vec<Correction>,
    used: BTreeSet<usize>,
    failures: Vec<String>,
}

impl Checker<'_> {
    /// Whether a source value that differs from every registry value is a
    /// documented correction to one of them.
    fn corrected(&mut self, field: &str, key: &str, source: &str, registry: &[String]) -> bool {
        let found = self.corrections.iter().position(|correction| {
            correction.field == field
                && correction.key == key
                && correction.source == source
                && registry.contains(&correction.registry)
        });
        if let Some(at) = found {
            self.used.insert(at);
        }
        found.is_some()
    }

    fn court(&mut self, fixture: &str, entry: &Value) -> bool {
        let key = entry["key"].as_str().unwrap();
        let found = courts(self.registry, key);
        if found.is_empty() {
            self.failures.push(format!("{fixture}: court {key:?} does not resolve"));
            return false;
        }
        let mut ok = true;
        if let Some(level) = entry["level"].as_str() {
            let levels: Vec<String> = found
                .iter()
                .map(|court| serde_json::to_value(court.level).unwrap().as_str().unwrap().to_owned())
                .collect();
            if !levels.iter().any(|candidate| candidate == level) && !self.corrected("level", key, level, &levels) {
                self.failures.push(format!("{fixture}: {key:?} is {levels:?} in the registry, {level} in the source"));
                ok = false;
            }
        }
        for field in ["canlii", "canlii_fr"] {
            let Some(route) = entry[field].as_str() else { continue };
            // A route names the court's English or French CanLII database.
            let routes: Vec<String> = found
                .iter()
                .flat_map(|court| court.canlii.iter().chain(&court.canlii_fr))
                .map(route_text)
                .collect();
            if !routes.iter().any(|candidate| candidate == route) && !self.corrected("canlii", key, route, &routes) {
                self.failures.push(format!("{fixture}: {key:?} routes {routes:?} in the registry, {route} in the source"));
                ok = false;
            }
        }
        if let Some(other) = entry["same_as"].as_str() {
            let theirs = court_ids(&courts(self.registry, other));
            if court_ids(&found).is_disjoint(&theirs) {
                self.failures.push(format!(
                    "{fixture}: {key:?} ({:?}) and {other:?} ({theirs:?}) name no common court",
                    court_ids(&found)
                ));
                ok = false;
            }
        }
        ok
    }

    fn series(&mut self, fixture: &str, entry: &Value) -> bool {
        let key = entry["key"].as_str().unwrap();
        let Some(series) = self.registry.series_by_surface(key) else {
            self.failures.push(format!("{fixture}: series {key:?} does not resolve"));
            return false;
        };
        let mut ok = true;
        if let Some(other) = entry["same_as"].as_str() {
            let theirs = series_ids(self.registry, other);
            if !theirs.contains(&series.id) {
                self.failures.push(format!("{fixture}: {key:?} is {:?}, {other:?} is {theirs:?}", series.id));
                ok = false;
            }
        }
        if let Some(segment) = entry["canlii_jurisdiction"].as_str() {
            let routes: Vec<String> = series.canlii.iter().map(|route| route.jurisdiction.clone()).collect();
            if !routes.iter().any(|candidate| candidate == segment) && !self.corrected("canlii_jurisdiction", key, segment, &routes)
            {
                self.failures.push(format!("{fixture}: {key:?} is under {routes:?} on CanLII, {segment} in the source"));
                ok = false;
            }
        }
        ok
    }

    fn jurisdiction(&mut self, fixture: &str, entry: &Value) -> bool {
        let key = entry["key"].as_str().unwrap();
        let Some(id) = jurisdiction_id(self.registry, key) else {
            self.failures.push(format!("{fixture}: jurisdiction {key:?} does not resolve"));
            return false;
        };
        let Some(segment) = entry["canlii"].as_str() else { return true };
        // The CanLII path segment the registry uses for that jurisdiction.
        let segments: BTreeSet<String> = self
            .registry
            .courts
            .iter()
            .filter(|court| court.jurisdiction == id)
            .filter_map(|court| court.canlii.as_ref())
            .chain(self.registry.series.iter().filter(|series| series.jurisdiction == id).filter_map(|series| series.canlii.as_ref()))
            .map(|route| route.jurisdiction.clone())
            .collect();
        let segments: Vec<String> = segments.into_iter().collect();
        if segments.iter().any(|candidate| candidate == segment) || self.corrected("canlii_jurisdiction", key, segment, &segments) {
            return true;
        }
        self.failures.push(format!("{fixture}: {id} is {segments:?} on CanLII, {segment} in the source"));
        false
    }

    fn resolves(&mut self, fixture: &Fixture, entry: &Value) -> bool {
        let key = entry["key"].as_str().unwrap_or_else(|| panic!("{}: entry without key: {entry}", fixture.name));
        let registry = self.registry;
        let reporter = || !registry.reporters_by_surface(key).is_empty();
        let journal = || registry.journal_by_surface(key).is_some();
        let simple = match fixture.kind.as_str() {
            "court" => return self.court(&fixture.name, entry),
            "series" => return self.series(&fixture.name, entry),
            "jurisdiction" => return self.jurisdiction(&fixture.name, entry),
            "reporter" => reporter(),
            "journal" => journal(),
            "reporter_or_journal" => reporter() || journal(),
            // Database services and digests: `CanLII` is a court-like neutral
            // identifier in the registry, the rest are reporters.
            "reporter_or_court" => reporter() || !registry.courts_by_surface(key).is_empty(),
            kind => panic!("{}: unknown kind {kind}", fixture.name),
        };
        if !simple {
            self.failures.push(format!("{}: {} {key:?} does not resolve", fixture.name, fixture.kind));
        }
        simple
    }
}

#[test]
fn registry_holds_every_legacy_source_table() {
    let registry = registry();
    let fixtures = fixtures();
    assert!(fixtures.len() >= 40, "{} fixtures", fixtures.len());
    let mut checker = Checker { registry, corrections: corrections(), used: BTreeSet::new(), failures: Vec::new() };
    let mut coverage = BTreeMap::new();
    for fixture in &fixtures {
        if fixture.kind == "pattern" {
            assert!(fixture.entries.is_empty(), "{}: a pattern table has no keys", fixture.name);
            continue;
        }
        assert!(!fixture.entries.is_empty(), "{}: no keys", fixture.name);
        let resolved = fixture.entries.iter().filter(|entry| checker.resolves(fixture, entry)).count();
        coverage.insert(fixture.name.clone(), (resolved, fixture.entries.len()));
    }
    for (name, (resolved, total)) in &coverage {
        eprintln!("{name}: {resolved}/{total}");
    }
    let unused: Vec<_> = checker
        .corrections
        .iter()
        .enumerate()
        .filter(|(at, _)| !checker.used.contains(at))
        .map(|(_, correction)| format!("{} {} {} -> {}", correction.field, correction.key, correction.source, correction.registry))
        .collect();
    assert!(unused.is_empty(), "corrections.json lists corrections no source needs: {unused:#?}");
    let count = checker.failures.len();
    checker.failures.truncate(200);
    assert!(count == 0, "{count} legacy keys are not held by the registry:\n{}", checker.failures.join("\n"));
}

/// Surfaces that name different things in different systems keep every
/// reading; the multi-match accessors return all of them.
#[test]
fn colliding_legacy_surfaces_keep_every_reading() {
    let registry = registry();
    let reporters = |surface: &str| -> Vec<String> {
        registry.reporters_by_surface(surface).iter().map(|(reporter, _)| reporter.id.clone()).collect()
    };
    // English Probate reports and the US Pacific Reporter.
    let p = reporters("P");
    assert!(p.contains(&"p-uk".to_owned()) && p.contains(&"p".to_owned()), "{p:?}");
    // Session Cases and the Statutes of Canada.
    assert!(reporters("SC").contains(&"sc-scot".to_owned()), "{:?}", reporters("SC"));
    assert_eq!(registry.series_by_surface("SC").unwrap().id, "sc");
    // International Law Reports alongside the McGill inventory entry.
    assert!(reporters("ILR").contains(&"ilr".to_owned()), "{:?}", reporters("ILR"));
}
