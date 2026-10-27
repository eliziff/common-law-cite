mod downstream_common;

use downstream_common::*;
use legal_citations::key::{key_in, v1, KeyError};
use legal_citations::{Authority, Citation, Form, Format, PinpointKind};

fn k(citation: &Citation) -> Option<String> {
    key_in(citation, registry())
}

fn neutral(core: &str, year: &str, series: &str, number: &str) -> Citation {
    case(0, core, 0).neutral(year, series, number).build()
}

fn reported(core: &str, year: Option<&str>, volume: Option<&str>, reporter: &str, page: &str) -> Citation {
    case(0, core, 0).reporter(year, volume, reporter, page).build()
}

fn statute(core: &str, series: Option<&str>, year: Option<&str>, chapter: &str) -> Citation {
    cite(0, Form::Full, Authority::Statute, core, 0)
        .format(Format::StatuteVolume)
        .fields(|fields| {
            fields.series = series.map(Into::into);
            fields.year = year.map(Into::into);
            fields.chapter = Some(chapter.into());
        })
        .build()
}

fn regulation(core: &str, series: &str, year: Option<&str>, number: Option<&str>, chapter: Option<&str>) -> Citation {
    cite(0, Form::Full, Authority::Regulation, core, 0)
        .format(Format::RegulationSeries)
        .fields(|fields| {
            fields.series = Some(series.into());
            fields.year = year.map(Into::into);
            fields.regulation = number.map(Into::into);
            fields.chapter = chapter.map(Into::into);
        })
        .build()
}

#[test]
fn neutral_english_and_french_collapse() {
    let english = neutral("2015 SCC 5", "2015", "SCC", "5");
    let french = neutral("2015 CSC 5", "2015", "CSC", "5");
    assert_eq!(k(&english).as_deref(), Some("2:neutral:2015:scc:5"));
    assert_eq!(k(&english), k(&french));
}

#[test]
fn neutral_prefers_resolved_court_and_strips_leading_zeros() {
    let with_court = case(0, "2019 ONCA 05", 0).neutral("2019", "ONCA", "05").court("onca").build();
    assert_eq!(k(&with_court).as_deref(), Some("2:neutral:2019:onca:5"));
}

#[test]
fn neutral_unknown_court_folds_the_written_code() {
    let citation = neutral("2020 ZZ Trib 6", "2020", "ZZ Trib", "6");
    assert_eq!(k(&citation).as_deref(), Some("2:neutral:2020:zztrib:6"));
    // A multi-word code the registry knows resolves to its court id.
    let english = neutral("2020 Comp Trib 6", "2020", "Comp Trib", "6");
    let french = neutral("2020 Trib conc 6", "2020", "Trib conc", "6");
    assert_eq!(k(&english).as_deref(), Some("2:neutral:2020:cact:6"));
    assert_eq!(k(&english), k(&french));
}

#[test]
fn neutral_distinct_numbers_and_courts_differ() {
    let a = neutral("2015 SCC 5", "2015", "SCC", "5");
    let b = neutral("2015 SCC 6", "2015", "SCC", "6");
    let c = neutral("2015 ONCA 5", "2015", "ONCA", "5");
    let d = neutral("2016 SCC 5", "2016", "SCC", "5");
    let keys = [k(&a), k(&b), k(&c), k(&d)];
    for (left, right) in [(0, 1), (0, 2), (0, 3), (1, 2)] {
        assert_ne!(keys[left], keys[right]);
    }
}

#[test]
fn scr_english_french_and_dotted_collapse_with_year() {
    let english = reported("[2015] 1 SCR 331", Some("2015"), Some("1"), "SCR", "331");
    let french = reported("[2015] 1 R.C.S. 331", Some("2015"), Some("1"), "R.C.S.", "331");
    let dotted = reported("[2015] 1 S.C.R. 331", Some("2015"), Some("1"), "S.C.R.", "331");
    assert_eq!(k(&english).as_deref(), Some("2:reporter:scr:2015:1:331"));
    assert_eq!(k(&english), k(&french));
    assert_eq!(k(&english), k(&dotted));
}

#[test]
fn scr_volume_number_distinguishes() {
    let one = reported("[2015] 1 SCR 331", Some("2015"), Some("1"), "SCR", "331");
    let two = reported("[2015] 2 SCR 331", Some("2015"), Some("2"), "SCR", "331");
    assert_ne!(k(&one), k(&two));
}

#[test]
fn general_reporter_ignores_decision_year() {
    let plain = reported("26 DLR (4th) 200", None, Some("26"), "DLR (4th)", "200");
    let with_year = reported("(1986), 26 D.L.R. (4th) 200", Some("1986"), Some("26"), "D.L.R. (4th)", "200");
    assert_eq!(k(&plain).as_deref(), Some("2:reporter:dlr4th:26:200"));
    assert_eq!(k(&plain), k(&with_year));
}

#[test]
fn reporter_editions_differ() {
    let third = reported("26 DLR (3d) 200", None, Some("26"), "DLR (3d)", "200");
    let fourth = reported("26 DLR (4th) 200", None, Some("26"), "DLR (4th)", "200");
    assert_ne!(k(&third), k(&fourth));
}

#[test]
fn year_volume_without_number() {
    let citation = reported("[1932] AC 562", Some("1932"), None, "AC", "562");
    assert_eq!(k(&citation).as_deref(), Some("2:reporter:ac:1932:562"));
    let dotted = reported("[1932] A.C. 562", Some("1932"), None, "A.C.", "562");
    assert_eq!(k(&citation), k(&dotted));
}

#[test]
fn us_reports_and_federal_reporter() {
    let us = reported("410 U.S. 113", Some("1973"), Some("410"), "U.S.", "113");
    let bare = reported("410 US 113", None, Some("410"), "US", "113");
    assert_eq!(k(&us).as_deref(), Some("2:reporter:us:410:113"));
    assert_eq!(k(&us), k(&bare));
    let f3d = reported("123 F.3d 456", None, Some("123"), "F.3d", "456");
    let spaced = reported("123 F. 3d 456", None, Some("123"), "F. 3d", "456");
    assert_eq!(k(&f3d).as_deref(), Some("2:reporter:f3d:123:456"));
    assert_eq!(k(&f3d), k(&spaced));
    let supp = reported("5 F. Supp. 2d 7", None, Some("5"), "F. Supp. 2d", "7");
    let compact = reported("5 F.Supp.2d 7", None, Some("5"), "F.Supp.2d", "7");
    assert_eq!(k(&supp), k(&compact));
    assert_eq!(k(&supp).as_deref(), Some("2:reporter:fsupp2d:5:7"));
}

#[test]
fn unknown_reporter_uses_bracket_to_decide_year() {
    let bracketed = reported("[1999] 2 XYZ 5", Some("1999"), Some("2"), "XYZ", "5");
    assert_eq!(k(&bracketed).as_deref(), Some("2:reporter:xyz:1999:2:5"));
    let plain = reported("(1999), 2 XYZ 5", Some("1999"), Some("2"), "XYZ", "5");
    assert_eq!(k(&plain).as_deref(), Some("2:reporter:xyz:2:5"));
}

#[test]
fn blank_page_or_missing_volume_has_no_key() {
    let blank = reported("600 U.S. ___", None, Some("600"), "U.S.", "___");
    assert_eq!(k(&blank), None);
    let no_volume = reported("DLR 200", None, None, "DLR (4th)", "200");
    assert_eq!(k(&no_volume), None);
}

#[test]
fn canlii_citation_is_unique_per_year() {
    let with_court = case(0, "2004 CanLII 12345 (ON CA)", 0)
        .format(Format::CanLii)
        .fields(|fields| {
            fields.year = Some("2004".into());
            fields.number = Some("12345".into());
        })
        .court("onca")
        .build();
    let without = case(0, "2004 CanLII 12345", 0)
        .format(Format::CanLii)
        .fields(|fields| {
            fields.year = Some("2004".into());
            fields.number = Some("12345".into());
        })
        .build();
    assert_eq!(k(&with_court).as_deref(), Some("2:canlii:2004:12345"));
    assert_eq!(k(&with_court), k(&without));
}

#[test]
fn database_citations() {
    let carswell = case(0, "2019 CarswellOnt 123", 0)
        .format(Format::Database)
        .fields(|fields| {
            fields.year = Some("2019".into());
            fields.reporter = Some("CarswellOnt".into());
            fields.number = Some("123".into());
        })
        .build();
    assert_eq!(k(&carswell).as_deref(), Some("2:database:carswellont:2019:123"));
    let quicklaw = |reporter: &str| {
        case(0, "[2019] OJ No 45", 0)
            .format(Format::Database)
            .fields(|fields| {
                fields.year = Some("2019".into());
                fields.reporter = Some(reporter.into());
                fields.number = Some("45".into());
            })
            .build()
    };
    assert_eq!(k(&quicklaw("OJ No")).as_deref(), Some("2:database:oj:2019:45"));
    assert_eq!(k(&quicklaw("O.J. No.")), k(&quicklaw("OJ No")));
    assert_eq!(k(&quicklaw("OJ")), k(&quicklaw("OJ No")));
}

#[test]
fn statutes_english_french_dotted_collapse() {
    let english = statute("RSC 1985, c C-46", Some("RSC"), Some("1985"), "C-46");
    let french = statute("LRC 1985, ch C-46", Some("LRC"), Some("1985"), "ch C-46");
    let dotted = statute("R.S.C. 1985, c. C-46", Some("R.S.C."), Some("1985"), "c. C-46");
    assert_eq!(k(&english).as_deref(), Some("2:statute:ca:rsc:1985:c-46"));
    assert_eq!(k(&english), k(&french));
    assert_eq!(k(&english), k(&dotted));
}

#[test]
fn statute_chapters_and_series_distinguish() {
    let criminal = statute("RSC 1985, c C-46", Some("RSC"), Some("1985"), "C-46");
    let other = statute("RSC 1985, c C-44", Some("RSC"), Some("1985"), "C-44");
    let ontario = statute("SO 2006, c 21", Some("SO"), Some("2006"), "21");
    let federal = statute("SC 2006, c 21", Some("SC"), Some("2006"), "21");
    assert_ne!(k(&criminal), k(&other));
    assert_ne!(k(&ontario), k(&federal));
    assert_eq!(k(&ontario).as_deref(), Some("2:statute:ca-on:so:2006:21"));
}

#[test]
fn statute_decimal_chapter_is_not_merged_with_integer() {
    let decimal = statute("RSC 1985, c S-22.6", Some("RSC"), Some("1985"), "S-22.6");
    let integer = statute("RSC 1985, c S-226", Some("RSC"), Some("1985"), "S-226");
    assert_eq!(k(&decimal).as_deref(), Some("2:statute:ca:rsc:1985:s-22.6"));
    assert_ne!(k(&decimal), k(&integer));
}

#[test]
fn statute_supplement_and_titled_uk_act() {
    let supplement = statute("RSC 1985, c 1 (2nd Supp)", Some("RSC"), Some("1985"), "1 (2nd Supp)");
    assert_eq!(k(&supplement).as_deref(), Some("2:statute:ca:rsc:1985:1-2nd-supp"));
    let mut uk = statute("Human Rights Act 1998 (UK), c 42", None, Some("1998"), "42");
    uk.jurisdiction = Some("uk".into());
    assert_eq!(k(&uk).as_deref(), Some("2:statute:uk:-:1998:42"));
}

#[test]
fn regulations_numbered_and_chaptered() {
    let sor = regulation("SOR/2002-227", "SOR", Some("2002"), Some("227"), None);
    let dors = regulation("DORS/2002-227", "DORS", None, Some("2002-227"), None);
    assert_eq!(k(&sor).as_deref(), Some("2:regulation:ca:sor:2002:227"));
    assert_eq!(k(&sor), k(&dors));
    let ontario = regulation("O Reg 191/11", "O Reg", None, Some("191/11"), None);
    let dotted = regulation("O. Reg. 191/11", "O. Reg.", Some("11"), Some("191"), None);
    assert_eq!(k(&ontario).as_deref(), Some("2:regulation:ca-on:oreg:2011:191"));
    assert_eq!(k(&ontario), k(&dotted));
    let old = regulation("SOR/98-123", "SOR", None, Some("98-123"), None);
    assert_eq!(k(&old).as_deref(), Some("2:regulation:ca:sor:1998:123"));
    let crc = regulation("CRC, c 870", "CRC", None, None, Some("c 870"));
    let crc_year = regulation("C.R.C. 1978, c. 870", "C.R.C.", Some("1978"), None, Some("870"));
    assert_eq!(k(&crc).as_deref(), Some("2:regulation:ca:crc:-:870"));
    assert_eq!(k(&crc), k(&crc_year));
    let rro = regulation("RRO 1990, Reg 194", "RRO", Some("1990"), Some("Reg 194"), None);
    assert_eq!(k(&rro).as_deref(), Some("2:regulation:ca-on:rro:1990:194"));
    let quebec = regulation("RLRQ, c C-12, r 1", "RLRQ", None, Some("r 1"), Some("c C-12"));
    assert_eq!(k(&quebec).as_deref(), Some("2:regulation:ca-qc:cqlr:-:c-12-r-1"));
}

#[test]
fn us_code() {
    let code = |series: &str, section: &str| {
        cite(0, Form::Full, Authority::Statute, "42 U.S.C. § 1983", 0)
            .format(Format::Code)
            .fields(|fields| {
                fields.volume = Some("42".into());
                fields.series = Some(series.into());
                fields.section = Some(section.into());
            })
            .build()
    };
    assert_eq!(k(&code("U.S.C.", "1983")).as_deref(), Some("2:code:usc:42:1983"));
    assert_eq!(k(&code("USC", "§ 1983(a)")), k(&code("U.S.C.", "1983")));
    assert_ne!(k(&code("U.S.C.", "1981")), k(&code("U.S.C.", "1983")));
}

#[test]
fn journal_drops_issue() {
    let journal = |volume: &str, reporter: &str| {
        cite(0, Form::Full, Authority::Journal, "(2010) 55:3 McGill LJ 1", 0)
            .format(Format::Publication)
            .fields(|fields| {
                fields.year = Some("2010".into());
                fields.volume = Some(volume.into());
                fields.reporter = Some(reporter.into());
                fields.page = Some("1".into());
            })
            .build()
    };
    assert_eq!(k(&journal("55:3", "McGill LJ")).as_deref(), Some("2:journal:mcgilllj:55:1"));
    assert_eq!(k(&journal("55", "McGill L.J.")), k(&journal("55:3", "McGill LJ")));
    assert_eq!(k(&journal("55(3)", "RD McGill")), k(&journal("55:3", "McGill LJ")));
}

#[test]
fn no_key_for_references_books_and_formatless() {
    let mut reference = neutral("2015 SCC 5", "2015", "SCC", "5");
    reference.form = Form::Supra;
    assert_eq!(k(&reference), None);
    let book = cite(0, Form::Full, Authority::Book, "(Toronto: Irwin Law, 2019)", 0)
        .format(Format::Publication)
        .build();
    assert_eq!(k(&book), None);
    let formatless = case(0, "2015 SCC 5", 0).build();
    assert_eq!(k(&formatless), None);
    let url = cite(0, Form::Full, Authority::Webpage, "https://example.org", 0)
        .format(Format::Url)
        .build();
    assert_eq!(k(&url), None);
}

#[test]
fn pinpoints_and_styles_never_enter_the_key() {
    let bare = neutral("2016 SCC 27", "2016", "SCC", "27");
    let styled = case(0, "2016 SCC 27", 16)
        .neutral("2016", "SCC", "27")
        .style("R v Jordan")
        .pin(PinpointKind::Paragraph, "12", None)
        .build();
    assert_eq!(k(&bare), k(&styled));
}

#[test]
fn docket_needs_a_known_court() {
    let docket = |court: Option<&str>| {
        let mut builder = case(0, "No 12-345", 0)
            .format(Format::Docket)
            .fields(|fields| fields.docket = Some("No. 12-345".into()));
        if let Some(court) = court {
            builder = builder.court(court);
        }
        builder.build()
    };
    assert_eq!(k(&docket(Some("onca"))).as_deref(), Some("2:docket:onca:no-12-345"));
    assert_eq!(k(&docket(None)), None);
}

#[test]
fn every_key_is_versioned_and_colon_safe() {
    let samples = [
        neutral("2015 SCC 5", "2015", "SCC", "5"),
        reported("[2015] 1 SCR 331", Some("2015"), Some("1"), "SCR", "331"),
        statute("RSC 1985, c C-46", Some("RSC"), Some("1985"), "C-46"),
        regulation("SOR/2002-227", "SOR", Some("2002"), Some("227"), None),
    ];
    for sample in &samples {
        let key = k(sample).unwrap();
        assert!(key.starts_with("2:"), "{key}");
        assert!(
            key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ":-.".contains(c)),
            "{key}"
        );
    }
}

#[test]
fn v1_matches_the_retired_normalizer() {
    assert_eq!(v1("R. v. Jordan, 2016 SCC 27"), "rvjordan2016scc27");
    assert_eq!(v1("SOR/2002-227"), "sor2002dash227");
    assert_eq!(v1("RSC 1985, c. C-46, s. 7.1"), "rsc1985cc46s7dot1");
    assert_eq!(v1("O Reg 191/11"), "oreg191slash11");
    assert_eq!(v1("Straße"), "strasse");
    assert_eq!(v1("12\u{2013}14"), "12dash14");
}

#[test]
fn key_error_messages() {
    assert_eq!(KeyError::Multiple(2).to_string(), "citation must identify one citation form; 2 citations were found");
    assert_eq!(KeyError::NoCitation.to_string(), "no citation was found");
}
