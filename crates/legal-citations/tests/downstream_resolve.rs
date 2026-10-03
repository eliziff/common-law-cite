mod downstream_common;

use downstream_common::*;
use legal_citations::resolve::{authorities, reference_name, resolve, resolve_with_reasons, Resolution};
use legal_citations::{Authority, Citation, Form, Format, NoteRange};

fn jordan(index: usize, start: usize) -> Citation {
    case(index, "2016 SCC 27", start)
        .neutral("2016", "SCC", "27")
        .style("R v Jordan")
        .parties("R", "Jordan")
        .build()
}

fn oakes(index: usize, start: usize) -> Citation {
    case(index, "[1986] 1 SCR 103", start)
        .reporter(Some("1986"), Some("1"), "SCR", "103")
        .style("R v Oakes")
        .parties("R", "Oakes")
        .build()
}

fn note(number: u32, start: usize, end: usize) -> NoteRange {
    NoteRange {
        number,
        start,
        end,
        sequence: 0,
    }
}

fn note_in(sequence: u32, number: u32, start: usize, end: usize) -> NoteRange {
    NoteRange {
        number,
        start,
        end,
        sequence,
    }
}

fn find(resolutions: &[Resolution], index: usize) -> (Option<usize>, &'static str) {
    let resolution = resolutions.iter().find(|r| r.index == index).expect("resolution");
    (resolution.antecedent, resolution.reason)
}

fn unknown(index: usize, start: usize) -> Citation {
    cite(index, Form::Unknown, Authority::Unknown, "§ 1983", start).build()
}

fn reference(index: usize, name: &str, start: usize) -> Citation {
    let mut citation = cite(index, Form::Reference, Authority::Unknown, &format!("{name} at para 12"), start).build();
    citation.short_name = Some(name.into());
    citation
}

#[test]
fn ibid_chain_without_notes() {
    let citations = vec![jordan(0, 20), ibid(1, 100), ibid(2, 200)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 1), (Some(0), "ibid_previous"));
    assert_eq!(find(&resolutions, 2), (Some(0), "ibid_previous"));
}

#[test]
fn unknown_citation_breaks_the_ibid_chain() {
    let citations = vec![jordan(0, 20), unknown(1, 100), ibid(2, 200), ibid(3, 300)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 1), (None, "unknown_form"));
    assert_eq!(find(&resolutions, 2), (None, "ibid_after_unresolved"));
    assert_eq!(find(&resolutions, 3), (None, "ibid_after_unresolved"));
}

#[test]
fn leading_ibid_has_no_antecedent() {
    let resolutions = resolve_with_reasons(&[ibid(0, 0)], None);
    assert_eq!(find(&resolutions, 0), (None, "ibid_no_previous"));
}

#[test]
fn ibid_after_a_parallel_group_names_the_group() {
    let mut first = jordan(0, 20);
    first.parallel_group = Some(0);
    let mut second = case(1, "[2016] 1 SCR 631", 40)
        .reporter(Some("2016"), Some("1"), "SCR", "631")
        .build();
    second.parallel_group = Some(0);
    let citations = vec![first, second, ibid(2, 100)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 2), (Some(0), "ibid_previous"));
}

#[test]
fn ibid_chains_across_notes() {
    // n1: A. n2: Ibid. n3: Ibid at 5.
    let citations = vec![jordan(0, 10), ibid(1, 110), ibid(2, 210)];
    let notes = [note(1, 0, 100), note(2, 100, 200), note(3, 200, 300)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 1), (Some(0), "ibid_previous_note"));
    assert_eq!(find(&resolutions, 2), (Some(0), "ibid_previous_note"));
}

#[test]
fn ibid_after_a_note_with_several_authorities_is_ambiguous() {
    let citations = vec![jordan(0, 10), oakes(1, 50), ibid(2, 110)];
    let notes = [note(1, 0, 100), note(2, 100, 200)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 2), (None, "ibid_after_multiple"));
}

#[test]
fn ibid_after_a_note_citing_one_decision_in_parallel_resolves() {
    let mut first = jordan(0, 10);
    first.parallel_group = Some(0);
    let mut second = case(1, "[2016] 1 SCR 631", 40).reporter(Some("2016"), Some("1"), "SCR", "631").build();
    second.parallel_group = Some(0);
    let citations = vec![first, second, ibid(2, 110)];
    let notes = [note(1, 0, 100), note(2, 100, 200)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 2), (Some(0), "ibid_previous_note"));
}

#[test]
fn ibid_after_a_note_without_citations() {
    let citations = vec![jordan(0, 10), ibid(1, 210)];
    let notes = [note(1, 0, 100), note(2, 100, 200), note(3, 200, 300)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 1), (None, "ibid_no_previous"));
}

#[test]
fn ibid_within_a_note_uses_the_previous_citation() {
    let citations = vec![jordan(0, 10), oakes(1, 40), ibid(2, 80)];
    let notes = [note(1, 0, 100)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 2), (Some(1), "ibid_previous"));
}

#[test]
fn ibid_follows_note_numbers_not_text_order() {
    // Note 2's text comes before note 1's (footnotes.xml order differs from
    // reference order); ibid in note 2 still refers to note 1.
    let citations = vec![ibid(0, 10), jordan(1, 110)];
    let notes = [note(2, 0, 100), note(1, 100, 200)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 0), (Some(1), "ibid_previous_note"));
}

#[test]
fn supra_with_note_and_matching_name() {
    let citations = vec![jordan(0, 10), oakes(1, 40), supra(2, Some("Jordan"), Some(1), 210)];
    let notes = [note(1, 0, 100), note(2, 100, 200), note(3, 200, 300)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 2), (Some(0), "note_and_name"));
}

#[test]
fn supra_with_note_only() {
    let citations = vec![jordan(0, 10), oakes(1, 110), supra(2, None, Some(1), 210)];
    let notes = [note(1, 0, 100), note(2, 100, 200), note(3, 200, 300)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 2), (Some(0), "note_only"));
}

#[test]
fn supra_name_disagreeing_with_note_is_left_unresolved() {
    let citations = vec![jordan(0, 10), oakes(1, 110), supra(2, Some("Oakes"), Some(1), 210)];
    let notes = [note(1, 0, 100), note(2, 100, 200), note(3, 200, 300)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 2), (None, "note_name_conflict"));
}

#[test]
fn supra_note_with_several_authorities_and_no_name() {
    let citations = vec![jordan(0, 10), oakes(1, 40), supra(2, None, Some(1), 210)];
    let notes = [note(1, 0, 100), note(3, 200, 300)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 2), (None, "ambiguous_note"));
}

#[test]
fn supra_note_out_of_range_and_empty_note() {
    let citations = vec![jordan(0, 10), supra(1, Some("Jordan"), Some(9), 210), supra(2, None, Some(2), 220)];
    let notes = [note(1, 0, 100), note(2, 100, 200), note(3, 200, 300)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 1), (None, "note_out_of_range"));
    assert_eq!(find(&resolutions, 2), (None, "note_without_authority"));
}

#[test]
fn supra_prefers_its_own_numbering_sequence() {
    // Chapter 1 note 1 cites Jordan; chapter 2 restarts numbering and its
    // note 1 cites Oakes. A supra note 1 in chapter 2 means Oakes.
    let citations = vec![jordan(0, 10), oakes(1, 110), supra(2, None, Some(1), 210), supra(3, None, Some(1), 400)];
    let notes = [note_in(0, 1, 0, 100), note_in(1, 1, 100, 200), note_in(1, 2, 200, 300)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 2), (Some(1), "note_only"));
    // Outside every note there is no own sequence and note 1 is ambiguous.
    assert_eq!(find(&resolutions, 3), (None, "ambiguous_note"));
}

#[test]
fn supra_does_not_cross_an_explicit_numbering_sequence() {
    let citations = vec![jordan(0, 10), supra(1, None, Some(1), 210)];
    let notes = [note_in(0, 1, 0, 100), note_in(1, 2, 200, 300)];
    let resolutions = resolve_with_reasons(&citations, Some(&notes));
    assert_eq!(find(&resolutions, 1), (None, "note_out_of_range"));
}

#[test]
fn supra_without_notes_matches_by_name() {
    let citations = vec![jordan(0, 10), oakes(1, 40), supra(2, Some("Oakes"), Some(4), 100)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 2), (Some(1), "name_only"));
}

#[test]
fn supra_name_must_be_unique() {
    let other_jordan = case(1, "2019 ONCA 1", 40)
        .neutral("2019", "ONCA", "1")
        .style("Jordan v Smith")
        .parties("Jordan", "Smith")
        .build();
    let citations = vec![jordan(0, 10), other_jordan, supra(2, Some("Jordan"), None, 100)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 2), (None, "ambiguous_name"));
}

#[test]
fn explicit_short_name_wins_over_loose_matches() {
    let first = case(0, "2019 ONCA 1", 10)
        .neutral("2019", "ONCA", "1")
        .style("Smith v Ontario")
        .explicit("Smith")
        .build();
    let second = case(1, "2019 ONCA 2", 40)
        .neutral("2019", "ONCA", "2")
        .style("Smith Estate v Jones")
        .build();
    let citations = vec![first, second, supra(2, Some("Smith"), None, 100)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 2), (Some(0), "name_only"));
}

#[test]
fn crown_hint_never_matches() {
    let citations = vec![jordan(0, 10), supra(1, Some("R"), None, 100)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 1), (None, "no_match"));
}

#[test]
fn supra_without_any_hint() {
    let citations = vec![jordan(0, 10), supra(1, None, None, 100)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 1), (None, "no_hint"));
}

#[test]
fn name_matching_ignores_case_accents_and_punctuation() {
    let first = case(0, "2019 QCCA 1", 10)
        .neutral("2019", "QCCA", "1")
        .style("Québec (Procureur général) c. Éditions Écosociété Inc.")
        .build();
    let citations = vec![first, reference(1, "editions ecosociete", 100)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 1), (Some(0), "name_only"));
}

#[test]
fn reference_resolves_to_an_earlier_case_name() {
    let citations = vec![jordan(0, 10), oakes(1, 40), reference(2, "Jordan", 100)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 2), (Some(0), "name_only"));
}

#[test]
fn reference_to_a_statute_short_name() {
    let charter = cite(0, Form::Full, Authority::Constitution, "Part I of the Constitution Act, 1982", 40)
        .style("Canadian Charter of Rights and Freedoms")
        .explicit("Charter")
        .build();
    let citations = vec![charter, reference(1, "Charter", 200)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 1), (Some(0), "name_only"));
}

#[test]
fn reference_never_looks_forward() {
    let citations = vec![reference(0, "Jordan", 0), jordan(1, 100)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 0), (None, "no_match"));
}

fn f3d(index: usize, page: &str, start: usize, style: Option<&str>) -> Citation {
    let mut builder = case(index, &format!("123 F.3d {page}"), start).reporter(None, Some("123"), "F.3d", page);
    if let Some(style) = style {
        builder = builder.style(style);
    }
    builder.build()
}

fn short(index: usize, pin: &str, start: usize, name: Option<&str>) -> Citation {
    let mut citation = cite(index, Form::Short, Authority::Case, &format!("123 F.3d at {pin}"), start)
        .reporter(None, Some("123"), "F.3d", pin)
        .build();
    citation.form = Form::Short;
    citation.short_name = name.map(Into::into);
    citation
}

#[test]
fn short_form_matches_reporter_volume_and_page() {
    let citations = vec![f3d(0, "400", 20, Some("Smith v Jones")), short(1, "456", 100, None)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 1), (Some(0), "short_reporter"));
}

#[test]
fn short_form_pin_before_first_page_does_not_match() {
    let citations = vec![f3d(0, "500", 20, Some("Smith v Jones")), short(1, "456", 100, None)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 1), (None, "short_no_match"));
}

#[test]
fn short_form_ambiguity_is_settled_by_name() {
    let citations = vec![
        f3d(0, "400", 20, Some("Smith v Jones")),
        f3d(1, "420", 60, Some("Brown v Board")),
        short(2, "456", 100, Some("Brown")),
        short(3, "456", 120, None),
    ];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(find(&resolutions, 2), (Some(1), "short_name"));
    assert_eq!(find(&resolutions, 3), (None, "short_ambiguous"));
}

#[test]
fn resolve_sets_antecedents_in_place() {
    let mut citations = vec![jordan(0, 10), ibid(1, 100), supra(2, Some("Jordan"), None, 200)];
    resolve(&mut citations, None);
    assert_eq!(citations[1].antecedent, Some(0));
    assert_eq!(citations[2].antecedent, Some(0));
    assert_eq!(citations[2].authority_index(), 0);
    assert_eq!(citations[0].antecedent, None);
}

#[test]
fn reference_name_falls_back_to_the_text_before_supra() {
    let mut citation = cite(0, Form::Supra, Authority::Unknown, "Jordan, supra note 4", 0).build();
    assert_eq!(reference_name(&citation).as_deref(), Some("Jordan"));
    citation.full_span.text = "Hunter v Southam (n 4)".into();
    assert_eq!(reference_name(&citation).as_deref(), Some("Hunter v Southam"));
    citation.full_span.text = "supra note 4".into();
    assert_eq!(reference_name(&citation), None);
}

fn keyed(mut citation: Citation, key: &str) -> Citation {
    citation.key = Some(key.into());
    citation
}

#[test]
fn authorities_cluster_full_citations_and_resolved_references() {
    let mut citations = vec![
        keyed(jordan(0, 10), "2:neutral:2016:scc:27"),
        keyed(oakes(1, 40), "2:reporter:scr:1986:1:103"),
        ibid(2, 100),
        supra(3, Some("Jordan"), None, 200),
        unknown(4, 300),
        ibid(5, 400),
    ];
    resolve(&mut citations, None);
    assert_eq!(authorities(&citations), vec![vec![0, 3], vec![1, 2]]);
}

#[test]
fn authorities_merge_the_same_authority_cited_differently() {
    // n1: 2015 SCC 5, [2015] 1 SCR 331. n3: [2015] 1 SCR 331. n5: 2015 SCC 5.
    let neutral = || case(0, "2015 SCC 5", 0).neutral("2015", "SCC", "5").build();
    let report = || case(0, "[2015] 1 SCR 331", 0).reporter(Some("2015"), Some("1"), "SCR", "331").build();
    let mut a = keyed(neutral(), "2:neutral:2015:scc:5");
    a.index = 0;
    a.parallel_group = Some(0);
    let mut b = keyed(report(), "2:reporter:scr:2015:1:331");
    b.index = 1;
    b.parallel_group = Some(0);
    let mut c = keyed(report(), "2:reporter:scr:2015:1:331");
    c.index = 2;
    let mut d = keyed(jordan(3, 0), "2:neutral:2016:scc:27");
    d.index = 3;
    let mut e = keyed(neutral(), "2:neutral:2015:scc:5");
    e.index = 4;
    let citations = vec![a, b, c, d, e];
    assert_eq!(authorities(&citations), vec![vec![0, 1, 2, 4], vec![3]]);
}

#[test]
fn authorities_merge_even_when_the_pair_appears_later() {
    let mut first = case(0, "2015 SCC 5", 0).neutral("2015", "SCC", "5").build();
    first.key = Some("2:neutral:2015:scc:5".into());
    let mut second = case(1, "[2015] 1 SCR 331", 30).reporter(Some("2015"), Some("1"), "SCR", "331").build();
    second.key = Some("2:reporter:scr:2015:1:331".into());
    let mut third = second.clone();
    third.index = 2;
    third.parallel_group = Some(2);
    let mut fourth = first.clone();
    fourth.index = 3;
    fourth.parallel_group = Some(2);
    let citations = vec![first, second, third, fourth];
    assert_eq!(authorities(&citations), vec![vec![0, 1, 2, 3]]);
}

#[test]
fn authorities_compute_missing_keys() {
    let first = case(0, "2015 SCC 5", 0).neutral("2015", "SCC", "5").court("scc").build();
    let second = case(1, "2015 CSC 5", 40).neutral("2015", "CSC", "5").court("scc").build();
    let other = case(2, "2015 SCC 6", 80).neutral("2015", "SCC", "6").court("scc").build();
    let citations = vec![first, second, other];
    assert_eq!(authorities(&citations), vec![vec![0, 1], vec![2]]);
}

#[test]
fn every_reason_is_reported_once_per_reference() {
    let citations = vec![jordan(0, 10), ibid(1, 50), supra(2, Some("Jordan"), None, 90), unknown(3, 120)];
    let resolutions = resolve_with_reasons(&citations, None);
    assert_eq!(resolutions.iter().map(|r| r.index).collect::<Vec<_>>(), vec![1, 2, 3]);
    let _ = Format::Neutral;
}
