mod downstream_common;

use downstream_common::*;
use legal_citations::parallel::{group, members, preferred_in, rank_in};
use legal_citations::{Authority, Citation, Form, Format};

fn scr(index: usize, text: &str, occurrence: usize, year: &str, volume: &str, page: &str) -> downstream_common::B {
    let core = format!("[{year}] {volume} SCR {page}");
    case(index, &core, at(text, &core, occurrence)).reporter(Some(year), Some(volume), "SCR", page)
}

fn dlr(index: usize, text: &str, volume: &str, page: &str) -> downstream_common::B {
    let core = format!("{volume} DLR (4th) {page}");
    case(index, &core, at(text, &core, 0)).reporter(None, Some(volume), "DLR (4th)", page)
}

fn scc(index: usize, text: &str, year: &str, number: &str) -> downstream_common::B {
    let core = format!("{year} SCC {number}");
    case(index, &core, at(text, &core, 0)).neutral(year, "SCC", number).court("scc")
}

#[test]
fn oakes_reporter_pair_groups_and_shares_style() {
    let text = "R v Oakes, [1986] 1 SCR 103, 26 DLR (4th) 200.";
    let mut citations = vec![
        scr(0, text, 0, "1986", "1", "103").style("R v Oakes").parties("R", "Oakes").build(),
        dlr(1, text, "26", "200").build(),
    ];
    group(text, &mut citations);
    assert_eq!(citations[0].parallel_group, Some(0));
    assert_eq!(citations[1].parallel_group, Some(0));
    assert_eq!(citations[1].short_name.as_deref(), Some("R v Oakes"));
    assert_eq!(citations[1].style.as_ref().map(|s| s.text.as_str()), Some("R v Oakes"));
    assert_eq!(citations[1].parties.as_ref().map(|p| p.defendant.as_str()), Some("Oakes"));
    assert_eq!(citations[1].fields.year.as_deref(), Some("1986"));
    assert_eq!(members(&citations, 0), vec![0, 1]);
}

#[test]
fn neutral_and_scr_group_and_neutral_is_preferred() {
    let text = "Canada v Craig, 2012 SCC 43, [2012] 2 SCR 489";
    let mut citations = vec![
        scc(0, text, "2012", "43").style("Canada v Craig").build(),
        scr(1, text, 0, "2012", "2", "489").build(),
    ];
    group(text, &mut citations);
    assert_eq!(citations[1].parallel_group, Some(0));
    assert_eq!(citations[1].court.as_ref().map(|c| c.id.as_str()), Some("scc"));
    assert_eq!(preferred_in(&citations, 0, registry()), 0);
}

#[test]
fn preferred_skips_a_leading_general_reporter() {
    let text = "Oakes (1986), 26 DLR (4th) 200, [1986] 1 SCR 103";
    let mut citations = vec![
        dlr(0, text, "26", "200").style("Oakes (1986)").build(),
        scr(1, text, 0, "1986", "1", "103").build(),
    ];
    group(text, &mut citations);
    assert_eq!(citations[1].parallel_group, Some(0));
    assert_eq!(preferred_in(&citations, 0, registry()), 1);
}

#[test]
fn ranks_follow_reporter_kinds() {
    let text = "x";
    let official = case(0, "x", 0).reporter(Some("1986"), Some("1"), "SCR", "1").build();
    let general = case(0, "x", 0).reporter(None, Some("1"), "DLR (4th)", "1").build();
    let specialty = case(0, "x", 0).reporter(None, Some("1"), "CCC (3d)", "1").build();
    let unknown = case(0, "x", 0).reporter(None, Some("1"), "Zzz Rep", "1").build();
    let digest = case(0, "x", 0).reporter(None, Some("1"), "ACWS (3d)", "1").build();
    let canlii = case(0, "x", 0).format(Format::CanLii).build();
    let database = case(0, "x", 0)
        .format(Format::Database)
        .fields(|f| f.reporter = Some("CarswellOnt".into()))
        .build();
    let neutral = case(0, "x", 0).neutral("2019", "ONCA", "1").build();
    let _ = text;
    let ranks = [&neutral, &official, &general, &specialty, &unknown, &canlii, &database, &digest]
        .map(|citation| rank_in(citation, registry()));
    assert_eq!(ranks, [0, 1, 2, 3, 3, 4, 5, 6]);
}

#[test]
fn database_beats_digest_when_both_are_all_there_is() {
    let text = "Smith v Jones, 300 ACWS (3d) 5, 2019 CarswellOnt 3";
    let mut citations = vec![
        case(0, "300 ACWS (3d) 5", at(text, "300", 0))
            .reporter(None, Some("300"), "ACWS (3d)", "5")
            .style("Smith v Jones")
            .build(),
        case(1, "2019 CarswellOnt 3", at(text, "2019", 0))
            .format(Format::Database)
            .fields(|f| {
                f.year = Some("2019".into());
                f.reporter = Some("CarswellOnt".into());
                f.number = Some("3".into());
            })
            .build(),
    ];
    group(text, &mut citations);
    assert_eq!(citations[1].parallel_group, Some(0));
    assert_eq!(preferred_in(&citations, 0, registry()), 1);
}

#[test]
fn three_member_group_chains_to_first_index() {
    let text = "R v Smith, [1990] 1 SCR 5, 26 DLR (4th) 200, 55 CCC (3d) 1";
    let mut citations = vec![
        scr(0, text, 0, "1990", "1", "5").style("R v Smith").build(),
        dlr(1, text, "26", "200").build(),
        case(2, "55 CCC (3d) 1", at(text, "55 CCC", 0)).reporter(None, Some("55"), "CCC (3d)", "1").build(),
    ];
    group(text, &mut citations);
    assert!(citations.iter().all(|c| c.parallel_group == Some(0)));
    assert_eq!(members(&citations, 0), vec![0, 1, 2]);
    assert_eq!(preferred_in(&citations, 0, registry()), 0);
}

#[test]
fn semicolon_separates_authorities() {
    let text = "2015 SCC 5; 2015 SCC 6";
    let mut citations = vec![scc(0, text, "2015", "5").build(), scc(1, text, "2015", "6").build()];
    group(text, &mut citations);
    assert!(citations.iter().all(|c| c.parallel_group.is_none()));
}

#[test]
fn two_different_neutral_citations_never_group() {
    let text = "2015 SCC 5, 2015 SCC 6";
    let mut citations = vec![scc(0, text, "2015", "5").build(), scc(1, text, "2015", "6").build()];
    group(text, &mut citations);
    assert!(citations.iter().all(|c| c.parallel_group.is_none()));
}

#[test]
fn a_new_style_of_cause_starts_a_new_authority() {
    let text = "R v A, 2015 SCC 5, R v B, [2015] 1 SCR 331";
    let mut citations = vec![
        scc(0, text, "2015", "5").style("R v A").build(),
        scr(1, text, 0, "2015", "1", "331").style("R v B").build(),
    ];
    group(text, &mut citations);
    assert!(citations.iter().all(|c| c.parallel_group.is_none()));
}

#[test]
fn intervening_words_break_a_group() {
    let text = "2015 SCC 5, aff'g [2014] 1 SCR 331";
    let mut citations = vec![scc(0, text, "2015", "5").build(), scr(1, text, 0, "2014", "1", "331").build()];
    group(text, &mut citations);
    assert!(citations[1].parallel_group.is_none());
}

#[test]
fn statutes_and_journals_never_group() {
    let text = "RSC 1985, c C-46, SC 2019, c 25";
    let mut citations = vec![
        cite(0, Form::Full, Authority::Statute, "RSC 1985, c C-46", 0).format(Format::StatuteVolume).build(),
        cite(1, Form::Full, Authority::Statute, "SC 2019, c 25", at(text, "SC 2019", 0)).format(Format::StatuteVolume).build(),
    ];
    group(text, &mut citations);
    assert!(citations.iter().all(|c| c.parallel_group.is_none()));
    let text = "(2010) 55 McGill LJ 1, 26 DLR (4th) 200";
    let mut citations = vec![
        cite(0, Form::Full, Authority::Journal, "(2010) 55 McGill LJ 1", 0).format(Format::Publication).build(),
        dlr(1, text, "26", "200").build(),
    ];
    group(text, &mut citations);
    assert!(citations.iter().all(|c| c.parallel_group.is_none()));
}

#[test]
fn same_reporter_at_two_pages_is_two_decisions() {
    let text = "[1986] 1 SCR 103, [1986] 1 SCR 200";
    let mut citations = vec![
        scr(0, text, 0, "1986", "1", "103").build(),
        scr(1, text, 0, "1986", "1", "200").build(),
    ];
    // Both cores start with "[1986] 1 SCR"; place the second one explicitly.
    citations[1].span.start = at(text, "[1986] 1 SCR 200", 0);
    citations[1].span.end = text.len();
    citations[1].full_span = citations[1].span.clone();
    group(text, &mut citations);
    assert!(citations[1].parallel_group.is_none());
}

#[test]
fn distant_years_are_not_parallel() {
    let text = "[1950] 1 SCR 5, [1990] 1 SCR 7";
    let mut citations = vec![
        scr(0, text, 0, "1950", "1", "5").build(),
        scr(1, text, 0, "1990", "1", "7").build(),
    ];
    group(text, &mut citations);
    assert!(citations[1].parallel_group.is_none());
}

#[test]
fn court_is_shared_backwards_from_a_trailing_parenthetical() {
    let text = "Smith v Jones, 2004 CanLII 12345, 70 OR (3d) 1 (ON CA)";
    let mut citations = vec![
        case(0, "2004 CanLII 12345", at(text, "2004", 0))
            .format(Format::CanLii)
            .fields(|f| {
                f.year = Some("2004".into());
                f.number = Some("12345".into());
            })
            .style("Smith v Jones")
            .build(),
        case(1, "70 OR (3d) 1", at(text, "70 OR", 0))
            .reporter(None, Some("70"), "OR (3d)", "1")
            .court("onca")
            .full_to(text, text.len())
            .build(),
    ];
    group(text, &mut citations);
    assert_eq!(citations[1].parallel_group, Some(0));
    assert_eq!(citations[0].court.as_ref().map(|c| c.id.as_str()), Some("onca"));
    assert_eq!(preferred_in(&citations, 0, registry()), 1);
}

#[test]
fn members_of_an_ungrouped_citation_is_itself() {
    let citations: Vec<Citation> = vec![case(0, "2015 SCC 5", 0).neutral("2015", "SCC", "5").build()];
    assert_eq!(members(&citations, 0), vec![0]);
    assert_eq!(preferred_in(&citations, 0, registry()), 0);
}
