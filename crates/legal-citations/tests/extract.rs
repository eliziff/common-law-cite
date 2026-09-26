//! Extraction stages (find, classify, metadata) through `extract`.

use legal_citations::find::note_references;
use legal_citations::{
    extract, Authority, Citation, Form, Format, NoteDirection, Options, ParentheticalKind,
    PinpointKind,
};

fn cite(text: &str) -> Vec<Citation> {
    extract(text, &Options::default())
}

fn one(text: &str) -> Citation {
    let citations = cite(text);
    assert_eq!(citations.len(), 1, "{text}: {citations:#?}");
    citations.into_iter().next().unwrap()
}

fn cores(citations: &[Citation]) -> Vec<&str> {
    citations.iter().map(|citation| citation.span.text.as_str()).collect()
}

fn pins(citation: &Citation) -> Vec<(PinpointKind, String, Option<String>)> {
    citation
        .pinpoints
        .iter()
        .map(|pinpoint| (pinpoint.kind, pinpoint.first.clone(), pinpoint.last.clone()))
        .collect()
}

fn pin(kind: PinpointKind, first: &str) -> (PinpointKind, String, Option<String>) {
    (kind, first.to_owned(), None)
}

fn range(kind: PinpointKind, first: &str, last: &str) -> (PinpointKind, String, Option<String>) {
    (kind, first.to_owned(), Some(last.to_owned()))
}

fn court(citation: &Citation) -> Option<&str> {
    citation.court.as_ref().map(|court| court.id.as_str())
}

/// Full spans never overlap, and the text is rebuilt exactly from them and
/// the gaps between them.
fn assert_lossless(text: &str, citations: &[Citation]) {
    let mut rebuilt = String::new();
    let mut cursor = 0;
    for citation in citations {
        let span = &citation.full_span;
        assert!(span.start >= cursor, "{text}: overlapping {citations:#?}");
        assert_eq!(&text[span.start..span.end], span.text);
        rebuilt.push_str(&text[cursor..span.start]);
        rebuilt.push_str(&span.text);
        cursor = span.end;
    }
    rebuilt.push_str(&text[cursor..]);
    assert_eq!(rebuilt, text);
}

// ---------------------------------------------------------------------------
// Ported from legal-structure-parser citator tests.

#[test]
fn citation_presence_accepts_plain_text_and_unicode_case_names() {
    assert!(!legal_citations::has_citation("no citation here at all"));
    assert!(legal_citations::has_citation("R. v. Jordan, 2016 SCC 27"));
    assert!(legal_citations::has_citation("Éditions Écosociété Inc. v. Banro Corp."));
}

#[test]
fn citation_occurrence_separates_style_core_and_multiple_pinpoints() {
    let text = "See R. v. Jordan, 2016 SCC 27 at paras. 20, 23 and 25.";
    let citation = one(text);
    assert_eq!(citation.full_span.text, "R. v. Jordan, 2016 SCC 27 at paras. 20, 23 and 25");
    assert_eq!(citation.style.as_ref().unwrap().text, "R. v. Jordan");
    assert_eq!(citation.span.text, "2016 SCC 27");
    assert_eq!(citation.authority, Authority::Case);
    assert_eq!(citation.short_name.as_deref(), Some("R. v. Jordan"));
    assert_eq!(
        pins(&citation),
        [
            pin(PinpointKind::Paragraph, "20"),
            pin(PinpointKind::Paragraph, "23"),
            pin(PinpointKind::Paragraph, "25"),
        ]
    );
    assert!(citation.span.end <= citation.pinpoints[0].span.start);
    assert!(citation
        .pinpoints
        .windows(2)
        .all(|pair| pair[0].span.end <= pair[1].span.start));
    assert_eq!(citation.signal.as_ref().unwrap().text, "see");
}

#[test]
fn citation_occurrences_keep_repeated_matches_distinct_and_short_forms_local() {
    let text = "Hansman v Neufeld, 2023 SCC 14 [Hansman]. Then 2023 SCC 14 at para 9.";
    let citations = cite(text);
    assert_eq!(citations.len(), 2);
    assert_eq!(citations[0].short_name.as_deref(), Some("Hansman v Neufeld"));
    assert_eq!(citations[0].explicit_short_name.as_deref(), Some("Hansman"));
    assert_eq!(citations[0].full_span.text, "Hansman v Neufeld, 2023 SCC 14 [Hansman].");
    assert_eq!(citations[1].span.text, "2023 SCC 14");
    assert_eq!(citations[1].pinpoints[0].first, "9");
    assert!(citations[0].full_span.end <= citations[1].full_span.start);
}

#[test]
fn citation_offsets_are_bytes() {
    let text = "🦫 Éditions Écosociété Inc. v. Banro Corp., 2012 SCC 18 at para 7";
    let citation = one(text);
    assert_eq!(citation.full_span.start, "🦫 ".len());
    assert_eq!(citation.span.start, "🦫 Éditions Écosociété Inc. v. Banro Corp., ".len());
    assert_eq!(citation.style.as_ref().unwrap().text, "Éditions Écosociété Inc. v. Banro Corp.");
    assert_eq!(citation.pinpoints[0].first, "7");
}

#[test]
fn authority_references_reuse_reference_and_pinpoint_grammars() {
    let text = "🦫 Ibid at para 7; Smith, supra note 4 at pp. 10-11.";
    let citations = cite(text);
    assert_eq!(citations.len(), 2);
    assert_eq!(citations[0].form, Form::Ibid);
    assert_eq!(citations[0].span.start, "🦫 ".len());
    assert_eq!(citations[0].pinpoints[0].first, "7");
    assert_eq!(citations[1].form, Form::Supra);
    assert_eq!(citations[1].fields.note, Some(4));
    // A range is one pinpoint now, not two.
    assert_eq!(pins(&citations[1]), [range(PinpointKind::Page, "10", "11")]);
}

#[test]
fn reporter_citations_retain_case_identity_and_full_style() {
    for citation in [
        "[2015] 1 S.C.R. 331",
        "[2015] 1 SCR 331",
        "[2015] 1 R.C.S. 331",
        "(1994) 117 DLR (4th) 577",
        "(2003), 227 DLR (4th) 282",
        "(1895), 24 SCR 650",
    ] {
        let text = format!("See Carter v. Canada (Attorney General), {citation} at para 7.");
        let item = one(&text);
        assert_eq!(item.authority, Authority::Case, "{citation}");
        assert_eq!(item.format, Some(Format::Reporter), "{citation}");
        assert_eq!(item.span.text, citation);
        assert_eq!(item.short_name.as_deref(), Some("Carter v. Canada (Attorney General)"));
        assert_eq!(item.pinpoints[0].first, "7");
    }
}

#[test]
fn article_dates_are_not_authorities() {
    for month in [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ] {
        assert!(cite(&format!("Vice, 21 {month} 2016.")).is_empty());
    }
    assert_eq!(cite("123 Mass 456").len(), 1);
}

#[test]
fn numbered_case_names_do_not_lose_their_first_words() {
    let text = "See 40 Days for Life v. Dietrich, 2024 ONCA 599 at para 2.";
    let citation = one(text);
    assert_eq!(citation.short_name.as_deref(), Some("40 Days for Life v. Dietrich"));
    assert_eq!(&text[citation.full_span.start..citation.full_span.end], citation.full_span.text);
}

#[test]
fn citation_expands_parenthesized_left_party() {
    let citation = one("Quebec (Attorney General) v. Blaikie, [1979] 2 SCR 1016");
    assert_eq!(citation.authority, Authority::Case);
    assert_eq!(
        citation.full_span.text,
        "Quebec (Attorney General) v. Blaikie, [1979] 2 SCR 1016"
    );
    assert_eq!(citation.span.text, "[1979] 2 SCR 1016");
    assert_eq!(citation.short_name.as_deref(), Some("Quebec (Attorney General) v. Blaikie"));
    assert!(citation.reasons.iter().any(|reason| reason == "same_text_style"));
    let parties = citation.parties.unwrap();
    assert_eq!(parties.plaintiff, "Quebec (Attorney General)");
    assert_eq!(parties.defendant, "Blaikie");
}

#[test]
fn numbered_paragraph_labels_and_lead_in_stay_out_of_style() {
    for (text, styled) in [
        ("1. In R v Oakes, [1986] 1 SCR 103 at 138, the Court set out the test.", "R v Oakes, [1986] 1 SCR 103"),
        ("3. R v Jordan, 2016 SCC 27 at para 5.", "R v Jordan, 2016 SCC 27"),
        ("[14] Haaretz.com v Goldhar, 2018 SCC 28 at para 12.", "Haaretz.com v Goldhar, 2018 SCC 28"),
        ("(b) In Smith v Jones, 2020 ONCA 11, the court agreed.", "Smith v Jones, 2020 ONCA 11"),
        ("1985 Sawridge Trust v. Alberta, 2017 ABCA 400", "1985 Sawridge Trust v. Alberta, 2017 ABCA 400"),
    ] {
        let citation = cite(text).remove(0);
        assert_eq!(&text[citation.full_span.start..citation.span.end], styled, "{text}");
    }
}

#[test]
fn non_party_parentheticals_stay_out_of_style() {
    for text in [
        "X (1998) v. Smith, 2020 SCC 1",
        "X (2d) v. Smith, 2012 SCC 1",
        "X (see below) v. Smith, 2015 SCC 1",
    ] {
        let citation = cite(text).pop().unwrap();
        assert_eq!(citation.authority, Authority::Case);
        assert!(citation.style.is_none(), "{text}");
        assert!(!citation.reasons.iter().any(|reason| reason == "same_text_style"));
    }
}

// ---------------------------------------------------------------------------
// Classification: neutral citations.

#[test]
fn neutral_citations_read_year_court_and_number() {
    let citation = one("R v Jordan, 2016 SCC 27");
    assert_eq!(citation.format, Some(Format::Neutral));
    assert_eq!(citation.fields.year.as_deref(), Some("2016"));
    assert_eq!(citation.fields.series.as_deref(), Some("SCC"));
    assert_eq!(citation.fields.number.as_deref(), Some("27"));
    assert_eq!(court(&citation), Some("scc"));
    assert_eq!(citation.court.as_ref().unwrap().text, "SCC");
    assert_eq!(citation.jurisdiction.as_deref(), Some("ca"));
}

#[test]
fn french_neutral_citations_map_to_the_english_court() {
    let citation = one("Voir R c Jordan, 2016 CSC 27, au para 12.");
    assert_eq!(court(&citation), Some("scc"));
    assert_eq!(citation.language.as_deref(), Some("fr"));
    assert_eq!(citation.signal.as_ref().unwrap().text, "voir");
    assert_eq!(citation.style.as_ref().unwrap().text, "R c Jordan");
    let parties = citation.parties.as_ref().unwrap();
    assert_eq!((parties.plaintiff.as_str(), parties.defendant.as_str()), ("R", "Jordan"));
    assert_eq!(pins(&citation), [pin(PinpointKind::Paragraph, "12")]);
}

#[test]
fn tribunal_neutral_citations_are_found_and_classified() {
    let citation = one("Commissioner of Competition v Superior Propane, 2020 Comp Trib 6 at para 3.");
    assert_eq!(citation.span.text, "2020 Comp Trib 6");
    assert_eq!(citation.format, Some(Format::Neutral));
    assert_eq!(citation.fields.series.as_deref(), Some("Comp Trib"));
    assert!(citation.court.is_some());
    let labour = one("Nav Canada, 2000 CIRB LD 213.");
    assert_eq!(labour.span.text, "2000 CIRB LD 213");
    assert_eq!(labour.fields.number.as_deref(), Some("213"));
    assert!(labour.court.is_some());
}

#[test]
fn commonwealth_bracketed_neutral_citations() {
    for (text, core, id, jurisdiction) in [
        ("R (Miller) v Secretary of State [2019] UKSC 5", "[2019] UKSC 5", "uksc", "uk"),
        ("Smith v Jones [2003] EWCA Civ 1", "[2003] EWCA Civ 1", "ewca-civ", "uk"),
        ("Love v Commonwealth [2020] HCA 5", "[2020] HCA 5", "hca", "au"),
        ("Ellis v R [2020] NZSC 5", "[2020] NZSC 5", "nzsc", "nz"),
    ] {
        let citation = one(text);
        assert_eq!(citation.span.text, core);
        assert_eq!(citation.format, Some(Format::Neutral), "{text}");
        assert_eq!(court(&citation), Some(id), "{text}");
        assert!(citation.jurisdiction.as_deref().unwrap().starts_with(jurisdiction), "{text}");
    }
}

#[test]
fn colliding_court_identifiers_are_told_apart_by_brackets() {
    let citations = cite("Wu v Minister [2020] FCA 5; Canada v Wu, 2020 FCA 5.");
    assert_eq!(court(&citations[0]), Some("fca-au"));
    assert_eq!(citations[0].jurisdiction.as_deref(), Some("au"));
    assert_eq!(court(&citations[1]), Some("fca"));
    assert_eq!(citations[1].jurisdiction.as_deref(), Some("ca"));
}

#[test]
fn uk_bracketed_paragraph_pinpoints() {
    let citation = one("Montgomery v Lanarkshire Health Board [2015] UKSC 11 at [87]-[90].");
    assert_eq!(pins(&citation), [range(PinpointKind::Paragraph, "87", "90")]);
}

// ---------------------------------------------------------------------------
// Classification: reporters, CanLII and databases.

#[test]
fn year_as_volume_reports_are_cases_with_canonical_reporters() {
    let jordan = one("R v Jordan, [2016] 1 SCR 631 at paras 62-64.");
    assert_eq!(jordan.authority, Authority::Case);
    assert_eq!(jordan.format, Some(Format::Reporter));
    assert_eq!(jordan.fields.year.as_deref(), Some("2016"));
    assert_eq!(jordan.fields.volume.as_deref(), Some("1"));
    assert_eq!(jordan.fields.reporter.as_deref(), Some("SCR"));
    assert_eq!(jordan.fields.reporter_canonical.as_deref(), Some("SCR"));
    assert_eq!(jordan.fields.page.as_deref(), Some("631"));
    assert_eq!(court(&jordan), Some("scc"));
    // A range is one pinpoint and keeps paragraph 63 inside it.
    assert_eq!(pins(&jordan), [range(PinpointKind::Paragraph, "62", "64")]);

    let donoghue = one("Donoghue v Stevenson [1932] AC 562 at 580.");
    assert_eq!(donoghue.authority, Authority::Case);
    assert_eq!(donoghue.fields.volume, None);
    assert_eq!(donoghue.fields.reporter_canonical.as_deref(), Some("AC"));
    assert_eq!(pins(&donoghue), [pin(PinpointKind::Page, "580")]);

    let mabo = one("Mabo v Queensland (No 2) (1992) 175 CLR 1 at 42.");
    assert_eq!(mabo.authority, Authority::Case);
    assert_eq!(mabo.format, Some(Format::Reporter));
    assert_eq!(mabo.jurisdiction.as_deref(), Some("au"));
    assert_eq!(mabo.style.as_ref().unwrap().text, "Mabo v Queensland (No 2)");
}

#[test]
fn report_start_pages_for_scr_headers() {
    // legal-structure-parser reads the page a reported judgment starts on.
    for header in ["[2016] 1 S.C.R. 631", "[2016] 1 R.C.S. 631", "[2016] 1 SCR 631"] {
        let citation = one(header);
        assert_eq!(citation.authority, Authority::Case, "{header}");
        assert_eq!(citation.format, Some(Format::Reporter), "{header}");
        assert_eq!(citation.fields.page.as_deref(), Some("631"), "{header}");
        assert_eq!(citation.fields.reporter_canonical.as_deref(), Some("SCR"), "{header}");
    }
    assert_eq!(one("[2016] 1 R.C.S. 631").language.as_deref(), Some("fr"));
}

#[test]
fn colliding_report_series_are_told_apart_by_year_style() {
    let citations = cite("Smith v Jones (2001) 110 FCR 1; Doe v Canada, [2005] 1 FCR 123.");
    assert_eq!(citations[0].jurisdiction.as_deref(), Some("au"));
    assert_eq!(citations[1].jurisdiction.as_deref(), Some("ca"));
}

#[test]
fn canlii_citations_take_their_court_from_the_parenthetical() {
    let citation = one("R v Smith, 2004 CanLII 12345 (ON CA) at para 5.");
    assert_eq!(citation.format, Some(Format::CanLii));
    assert_eq!(citation.fields.year.as_deref(), Some("2004"));
    assert_eq!(citation.fields.number.as_deref(), Some("12345"));
    assert_eq!(court(&citation), Some("onca"));
    assert_eq!(citation.jurisdiction.as_deref(), Some("ca-on"));
    assert_eq!(citation.parentheticals[0].kind, ParentheticalKind::Court);
    assert_eq!(pins(&citation), [pin(PinpointKind::Paragraph, "5")]);
}

#[test]
fn database_identifiers() {
    let citations = cite(
        "Smith v Jones, 2019 CarswellOnt 123 (Ont SCJ); Doe v Roe, [2019] OJ No 45 (QL); Roe v Doe, 2019 WL 123456 (2d Cir. 1999).",
    );
    assert_eq!(cores(&citations), ["2019 CarswellOnt 123", "[2019] OJ No 45", "2019 WL 123456"]);
    for citation in &citations {
        assert_eq!(citation.format, Some(Format::Database), "{}", citation.span.text);
        assert_eq!(citation.authority, Authority::Case);
    }
    assert_eq!(citations[0].fields.reporter.as_deref(), Some("CarswellOnt"));
    assert_eq!(court(&citations[0]), Some("onsc"));
    assert_eq!(citations[0].jurisdiction.as_deref(), Some("ca-on"));
    assert_eq!(citations[1].fields.number.as_deref(), Some("45"));
    assert_eq!(citations[1].parentheticals[0].kind, ParentheticalKind::Source);
    assert_eq!(citations[2].fields.reporter.as_deref(), Some("WL"));
    assert_eq!(court(&citations[2]), Some("ca2"));
    assert_eq!(citations[2].fields.year.as_deref(), Some("2019"));
}

#[test]
fn us_reporters_bluebook_pinpoints_and_date_parentheticals() {
    let citation = cite("Roe v. Wade, 410 U.S. 113, 153 (1973).").remove(0);
    assert_eq!(citation.format, Some(Format::Reporter));
    assert_eq!(citation.fields.volume.as_deref(), Some("410"));
    assert_eq!(citation.fields.reporter_canonical.as_deref(), Some("U.S."));
    assert_eq!(citation.fields.page.as_deref(), Some("113"));
    assert_eq!(citation.fields.year.as_deref(), Some("1973"));
    assert_eq!(pins(&citation), [pin(PinpointKind::Page, "153")]);
    assert_eq!(citation.parentheticals[0].kind, ParentheticalKind::Date);
    assert_eq!(citation.full_span.text, "Roe v. Wade, 410 U.S. 113, 153 (1973)");
}

#[test]
fn us_journals_are_journals_not_cases() {
    let citation = one("Richard Posner, The Problems, 100 Harv. L. Rev. 1234, 1240 (1987).");
    assert_eq!(citation.authority, Authority::Journal);
    assert_eq!(citation.fields.volume.as_deref(), Some("100"));
    assert_eq!(citation.fields.page.as_deref(), Some("1234"));
    assert_eq!(pins(&citation), [pin(PinpointKind::Page, "1240")]);
    assert_eq!(citation.style.as_ref().unwrap().text, "Richard Posner, The Problems");
}

#[test]
fn mcgill_journal_articles() {
    let citation = one("Alice Woolley, \"Lawyer Regulation in Canada\" (2017) 95:4 Can Bar Rev 893 at 900.");
    assert_eq!(citation.authority, Authority::Journal);
    assert_eq!(citation.format, Some(Format::Publication));
    assert_eq!(citation.fields.year.as_deref(), Some("2017"));
    assert_eq!(citation.fields.volume.as_deref(), Some("95"));
    assert_eq!(citation.fields.reporter.as_deref(), Some("Can Bar Rev"));
    assert_eq!(citation.fields.page.as_deref(), Some("893"));
    assert_eq!(pins(&citation), [pin(PinpointKind::Page, "900")]);
}

// ---------------------------------------------------------------------------
// Classification: legislation.

#[test]
fn statutes_and_french_statutes() {
    let citations = cite("Criminal Code, RSC 1985, c C-46, s 718; Code criminel, LRC 1985, ch C-46, art 7.");
    assert_eq!(citations[0].authority, Authority::Statute);
    assert_eq!(citations[0].format, Some(Format::StatuteVolume));
    assert_eq!(citations[0].fields.series.as_deref(), Some("RSC"));
    assert_eq!(citations[0].fields.year.as_deref(), Some("1985"));
    assert_eq!(citations[0].fields.chapter.as_deref(), Some("C-46"));
    assert_eq!(citations[0].style.as_ref().unwrap().text, "Criminal Code");
    assert_eq!(pins(&citations[0]), [pin(PinpointKind::Section, "718")]);
    assert_eq!(citations[1].fields.chapter.as_deref(), Some("C-46"));
    assert_eq!(citations[1].language.as_deref(), Some("fr"));
    assert_eq!(pins(&citations[1]), [pin(PinpointKind::Article, "7")]);
}

#[test]
fn quebec_compiled_statutes_are_french() {
    let citation = one("Charte des droits et libertés de la personne, RLRQ c C-12, art 10.");
    assert_eq!(citation.authority, Authority::Statute);
    assert_eq!(citation.fields.chapter.as_deref(), Some("C-12"));
    assert_eq!(citation.language.as_deref(), Some("fr"));
    assert_eq!(citation.jurisdiction.as_deref(), Some("ca-qc"));
}

#[test]
fn regulations_in_every_series() {
    let citations = cite(
        "Food and Drug Regulations, CRC, c 870; SOR/2002-227; O Reg 191/11; Règl de l'Ont 191/11; RRO 1990, Reg 194.",
    );
    assert_eq!(
        cores(&citations),
        ["CRC, c 870", "SOR/2002-227", "O Reg 191/11", "Règl de l'Ont 191/11", "RRO 1990, Reg 194"]
    );
    for citation in &citations {
        assert_eq!(citation.authority, Authority::Regulation, "{}", citation.span.text);
        assert_eq!(citation.format, Some(Format::RegulationSeries), "{}", citation.span.text);
    }
    assert_eq!(citations[0].fields.chapter.as_deref(), Some("870"));
    assert_eq!(citations[1].fields.series.as_deref(), Some("SOR"));
    assert_eq!(citations[1].fields.regulation.as_deref(), Some("2002-227"));
    assert_eq!(citations[2].fields.series.as_deref(), Some("O Reg"));
    assert_eq!(citations[2].jurisdiction.as_deref(), Some("ca-on"));
    assert_eq!(citations[3].language.as_deref(), Some("fr"));
    assert_eq!(citations[3].jurisdiction.as_deref(), Some("ca-on"));
    assert_eq!(citations[4].fields.regulation.as_deref(), Some("194"));
}

#[test]
fn court_rules_are_their_own_authority() {
    let citations = cite(
        "Rules of Civil Procedure, RRO 1990, Reg 194, r 20.04; Federal Courts Rules, SOR/98-106, r 3.",
    );
    assert_eq!(citations[0].authority, Authority::CourtRule);
    assert_eq!(pins(&citations[0]), [pin(PinpointKind::Rule, "20.04")]);
    assert_eq!(citations[1].authority, Authority::CourtRule);
}

#[test]
fn charter_is_one_constitutional_authority_with_its_pinpoint() {
    let text = "Canadian Charter of Rights and Freedoms, s 7, Part I of the Constitution Act, 1982, being Schedule B to the Canada Act 1982 (UK), 1982, c 11.";
    let citation = one(text);
    assert_eq!(citation.authority, Authority::Constitution);
    assert_eq!(citation.full_span.text, &text[..text.len() - 1]);
    assert_eq!(citation.style.as_ref().unwrap().text, "Canadian Charter of Rights and Freedoms");
    assert!(citation.span.text.starts_with("Part I of the Constitution Act, 1982"));
    assert_eq!(pins(&citation), [pin(PinpointKind::Section, "7")]);
    // The pinpoint is not swallowed into the core.
    assert!(!citation.span.text.contains("s 7"));
    let french = one("Charte canadienne des droits et libertés, art 7, partie I de la Loi constitutionnelle de 1982, constituant l'annexe B de la Loi de 1982 sur le Canada (R-U), 1982, c 11");
    assert_eq!(french.authority, Authority::Constitution);
    assert_eq!(french.language.as_deref(), Some("fr"));
    assert_eq!(pins(&french), [pin(PinpointKind::Article, "7")]);
}

#[test]
fn constitution_acts_are_constitutions() {
    let citation = one("Constitution Act, 1982, s 35.");
    assert_eq!(citation.authority, Authority::Constitution);
    assert_eq!(pins(&citation), [pin(PinpointKind::Section, "35")]);
    // A dated title in front of its enacting source is that source's style.
    let citation = one("Constitution Act, 1867 (UK), 30 & 31 Vict, c 3, s 91(24).");
    assert_eq!(citation.authority, Authority::Constitution);
    assert_eq!(citation.span.text, "30 & 31 Vict, c 3");
    assert_eq!(citation.style.as_ref().unwrap().text, "Constitution Act, 1867 (UK)");
    assert_eq!(pins(&citation), [pin(PinpointKind::Section, "91(24)")]);
}

#[test]
fn us_codes_are_statutes_in_code_format() {
    let citations = cite("42 U.S.C. § 1983; 26 C.F.R. § 1.401(a)-1.");
    assert_eq!(citations[0].authority, Authority::Statute);
    assert_eq!(citations[0].format, Some(Format::Code));
    assert_eq!(citations[0].fields.volume.as_deref(), Some("42"));
    assert_eq!(citations[0].fields.section.as_deref(), Some("1983"));
    assert_eq!(citations[0].jurisdiction.as_deref(), Some("us"));
    assert_eq!(citations[1].authority, Authority::Regulation);
    assert_eq!(citations[1].span.text, "26 C.F.R. § 1.401(a)-1");
    assert!(citations[1].parentheticals.is_empty());
}

#[test]
fn treaties() {
    let citation = one("Convention for the Protection of Human Rights and Fundamental Freedoms, 4 November 1950, 213 UNTS 221, art 8.");
    assert_eq!(citation.authority, Authority::Treaty);
    assert_eq!(citation.fields.volume.as_deref(), Some("213"));
    assert_eq!(citation.fields.series.as_deref(), Some("UNTS"));
    assert_eq!(citation.fields.page.as_deref(), Some("221"));
    assert_eq!(
        citation.short_name.as_deref(),
        Some("Convention for the Protection of Human Rights and Fundamental Freedoms")
    );
    assert_eq!(pins(&citation), [pin(PinpointKind::Article, "8")]);
    let citation = one("Agreement between Canada and France, Can TS 1976 No 47.");
    assert_eq!(citation.authority, Authority::Treaty);
    assert_eq!(citation.fields.number.as_deref(), Some("47"));
}

#[test]
fn bills_debates_and_parliamentary_papers() {
    let bill = one("Bill C-25, An Act to amend the National Defence Act, 1st Sess, 42nd Parl, 2016.");
    assert_eq!(bill.authority, Authority::Bill);
    assert_eq!(bill.fields.bill.as_deref(), Some("C-25"));
    let debate = one("House of Commons Debates, 42-1 (5 May 2017) at 1234 (Hon Jody Wilson-Raybould).");
    assert_eq!(debate.authority, Authority::Debate);
    assert!(debate.span.text.starts_with("House of Commons Debates"));
    assert_eq!(debate.fields.session.as_deref(), Some("42-1"));
    assert_eq!(pins(&debate), [pin(PinpointKind::Page, "1234")]);
    assert_eq!(debate.parentheticals[0].kind, ParentheticalKind::Explanatory);
    let citations = cite("HC Deb 3 March 2020, vol 672, col 45; Home Office, Rights Brought Home (Cm 3782, 1997).");
    assert_eq!(citations[0].authority, Authority::Debate);
    assert_eq!(citations[0].fields.volume.as_deref(), Some("672"));
    assert_eq!(citations[1].authority, Authority::ParliamentaryPaper);
    assert_eq!(citations[1].fields.number.as_deref(), Some("3782"));
}

#[test]
fn books_and_book_chapters() {
    let book = one("Peter Hogg, Constitutional Law of Canada, 5th ed (Toronto: Carswell, 2007) at 121.");
    assert_eq!(book.authority, Authority::Book);
    assert_eq!(book.fields.edition.as_deref(), Some("5th"));
    assert_eq!(book.fields.place.as_deref(), Some("Toronto"));
    assert_eq!(book.fields.publisher.as_deref(), Some("Carswell"));
    assert_eq!(book.fields.year.as_deref(), Some("2007"));
    assert_eq!(pins(&book), [pin(PinpointKind::Page, "121")]);
    let chapter = one("Jane Doe, \"Killing\" in John Roe, ed, The Ethics of Killing (New York: Oxford University Press, 2015) 45 at 50.");
    assert_eq!(chapter.authority, Authority::BookChapter);
    assert_eq!(chapter.fields.page.as_deref(), Some("45"));
    assert_eq!(pins(&chapter), [pin(PinpointKind::Page, "50")]);
}

#[test]
fn webpages() {
    let citation = cite("Supreme Court Act, online: <https://laws-lois.justice.gc.ca/eng/acts/S-26/>.").pop().unwrap();
    assert_eq!(citation.authority, Authority::Webpage);
    assert_eq!(citation.format, Some(Format::Url));
    assert_eq!(
        citation.fields.url.as_deref(),
        Some("https://laws-lois.justice.gc.ca/eng/acts/S-26/")
    );
}

// ---------------------------------------------------------------------------
// Pinpoints.

#[test]
fn pinpoint_kinds_ranges_and_lists() {
    let citation = one("Criminal Code, RSC 1985, c C-46, ss 7(2)-(4), 8 and 9.");
    assert_eq!(
        pins(&citation),
        [
            range(PinpointKind::Section, "7(2)", "7(4)"),
            pin(PinpointKind::Section, "8"),
            pin(PinpointKind::Section, "9"),
        ]
    );
    let citation = one("R v Oakes, [1986] 1 SCR 103 at 553, 559.");
    assert_eq!(pins(&citation), [pin(PinpointKind::Page, "553"), pin(PinpointKind::Page, "559")]);
    let citation = one("R v Oakes, [1986] 1 SCR 103 at 191-92.");
    assert_eq!(pins(&citation), [range(PinpointKind::Page, "191", "192")]);
    let citation = one("Jordan, supra note 4 at 12 n 4.");
    assert_eq!(pins(&citation), [pin(PinpointKind::Page, "12"), pin(PinpointKind::Footnote, "4")]);
    let citation = one("Smith v Jones, 2020 ONCA 1 at ¶ 12.");
    assert_eq!(pins(&citation), [pin(PinpointKind::Paragraph, "12")]);
    let citation = one("Id. at p. 5.");
    assert_eq!(pins(&citation), [pin(PinpointKind::Page, "5")]);
    let citation = one("R v Smith, 2020 SCC 1 at xii.");
    assert_eq!(pins(&citation), [pin(PinpointKind::Page, "xii")]);
}

#[test]
fn french_pinpoints() {
    let citation = one("Voir Jordan, précité note 4, aux paras 23 à 27.");
    assert_eq!(pins(&citation), [range(PinpointKind::Paragraph, "23", "27")]);
    let citation = one("Code civil du Québec, RLRQ c CCQ-1991, à l'art 1457.");
    assert_eq!(pins(&citation), [pin(PinpointKind::Article, "1457")]);
    let citation = one("R c Jordan, 2016 CSC 27, au par 12.");
    assert_eq!(pins(&citation), [pin(PinpointKind::Paragraph, "12")]);
}

#[test]
fn schedule_subsection_rule_and_clause_pinpoints() {
    let citation = one("Government Organization Act, RSA 2000, c G-10, Sched 9, s 2(b).");
    assert_eq!(citation.fields.schedule.as_deref(), Some("9"));
    assert_eq!(pins(&citation), [pin(PinpointKind::Section, "2(b)")]);
    let citation = one("DOLA, SO 1990, c D.16, subsection 4(4).");
    assert_eq!(pins(&citation), [pin(PinpointKind::Subsection, "4(4)")]);
    let citation = one("Criminal Code, RSC 1985, c C-46, cl 5.");
    assert_eq!(pins(&citation), [pin(PinpointKind::Clause, "5")]);
    let citation = one("Smith v Jones, 2020 ONCA 1, sch 2.");
    assert_eq!(pins(&citation), [pin(PinpointKind::Schedule, "2")]);
}

#[test]
fn pinpoints_never_swallow_the_next_citation() {
    let citations = cite("Groia v Law Society, 2018 SCC 27, [2018] 1 SCR 772 at paras 64–67.");
    assert_eq!(cores(&citations), ["2018 SCC 27", "[2018] 1 SCR 772"]);
    assert!(citations[0].pinpoints.is_empty());
    assert_eq!(pins(&citations[1]), [range(PinpointKind::Paragraph, "64", "67")]);
}

// ---------------------------------------------------------------------------
// Parentheticals and history.

#[test]
fn explanatory_court_and_source_parentheticals() {
    let citations = cite(
        "R v Oakes, [1986] 1 SCR 103 (holding that the test applies); R v Smith, 2015 SCC 34 (Abella J dissenting); Rylands v Fletcher (1868), LR 3 HL 330 (HL).",
    );
    assert_eq!(citations[0].parentheticals[0].kind, ParentheticalKind::Explanatory);
    assert_eq!(citations[0].parentheticals[0].content, "holding that the test applies");
    assert_eq!(citations[1].parentheticals[0].kind, ParentheticalKind::Explanatory);
    let last = citations.last().unwrap();
    assert_eq!(last.parentheticals.last().unwrap().kind, ParentheticalKind::Court);
}

#[test]
fn nested_parentheticals_stay_balanced() {
    let citation = one("R v Smith, 2015 SCC 34 (quoting the trial judge (at para 4)).");
    assert_eq!(citation.parentheticals.len(), 1);
    assert_eq!(citation.parentheticals[0].content, "quoting the trial judge (at para 4)");
}

#[test]
fn a_parenthetical_holding_a_citation_is_left_to_that_citation() {
    let text = "R v Jones, 2017 SCC 60 (citing R v Oakes, [1986] 1 SCR 103).";
    let citations = cite(text);
    assert_eq!(cores(&citations), ["2017 SCC 60", "[1986] 1 SCR 103"]);
    assert_lossless(text, &citations);
}

#[test]
fn subsequent_history_links_to_the_next_citation() {
    let citations = cite("R v First, 2020 SCC 1 aff’d R v Second, 2021 SCC 2.");
    assert_eq!(citations.len(), 2);
    assert_eq!(citations[0].history[0].relation, "affirmed");
    assert_eq!(citations[0].history[0].target, Some(1));
    assert_eq!(citations[1].style.as_ref().unwrap().text, "R v Second");

    let citations = cite("Smith v Jones, 2018 ONSC 1, rev'd 2019 ONCA 5, aff'd 2020 SCC 3.");
    assert_eq!(citations.len(), 3);
    assert_eq!(citations[0].history[0].relation, "reversed");
    assert_eq!(citations[0].history[0].target, Some(1));
    assert_eq!(citations[1].history[0].relation, "affirmed");
    assert_eq!(citations[1].history[0].target, Some(2));
}

#[test]
fn leave_and_appeal_history() {
    let citations = cite("Smith v Jones, 2019 ONCA 5, leave to appeal to SCC refused, 2020 CanLII 1234 (SCC).");
    assert_eq!(citations.len(), 2);
    assert_eq!(citations[0].history[0].relation, "leave_refused");
    assert_eq!(citations[0].history[0].span.text, "leave to appeal to SCC refused");
    assert_eq!(citations[0].history[0].target, Some(1));
    assert_eq!(court(&citations[1]), Some("scc"));

    let citations = cite("Tremblay c Roy, 2019 QCCA 5, autorisation d'appel à la CSC refusée, 2020 CanLII 99 (CSC).");
    assert_eq!(citations[0].history[0].relation, "leave_refused");
    assert_eq!(citations[0].history[0].target, Some(1));

    for (phrase, relation) in [
        ("leave to appeal granted", "leave_granted"),
        ("appeal dismissed", "appeal_dismissed"),
        ("var'd", "varied"),
        ("affirmed", "affirmed"),
        ("overruled by", "overruled"),
        ("rev’g", "reversing"),
    ] {
        let text = format!("Smith v Jones, 2019 ONCA 5, {phrase} 2020 SCC 3.");
        let citations = cite(&text);
        assert_eq!(citations[0].history[0].relation, relation, "{text}");
        assert_eq!(citations[0].history[0].target, Some(1), "{text}");
    }
}

#[test]
fn history_without_a_following_citation_has_no_target() {
    let citation = one("Smith v Jones, 2019 ONCA 5, leave to appeal to SCC refused.");
    assert_eq!(citation.history[0].relation, "leave_refused");
    assert_eq!(citation.history[0].target, None);
}

// ---------------------------------------------------------------------------
// Parties and styles.

#[test]
fn single_party_styles_are_kept_without_parties() {
    let citations = cite(
        "Re Moore, 2020 ONCA 1 at para 4; Reference re Secession of Quebec, [1998] 2 SCR 217; Renvoi relatif à la sécession du Québec, [1998] 2 RCS 217; Moore (Re), 2021 ONCA 2.",
    );
    let styles = citations
        .iter()
        .map(|citation| citation.style.as_ref().map(|style| style.text.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        styles,
        [
            Some("Re Moore"),
            Some("Reference re Secession of Quebec"),
            Some("Renvoi relatif à la sécession du Québec"),
            Some("Moore (Re)"),
        ]
    );
    assert!(citations.iter().all(|citation| citation.parties.is_none()));
    assert_eq!(citations[0].short_name.as_deref(), Some("Re Moore"));
}

#[test]
fn corporate_names_keep_their_periods_and_versus() {
    let text = "1068490 Ontario Ltd. V. Marlin Center Mobile Homes Inc. and Howard Geisler, 2001 CarswellOnt 4564, at para. 21 (Book of Authorities TAB 17)";
    let citation = one(text);
    assert_eq!(citation.full_span.text, text);
    assert_eq!(citation.format, Some(Format::Database));
    let parties = citation.parties.as_ref().unwrap();
    assert_eq!(parties.plaintiff, "1068490 Ontario Ltd.");
    assert_eq!(parties.defendant, "Marlin Center Mobile Homes Inc. and Howard Geisler");
    assert_eq!(pins(&citation), [pin(PinpointKind::Paragraph, "21")]);
    assert_eq!(citation.parentheticals[0].kind, ParentheticalKind::Explanatory);
}

#[test]
fn versus_variants_split_parties() {
    for (text, plaintiff, defendant) in [
        ("Smith v. Jones, 2020 ONCA 1", "Smith", "Jones"),
        ("Smith vs. Jones, 2020 ONCA 1", "Smith", "Jones"),
        ("Québec (Procureur général) c. Blaikie, [1979] 2 RCS 1016", "Québec (Procureur général)", "Blaikie"),
    ] {
        let parties = one(text).parties.expect(text);
        assert_eq!((parties.plaintiff.as_str(), parties.defendant.as_str()), (plaintiff, defendant));
    }
}

// ---------------------------------------------------------------------------
// Short forms, supra, ibid, references, unknown citations and signals.

#[test]
fn ibid_and_id_with_pinpoints() {
    let citations = cite("R v Jordan, 2016 SCC 27. Ibid at para 7. Id. at 9. ibid. Ibid., s 49.2.");
    let forms = citations.iter().map(|citation| citation.form).collect::<Vec<_>>();
    assert_eq!(forms, [Form::Full, Form::Ibid, Form::Ibid, Form::Ibid, Form::Ibid]);
    assert_eq!(pins(&citations[1]), [pin(PinpointKind::Paragraph, "7")]);
    assert_eq!(pins(&citations[2]), [pin(PinpointKind::Page, "9")]);
    assert!(citations[3].pinpoints.is_empty());
    assert_eq!(pins(&citations[4]), [pin(PinpointKind::Section, "49.2")]);
}

#[test]
fn supra_forms_across_citation_styles() {
    let citations = cite(
        "Jordan, supra note 4 at para 5; Jordan, above n 4, 353; Jordan (n 4) [12]; Jordan, précité, au para 12; Jordan, supra at paras 3-5; Jordan (n 4) 353.",
    );
    assert!(citations.iter().all(|citation| citation.form == Form::Supra));
    let notes = citations.iter().map(|citation| citation.fields.note).collect::<Vec<_>>();
    assert_eq!(notes, [Some(4), Some(4), Some(4), None, None, Some(4)]);
    assert!(citations
        .iter()
        .all(|citation| citation.short_name.as_deref() == Some("Jordan")));
    assert_eq!(pins(&citations[0]), [pin(PinpointKind::Paragraph, "5")]);
    assert_eq!(pins(&citations[2]), [pin(PinpointKind::Paragraph, "12")]);
    assert_eq!(citations[3].language.as_deref(), Some("fr"));
    assert_eq!(pins(&citations[4]), [range(PinpointKind::Paragraph, "3", "5")]);
    assert_eq!(pins(&citations[5]), [pin(PinpointKind::Page, "353")]);
}

#[test]
fn supra_names_skip_signals_and_relation_words() {
    let citations = cite("Johnson, supra note 243 at para 19 citing Lyons, supra note 243 at page 339.");
    assert_eq!(citations.len(), 2);
    assert_eq!(citations[0].short_name.as_deref(), Some("Johnson"));
    assert_eq!(citations[1].short_name.as_deref(), Some("Lyons"));
    assert_eq!(pins(&citations[1]), [pin(PinpointKind::Page, "339")]);
    let citation = one("See also Jordan, supra note 4.");
    assert_eq!(citation.short_name.as_deref(), Some("Jordan"));
    assert_eq!(citation.signal.as_ref().unwrap().text, "see also");
}

#[test]
fn us_short_forms_read_volume_reporter_and_page() {
    let citations = cite("Roe v. Wade, 410 U.S. 113 (1973). Roe, 410 U.S. at 160. 123 F.3d at 456.");
    assert_eq!(citations[1].form, Form::Short);
    assert_eq!(citations[1].span.text, "410 U.S. at 160");
    assert_eq!(citations[1].fields.volume.as_deref(), Some("410"));
    assert_eq!(citations[1].fields.reporter.as_deref(), Some("U.S."));
    assert_eq!(citations[1].fields.page.as_deref(), Some("160"));
    assert_eq!(citations[1].short_name.as_deref(), Some("Roe"));
    let last = citations.last().unwrap();
    assert_eq!(last.form, Form::Short);
    assert_eq!(last.fields.reporter.as_deref(), Some("F.3d"));
    assert_eq!(last.fields.page.as_deref(), Some("456"));
}

#[test]
fn case_name_references_need_an_earlier_full_citation() {
    let citations = cite("R v Jordan, 2016 SCC 27 [Jordan]. Later, Jordan at para 12 held. Roe at 240.");
    let references = citations
        .iter()
        .filter(|citation| citation.form == Form::Reference)
        .collect::<Vec<_>>();
    assert_eq!(references.len(), 1);
    assert_eq!(references[0].span.text, "Jordan");
    assert_eq!(references[0].full_span.text, "Jordan at para 12");
    assert_eq!(pins(references[0]), [pin(PinpointKind::Paragraph, "12")]);

    let citations = cite("Roe v. Wade, 410 U.S. 113 (1973). Roe at 240.");
    assert_eq!(citations[1].form, Form::Reference);
    assert_eq!(citations[1].full_span.text, "Roe at 240");
    assert_eq!(pins(&citations[1]), [pin(PinpointKind::Page, "240")]);
}

#[test]
fn case_names_in_prose_are_not_references() {
    let citations = cite("R v Jordan, 2016 SCC 27. The court in Jordan agreed with the Crown.");
    assert_eq!(citations.len(), 1);
    // The Crown is never a reference name.
    let citations = cite("R v Jordan, 2016 SCC 27. R at para 3 is not a reference.");
    assert_eq!(citations.len(), 1);
}

#[test]
fn conjoined_short_forms_are_their_own_references() {
    let text = "R v Oakes, [1986] 1 SCR 103; R v Jordan, 2016 SCC 27; see Smith, supra note 4 and Oakes.";
    let citations = cite(text);
    let last = citations.last().unwrap();
    assert_eq!(last.form, Form::Reference);
    assert_eq!(last.full_span.text, "Oakes");
    assert_eq!(citations[citations.len() - 2].full_span.text, "Smith, supra note 4");
    assert_lossless(text, &citations);
}

#[test]
fn bare_section_symbols_are_unknown_citations() {
    let citations = cite("42 U.S.C. § 1983 applies. Under § 1985 the claim fails. Id. § 3.");
    let forms = citations.iter().map(|citation| citation.form).collect::<Vec<_>>();
    assert_eq!(forms, [Form::Full, Form::Unknown, Form::Ibid]);
    assert_eq!(citations[1].fields.section.as_deref(), Some("1985"));
    assert_eq!(pins(&citations[2]), [pin(PinpointKind::Section, "3")]);
}

#[test]
fn introductory_signals() {
    let citations = cite(
        "See, e.g., R v Oakes, [1986] 1 SCR 103 at 138; But see R v Sparrow, [1990] 1 SCR 1075; Cf R v Grant, 2009 SCC 32; Contra Smith v Jones, 2010 ONCA 1; E.g. R v Kokopenace, 2015 SCC 28; Comparer R c Grant, 2009 CSC 32; Voir aussi R c Mann, 2004 CSC 52.",
    );
    let signals = citations
        .iter()
        .map(|citation| citation.signal.as_ref().map(|signal| signal.text.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        signals,
        [
            Some("see eg"),
            Some("but see"),
            Some("cf"),
            Some("contra"),
            Some("eg"),
            Some("comparer"),
            Some("voir aussi"),
        ]
    );
    // Signals are never part of the style or the parties.
    assert!(citations
        .iter()
        .all(|citation| citation.style.as_ref().unwrap().text.starts_with('R')
            || citation.style.as_ref().unwrap().text.starts_with("Smith")));
    let signal = citations[1].signal.as_ref().unwrap();
    assert!(signal.end <= citations[1].full_span.start);
}

#[test]
fn prose_verbs_are_not_signals() {
    let citations = cite("The court did not see R v Jordan, 2016 SCC 27 as binding.");
    assert!(citations[0].signal.is_none());
}

// ---------------------------------------------------------------------------
// Footnote splitting fidelity.

#[test]
fn quoted_semicolons_do_not_split_an_authority() {
    let text = "Jane Doe, \"A Title; With a Subtitle\" (2020) 1 Queen's LJ 10; commentary; R v Oakes, [1986] 1 SCR 103.";
    let citations = cite(text);
    assert_eq!(citations.len(), 2);
    assert_eq!(
        citations[0].full_span.text,
        "Jane Doe, \"A Title; With a Subtitle\" (2020) 1 Queen's LJ 10"
    );
    assert_eq!(citations[0].authority, Authority::Journal);
    assert_eq!(citations[1].full_span.text, "R v Oakes, [1986] 1 SCR 103");
    assert_lossless(text, &citations);
}

#[test]
fn footnotes_rebuild_losslessly_from_spans_and_gaps() {
    for text in [
        "2018 SCC 27; 2019 SCC 1",
        "Johnson, supra note 243 at para 19 citing Lyons, supra note 243 at page 339.",
        "Groia v Law Society, 2018 SCC 27, [2018] 1 SCR 772 at paras 64–67.",
        "🦫 R v First, 2020 SCC 1 aff’d R v Second, 2021 SCC 2.",
        "Roe v Wade, 410 U.S. 113; claim under 42 U.S.C. § 1983.",
        "See e.g. R v Oakes, [1986] 1 SCR 103 at 138 (holding that the test applies); Ibid at 140; Jordan, supra note 4 [emphasis added].",
        "Alice Woolley, “Lawyer Regulation; A Survey” (2017) 95:4 Can Bar Rev 893 at 900; see also Hogg, Constitutional Law (Toronto: Carswell, 2007) at 121.",
        "Canadian Charter of Rights and Freedoms, s 7, Part I of the Constitution Act, 1982, being Schedule B to the Canada Act 1982 (UK), 1982, c 11; Criminal Code, RSC 1985, c C-46, s 718.",
    ] {
        let citations = cite(text);
        assert!(!citations.is_empty(), "{text}");
        assert_lossless(text, &citations);
    }
    let text = "2018 SCC 27; 2019 SCC 1";
    let citations = cite(text);
    assert_eq!(&text[citations[0].full_span.end..citations[1].full_span.start], "; ");
}

#[test]
fn parallel_reporters_and_us_statutes_split_by_grammar() {
    let citations = cite("Roe v Wade, 410 U.S. 113; claim under 42 U.S.C. § 1983.");
    assert_eq!(
        citations.iter().map(|citation| citation.authority).collect::<Vec<_>>(),
        [Authority::Case, Authority::Statute]
    );
    let citations = cite("Groia v Law Society, 2018 SCC 27, [2018] 1 SCR 772 at paras 64–67.");
    assert_eq!(citations.len(), 2);
}

// ---------------------------------------------------------------------------
// Note cross-references.

#[test]
fn note_references_cover_every_cross_reference_form() {
    let text = "Smith, supra note 4; see infra, footnote 12; op. cit. note 3; See also footnote 7; Jordan, above n 5; below n 9; Jordan (n 6); voir la note 11 ci-dessus; note 13 ci-dessous; supra notes 14.";
    let found = note_references(text)
        .into_iter()
        .map(|reference| (reference.note, reference.direction))
        .collect::<Vec<_>>();
    assert_eq!(
        found,
        [
            (4, NoteDirection::Back),
            (12, NoteDirection::Forward),
            (3, NoteDirection::Back),
            (7, NoteDirection::Unspecified),
            (5, NoteDirection::Back),
            (9, NoteDirection::Forward),
            (6, NoteDirection::Back),
            (11, NoteDirection::Back),
            (13, NoteDirection::Forward),
            (14, NoteDirection::Back),
        ]
    );
}

#[test]
fn note_references_are_a_superset_of_the_legacy_crossref_grammar() {
    let legacy = regex::Regex::new(r"(?i)\b(?:(?:supra|infra),?\s+(?:foot)?notes?|op\.?\s*cit\.?,?\s+(?:foot)?notes?|see\s+(?:also\s+)?footnote)\s+([0-9]{1,3})\b").unwrap();
    let text = "Supra, note 4; infra footnotes 12; op cit, footnote 3; SEE ALSO FOOTNOTE 7; see footnote 8; supra note 1234; the notes 5 were taken.";
    let found = note_references(text);
    for captures in legacy.captures_iter(text) {
        let whole = captures.get(0).unwrap();
        let number: u32 = captures[1].parse().unwrap();
        assert!(
            found.iter().any(|reference| reference.note == number
                && reference.span.start <= whole.start()
                && whole.end() <= reference.span.end),
            "{}",
            whole.as_str()
        );
    }
}

// ---------------------------------------------------------------------------
// Excerpt classification (Beaver's citator treatment filter).

#[test]
fn excerpt_classification_is_unchanged() {
    use legal_citations::excerpt::classify_citator_excerpt;
    let list = classify_citator_excerpt(
        "R v Oakes, [1986] 1 SCR 103; R v Jordan, 2016 SCC 27; R v Grant, 2009 SCC 32; R v Mann, 2004 SCC 52.",
    );
    assert_eq!(list.kind, "authority_list");
    let prose = classify_citator_excerpt(
        "The court considered whether the delay was unreasonable in the circumstances of the case, and it held that the trial judge was right to stay the proceedings.",
    );
    assert_eq!(prose.kind, "prose");
    assert_eq!(classify_citator_excerpt("too short").kind, "insufficient");
}
