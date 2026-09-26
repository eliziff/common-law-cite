mod downstream_common;

use downstream_common::*;
use legal_citations::format::{
    article, attach, case_name, collapse, database_citation, full, heading, ibid, normalize_citation, party,
    pinpoint, pinpoints, short_label, supra, ta_category, toa_sort_key, Article, Heading, Language, Style,
};
use legal_citations::{Authority, Form, Format, PinpointKind};

const EN: Style = Style {
    language: Language::En,
    range_dash: "-",
};

fn fr() -> Style {
    Style::french()
}

fn singles<'a>(values: &[&'a str]) -> Vec<(&'a str, Option<&'a str>)> {
    values.iter().map(|value| (*value, None)).collect()
}

#[test]
fn paragraph_labels() {
    assert_eq!(pinpoint(PinpointKind::Paragraph, &singles(&["12"]), EN), "at para 12");
    assert_eq!(pinpoint(PinpointKind::Paragraph, &[("12", Some("14"))], EN), "at paras 12-14");
    assert_eq!(pinpoint(PinpointKind::Paragraph, &singles(&["20", "23", "25"]), EN), "at paras 20, 23, 25");
    assert_eq!(pinpoint(PinpointKind::Paragraph, &singles(&["12", "13", "14", "20"]), EN), "at paras 12-14, 20");
}

#[test]
fn page_labels_never_use_p() {
    assert_eq!(pinpoint(PinpointKind::Page, &singles(&["353"]), EN), "at 353");
    assert_eq!(pinpoint(PinpointKind::Page, &singles(&["553", "559"]), EN), "at 553, 559");
    assert_eq!(pinpoint(PinpointKind::Page, &[("553", Some("559"))], EN), "at 553-559");
    for label in [
        pinpoint(PinpointKind::Page, &singles(&["353"]), EN),
        pinpoint(PinpointKind::Page, &singles(&["1", "5"]), EN),
    ] {
        assert!(!label.contains(" p ") && !label.contains(" pp "), "{label}");
    }
}

#[test]
fn section_labels_collapse_shared_roots() {
    assert_eq!(pinpoint(PinpointKind::Section, &singles(&["7"]), EN), "s 7");
    assert_eq!(pinpoint(PinpointKind::Section, &singles(&["7(2)"]), EN), "s 7(2)");
    assert_eq!(pinpoint(PinpointKind::Section, &singles(&["7(2)", "7(3)", "7(4)"]), EN), "ss 7(2)-(4)");
    assert_eq!(pinpoint(PinpointKind::Section, &[("7(2)", Some("7(4)"))], EN), "ss 7(2)-(4)");
    assert_eq!(pinpoint(PinpointKind::Section, &singles(&["20(a)", "20(b)(i)"]), EN), "ss 20(a), (b)(i)");
    assert_eq!(pinpoint(PinpointKind::Section, &singles(&["7", "9"]), EN), "ss 7, 9");
    assert_eq!(pinpoint(PinpointKind::Section, &singles(&["8(1)(a)", "8(1)(b)", "8(1)(c)"]), EN), "ss 8(1)(a)-(c)");
}

#[test]
fn decimal_sections_and_roman_suffixes() {
    assert_eq!(pinpoint(PinpointKind::Section, &singles(&["2.1", "2.2"]), EN), "ss 2.1-2.2");
    assert_eq!(pinpoint(PinpointKind::Section, &singles(&["2.1", "3.1"]), EN), "ss 2.1, 3.1");
    assert_eq!(pinpoint(PinpointKind::Section, &singles(&["5(a)(ii)", "5(a)(iii)"]), EN), "ss 5(a)(ii)-(iii)");
}

#[test]
fn rule_article_note_labels() {
    assert_eq!(pinpoint(PinpointKind::Rule, &singles(&["3"]), EN), "r 3");
    assert_eq!(pinpoint(PinpointKind::Rule, &singles(&["3", "4"]), EN), "rr 3-4");
    assert_eq!(pinpoint(PinpointKind::Article, &singles(&["1457"]), EN), "art 1457");
    assert_eq!(pinpoint(PinpointKind::Article, &singles(&["1457", "1458"]), EN), "arts 1457-1458");
    assert_eq!(pinpoint(PinpointKind::Footnote, &singles(&["4"]), EN), "n 4");
    assert_eq!(pinpoint(PinpointKind::Footnote, &singles(&["4", "7"]), EN), "nn 4, 7");
}

#[test]
fn french_labels() {
    assert_eq!(pinpoint(PinpointKind::Paragraph, &singles(&["12"]), fr()), "au para 12");
    assert_eq!(pinpoint(PinpointKind::Paragraph, &[("12", Some("14"))], fr()), "aux paras 12-14");
    assert_eq!(pinpoint(PinpointKind::Page, &singles(&["353"]), fr()), "\u{e0} la p 353");
    assert_eq!(pinpoint(PinpointKind::Page, &singles(&["353", "359"]), fr()), "aux pp 353, 359");
    assert_eq!(pinpoint(PinpointKind::Article, &singles(&["1457"]), fr()), "art 1457");
    assert_eq!(pinpoint(PinpointKind::Section, &singles(&["7(2)"]), fr()), "art 7(2)");
}

#[test]
fn en_dash_style_for_beaver() {
    let style = Style {
        range_dash: "\u{2013}",
        ..Style::default()
    };
    assert_eq!(pinpoint(PinpointKind::Paragraph, &[("12", Some("14"))], style), "at paras 12\u{2013}14");
}

#[test]
fn collapse_dedupes_and_keeps_order() {
    assert_eq!(collapse(&singles(&["5", "5", "6", "9"]), "-"), "5-6, 9");
    assert_eq!(collapse(&singles(&[" 12 ", ""]), "-"), "12");
    // Front-matter pages do not parse as locators and never collapse.
    assert_eq!(collapse(&singles(&["xii", "xiii"]), "-"), "xii, xiii");
}

#[test]
fn pinpoints_from_citation_values() {
    let citation = case(0, "2016 SCC 27", 20)
        .pin(PinpointKind::Paragraph, "20", None)
        .pin(PinpointKind::Paragraph, "23", None)
        .pin(PinpointKind::Paragraph, "25", None)
        .build();
    assert_eq!(pinpoints(&citation.pinpoints, EN), "at paras 20, 23, 25");
    let statute = case(0, "RSC 1985, c C-46", 0)
        .pin(PinpointKind::Section, "7(2)", Some("7(4)"))
        .build();
    assert_eq!(pinpoints(&statute.pinpoints, EN), "ss 7(2)-(4)");
}

#[test]
fn attach_ibid_supra() {
    assert_eq!(attach("R v Jordan, 2016 SCC 27", "at para 5"), "R v Jordan, 2016 SCC 27 at para 5");
    assert_eq!(attach("Criminal Code, RSC 1985, c C-46", "s 7"), "Criminal Code, RSC 1985, c C-46, s 7");
    assert_eq!(attach("X", ""), "X");
    assert_eq!(ibid(None), "Ibid");
    assert_eq!(ibid(Some("at para 5")), "Ibid at para 5");
    assert_eq!(ibid(Some("au para 5")), "Ibid au para 5");
    assert_eq!(ibid(Some("s 7")), "Ibid, s 7");
    assert_eq!(supra("Jordan", 4, None), "Jordan, supra note 4");
    assert_eq!(supra("Jordan", 4, Some("at para 5")), "Jordan, supra note 4 at para 5");
    assert_eq!(supra("Charter", 2, Some("s 7")), "Charter, supra note 2, s 7");
}

#[test]
fn crown_becomes_r() {
    for style in ["R. v. Jordan", "Regina v. Jordan", "The Queen v. Jordan", "Her Majesty the Queen v. Jordan", "Rex v Jordan", "R v Jordan"] {
        assert_eq!(case_name(style, Language::En), "R v Jordan", "{style}");
    }
    assert_eq!(case_name("Sa Majesté la Reine c. Jordan", Language::Fr), "R c Jordan");
    assert_eq!(case_name("R. v. Jordan", Language::Fr), "R c Jordan");
}

#[test]
fn attorney_general() {
    assert_eq!(case_name("Attorney General of Canada v. Bedford", Language::En), "Canada (AG) v Bedford");
    assert_eq!(case_name("Canada (Attorney General) v. Bedford", Language::En), "Canada (AG) v Bedford");
    assert_eq!(case_name("Canada (Attorney General) v. Bedford", Language::Fr), "Canada (PG) c Bedford");
    assert_eq!(case_name("Procureur général du Québec c. Smith", Language::Fr), "Québec (PG) c Smith");
    assert_eq!(party("Attorney General for Ontario", Language::En), "Ontario (AG)");
}

#[test]
fn periods_and_abbreviations() {
    assert_eq!(case_name("Hunter v. Southam Inc.", Language::En), "Hunter v Southam Inc");
    assert_eq!(case_name("R. v. D.B.", Language::En), "R v DB");
    assert_eq!(case_name("Smith v. Jones Ltd. et al.", Language::En), "Smith v Jones Ltd");
    assert_eq!(case_name("R. v. J. (J.T.)", Language::En), "R v J (JT)");
    assert_eq!(case_name("Smith vs. Jones", Language::En), "Smith v Jones");
}

#[test]
fn re_styles() {
    assert_eq!(case_name("Eurig Estate, Re", Language::En), "Re Eurig Estate");
    assert_eq!(case_name("Eurig Estate (Re)", Language::En), "Re Eurig Estate");
    assert_eq!(case_name("In re Smith", Language::En), "Re Smith");
    assert_eq!(case_name("Reference re Secession of Quebec", Language::En), "Reference re Secession of Quebec");
    assert_eq!(case_name("R. v. Jordan (S.C.C.)", Language::En), "R v Jordan");
}

#[test]
fn short_labels() {
    let jordan = case(0, "2016 SCC 27", 20).style("R v Jordan").parties("R", "Jordan").build();
    assert_eq!(short_label(&jordan, Language::En), "Jordan");
    let hunter = case(0, "[1984] 2 SCR 145", 30).style("Hunter v. Southam Inc.").build();
    assert_eq!(short_label(&hunter, Language::En), "Hunter");
    let crown_style = case(0, "2016 SCC 27", 20).style("R. v. Jordan").build();
    assert_eq!(short_label(&crown_style, Language::En), "Jordan");
    let explicit = case(0, "2016 SCC 27", 20).style("R v Jordan").explicit("Jordan SCC").build();
    assert_eq!(short_label(&explicit, Language::En), "Jordan SCC");
    let bare = case(0, "[1964] S.C.R. 642", 0).build();
    assert_eq!(short_label(&bare, Language::En), "[1964] SCR 642");
    let statute = cite(0, Form::Full, Authority::Statute, "RSC 1985, c C-46", 20).style("Criminal Code").build();
    assert_eq!(short_label(&statute, Language::En), "Criminal Code");
}

#[test]
fn normalize_and_database_citations() {
    assert_eq!(normalize_citation("[1964] S.C.R. 642"), "[1964] SCR 642");
    assert_eq!(normalize_citation("26 D.L.R. (4th) 200"), "26 DLR (4th) 200");
    assert_eq!(normalize_citation("2004 CanLII 12345 (CanLII)"), "2004 CanLII 12345");
    assert_eq!(normalize_citation("R.S.C. 1985, c. C-46"), "RSC 1985, c C-46");
    assert_eq!(normalize_citation("2016 SCC 27"), "2016 SCC 27");
    assert_eq!(database_citation("2019 CarswellOnt 123"), "2019 CarswellOnt 123 (WL Can)");
    assert_eq!(database_citation("[2019] O.J. No. 45"), "[2019] OJ No 45 (QL)");
    assert_eq!(database_citation("2016 SCC 27"), "2016 SCC 27");
}

#[test]
fn full_citations() {
    let jordan = case(0, "2016 SCC 27", 20)
        .neutral("2016", "SCC", "27")
        .style("R. v. Jordan")
        .pin(PinpointKind::Paragraph, "5", None)
        .build();
    assert_eq!(full(&jordan, EN), "R v Jordan, 2016 SCC 27 at para 5");
    assert_eq!(full(&jordan, fr()), "R c Jordan, 2016 SCC 27 au para 5");
    let code = cite(0, Form::Full, Authority::Statute, "R.S.C. 1985, c. C-46", 20)
        .format(Format::StatuteVolume)
        .style("Criminal Code")
        .pin(PinpointKind::Section, "7", None)
        .build();
    assert_eq!(full(&code, EN), "Criminal Code, RSC 1985, c C-46, s 7");
    let database = case(0, "2019 CarswellOnt 123", 20).format(Format::Database).style("Smith v. Jones").build();
    assert_eq!(full(&database, EN), "Smith v Jones, 2019 CarswellOnt 123 (WL Can)");
}

#[test]
fn articles() {
    let one = Article {
        authors: vec!["Jane Doe".into()],
        title: "Speciesism".into(),
        year: Some("2010".into()),
        volume: Some("55".into()),
        issue: Some("3".into()),
        journal: Some("McGill LJ".into()),
        first_page: Some("1".into()),
        document: None,
    };
    assert_eq!(article(&one), "Jane Doe, \u{201c}Speciesism\u{201d} (2010) 55:3 McGill LJ 1");
    let three = Article {
        authors: vec!["A".into(), "B".into(), "C".into()],
        ..one.clone()
    };
    assert!(article(&three).starts_with("A, B & C, "));
    let four = Article {
        authors: vec!["A".into(), "B".into(), "C".into(), "D".into()],
        ..one.clone()
    };
    assert!(article(&four).starts_with("A et al, "));
    let unpublished = Article {
        authors: vec!["Jane Doe".into()],
        title: "Draft".into(),
        year: Some("2021-05-01".into()),
        journal: None,
        document: Some("SSRN 12345".into()),
        ..Article::default()
    };
    assert_eq!(article(&unpublished), "Jane Doe, \u{201c}Draft\u{201d} (2021), SSRN 12345");
    // Beaver's row without volume or page.
    let row = Article {
        authors: vec![],
        title: "T".into(),
        year: Some("2020".into()),
        journal: Some("Journal".into()),
        ..Article::default()
    };
    assert_eq!(article(&row), "\u{201c}T\u{201d} (2020) Journal");
}

#[test]
fn toa_sort_keys() {
    let mut names = vec!["R v Jordan", "Reference re Secession of Quebec", "The Queen v Adams", "Re Eurig Estate", "Babcock v Canada", "Évêque v Smith"];
    names.sort_by_key(|name| toa_sort_key(name));
    assert_eq!(names, vec!["Babcock v Canada", "Re Eurig Estate", "Évêque v Smith", "R v Jordan", "The Queen v Adams", "Reference re Secession of Quebec"]);
    assert_eq!(toa_sort_key("R v Jordan"), toa_sort_key("jordan"));
    assert!(toa_sort_key("Smith 9") < toa_sort_key("Smith 10"));
    assert!(toa_sort_key("Bill C-9") < toa_sort_key("Bill C-10"));
}

#[test]
fn toa_categories_and_headings() {
    assert_eq!(ta_category(Authority::Case), 1);
    assert_eq!(ta_category(Authority::Statute), 2);
    assert_eq!(ta_category(Authority::Treaty), 3);
    assert_eq!(ta_category(Authority::CourtRule), 4);
    assert_eq!(ta_category(Authority::Journal), 5);
    assert_eq!(ta_category(Authority::Book), 5);
    assert_eq!(ta_category(Authority::Regulation), 6);
    assert_eq!(ta_category(Authority::Constitution), 7);
    assert_eq!(ta_category(Authority::Debate), 3);
    assert_eq!(heading(Authority::Case).label(Language::Fr), "Jurisprudence");
    assert_eq!(heading(Authority::Statute).label(Language::Fr), "L\u{e9}gislation");
    assert_eq!(heading(Authority::Regulation).label(Language::Fr), "R\u{e8}glements");
    assert_eq!(heading(Authority::Journal).label(Language::Fr), "Doctrine");
    assert_eq!(heading(Authority::Journal).label(Language::En), "Secondary sources");
    assert_eq!(heading(Authority::ParliamentaryPaper), Heading::Government);
    assert_eq!(Heading::Government.ta_category(), 3);
    assert_eq!(Language::from_code("fr-CA"), Language::Fr);
    assert_eq!(Language::from_code("en"), Language::En);
}
