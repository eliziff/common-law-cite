//! End-to-end checks of the downstream stages through `extract` with the
//! embedded registry.

use legal_citations::annotate::{annotate, annotate_source, citation_annotations, Extent, Unbalanced};
use legal_citations::clean::{clean, Step};
use legal_citations::format::{self, Language, Style};
use legal_citations::key::{key_for_text, KeyError};
use legal_citations::resolve::{authorities, resolve_with_reasons};
use legal_citations::{extract, parallel, url, Citation, Form, NoteRange, Options};

fn full(citations: &[Citation]) -> Vec<&Citation> {
    citations.iter().filter(|citation| citation.form == Form::Full).collect()
}

#[test]
fn oakes_parallel_group_prefers_the_official_report() {
    let text = "R v Oakes, [1986] 1 SCR 103, 26 DLR (4th) 200.";
    let citations = extract(text, &Options::default());
    assert_eq!(citations.len(), 2);
    assert!(citations.iter().all(|citation| citation.parallel_group == Some(0)));
    assert_eq!(citations[1].short_name.as_deref(), Some("R v Oakes"));
    assert_eq!(parallel::preferred(&citations, 0), 0);
    assert_eq!(citations[0].key.as_deref(), Some("2:reporter:scr:1986:1:103"));
    assert_eq!(citations[1].key.as_deref(), Some("2:reporter:dlr4th:26:200"));
}

#[test]
fn reporter_first_parallel_still_prefers_the_neutral_citation() {
    let text = "Canada v Craig, [2012] 2 SCR 489, 2012 SCC 43.";
    let citations = extract(text, &Options::default());
    let group = citations[0].parallel_group.expect("grouped");
    let preferred = parallel::preferred(&citations, group);
    assert_eq!(citations[preferred].span.text, "2012 SCC 43");
}

#[test]
fn semicolons_separate_authorities() {
    let text = "Canada v Craig, 2012 SCC 43, [2012] 2 SCR 489; R v Oakes, [1986] 1 SCR 103.";
    let citations = extract(text, &Options::default());
    assert_eq!(citations[0].parallel_group, Some(0));
    assert_eq!(citations[1].parallel_group, Some(0));
    assert_eq!(citations[2].parallel_group, None);
}

#[test]
fn ibid_and_pinpoint_formatting() {
    let text = "See R v Jordan, 2016 SCC 27 at paras 20, 23 and 25. Ibid at para 5.";
    let citations = extract(text, &Options::default());
    assert_eq!(citations[1].form, Form::Ibid);
    assert_eq!(citations[1].antecedent, Some(0));
    assert_eq!(
        format::full(&citations[0], Style::default()),
        "R v Jordan, 2016 SCC 27 at paras 20, 23, 25"
    );
    assert_eq!(format::ibid(Some(&format::pinpoints(&citations[1].pinpoints, Style::default()))), "Ibid at para 5");
    assert_eq!(format::short_label(&citations[0], Language::En), "Jordan");
}

#[test]
fn supra_with_explicit_short_form() {
    let text = "R v Jordan, 2016 SCC 27 [Jordan]. Jordan, supra note 1 at para 5.";
    let citations = extract(text, &Options::default());
    let supra = citations.iter().find(|citation| citation.form == Form::Supra).expect("supra");
    assert_eq!(supra.antecedent, Some(0));
    let split = text.find("Jordan, supra").unwrap();
    let notes = [
        NoteRange { number: 1, start: 0, end: split, sequence: 0, anchor: None },
        NoteRange { number: 2, start: split, end: text.len(), sequence: 0, anchor: None },
    ];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    let resolution = resolutions.iter().find(|r| r.index == supra.index).unwrap();
    assert_eq!((resolution.antecedent, resolution.reason), (Some(0), "note_and_name"));
}

/// A note that cites a hearing transcript names the case but not its
/// decision: a supra to that note, and an ibid after it, stay unresolved on
/// both resolution paths (with and without the document's source parts),
/// while a supra to the note citing the decision still resolves.
#[test]
fn references_to_a_record_document_do_not_resolve_to_the_decision() {
    let notes = [
        "Halvorsen v Tidewater Ferries Ltd, 2030 SCC 12 [Halvorsen].",
        "See Halvorsen v Tidewater Ferries Ltd, 2030 SCC 12 (Transcript of hearing at 41 lines 3–8) [Halvorsen transcript].",
        "Halvorsen transcript, supra note 2 at 44 lines 1–6.",
        "Ibid at 45.",
        "Halvorsen, supra note 1 at para 30.",
    ];
    let text = notes.join("\n\n");
    let mut start = 0;
    let ranges = notes.iter().enumerate().map(|(index, note)| {
        let range = serde_json::json!({ "number": index + 1, "start": start, "end": start + note.len(), "sequence": 0 });
        start += note.len() + 2;
        range
    }).collect::<Vec<_>>();
    let extracted = legal_citations::api::call_value("extract", serde_json::json!({
        "text": text, "options": { "resolve": false, "notes": ranges } })).unwrap();
    let citations = extracted["citations"].as_array().unwrap();
    let transcript = citations.iter().find(|citation| citation["fullSpan"]["text"].as_str()
        .is_some_and(|value| value.contains("Transcript"))).unwrap();
    assert_eq!(transcript["key"], citations[0]["key"], "the transcript's citation still names the case");
    assert_eq!(transcript["parentheticals"][0]["kind"], "record");
    let index = |needle: &str| citations.iter().find(|citation| citation["fullSpan"]["text"].as_str()
        .is_some_and(|value| value.contains(needle))).unwrap()["index"].as_u64().unwrap();
    for parts in [serde_json::json!([]), extracted["sourceParts"].clone()] {
        let resolved = legal_citations::api::call_value("resolve", serde_json::json!({
            "citations": extracted["citations"], "notes": ranges, "sourceParts": parts,
            "supraHintMode": "aggressive", "supraLinkingMode": "safe" })).unwrap();
        let antecedent = |reference: u64| resolved["resolutions"].as_array().unwrap().iter()
            .find(|resolution| resolution["index"] == reference).unwrap()["antecedent"].clone();
        assert_eq!(antecedent(index("supra note 2")), serde_json::Value::Null, "{parts}");
        assert_eq!(antecedent(index("Ibid")), serde_json::Value::Null, "{parts}");
        assert_eq!(antecedent(index("supra note 1")), citations[0]["index"], "{parts}");
    }
}

#[test]
fn keys_for_text() {
    assert_eq!(key_for_text("R v Jordan, 2016 SCC 27 at para 5").unwrap(), "2:neutral:2016:scc:27");
    assert_eq!(key_for_text("2016 CSC 27").unwrap(), "2:neutral:2016:scc:27");
    assert_eq!(
        key_for_text("Criminal Code, RSC 1985, c C-46, s 7").unwrap(),
        key_for_text("Code criminel, LRC 1985, ch C-46").unwrap()
    );
    assert_eq!(key_for_text("SOR/2002-227").unwrap(), "2:regulation:ca:sor:2002:227");
    assert_eq!(key_for_text("O Reg 191/11").unwrap(), "2:regulation:ca-on:oreg:2011:191");
    assert_eq!(key_for_text("42 U.S.C. § 1983").unwrap(), "2:code:usc:42:1983");
    assert_eq!(key_for_text("Roe v. Wade, 410 U.S. 113 (1973)").unwrap(), "2:reporter:us:410:113");
    assert_eq!(key_for_text("Smith v Jones, 2004 CanLII 12345 (ON CA)").unwrap(), "2:canlii:2004:12345");
    assert_eq!(key_for_text("no citation here"), Err(KeyError::NoCitation));
    assert_eq!(key_for_text("2015 SCC 5, [2015] 1 SCR 331"), Err(KeyError::Multiple(2)));
}

#[test]
fn same_authority_cited_differently_across_notes_merges() {
    let text = "1. Tanudjaja v Canada, 2015 SCC 5, [2015] 1 SCR 331.\n\n2. [2015] 1 SCR 331.\n\n3. 2015 SCC 5.\n\n4. R v Oakes, [1986] 1 SCR 103.";
    let citations = extract(text, &Options::default());
    let clusters = authorities(&citations);
    let cores = |cluster: &Vec<usize>| cluster.iter().map(|&index| citations[index].span.text.as_str()).collect::<Vec<_>>();
    assert_eq!(clusters.len(), 2, "{clusters:?}");
    assert_eq!(cores(&clusters[0]), vec!["2015 SCC 5", "[2015] 1 SCR 331", "[2015] 1 SCR 331", "2015 SCC 5"]);
    assert_eq!(cores(&clusters[1]), vec!["[1986] 1 SCR 103"]);
}

#[test]
fn public_urls_from_extracted_citations() {
    let first = |text: &str| extract(text, &Options::default()).into_iter().find(|c| c.form == Form::Full).unwrap();
    assert_eq!(
        url::url(&first("R v Jordan, 2016 SCC 27"), Language::En).as_deref(),
        Some("https://www.canlii.org/en/ca/scc/doc/2016/2016scc27/2016scc27.html")
    );
    assert_eq!(
        url::url(&first("R c Jordan, 2016 CSC 27"), Language::En).as_deref(),
        Some("https://www.canlii.org/fr/ca/csc/doc/2016/2016csc27/2016csc27.html")
    );
    assert_eq!(
        url::url(&first("Smith v Jones, 2004 CanLII 12345 (ON CA)"), Language::En).as_deref(),
        Some("https://www.canlii.org/en/on/onca/doc/2004/2004canlii12345/2004canlii12345.html")
    );
    assert_eq!(
        url::url(&first("Criminal Code, RSC 1985, c C-46"), Language::En).as_deref(),
        Some("https://www.canlii.org/en/ca/laws/stat/rsc-1985-c-c-46/latest/rsc-1985-c-c-46.html")
    );
    assert_eq!(
        url::justice_laws(&first("Criminal Code, RSC 1985, c C-46"), Language::En).as_deref(),
        Some("https://laws-lois.justice.gc.ca/eng/acts/C-46/")
    );
    assert_eq!(
        url::url(&first("Immigration and Refugee Protection Regulations, SOR/2002-227"), Language::En).as_deref(),
        Some("https://www.canlii.org/en/ca/laws/regu/sor-2002-227/latest/sor-2002-227.html")
    );
    assert_eq!(
        url::url(&first("Roe v. Wade, 410 U.S. 113 (1973)"), Language::En).as_deref(),
        Some("https://www.courtlistener.com/c/U.S./410/113/")
    );
    assert_eq!(
        url::url(&first("Smith v Jones, [2019] UKSC 5"), Language::En).as_deref(),
        Some("https://caselaw.nationalarchives.gov.uk/uksc/2019/5")
    );
    assert_eq!(url::url(&first("Donoghue v Stevenson, [1932] AC 562 (HL)"), Language::En), None);
}

#[test]
fn html_round_trip_through_clean_extract_and_annotate() {
    let source = "<p>See <i>R&nbsp;v Jordan</i>, 2016 SCC 27 at para&nbsp;5.</p><p>Ibid.</p>";
    let cleaned = clean(source, &[Step::Html, Step::InlineWhitespace]);
    let citations = extract(&cleaned.text, &Options::default());
    let full_citations = full(&citations);
    assert_eq!(full_citations.len(), 1);
    let core = citation_annotations(&citations, Extent::Core, |citation| {
        (citation.form == Form::Full).then(|| ("<a>".to_owned(), "</a>".to_owned()))
    });
    assert_eq!(
        annotate_source(source, &cleaned, &core, Unbalanced::Skip),
        "<p>See <i>R&nbsp;v Jordan</i>, <a>2016 SCC 27</a> at para&nbsp;5.</p><p>Ibid.</p>"
    );
    let styled = citation_annotations(&citations, Extent::Full, |citation| {
        (citation.form == Form::Full).then(|| ("<a>".to_owned(), "</a>".to_owned()))
    });
    assert_eq!(annotate_source(source, &cleaned, &styled, Unbalanced::Skip), source);
    assert_eq!(
        annotate_source(source, &cleaned, &styled, Unbalanced::Wrap),
        "<p>See <i><a>R&nbsp;v Jordan</a></i><a>, 2016 SCC 27 at para&nbsp;5</a>.</p><p>Ibid.</p>"
    );
    let plain = annotate(&cleaned.text, &core);
    assert!(plain.contains("<a>2016 SCC 27</a>"), "{plain}");
}
