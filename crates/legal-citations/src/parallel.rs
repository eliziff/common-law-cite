//! Group citations to the same decision printed side by side.
//!
//! `R v Oakes, [1986] 1 SCR 103, 26 DLR (4th) 200` and
//! `2015 SCC 5, [2015] 1 SCR 331` are one decision cited in several reporters.
//! Following eyecite's `is_parallel_citation`, two consecutive case citations
//! are parallel when nothing but commas and whitespace separates them and the
//! later one carries no style of cause or signal of its own. A semicolon, a
//! sentence break, a name or any other citation in between ends the group.
//!
//! On top of eyecite, two readings are refused because they always name two
//! decisions: two neutral citations with different identities, and two cites
//! to the same reporter at different volumes or pages. Citations with
//! different registry courts, or whose years are more than two years apart,
//! are never grouped either. Statutes, journals and every other non-case
//! authority are never grouped.
//!
//! Every member's [`Citation::parallel_group`] is the index of the group's
//! first member. Later members inherit the first member's parties, style,
//! short names and year when they have none; court and jurisdiction are shared
//! from whichever member names them (a trailing `(ON CA)` usually sits on the
//! last one).
//!
//! [`preferred`] picks the member a consumer that keeps only one citation per
//! decision should keep.

use crate::key;
use crate::model::{Authority, Citation, Form, Format};
use crate::registry::{self, fold, Registry, ReporterKind};

/// Group parallel citations in place. `text` is the text the citations were
/// extracted from; spans index into it.
pub fn group(text: &str, citations: &mut [Citation]) {
    for position in 1..citations.len() {
        if !joinable(text, &citations[position - 1], &citations[position]) {
            continue;
        }
        let first = citations[position - 1]
            .parallel_group
            .unwrap_or(citations[position - 1].index);
        if citations[..position].iter().any(|member|
            (member.index == first || member.parallel_group == Some(first))
                && distinct(member, &citations[position])) {
            continue;
        }
        citations[position - 1].parallel_group = Some(first);
        citations[position].parallel_group = Some(first);
    }
    share_metadata(citations);
}

fn case_like(citation: &Citation) -> bool {
    citation.form == Form::Full
        && match citation.authority {
            Authority::Case => true,
            Authority::Unknown => matches!(
                citation.format,
                Some(Format::Neutral | Format::Reporter | Format::CanLii | Format::Database)
            ),
            _ => false,
        }
}

fn joinable(text: &str, previous: &Citation, current: &Citation) -> bool {
    if !case_like(previous) || !case_like(current) {
        return false;
    }
    if current.style.is_some() || current.signal.is_some() {
        return false;
    }
    let start = current.full_span.start;
    // A shared trailing court/date extends the first full span across its
    // parallels. Compare the core and its own locators, while still excluding
    // citations nested inside an explanatory parenthetical.
    let end = if previous.full_span.end > start {
        if previous.parentheticals.iter().any(|part|
            part.span.start <= current.span.start && current.span.start < part.span.end) {
            return false;
        }
        previous.pinpoints.iter().map(|pin| pin.span.end)
            .chain(previous.parentheticals.iter().map(|part| part.span.end))
            .filter(|end| *end <= start).max().unwrap_or(previous.span.end)
            .max(previous.span.end)
    } else { previous.full_span.end };
    if start < end {
        return false;
    }
    let Some(gap) = text.get(end..start) else {
        return false;
    };
    let only_commas = gap
        .chars()
        .all(|character| character == ',' || character.is_whitespace());
    let comma = gap.contains(',') || previous.full_span.text.trim_end().ends_with(',');
    if !only_commas || !comma {
        return false;
    }
    !distinct(previous, current)
}

/// Readings that always name two different decisions.
pub(crate) fn distinct(left: &Citation, right: &Citation) -> bool {
    if left.is_ambiguous() || right.is_ambiguous() { return true; }
    // Evidence-backed aliases can contradict a printed parallel even when
    // the two members use different reporters and have no written courts.
    let canonical = |citation: &Citation| {
        crate::aliases::resolve(citation).map(|target| target.key.clone())
            .or_else(|| (citation.format == Some(Format::Neutral)).then(|| key::key(citation)).flatten())
    };
    if let (Some(left), Some(right)) = (canonical(left), canonical(right)) {
        if left != right { return true; }
    }
    if left.format == Some(Format::Neutral) && right.format == Some(Format::Neutral) {
        let identity = |citation: &Citation| {
            key::key(citation).or_else(|| {
                Some(format!(
                    "{:?}:{:?}:{:?}",
                    citation.fields.year, citation.fields.series, citation.fields.number
                ))
            })
        };
        if identity(left) != identity(right) {
            return true;
        }
    }
    if let (Some(left_court), Some(right_court)) = (&left.court, &right.court) {
        if left_court.id != right_court.id {
            return true;
        }
    }
    let reporter = |citation: &Citation| {
        citation
            .fields
            .reporter_canonical
            .as_deref()
            .or(citation.fields.reporter.as_deref())
            .map(fold)
    };
    if left.format == Some(Format::Reporter)
        && right.format == Some(Format::Reporter)
        && reporter(left).is_some()
        && reporter(left) == reporter(right)
        && (left.fields.volume != right.fields.volume || left.fields.page != right.fields.page
            || (left.fields.reporter_id.is_some() && right.fields.reporter_id.is_some()
                && left.fields.reporter_id != right.fields.reporter_id))
    {
        return true;
    }
    if left.format == Some(Format::Reporter) && right.format == Some(Format::Reporter)
        && reporter(left).is_some() && reporter(left) == reporter(right)
    {
        if let (Some(left), Some(right)) = (key::key(left), key::key(right)) {
            if left != right { return true; }
        }
    }
    let year = |citation: &Citation| {
        citation
            .fields
            .year
            .as_deref()
            .and_then(|year| year.trim_matches(|c: char| !c.is_ascii_digit()).parse::<i32>().ok())
    };
    if let (Some(left_year), Some(right_year)) = (year(left), year(right)) {
        if (left_year - right_year).abs() > 2 {
            return true;
        }
    }
    false
}

fn share_metadata(citations: &mut [Citation]) {
    let mut position = 0;
    while position < citations.len() {
        let Some(group) = citations[position].parallel_group else {
            position += 1;
            continue;
        };
        let mut end = position + 1;
        while end < citations.len() && citations[end].parallel_group == Some(group) {
            end += 1;
        }
        let members = &mut citations[position..end];
        let first = members[0].clone();
        let court = members.iter().find_map(|member| member.court.clone());
        let jurisdiction = members.iter().find_map(|member| member.jurisdiction.clone());
        let year = members.iter().find_map(|member| member.fields.year.clone());
        let year_number = members.iter().find_map(|member| member.fields.year.as_ref().map(|_| member.fields.year_number)).flatten();
        for member in members.iter_mut() {
            if member.court.is_none() {
                member.court.clone_from(&court);
            }
            if member.jurisdiction.is_none() {
                member.jurisdiction.clone_from(&jurisdiction);
            }
            if member.fields.year.is_none() {
                member.fields.year.clone_from(&year);
                member.fields.year_number = year_number;
            }
        }
        for member in members.iter_mut().skip(1) {
            if member.parties.is_none() {
                member.parties.clone_from(&first.parties);
            }
            if member.style.is_none() {
                member.style.clone_from(&first.style);
            }
            if member.short_name.is_none() {
                member.short_name.clone_from(&first.short_name);
            }
            if member.explicit_short_name.is_none() {
                member.explicit_short_name.clone_from(&first.explicit_short_name);
            }
        }
        position = end;
    }
}

/// Indices of the members of parallel group `group` (the group's first
/// index), in document order. A citation outside any group is its own group.
pub fn members(citations: &[Citation], group: usize) -> Vec<usize> {
    let members = citations
        .iter()
        .filter(|citation| citation.parallel_group == Some(group))
        .map(|citation| citation.index)
        .collect::<Vec<_>>();
    if members.is_empty() {
        vec![group]
    } else {
        members
    }
}

/// How strongly a consumer should prefer this citation within its group:
/// 0 neutral, 1 official reporter, 2 general reporter, 3 specialty reporter
/// or a reporter the registry does not know (or only knows unverified), 4
/// CanLII, 5 commercial database, 6 digest. Lower is better.
pub fn rank(citation: &Citation) -> u8 {
    rank_in(citation, registry::registry())
}

/// [`rank`] against `registry`.
pub fn rank_in(citation: &Citation, registry: &Registry) -> u8 {
    let kind = key::selected_reporter(citation, registry)
        // An unverified registry entry is no evidence of kind.
        .filter(|(reporter, _)| reporter.verified)
        .map(|(reporter, _)| reporter.kind);
    match (citation.format, kind) {
        (Some(Format::Neutral), _) => 0,
        (_, Some(ReporterKind::Digest)) => 6,
        (Some(Format::Database), _) | (_, Some(ReporterKind::Database)) => 5,
        (Some(Format::CanLii), _) => 4,
        (_, Some(ReporterKind::Official)) => 1,
        (_, Some(ReporterKind::General)) => 2,
        (_, Some(ReporterKind::Specialty)) | (_, None) => 3,
    }
}

/// The member of parallel group `group` a consumer keeping one citation per
/// decision should keep: neutral > official reporter > general > specialty >
/// CanLII > commercial database > digest; the earliest wins a tie.
pub fn preferred(citations: &[Citation], group: usize) -> usize {
    preferred_in(citations, group, registry::registry())
}

/// [`preferred`] against `registry`.
pub fn preferred_in(citations: &[Citation], group: usize, registry: &Registry) -> usize {
    members(citations, group)
        .into_iter()
        .filter_map(|index| citations.iter().find(|citation| citation.index == index))
        .min_by_key(|citation| (rank_in(citation, registry), citation.index))
        .map_or(group, |citation| citation.index)
}
