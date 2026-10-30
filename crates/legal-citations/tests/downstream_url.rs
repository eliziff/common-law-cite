mod downstream_common;

use downstream_common::*;
use legal_citations::format::Language;
use legal_citations::url::{
    canlii_case_in, canlii_citation_url, canlii_legislation_in, canlii_noteup, canlii_pdf_url, canlii_search,
    courtlistener_in, encode_component, is_canlii_url, justice_laws, legislation_gov_uk, source_anchor, uk_caselaw,
    url_in, with_pinpoint, SearchField,
};
use legal_citations::{Authority, Citation, Form, Format, PinpointKind};

fn neutral(year: &str, code: &str, number: &str) -> Citation {
    case(0, &format!("{year} {code} {number}"), 0).neutral(year, code, number).build()
}

fn canlii(url: Option<String>) -> String {
    url.expect("a CanLII URL")
}

#[test]
fn canlii_neutral_english() {
    assert_eq!(
        canlii(canlii_case_in(&neutral("2016", "SCC", "27"), Language::En, registry())),
        "https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html"
    );
    assert_eq!(
        canlii(canlii_case_in(&neutral("2019", "ONCA", "123"), Language::En, registry())),
        "https://www.canlii.org/en/on/onca/doc/2019/2019onca123/2019onca123.html"
    );
}

#[test]
fn canlii_irregular_database_keeps_slug_from_written_code() {
    assert_eq!(
        canlii(canlii_case_in(&neutral("2019", "FC", "123"), Language::En, registry())),
        "https://www.canlii.org/en/ca/fct/doc/2019/2019fc123/2019fc123.html"
    );
    assert_eq!(
        canlii(canlii_case_in(&neutral("2020", "HRTO", "5"), Language::En, registry())),
        "https://www.canlii.org/en/on/onhrt/doc/2020/2020hrto5/2020hrto5.html"
    );
    assert_eq!(
        canlii(canlii_case_in(&neutral("2019", "NWTCA", "5"), Language::En, registry())),
        "https://www.canlii.org/en/nt/ntca/doc/2019/2019nwtca5/2019nwtca5.html"
    );
}

#[test]
fn canlii_french_codes_and_pages() {
    // A French neutral code names the French page and database.
    assert_eq!(
        canlii(canlii_case_in(&neutral("2016", "CSC", "27"), Language::En, registry())),
        "https://www.canlii.org/fr/ca/csc/doc/2016/2016csc27/2016csc27.html"
    );
    // A French page of an English SCC citation uses the French code.
    assert_eq!(
        canlii(canlii_case_in(&neutral("2016", "SCC", "27"), Language::Fr, registry())),
        "https://www.canlii.org/fr/ca/csc/doc/2016/2016csc27/2016csc27.html"
    );
    assert_eq!(
        canlii(canlii_case_in(&neutral("2019", "FC", "5"), Language::Fr, registry())),
        "https://www.canlii.org/fr/ca/cf/doc/2019/2019cf5/2019cf5.html"
    );
    // Any other court keeps its English route under /fr/.
    assert_eq!(
        canlii(canlii_case_in(&neutral("2019", "ONCA", "1"), Language::Fr, registry())),
        "https://www.canlii.org/fr/on/onca/doc/2019/2019onca1/2019onca1.html"
    );
    // NBBR keeps CanLII's exact database casing.
    assert_eq!(
        canlii(canlii_case_in(&neutral("2019", "NBBR", "12"), Language::En, registry())),
        "https://www.canlii.org/fr/nb/NBQB/doc/2019/2019nbbr12/2019nbbr12.html"
    );
    assert_eq!(
        canlii(canlii_case_in(&neutral("2019", "NBQB", "12"), Language::En, registry())),
        "https://www.canlii.org/en/nb/nbqb/doc/2019/2019nbqb12/2019nbqb12.html"
    );
}

#[test]
fn canlii_citation_uses_the_court_route() {
    let citation = case(0, "2004 CanLII 12345 (ON CA)", 0)
        .format(Format::CanLii)
        .fields(|f| {
            f.year = Some("2004".into());
            f.number = Some("12345".into());
        })
        .court("onca")
        .build();
    assert_eq!(
        canlii(canlii_case_in(&citation, Language::En, registry())),
        "https://www.canlii.org/en/on/onca/doc/2004/2004canlii12345/2004canlii12345.html"
    );
    let mut without_court = citation.clone();
    without_court.court = None;
    assert_eq!(canlii_case_in(&without_court, Language::En, registry()), None);
}

#[test]
fn canlii_abstains_when_uncertain() {
    // Unknown court.
    assert_eq!(canlii_case_in(&neutral("2019", "ZZCA", "1"), Language::En, registry()), None);
    // Known court without a CanLII route.
    assert_eq!(canlii_case_in(&neutral("2019", "BCCA", "1"), Language::En, registry()), None);
    // A court's alias is not a neutral code.
    let mut alias = neutral("2019", "Ont CA", "1");
    alias.fields.series = Some("OntCA".into());
    assert_eq!(canlii_case_in(&alias, Language::En, registry()), None);
    // Multi-word code, non-Canadian court, non-numeric number, missing year.
    assert_eq!(canlii_case_in(&neutral("2020", "Comp Trib", "6"), Language::En, registry()), None);
    assert_eq!(canlii_case_in(&neutral("2019", "UKPC", "5"), Language::En, registry()), None);
    assert_eq!(canlii_case_in(&neutral("2019", "SCC", "5a"), Language::En, registry()), None);
    let mut no_year = neutral("2019", "SCC", "5");
    no_year.fields.year = None;
    assert_eq!(canlii_case_in(&no_year, Language::En, registry()), None);
    // Not a full citation.
    let mut supra = neutral("2016", "SCC", "27");
    supra.form = Form::Supra;
    assert_eq!(canlii_case_in(&supra, Language::En, registry()), None);
}

#[test]
fn canlii_decision_id_codes() {
    // Codes CanLII uses in a court's decision ids without the court being identified by them:
    // a French code under /fr/ and the court's route, a division's code under the court's route.
    // Each URL was checked against canlii.org. Against the built-in registry, which carries them.
    let cases = [
        ("2010", "PSLRB", "14", "https://www.canlii.org/en/ca/pslrb/doc/2010/2010pslrb14/2010pslrb14.html"),
        ("2010", "CRTFP", "14", "https://www.canlii.org/fr/ca/pslrb/doc/2010/2010crtfp14/2010crtfp14.html"),
        ("2015", "CRTEFP", "40", "https://www.canlii.org/fr/ca/pslreb/doc/2015/2015crtefp40/2015crtefp40.html"),
        ("2021", "CRTESPF", "110", "https://www.canlii.org/fr/ca/pslreb/doc/2021/2021crtespf110/2021crtespf110.html"),
        ("2009", "TDFP", "0013", "https://www.canlii.org/fr/ca/psst/doc/2009/2009tdfp13/2009tdfp13.html"),
        ("2019", "TSS", "1537", "https://www.canlii.org/fr/ca/sst/doc/2019/2019tss1537/2019tss1537.html"),
        ("2014", "SSTAD", "300", "https://www.canlii.org/en/ca/sst/doc/2014/2014sstad300/2014sstad300.html"),
        ("2014", "TSSDA", "300", "https://www.canlii.org/fr/ca/sst/doc/2014/2014tssda300/2014tssda300.html"),
        ("2019", "TPFD", "1", "https://www.canlii.org/fr/ca/psdpt/doc/2019/2019tpfd1/2019tpfd1.html"),
        ("2021", "TATCE", "22", "https://www.canlii.org/en/ca/tatc/doc/2021/2021tatce22/2021tatce22.html"),
        ("2021", "TATCF", "22", "https://www.canlii.org/fr/ca/tatc/doc/2021/2021tatcf22/2021tatcf22.html"),
        ("2022", "CI", "62", "https://www.canlii.org/fr/ca/oic/doc/2022/2022ci62/2022ci62.html"),
        ("2020", "SCTC", "1", "https://www.canlii.org/en/ca/sct/doc/2020/2020sctc1/2020sctc1.html"),
    ];
    let missing: Vec<_> = cases.iter().filter(|(year, code, number, _)| canlii_case_in(&neutral(year, code, number), Language::En, legal_citations::registry::registry()).is_none())
        .map(|(year, code, number, _)| format!("{year} {code} {number}")).collect();
    assert!(missing.is_empty(), "no CanLII URL for {missing:?}");
    for (year, code, number, url) in cases {
        assert_eq!(canlii(canlii_case_in(&neutral(year, code, number), Language::En, legal_citations::registry::registry())), url, "{year} {code} {number}");
        assert_eq!(canlii_citation_url(&format!("{year} {code} {number}"), "en").as_deref(), Some(url), "{year} {code} {number}");
    }
    // A decision number's leading zeros are not part of CanLII's id.
    assert_eq!(
        canlii_citation_url("2009 PSST 0013", "en").as_deref(),
        Some("https://www.canlii.org/en/ca/psst/doc/2009/2009psst13/2009psst13.html")
    );
    // Codes CanLII does not use in decision ids stay without a link (the Competition
    // Tribunal's decisions are CanLII-cited; SST's EI and income security divisions are not
    // published under their own codes; CRAC is also a reporter).
    for text in ["2020 Comp Trib 7", "2016 SSTADEI 250", "2017 SSTGDIS 32", "2026 CRAC 18"] {
        assert_eq!(canlii_citation_url(text, "en"), None, "{text}");
    }
}

#[test]
fn search_and_noteup() {
    assert_eq!(
        canlii_search("R v Jordan", SearchField::Id).unwrap(),
        "https://www.canlii.org/en/#search/id=R%20v%20Jordan"
    );
    assert_eq!(
        canlii_search(" unreasonable delay & s 11(b) ", SearchField::Text).unwrap(),
        "https://www.canlii.org/en/#search/text=unreasonable%20delay%20%26%20s%2011(b)"
    );
    assert_eq!(canlii_search("   ", SearchField::Text), None);
    assert_eq!(canlii_search(&"x".repeat(4097), SearchField::Text), None);
    // The noteup builds on the embedded registry, which routes SCC to CanLII.
    let jordan = case(0, "2016 SCC 27", 0).neutral("2016", "SCC", "27").court("scc").build();
    if let Some(noteup) = canlii_noteup(&jordan, "2016 SCC 27") {
        assert_eq!(
            noteup,
            "https://www.canlii.org/en/#search/origin1=%2Fen%2Fca%2Fscc%2Fdoc%2F2016%2F2016scc27%2F2016scc27.html&nquery1=2016%20SCC%2027"
        );
    }
}

#[test]
fn anchors() {
    let page = "https://www.canlii.org/en/ca/laws/stat/rsc-1985-c-c-46/latest/rsc-1985-c-c-46.html";
    let decision = "https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html";
    assert_eq!(source_anchor(decision, "paragraph", "par12").unwrap(), "par12");
    assert_eq!(source_anchor(page, "section", "7").unwrap(), "sec7");
    assert_eq!(source_anchor(page, "section", "7(2)").unwrap(), "sec7subsec2");
    assert_eq!(source_anchor(page, "section", "7(2)(a)").unwrap(), "sec7subsec2");
    assert_eq!(source_anchor(page, "article", "1457").unwrap(), "art1457");
    assert_eq!(source_anchor(page, "rule", "3").unwrap(), "rule3");
    let quebec = "https://www.canlii.org/fr/qc/laws/stat/cqlr-c-c-12/latest/cqlr-c-c-12.html";
    assert_eq!(source_anchor(quebec, "section", "18.1").unwrap(), "se:18_1");
    assert_eq!(source_anchor(page, "section", "abc"), None);
}

#[test]
fn with_pinpoint_only_for_a_single_pinpoint() {
    let page = "https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html";
    let one = case(0, "2016 SCC 27", 0).pin(PinpointKind::Paragraph, "5", None).build();
    assert_eq!(with_pinpoint(page, &one), format!("{page}#par5"));
    let two = case(0, "2016 SCC 27", 0)
        .pin(PinpointKind::Paragraph, "5", None)
        .pin(PinpointKind::Paragraph, "9", None)
        .build();
    assert_eq!(with_pinpoint(page, &two), page);
    let range = case(0, "2016 SCC 27", 0).pin(PinpointKind::Paragraph, "5", Some("9")).build();
    assert_eq!(with_pinpoint(page, &range), page);
}

#[test]
fn canlii_url_hosts() {
    assert!(is_canlii_url("https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html"));
    assert!(is_canlii_url("http://canlii.ca/t/1234"));
    assert!(is_canlii_url("https://primary.CanLII.ca./x"));
    assert!(is_canlii_url("https://user:pw@www.canlii.org:443/x"));
    assert!(!is_canlii_url("https://canlii.org.evil.com/x"));
    assert!(!is_canlii_url("https://notcanlii.org/x"));
    assert!(!is_canlii_url("www.canlii.org/en"));
    assert!(!is_canlii_url("mailto:someone@canlii.org"));
    assert!(!is_canlii_url(""));
}

#[test]
fn canlii_pdf_sibling() {
    assert_eq!(
        canlii_pdf_url("https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html").unwrap(),
        "https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.pdf"
    );
    assert_eq!(
        canlii_pdf_url("https://www.canlii.org/en/ukjcpc/doc/1932/1932canlii354/1932canlii354.html").unwrap(),
        "https://www.canlii.org/en/ukjcpc/doc/1932/1932canlii354/1932canlii354.pdf"
    );
    for rejected in [
        "http://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html",
        "https://canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html",
        "https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc28.html",
        "https://www.canlii.org/en/ca/scc/doc/2016/2015scc27/2015scc27.html",
        "https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html?x=1",
        "https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html#par5",
        "https://www.canlii.org/en/ca/laws/stat/rsc-1985-c-c-46/latest/rsc-1985-c-c-46.html",
        "not a url",
    ] {
        assert_eq!(canlii_pdf_url(rejected), None, "{rejected}");
    }
}

fn statute(core: &str, series: &str, year: Option<&str>, chapter: &str) -> Citation {
    cite(0, Form::Full, Authority::Statute, core, 0)
        .format(Format::StatuteVolume)
        .fields(|f| {
            f.series = Some(series.into());
            f.year = year.map(Into::into);
            f.chapter = Some(chapter.into());
        })
        .build()
}

fn regulation(core: &str, series: &str, year: Option<&str>, number: Option<&str>, chapter: Option<&str>) -> Citation {
    cite(0, Form::Full, Authority::Regulation, core, 0)
        .format(Format::RegulationSeries)
        .fields(|f| {
            f.series = Some(series.into());
            f.year = year.map(Into::into);
            f.regulation = number.map(Into::into);
            f.chapter = chapter.map(Into::into);
        })
        .build()
}

#[test]
fn canlii_legislation_ids() {
    let code = statute("RSC 1985, c C-46", "RSC", Some("1985"), "C-46");
    assert_eq!(
        canlii_legislation_in(&code, Language::En, registry()).unwrap(),
        "https://www.canlii.org/en/ca/laws/stat/rsc-1985-c-c-46/latest/rsc-1985-c-c-46.html"
    );
    let french = statute("LRC 1985, ch C-46", "LRC", Some("1985"), "ch C-46");
    assert_eq!(
        canlii_legislation_in(&french, Language::En, registry()).unwrap(),
        "https://www.canlii.org/en/ca/laws/stat/rsc-1985-c-c-46/latest/rsc-1985-c-c-46.html"
    );
    let annual = statute("SC 2012, c 1", "SC", Some("2012"), "1");
    assert_eq!(
        canlii_legislation_in(&annual, Language::En, registry()).unwrap(),
        "https://www.canlii.org/en/ca/laws/astat/sc-2012-c-1/latest/sc-2012-c-1.html"
    );
    let sor = regulation("SOR/2002-227", "SOR", Some("2002"), Some("227"), None);
    assert_eq!(
        canlii_legislation_in(&sor, Language::En, registry()).unwrap(),
        "https://www.canlii.org/en/ca/laws/regu/sor-2002-227/latest/sor-2002-227.html"
    );
    let ontario = regulation("O Reg 191/11", "O Reg", None, Some("191/11"), None);
    assert_eq!(
        canlii_legislation_in(&ontario, Language::En, registry()).unwrap(),
        "https://www.canlii.org/en/on/laws/regu/o-reg-191-11/latest/o-reg-191-11.html"
    );
    let crc = regulation("CRC, c 870", "CRC", None, None, Some("c 870"));
    assert_eq!(
        canlii_legislation_in(&crc, Language::En, registry()).unwrap(),
        "https://www.canlii.org/en/ca/laws/regu/crc-c-870/latest/crc-c-870.html"
    );
    let quebec = statute("CQLR c C-12", "CQLR", None, "c C-12");
    assert_eq!(
        canlii_legislation_in(&quebec, Language::Fr, registry()).unwrap(),
        "https://www.canlii.org/fr/qc/laws/stat/cqlr-c-c-12/latest/cqlr-c-c-12.html"
    );
}

#[test]
fn canlii_legislation_abstains_without_a_route() {
    let ontario = statute("SO 2006, c 21", "SO", Some("2006"), "21");
    assert_eq!(canlii_legislation_in(&ontario, Language::En, registry()), None);
    let unknown = statute("XYZ 2006, c 21", "XYZ", Some("2006"), "21");
    assert_eq!(canlii_legislation_in(&unknown, Language::En, registry()), None);
}

#[test]
fn justice_laws_urls() {
    let code = statute("RSC 1985, c C-46", "RSC", Some("1985"), "C-46");
    assert_eq!(justice_laws(&code, Language::En).unwrap(), "https://laws-lois.justice.gc.ca/eng/acts/C-46/");
    assert_eq!(justice_laws(&code, Language::Fr).unwrap(), "https://laws-lois.justice.gc.ca/fra/lois/C-46/");
    let sor = regulation("SOR/2002-227", "SOR", Some("2002"), Some("227"), None);
    assert_eq!(
        justice_laws(&sor, Language::En).unwrap(),
        "https://laws-lois.justice.gc.ca/eng/regulations/SOR-2002-227/"
    );
    assert_eq!(
        justice_laws(&sor, Language::Fr).unwrap(),
        "https://laws-lois.justice.gc.ca/fra/reglements/DORS-2002-227/"
    );
    let si = regulation("SI/2000-1", "SI", None, Some("2000-1"), None);
    assert_eq!(justice_laws(&si, Language::Fr).unwrap(), "https://laws-lois.justice.gc.ca/fra/reglements/TR-2000-1/");
    let crc = regulation("CRC, c 870", "CRC", None, None, Some("c 870"));
    assert_eq!(
        justice_laws(&crc, Language::En).unwrap(),
        "https://laws-lois.justice.gc.ca/eng/regulations/C.R.C.,_c._870/"
    );
    // Annual statutes and supplements have no id derivable from the citation.
    let annual = statute("SC 2001, c 27", "SC", Some("2001"), "27");
    assert_eq!(justice_laws(&annual, Language::En), None);
    let supplement = statute("RSC 1985, c 1 (5th Supp)", "RSC", Some("1985"), "1 (5th Supp)");
    assert_eq!(justice_laws(&supplement, Language::En), None);
    let old = statute("RSC 1970, c C-34", "RSC", Some("1970"), "C-34");
    assert_eq!(justice_laws(&old, Language::En), None);
}

#[test]
fn legislation_gov_uk_urls() {
    let mut act = statute("Human Rights Act 1998 (UK), c 42", "", Some("1998"), "42");
    act.fields.series = None;
    act.jurisdiction = Some("uk".into());
    assert_eq!(legislation_gov_uk(&act).unwrap(), "https://www.legislation.gov.uk/ukpga/1998/42");
    let mut regnal = act.clone();
    regnal.fields.year = Some("1867".into());
    regnal.fields.regnal = Some("30 & 31 Vict".into());
    regnal.fields.chapter = Some("3".into());
    assert_eq!(legislation_gov_uk(&regnal).unwrap(), "https://www.legislation.gov.uk/ukpga/Vict/30-31/3");
    let mut westminster = regnal.clone();
    westminster.fields.regnal = Some("22 & 23 Geo V".into());
    westminster.fields.chapter = Some("4".into());
    assert_eq!(legislation_gov_uk(&westminster).unwrap(), "https://www.legislation.gov.uk/ukpga/Geo5/22-23/4");
    let mut early = act.clone();
    early.fields.year = Some("1925".into());
    assert_eq!(legislation_gov_uk(&early), None);
    let mut canadian = act.clone();
    canadian.jurisdiction = Some("ca".into());
    assert_eq!(legislation_gov_uk(&canadian), None);
}

fn uk(core: &str, full: &str, year: &str, series: &str, number: &str) -> Citation {
    let mut citation = case(0, core, 0).neutral(year, series, number).build();
    citation.full_span.text = full.into();
    citation
}

#[test]
fn uk_find_case_law() {
    assert_eq!(
        uk_caselaw(&uk("[2019] UKSC 5", "[2019] UKSC 5", "2019", "UKSC", "5")).unwrap(),
        "https://caselaw.nationalarchives.gov.uk/uksc/2019/5"
    );
    assert_eq!(
        uk_caselaw(&uk("[2020] EWCA Civ 12", "[2020] EWCA Civ 12 at [5]", "2020", "EWCA Civ", "12")).unwrap(),
        "https://caselaw.nationalarchives.gov.uk/ewca/civ/2020/12"
    );
    assert_eq!(
        uk_caselaw(&uk("[2019] EWHC 123", "Smith v Jones [2019] EWHC 123 (Ch)", "2019", "EWHC", "123")).unwrap(),
        "https://caselaw.nationalarchives.gov.uk/ewhc/ch/2019/123"
    );
    // EWHC without its division, House of Lords, lettered family series: abstain.
    assert_eq!(uk_caselaw(&uk("[2019] EWHC 123", "[2019] EWHC 123", "2019", "EWHC", "123")), None);
    assert_eq!(uk_caselaw(&uk("[2005] UKHL 1", "[2005] UKHL 1", "2005", "UKHL", "1")), None);
    assert_eq!(uk_caselaw(&uk("[2020] EWFC 12", "[2020] EWFC 12 (B)", "2020", "EWFC", "12")), None);
}

#[test]
fn courtlistener_urls() {
    let us = case(0, "410 U.S. 113", 0).reporter(Some("1973"), Some("410"), "U.S.", "113").build();
    assert_eq!(courtlistener_in(&us, registry()).unwrap(), "https://www.courtlistener.com/c/U.S./410/113/");
    let supp = case(0, "5 F.Supp.2d 7", 0).reporter(None, Some("5"), "F.Supp.2d", "7").build();
    assert_eq!(courtlistener_in(&supp, registry()).unwrap(), "https://www.courtlistener.com/c/F.%20Supp.%202d/5/7/");
    let canadian = case(0, "26 DLR (4th) 200", 0).reporter(None, Some("26"), "DLR (4th)", "200").build();
    assert_eq!(courtlistener_in(&canadian, registry()), None);
    let blank = case(0, "600 U.S. ___", 0).reporter(None, Some("600"), "U.S.", "___").build();
    assert_eq!(courtlistener_in(&blank, registry()), None);
}

#[test]
fn best_url_routes_by_format() {
    assert_eq!(
        url_in(&neutral("2016", "SCC", "27"), Language::En, registry()).unwrap(),
        "https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html"
    );
    assert_eq!(
        url_in(&uk("[2019] UKSC 5", "[2019] UKSC 5", "2019", "UKSC", "5"), Language::En, registry()).unwrap(),
        "https://caselaw.nationalarchives.gov.uk/uksc/2019/5"
    );
    let si = regulation("SI/2000-1", "SI", None, Some("2000-1"), None);
    assert_eq!(
        url_in(&si, Language::En, registry()).unwrap(),
        "https://laws-lois.justice.gc.ca/eng/regulations/SI-2000-1/"
    );
    let book = cite(0, Form::Full, Authority::Book, "(Toronto: Irwin, 2019)", 0).format(Format::Publication).build();
    assert_eq!(url_in(&book, Language::En, registry()), None);
}

#[test]
fn encode_component_matches_javascript() {
    assert_eq!(encode_component("a b&c/d?e=f"), "a%20b%26c%2Fd%3Fe%3Df");
    assert_eq!(encode_component("-_.!~*'()"), "-_.!~*'()");
    assert_eq!(encode_component("é"), "%C3%A9");
}

#[test]
fn canlii_citation_keys_match_canlii_printed_citations() {
    use legal_citations::url::{canlii_citation_key, canlii_citation_keys, legislation_lookup};
    // A provider's periods and CanLII's McGill spelling fold to one key.
    assert_eq!(canlii_citation_key("R.S.O. 1990, c. H.8"), "rso-1990-c-h.8");
    assert_eq!(canlii_citation_key("RSO 1990, c H.8"), "rso-1990-c-h.8");
    assert_eq!(canlii_citation_key("RRS c T-18.1 Reg 1"), "rrs-c-t-18.1-reg-1");
    assert_eq!(canlii_citation_key("CQLR c C-25.01, r 0.1"), "cqlr-c-c-25.01-r-0.1");
    assert_eq!(canlii_citation_key("SO 2010, c 1, Sch 24"), "so-2010-c-1-sch-24");
    assert_eq!(canlii_citation_key("NWT Reg (Nu) 060-93"), "nwt-reg-nu-060-93");
    // The citation as written comes first; without its pinpoint, after every spelling of it.
    let keys = canlii_citation_keys("RSC 1985, c C-46, s 2");
    assert_eq!(keys[0], "rsc-1985-c-c-46-s-2");
    assert!(keys.iter().position(|key| key == "rsc-1985-c-c-46") > keys.iter().position(|key| key == "lrc-1985-c-c-46-s-2"));
    assert_eq!(canlii_citation_key("SC 2005, c 11, s 7"), "sc-2005-c-11-s-7");
    assert!(canlii_citation_keys("RLRQ c C-12").contains(&"cqlr-c-c-12".to_owned()));
    assert!(canlii_citation_keys("O. Reg. 191/2011").contains(&"o-reg-191-11".to_owned()));
    assert!(canlii_citation_keys("BC Reg 337/95").contains(&"bc-reg-337-1995".to_owned()));
    // Series the fixed lookup patterns never covered now yield CanLII's identifiers.
    for (citation, id, jurisdiction) in [
        ("RRO 1990, Reg 194", "rro-1990-reg-194", "on"), ("NLR 2/05", "nlr-2-05", "nl"),
        ("CQLR c C-12", "cqlr-c-c-12", "qc"), ("CCSM c E110", "ccsm-c-e110", "mb"),
        ("RRS c T-18.1 Reg 1", "rrs-c-t-18.1-reg-1", "sk"), ("YOIC 2020/68", "yoic-2020-68", "yk"),
        ("NWT Reg 089-2019", "nwt-reg-089-2019", "nt"), ("Nu Reg 001-2007", "nu-reg-001-2007", "nu"),
        ("PEI Reg EC342/06", "pei-reg-ec342-06", "pe"), ("NB Reg 84-20", "nb-reg-84-20", "nb"),
        ("SQ 2019, c 28", "sq-2019-c-28", "qc"), ("SNu 2008, c 15", "snu-2008-c-15", "nu"),
        ("RSO 1990, c H.8", "rso-1990-c-h8", "on"),
    ] {
        let lookup = legislation_lookup(citation);
        assert!(lookup.candidates.contains(&id.to_owned()), "{citation}: {:?}", lookup.candidates);
        assert_eq!(lookup.jurisdiction, jurisdiction, "{citation}");
    }
}
