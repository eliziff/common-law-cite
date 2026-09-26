//! Integrity and lookup tests for the embedded registry tables.

use legal_citations::registry::{registry, CourtLevel, ReporterKind, SeriesKind};
use std::path::Path;
use std::collections::HashSet;

fn unique<'a>(table: &str, ids: impl Iterator<Item = &'a String>) {
    let mut seen = HashSet::new();
    for id in ids {
        assert!(seen.insert(id.as_str()), "{table}: duplicate id {id:?}");
    }
}

#[test]
fn registry_loads_every_table() {
    let registry = registry();
    assert!(registry.jurisdictions.len() >= 80, "{}", registry.jurisdictions.len());
    assert!(registry.courts.len() > 3_000, "{}", registry.courts.len());
    assert!(registry.reporters.len() > 1_700, "{}", registry.reporters.len());
    assert!(registry.series.len() > 400, "{}", registry.series.len());
    assert!(registry.journals.len() > 1_700, "{}", registry.journals.len());
    for source in ["authored", "reporters-db", "mcgill"] {
        assert!(registry.reporters.iter().any(|reporter| reporter.source == source), "{source}");
    }
}

#[test]
fn ids_are_unique_in_every_table() {
    let registry = registry();
    unique("jurisdictions", registry.jurisdictions.iter().map(|row| &row.id));
    unique("courts", registry.courts.iter().map(|row| &row.id));
    unique("reporters", registry.reporters.iter().map(|row| &row.id));
    unique("series", registry.series.iter().map(|row| &row.id));
    unique("journals", registry.journals.iter().map(|row| &row.id));
}

#[test]
fn references_between_tables_resolve() {
    let registry = registry();
    for jurisdiction in &registry.jurisdictions {
        if let Some(parent) = &jurisdiction.parent {
            assert!(registry.jurisdiction(parent).is_some(), "{} parent {parent}", jurisdiction.id);
        }
    }
    for court in &registry.courts {
        assert!(registry.jurisdiction(&court.jurisdiction).is_some(), "{} in {}", court.id, court.jurisdiction);
        if let Some(parent) = &court.parent {
            assert!(registry.court(parent).is_some(), "{} parent {parent}", court.id);
        }
        if let (Some(start), Some(end)) = (court.start, court.end) {
            assert!(start <= end, "{} {start}-{end}", court.id);
        }
    }
    for reporter in &registry.reporters {
        assert!(!reporter.editions.is_empty(), "{} has no edition", reporter.id);
        if let Some(jurisdiction) = &reporter.jurisdiction {
            assert!(registry.jurisdiction(jurisdiction).is_some(), "{} in {jurisdiction}", reporter.id);
        }
        for court in &reporter.courts {
            assert!(registry.court(court).is_some(), "{} publishes {court}", reporter.id);
        }
        for canonical in reporter.variations.values() {
            assert!(
                reporter.editions.iter().any(|edition| &edition.abbreviation == canonical),
                "{}: variation target {canonical:?} is not an edition",
                reporter.id
            );
        }
    }
    for series in &registry.series {
        assert!(registry.jurisdiction(&series.jurisdiction).is_some(), "{} in {}", series.id, series.jurisdiction);
    }
    for journal in &registry.journals {
        if let Some(jurisdiction) = &journal.jurisdiction {
            assert!(registry.jurisdiction(jurisdiction).is_some(), "{} in {jurisdiction}", journal.id);
        }
    }
}

/// Spot checks against Beaver's verified A2AJ_CANLII_COURT_ROUTES, including
/// the irregular database slugs. Two routes are deliberately corrected here:
/// CanLII serves Yukon under `yk` (Beaver: `yt/...`), and the Privy Council
/// database sits under `ca` (Beaver: bare `ukjcpc`).
#[test]
fn canlii_routes_match_the_verified_table() {
    let registry = registry();
    let cases = [
        ("SCC", "ca", "scc"),
        ("FCA", "ca", "fca"),
        ("FC", "ca", "fct"),
        ("TCC", "ca", "tcc"),
        ("SCC-L", "ca", "scc-l"),
        ("ONCA", "on", "onca"),
        ("ONSCDC", "on", "onscdc"),
        ("HRTO", "on", "onhrt"),
        ("ONHRT", "on", "onhrt"),
        ("ABKB", "ab", "abkb"),
        ("ABQB", "ab", "abqb"),
        ("BCWCAT", "bc", "bwcwcat"),
        ("NBSM", "nb", "nbs"),
        ("NSLRB", "ns", "nsrb"),
        ("NTYDAB", "nt", "ntyadab"),
        ("QCCQLC", "qc", "qcqlc"),
        ("SKAIA", "sk", "skia"),
        ("QCCA", "qc", "qcca"),
        ("QCCDOOOQ", "qc", "qccdoooq"),
        ("NTSC", "nt", "ntsc"),
        ("PESCAD", "pe", "pescad"),
        ("YJCN", "nu", "yjcn"),
        ("YKCA", "yk", "ykca"),
        ("YTRTO", "yk", "ytrto"),
        ("UKJCPC", "ca", "ukjcpc"),
    ];
    for (surface, jurisdiction, database) in cases {
        let court = registry.court_by_surface(surface).unwrap_or_else(|| panic!("{surface}"));
        let route = court.canlii.as_ref().unwrap_or_else(|| panic!("{surface}: no route"));
        assert_eq!((route.jurisdiction.as_str(), route.database.as_str()), (jurisdiction, database), "{surface}");
    }
    // NBBR (the French neutral of the Court of Queen's Bench) keeps its exact casing.
    let nbqb = registry.court_by_surface("NBBR").unwrap();
    assert_eq!(nbqb.id, "nbqb");
    let french = nbqb.canlii_fr.as_ref().unwrap();
    assert_eq!((french.jurisdiction.as_str(), french.database.as_str()), ("nb", "NBQB"));
    for (id, database) in [("scc", "csc"), ("fca", "caf"), ("fc", "cf"), ("tcc", "cci"), ("cmac", "cacm")] {
        assert_eq!(registry.court(id).unwrap().canlii_fr.as_ref().unwrap().database, database, "{id}");
    }
    let segments = ["ca", "ab", "bc", "mb", "nb", "nl", "ns", "nt", "nu", "on", "pe", "qc", "sk", "yk"];
    let routed = registry.courts.iter().filter_map(|court| court.canlii.as_ref().map(|route| (court, route)));
    let mut count = 0;
    for (court, route) in routed {
        assert!(segments.contains(&route.jurisdiction.as_str()), "{}: {}", court.id, route.jurisdiction);
        count += 1;
    }
    assert!(count >= 405, "{count} routed courts");
}

#[test]
fn supreme_court_of_canada_resolves_from_every_form() {
    let registry = registry();
    for surface in ["SCC", "CSC", "S.C.C.", "scc", "(SCC)"] {
        assert_eq!(registry.court_by_surface(surface).map(|court| court.id.as_str()), Some("scc"), "{surface}");
    }
    let scc = registry.court("scc").unwrap();
    assert_eq!(scc.level, CourtLevel::Apex);
    assert_eq!(scc.name.fr.as_deref(), Some("Cour suprême du Canada"));
    assert_eq!(registry.court_by_surface("CAF").unwrap().id, "fca");
    assert_eq!(registry.court_by_surface("CFPI").unwrap().id, "fc");
    assert_eq!(registry.court_by_surface("CCI").unwrap().id, "tcc");
    assert_eq!(registry.court_by_surface("CACM").unwrap().id, "cmac");
    assert_eq!(registry.court_by_surface("Trib conc").unwrap().id, "cact");
    assert_eq!(registry.court_by_surface("CCRI LD").unwrap().id, "cirb");
}

#[test]
fn parenthetical_and_mcgill_court_forms_resolve() {
    let registry = registry();
    for (surface, id) in [
        ("ON CA", "onca"),
        ("Ont CA", "onca"),
        ("ON SC", "onsc"),
        ("BC CA", "bcca"),
        ("Alta QB", "abqb"),
        ("NWTSC", "nwtsc"),
        ("NLTD(G)", "nlsc"),
        ("PECA", "peca"),
        ("UK JCPC", "ukpc"),
        ("EWCA Civ", "ewca-civ"),
        ("EWHC (Admin)", "ewhc-admin"),
        ("HCA", "hca"),
        ("NZSC", "nzsc"),
        ("9th Cir.", "ca9"),
        // A2AJ dataset codes from Beaver's courtLevels.ts.
        ("CT", "cact"),
        ("FPSLREB", "pslreb"),
        ("RAD", "rad"),
        ("RLLR", "rpd"),
    ] {
        assert_eq!(registry.court_by_surface(surface).map(|court| court.id.as_str()), Some(id), "{surface}");
    }
}

#[test]
fn renamed_courts_carry_their_years() {
    let registry = registry();
    for (old, new) in [("abqb", "abkb"), ("skqb", "skkb"), ("mbqb", "mbkb"), ("nbqb", "nbkb"), ("abpc", "abcj")] {
        assert_eq!(registry.court(old).unwrap().end, Some(2022), "{old}");
        let successor = registry.court(new).unwrap();
        assert_eq!(successor.start, Some(2022), "{new}");
        assert_eq!(successor.parent.as_deref(), Some(old), "{new}");
    }
    assert_eq!(registry.court("abkb").unwrap().level, CourtLevel::SuperiorTrial);
    assert_eq!(registry.court("onca").unwrap().level, CourtLevel::Appellate);
    assert_eq!(registry.court("chrt").unwrap().level, CourtLevel::Tribunal);
    assert_eq!(registry.court("yksc").unwrap().jurisdiction, "ca-yt");
}

#[test]
fn colliding_surfaces_prefer_canada_and_list_every_reading() {
    let registry = registry();
    let fca: Vec<_> = registry.courts_by_surface("FCA").iter().map(|court| court.id.as_str()).collect();
    assert_eq!(fca, ["fca", "fca-au"]);
    let ntsc: Vec<_> = registry.courts_by_surface("NTSC").iter().map(|court| court.id.as_str()).collect();
    assert_eq!(ntsc, ["nwtsc", "ntsc-au"]);
    for (surface, first, other) in [("CLR", "clr", "clr-au"), ("FCR", "fcr", "fcr-au"), ("OR", "or", "or-us")] {
        let found: Vec<_> = registry.reporters_by_surface(surface).iter().map(|(reporter, _)| reporter.id.as_str()).collect();
        assert_eq!(found.first().copied(), Some(first), "{surface}: {found:?}");
        assert!(found.contains(&other), "{surface}: {found:?}");
    }
    assert_eq!(registry.reporter_by_surface("CLR (2d)").unwrap().0.id, "clr");
    assert_eq!(registry.series_by_surface("SI").unwrap().id, "si");
}

#[test]
fn reporter_surfaces_fold_to_canonical_editions() {
    let registry = registry();
    for surface in ["R.C.S.", "RCS", "S.C.R.", "SCR"] {
        let (reporter, canonical) = registry.reporter_by_surface(surface).unwrap_or_else(|| panic!("{surface}"));
        assert_eq!((reporter.id.as_str(), canonical), ("scr", "SCR"), "{surface}");
    }
    let scr = registry.reporter_by_surface("SCR").unwrap().0;
    assert!(scr.year_volume);
    assert_eq!(scr.kind, ReporterKind::Official);
    for surface in ["DLR (4th)", "D.L.R. (4th)", "DLR 4th", "DLR (4e)"] {
        let (reporter, canonical) = registry.reporter_by_surface(surface).unwrap_or_else(|| panic!("{surface}"));
        assert_eq!((reporter.id.as_str(), canonical), ("dlr", "DLR (4th)"), "{surface}");
    }
    assert!(!registry.reporter_by_surface("DLR").unwrap().0.year_volume);
    assert_eq!(registry.reporter_by_surface("RCF").unwrap(), (registry.reporter_by_surface("FCR").unwrap().0, "FCR"));
    assert_eq!(registry.reporter_by_surface("O.R. (3d)").unwrap().1, "OR (3d)");
    assert_eq!(registry.reporter_by_surface("CR (7th)").unwrap().0.id, "cr");
    assert_eq!(registry.reporter_by_surface("Alta LR (7th)").unwrap().0.id, "altalr");
    assert_eq!(registry.reporter_by_surface("OJ No").unwrap().0.kind, ReporterKind::Database);
    assert_eq!(registry.reporter_by_surface("JQ no").unwrap().1, "QJ No");
    assert_eq!(registry.reporter_by_surface("CarswellOnt").unwrap().0.kind, ReporterKind::Database);
    for (surface, year_volume) in [("AC", true), ("WLR", true), ("All ER", true), ("NZLR", true), ("Cr App R", false)] {
        assert_eq!(registry.reporter_by_surface(surface).unwrap().0.year_volume, year_volume, "{surface}");
    }
    let (us, _) = registry.reporter_by_surface("U.S.").unwrap();
    assert_eq!((us.id.as_str(), us.kind, us.source.as_str()), ("us", ReporterKind::Official, "reporters-db"));
    assert_eq!(registry.reporter_by_surface("F.3d").unwrap(), (registry.reporter_by_surface("F.").unwrap().0, "F.3d"));
}

#[test]
fn digests_have_kind_digest() {
    let registry = registry();
    for surface in ["ACWS", "ACWS (3d)", "WCB", "WCB (2d)", "WDFL", "JE"] {
        let (reporter, _) = registry.reporter_by_surface(surface).unwrap_or_else(|| panic!("{surface}"));
        assert_eq!(reporter.kind, ReporterKind::Digest, "{surface}");
    }
}

#[test]
fn statute_and_regulation_series_resolve() {
    let registry = registry();
    for (surface, id) in [
        ("RSC", "rsc"),
        ("R.S.C.", "rsc"),
        ("LRC", "rsc"),
        ("L.R.C.", "rsc"),
        ("LC", "sc"),
        ("SOR", "sor"),
        ("DORS", "sor"),
        ("TR", "si"),
        ("CRC", "crc"),
        ("LRO", "rso"),
        ("O Reg", "oreg"),
        ("Règl de l'Ont", "oreg"),
        ("CPLM", "ccsm"),
        ("RLRQ", "cqlr"),
        ("LRQ", "rsq"),
        ("LQ", "sq"),
        ("LN-B", "snb"),
        ("SNu", "snu"),
        ("U.S.C.", "usc"),
        ("C.F.R.", "cfr"),
    ] {
        assert_eq!(registry.series_by_surface(surface).map(|series| series.id.as_str()), Some(id), "{surface}");
    }
    let rsc = registry.series_by_surface("LRC").unwrap();
    assert_eq!(rsc.kind, SeriesKind::RevisedStatutes);
    assert_eq!(rsc.canlii.as_ref().unwrap().database, "cas");
    assert_eq!(registry.series_by_surface("SOR").unwrap().kind, SeriesKind::Regulations);
    assert_eq!(registry.series_by_surface("RSY").unwrap().canlii.as_ref().unwrap().jurisdiction, "yk");
    assert_eq!(registry.series_by_surface("U.S.C.").unwrap().kind, SeriesKind::Code);
}

#[test]
fn journals_resolve_from_both_sources() {
    let registry = registry();
    let harvard = registry.journal_by_surface("Harv. L. Rev.").unwrap();
    assert_eq!(harvard.source, "reporters-db");
    assert_eq!(registry.journal_by_surface("Harv L Rev").unwrap().id, harvard.id);
    assert_eq!(registry.journal_by_surface("UTLJ").unwrap().source, "mcgill");
    assert_eq!(registry.journal_by_surface("Can Bar Rev").unwrap().source, "mcgill");
    assert!(registry.journal_by_surface("SCR").is_none());
}

#[test]
fn upstream_courts_keep_courts_db_ids() {
    let registry = registry();
    let scotus = registry.court("scotus").unwrap();
    assert_eq!((scotus.jurisdiction.as_str(), scotus.level), ("us", CourtLevel::Apex));
    let california = registry.court("cal").unwrap();
    assert_eq!(california.jurisdiction, "us-ca");
    assert_eq!(registry.court_by_surface("D.D.C.").unwrap().id, "dcd");
}

#[test]
fn unverified_mcgill_entries_are_flagged_and_outranked() {
    let registry = registry();
    // Only the McGill inventory has unverified rows, and only among journals.
    assert!(registry.reporters.iter().all(|reporter| reporter.verified));
    let unverified: Vec<_> = registry.journals.iter().filter(|journal| !journal.verified).collect();
    assert!(unverified.len() > 300, "{}", unverified.len());
    assert!(unverified.iter().all(|journal| journal.source == "mcgill"));
    let aalr = registry.journal_by_surface("AALR").unwrap();
    assert!(!aalr.verified);
    assert!(registry.journal_by_surface("Can Bar Rev").unwrap().verified);
    assert!(registry.journal_by_surface("Harv. L. Rev.").unwrap().verified);
    // Identified law reports moved out of the uncertain journal pile.
    for (surface, kind) in [
        ("EHRR", ReporterKind::General),
        ("ECHR (Ser A)", ReporterKind::Official),
        ("ACLR", ReporterKind::Specialty),
        ("Alta LRBR", ReporterKind::Specialty),
        ("CLRBR (2d)", ReporterKind::Specialty),
        ("Adam", ReporterKind::General),
        ("Ves Jr", ReporterKind::General),
        ("M & W", ReporterKind::General),
        ("UCQB", ReporterKind::General),
        ("Ont D Crim", ReporterKind::Digest),
    ] {
        let (reporter, _) = registry.reporter_by_surface(surface).unwrap_or_else(|| panic!("{surface}"));
        assert_eq!(reporter.kind, kind, "{surface}");
        assert!(reporter.verified, "{surface}");
        assert!(registry.journal_by_surface(surface).is_none(), "{surface} is also a journal");
    }
}

#[test]
fn crate_license_and_notice_match_the_workspace_copies() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = crate_dir.join("../..");
    for file in ["LICENSE", "NOTICE"] {
        let workspace = std::fs::read_to_string(root.join(file)).unwrap();
        for krate in ["legal-citations", "legal-grammar"] {
            let copy = std::fs::read_to_string(root.join("crates").join(krate).join(file)).unwrap();
            assert_eq!(copy, workspace, "crates/{krate}/{file} is out of sync");
        }
    }
    assert!(std::fs::read_to_string(root.join("NOTICE")).unwrap().contains("courts-db"));
}
