//! Attach short forms, supra, ibid and references to their antecedents.
//!
//! Semantics follow eyecite's `resolve_citations`, extended for McGill and
//! OSCOLA footnote practice. A citation's *authority* is the full citation it
//! ultimately names; a parallel group counts as one authority whose index is
//! the group's first member ([`Citation::parallel_group`]). Every antecedent
//! this module sets is such an index, never another short form.
//!
//! * [`Form::Ibid`] (`Ibid`, `Id.`) refers to the authority of the citation
//!   immediately before it. When it opens a footnote ([`NoteRange`]s given),
//!   it refers to the previous note's authority, which must be single: a
//!   previous note citing several authorities makes it ambiguous
//!   (`ibid_after_multiple`). Ibid chains across notes (`n1: A. n2: Ibid.
//!   n3: Ibid at 5` all name A). As in eyecite, an [`Form::Unknown`] citation
//!   or any unresolved reference breaks the chain.
//! * [`Form::Supra`] with a note number (`Jordan, supra note 4`, `(n 4)`)
//!   looks in note N of the reference's own numbering sequence, then in a
//!   unique note N of any sequence. The note's authorities are narrowed by the
//!   name hint (short name, explicit short name, style or the text before
//!   `supra`). A hint that matches none of them, several remaining
//!   candidates, or a note number that does not exist leaves it unresolved.
//!   Without note ranges, or without a note number, the name hint alone must
//!   match exactly one earlier authority.
//! * [`Form::Reference`] (`Jordan at para 12`) matches its name against
//!   earlier authorities; only a unique match resolves.
//! * [`Form::Short`] (`123 F.3d at 456`) matches an earlier full citation
//!   with the same reporter and volume whose first page is not after the pin;
//!   several candidates are narrowed by name, else it stays unresolved.
//!
//! Name matching is case-, accent- and punctuation-insensitive and matches a
//! hint against a contiguous run of words of the candidate's names
//! (`Jordan` matches `R v Jordan`); a Crown-only hint (`R`) never matches.
//!
//! Note numbering is the caller's: this module honours
//! [`NoteRange::number`] and [`NoteRange::sequence`] and reads notes in
//! `(sequence, number)` order; it never infers numbering.

use crate::key;
use crate::model::{Citation, Form};
use crate::registry::fold;
use crate::NoteRange;
use serde::Serialize;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

/// How one non-full citation was (or was not) resolved.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Resolution {
    /// [`Citation::index`] of the reference.
    pub index: usize,
    /// The authority it resolves to (a full citation's index, or its parallel
    /// group's first index).
    pub antecedent: Option<usize>,
    /// Machine-readable reason:
    ///
    /// * ibid: `ibid_previous`, `ibid_previous_note`, `ibid_after_multiple`,
    ///   `ibid_after_unresolved`, `ibid_no_previous`;
    /// * supra and references: `note_and_name`, `note_only`, `name_only`,
    ///   `ambiguous_note`, `ambiguous_name`, `note_out_of_range`,
    ///   `note_without_authority`, `note_name_conflict`, `no_match`, `no_hint`;
    /// * short forms: `short_reporter`, `short_name`, `short_ambiguous`,
    ///   `short_no_match`;
    /// * `unknown_form` for [`Form::Unknown`].
    pub reason: &'static str,
}

/// Set [`Citation::antecedent`] on every short form, supra, ibid and reference.
pub fn resolve(citations: &mut [Citation], notes: Option<&[NoteRange]>) {
    let resolutions = resolve_with_reasons(citations, notes);
    for resolution in resolutions {
        if let Some(citation) = citations
            .iter_mut()
            .find(|citation| citation.index == resolution.index)
        {
            citation.antecedent = resolution.antecedent;
        }
    }
}

/// The resolution of every non-full citation with the reason for it, without
/// changing the citations.
pub fn resolve_with_reasons(citations: &[Citation], notes: Option<&[NoteRange]>) -> Vec<Resolution> {
    Resolver::new(citations, notes).run()
}

struct Resolver<'a> {
    citations: &'a [Citation],
    notes: Option<&'a [NoteRange]>,
    /// Note position (into `notes`) holding each citation.
    note_of: Vec<Option<usize>>,
    /// Reading rank of each note: `(sequence, number)` order.
    note_rank: Vec<usize>,
    /// Resolved authority of each citation position.
    resolved: Vec<Option<usize>>,
    done: Vec<bool>,
}

impl<'a> Resolver<'a> {
    fn new(citations: &'a [Citation], notes: Option<&'a [NoteRange]>) -> Self {
        let notes = notes.filter(|notes| !notes.is_empty());
        let note_of = citations
            .iter()
            .map(|citation| {
                notes.and_then(|notes| {
                    notes.iter().position(|note| {
                        note.start <= citation.span.start && citation.span.start < note.end
                    })
                })
            })
            .collect();
        let mut note_rank = Vec::new();
        if let Some(notes) = notes {
            let mut order = (0..notes.len()).collect::<Vec<_>>();
            order.sort_by_key(|&position| (notes[position].sequence, notes[position].number, notes[position].start));
            note_rank = vec![0; notes.len()];
            for (rank, position) in order.into_iter().enumerate() {
                note_rank[position] = rank;
            }
        }
        Self {
            citations,
            notes,
            note_of,
            note_rank,
            resolved: vec![None; citations.len()],
            done: vec![false; citations.len()],
        }
    }

    /// Citations in reading order: notes in `(sequence, number)` order, and a
    /// citation outside every note after the last note that starts before it.
    fn order(&self) -> Vec<usize> {
        let mut order = (0..self.citations.len()).collect::<Vec<_>>();
        if let Some(notes) = self.notes {
            let rank = |position: usize| -> (i64, usize) {
                let citation = &self.citations[position];
                let note_rank = match self.note_of[position] {
                    Some(note) => self.note_rank[note] as i64,
                    None => notes
                        .iter()
                        .enumerate()
                        .filter(|(_, note)| note.end <= citation.span.start)
                        .map(|(note, _)| self.note_rank[note] as i64)
                        .max()
                        .unwrap_or(-1),
                };
                (note_rank, citation.span.start)
            };
            order.sort_by_key(|&position| rank(position));
        }
        order
    }

    fn run(mut self) -> Vec<Resolution> {
        let mut output = Vec::new();
        let mut previous: Option<usize> = None;
        for position in self.order() {
            let citation = &self.citations[position];
            let (antecedent, reason) = match citation.form {
                Form::Full => {
                    self.resolved[position] = Some(authority_of_full(citation));
                    self.done[position] = true;
                    previous = Some(position);
                    continue;
                }
                Form::Ibid => self.ibid(position, previous),
                Form::Supra => self.supra(position),
                Form::Reference => self.by_name(position, reference_name(citation)),
                Form::Short => self.short(position),
                Form::Unknown => (None, "unknown_form"),
            };
            self.resolved[position] = antecedent;
            self.done[position] = true;
            previous = Some(position);
            output.push(Resolution {
                index: citation.index,
                antecedent,
                reason,
            });
        }
        output.sort_by_key(|resolution| resolution.index);
        output
    }

    fn ibid(&self, position: usize, previous: Option<usize>) -> (Option<usize>, &'static str) {
        let note = self.note_of[position];
        let opens_note = note.is_some() && previous.map_or(true, |previous| self.note_of[previous] != note);
        if let (Some(note), true) = (note, opens_note) {
            let rank = self.note_rank[note];
            let Some(previous_note) = rank
                .checked_sub(1)
                .and_then(|wanted| self.note_rank.iter().position(|&rank| rank == wanted))
            else {
                return (None, "ibid_no_previous");
            };
            let authorities = (0..self.citations.len())
                .filter(|&other| self.note_of[other] == Some(previous_note) && self.done[other])
                .map(|other| self.resolved[other])
                .collect::<Vec<_>>();
            if authorities.is_empty() {
                return (None, "ibid_no_previous");
            }
            if authorities.iter().any(Option::is_none) {
                return (None, "ibid_after_unresolved");
            }
            let mut distinct = authorities.into_iter().flatten().collect::<Vec<_>>();
            distinct.dedup();
            distinct.sort_unstable();
            distinct.dedup();
            return match distinct.as_slice() {
                [only] => (Some(*only), "ibid_previous_note"),
                _ => (None, "ibid_after_multiple"),
            };
        }
        match previous {
            None => (None, "ibid_no_previous"),
            Some(previous) => match self.resolved[previous] {
                Some(authority) => (Some(authority), "ibid_previous"),
                None => (None, "ibid_after_unresolved"),
            },
        }
    }

    fn supra(&self, position: usize) -> (Option<usize>, &'static str) {
        let citation = &self.citations[position];
        let hint = reference_name(citation);
        let (Some(number), Some(notes)) = (citation.fields.note, self.notes) else {
            return self.by_name(position, hint);
        };
        let own_sequence = self.note_of[position].map(|note| notes[note].sequence);
        let numbered = |sequence: Option<u32>| {
            notes
                .iter()
                .enumerate()
                .filter(|(_, note)| note.number == number && sequence.map_or(true, |sequence| note.sequence == sequence))
                .map(|(position, _)| position)
                .collect::<Vec<_>>()
        };
        let mut found = own_sequence.map(|sequence| numbered(Some(sequence))).unwrap_or_default();
        if found.is_empty() {
            found = numbered(None);
        }
        let target = match found.as_slice() {
            [] => return (None, "note_out_of_range"),
            [only] => *only,
            _ => return (None, "ambiguous_note"),
        };
        let mut candidates = Vec::new();
        for other in 0..self.citations.len() {
            let candidate = &self.citations[other];
            if self.note_of[other] == Some(target) && candidate.form == Form::Full && other != position {
                let authority = authority_of_full(candidate);
                if !candidates.iter().any(|(existing, _)| *existing == authority) {
                    candidates.push((authority, other));
                }
            }
        }
        if candidates.is_empty() {
            return (None, "note_without_authority");
        }
        match hint {
            None => match candidates.as_slice() {
                [(authority, _)] => (Some(*authority), "note_only"),
                _ => (None, "ambiguous_note"),
            },
            Some(hint) => {
                let matched = self.matching(&hint, candidates.iter().map(|&(authority, other)| (authority, other)));
                match matched.as_slice() {
                    [only] => (Some(*only), "note_and_name"),
                    [] => (None, "note_name_conflict"),
                    _ => (None, "ambiguous_name"),
                }
            }
        }
    }

    fn by_name(&self, position: usize, hint: Option<String>) -> (Option<usize>, &'static str) {
        let Some(hint) = hint else {
            return (None, "no_hint");
        };
        let matched = self.matching(&hint, self.earlier_full(position));
        match matched.as_slice() {
            [only] => (Some(*only), "name_only"),
            [] => (None, "no_match"),
            _ => (None, "ambiguous_name"),
        }
    }

    /// `(authority, position)` of every full citation read before `position`.
    fn earlier_full(&self, position: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        (0..self.citations.len())
            .filter(move |&other| other != position && self.done[other] && self.citations[other].form == Form::Full)
            .map(|other| (authority_of_full(&self.citations[other]), other))
    }

    /// Distinct authorities whose names match `hint`. An exact name match
    /// (explicit short name or observed name) wins over looser word matches.
    fn matching(&self, hint: &str, candidates: impl Iterator<Item = (usize, usize)>) -> Vec<usize> {
        let hint_words = words(hint);
        if hint_words.is_empty() || is_crown(&hint_words) {
            return Vec::new();
        }
        let mut exact = Vec::new();
        let mut loose = Vec::new();
        for (authority, other) in candidates {
            let candidate = &self.citations[other];
            let names = candidate_names(candidate);
            if names.iter().any(|name| words(name) == hint_words)
                && !exact.contains(&authority)
            {
                exact.push(authority);
            }
            if names.iter().any(|name| contains_words(&words(name), &hint_words)) && !loose.contains(&authority) {
                loose.push(authority);
            }
        }
        if exact.len() == 1 {
            exact
        } else {
            loose
        }
    }

    fn short(&self, position: usize) -> (Option<usize>, &'static str) {
        let citation = &self.citations[position];
        let reporter = reporter_fold(citation);
        let volume = citation.fields.volume.as_deref().map(normal_number);
        let (Some(reporter), Some(volume)) = (reporter, volume) else {
            return (None, "short_no_match");
        };
        let pin = citation
            .fields
            .page
            .as_deref()
            .or_else(|| citation.pinpoints.first().map(|pinpoint| pinpoint.first.as_str()))
            .and_then(|value| value.trim().parse::<u64>().ok());
        let mut candidates = Vec::new();
        for (authority, other) in self.earlier_full(position) {
            let candidate = &self.citations[other];
            if reporter_fold(candidate).as_deref() != Some(reporter.as_str())
                || candidate.fields.volume.as_deref().map(normal_number).as_deref() != Some(volume.as_str())
            {
                continue;
            }
            let page = candidate
                .fields
                .page
                .as_deref()
                .and_then(|value| value.trim().parse::<u64>().ok());
            if let (Some(pin), Some(page)) = (pin, page) {
                if page > pin {
                    continue;
                }
            }
            if !candidates.iter().any(|(existing, _)| *existing == authority) {
                candidates.push((authority, other));
            }
        }
        match candidates.as_slice() {
            [] => (None, "short_no_match"),
            [(authority, _)] => (Some(*authority), "short_reporter"),
            _ => match reference_name(citation) {
                Some(hint) => match self.matching(&hint, candidates.iter().copied()).as_slice() {
                    [only] => (Some(*only), "short_name"),
                    _ => (None, "short_ambiguous"),
                },
                None => (None, "short_ambiguous"),
            },
        }
    }
}

fn authority_of_full(citation: &Citation) -> usize {
    citation.parallel_group.unwrap_or(citation.index)
}

fn reporter_fold(citation: &Citation) -> Option<String> {
    let fields = &citation.fields;
    let surface = fields.reporter_canonical.as_deref().or(fields.reporter.as_deref())?;
    let folded = fold(surface);
    (!folded.is_empty()).then_some(folded)
}

fn normal_number(value: &str) -> String {
    let trimmed = value.trim().trim_start_matches('0');
    if trimmed.is_empty() {
        "0".into()
    } else {
        trimmed.to_lowercase()
    }
}

/// Every name a later reference may use for this full citation.
fn candidate_names(citation: &Citation) -> Vec<String> {
    let mut names = Vec::new();
    names.extend(citation.explicit_short_name.clone());
    names.extend(citation.short_name.clone());
    names.extend(citation.style.as_ref().map(|style| style.text.clone()));
    if let Some(parties) = &citation.parties {
        names.push(parties.plaintiff.clone());
        names.push(parties.defendant.clone());
    }
    if names.is_empty() {
        names.push(citation.full_span.text.clone());
    }
    names
}

/// The name a supra, reference or short form uses: its short name, explicit
/// short name, style or parties, else the text in front of `supra`/`above`/
/// `(n 4)`/`at`.
pub fn reference_name(citation: &Citation) -> Option<String> {
    let direct = citation
        .explicit_short_name
        .clone()
        .or_else(|| citation.short_name.clone())
        .or_else(|| citation.style.as_ref().map(|style| style.text.clone()))
        .or_else(|| citation.parties.as_ref().map(|parties| parties.plaintiff.clone()));
    let cleaned = |value: &str| {
        let value = value
            .trim()
            .trim_matches(|character: char| ",;:.[]()\"'\u{201c}\u{201d}\u{2018}\u{2019}".contains(character) || character.is_whitespace());
        (!value.is_empty()).then(|| value.to_owned())
    };
    if let Some(name) = direct.as_deref().and_then(cleaned) {
        return Some(name);
    }
    let text = if citation.full_span.text.is_empty() {
        &citation.span.text
    } else {
        &citation.full_span.text
    };
    let lower = text.to_lowercase();
    let cut = [
        "supra", "above", "ci-dessus", "(n ", "(n\u{a0}", " at ", " au ", " aux ", " à la ", "ibid", " id.",
    ]
    .iter()
    .filter_map(|marker| lower.find(marker))
    .min()?;
    cleaned(&text[..cut])
}

/// Case-, accent- and punctuation-insensitive words; `v`, `c`, `vs` and
/// `versus` read as `v`; a leading `the` is dropped.
fn words(value: &str) -> Vec<String> {
    let folded = value
        .nfkd()
        .filter(|character| !unicode_normalization::char::is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect::<String>();
    let mut output = folded
        .split_whitespace()
        .map(|word| match word {
            "c" | "vs" | "versus" => "v".to_owned(),
            other => other.to_owned(),
        })
        .collect::<Vec<_>>();
    if output.first().is_some_and(|word| word == "the") {
        output.remove(0);
    }
    output
}

fn is_crown(words: &[String]) -> bool {
    let joined = words.join(" ");
    matches!(
        joined.as_str(),
        "r" | "regina" | "rex" | "reginam" | "queen" | "king" | "her majesty the queen" | "his majesty the king"
            | "her majesty" | "his majesty" | "sa majeste la reine" | "sa majeste le roi" | "united states"
            | "people" | "state" | "commonwealth"
    )
}

fn contains_words(haystack: &[String], needle: &[String]) -> bool {
    !needle.is_empty()
        && haystack.len() >= needle.len()
        && haystack.windows(needle.len()).any(|window| window == needle)
}

/// Cluster citations by authority: the shape eyecite's `resolve_citations`
/// returns. Each cluster lists citation indices in document order; clusters
/// are ordered by their first citation. Unresolved short forms, supra, ibid,
/// references and unknown citations belong to no cluster.
///
/// Full citations merge when they share a parallel group or a key (see
/// [`crate::key`]), so the same authority cited differently merges: `2015 SCC
/// 5` in one note and `[2015] 1 SCR 331` in another join when either appears
/// as a parallel pair anywhere in the document.
pub fn authorities(citations: &[Citation]) -> Vec<Vec<usize>> {
    let position_of = citations
        .iter()
        .enumerate()
        .map(|(position, citation)| (citation.index, position))
        .collect::<HashMap<_, _>>();
    let mut parent = (0..citations.len()).collect::<Vec<_>>();
    fn find(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }
    fn union(parent: &mut [usize], left: usize, right: usize) {
        let (left, right) = (find(parent, left), find(parent, right));
        if left != right {
            let (low, high) = if left < right { (left, right) } else { (right, left) };
            parent[high] = low;
        }
    }
    let member = |citation: &Citation| match citation.form {
        Form::Full => true,
        Form::Unknown => false,
        _ => citation.antecedent.is_some(),
    };
    let mut by_key: HashMap<String, usize> = HashMap::new();
    for (position, citation) in citations.iter().enumerate() {
        if !member(citation) {
            continue;
        }
        if citation.form == Form::Full {
            if let Some(&group) = citation.parallel_group.as_ref().and_then(|group| position_of.get(group)) {
                union(&mut parent, position, group);
            }
            let identity = citation.key.clone().or_else(|| key::key(citation));
            if let Some(identity) = identity {
                match by_key.get(&identity) {
                    Some(&other) => union(&mut parent, position, other),
                    None => {
                        by_key.insert(identity, position);
                    }
                }
            }
        } else if let Some(&target) = citation.antecedent.as_ref().and_then(|antecedent| position_of.get(antecedent)) {
            union(&mut parent, position, target);
        }
    }
    let mut clusters: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut cluster_of: HashMap<usize, usize> = HashMap::new();
    for (position, citation) in citations.iter().enumerate() {
        if !member(citation) {
            continue;
        }
        let root = find(&mut parent, position);
        let slot = *cluster_of.entry(root).or_insert_with(|| {
            clusters.push((position, Vec::new()));
            clusters.len() - 1
        });
        clusters[slot].1.push(citation.index);
    }
    clusters.sort_by_key(|(first, _)| *first);
    clusters.into_iter().map(|(_, members)| members).collect()
}

/// The first full citation of an authority cluster from [`authorities`]:
/// the citation a table of authorities lists the authority under.
pub fn representative(citations: &[Citation], cluster: &[usize]) -> Option<usize> {
    cluster.iter().copied().find(|index| {
        citations
            .iter()
            .any(|citation| citation.index == *index && citation.form == Form::Full)
    })
}
