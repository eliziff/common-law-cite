//! Attach short forms, supra, ibid and references to their antecedents.
//!
//! Semantics follow eyecite's `resolve_citations`, extended for McGill and
//! OSCOLA footnote practice. A citation's *authority* is the full citation it
//! ultimately names; a parallel group counts as one authority whose index is
//! the group's first member ([`Citation::parallel_group`]). Every antecedent
//! this module sets is such an index, never another short form.
//!
//! * [`Form::Ibid`] (`Ibid`, `Id.`) follows the preceding citation in its
//!   note. At the start of a note it requires exactly one resolved authority
//!   in the immediately preceding numbered note; an empty or mixed note
//!   cannot supply an antecedent. Supplied note numbering determines reading
//!   order even when note text is stored out of order.
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
//!   with the same reporter and volume;
//!   several candidates are narrowed by name, else it stays unresolved.
//!
//! Name matching reuses ALR's exact-short-form, token-short-form, verbatim
//! and bracket-definition tiers, followed by its inferred-short-form lookup.
//! U.S. reporter short forms use Eyecite's party-name matching primitive.
//!
//! Note numbering is the caller's: this module honours
//! [`NoteRange::number`] and [`NoteRange::sequence`] and reads notes in
//! `(sequence, number)` order; it never infers numbering.

use crate::key;
use crate::model::{Citation, Form};
use crate::registry::fold;
use crate::NoteRange;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

/// How one non-full citation was (or was not) resolved.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct Resolution {
    /// [`Citation::index`] of the reference.
    pub index: usize,
    /// The authority it resolves to (a full citation's index, or its parallel
    /// group's first index).
    pub antecedent: Option<usize>,
    /// Machine-readable reason:
    ///
    /// * ibid: `ibid_previous`, `ibid_previous_note`, `ibid_after_multiple`, `ibid_after_unresolved`,
    ///   `ibid_no_previous`, `ibid_invalid_pinpoint`;
    /// * supra and references: `note_and_name`, `note_only`, `name_only`,
    ///   `ambiguous_note`, `ambiguous_name`, `note_out_of_range`,
    ///   `note_without_authority`, `note_name_conflict`, `no_match`, `no_hint`;
    /// * short forms: `short_reporter`, `short_name`, `short_ambiguous`,
    ///   `short_no_match`;
    /// * `unknown_form` for [`Form::Unknown`].
    pub reason: &'static str,
}

/// The resolution-facing representation used by hosts with mutable citation
/// objects and custom resource types. Value tokens preserve host equality for
/// arbitrary metadata values; resource tokens preserve custom resource identity.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ReferenceRecord {
    pub form: Form,
    pub authority: crate::Authority,
    #[serde(default)] pub format: Option<crate::Format>,
    #[serde(default)] pub jurisdiction: Option<String>,
    #[serde(default)] pub reporter: Option<String>,
    #[serde(default)] pub volume: Option<String>,
    #[serde(default)] pub page: Option<String>,
    #[serde(default)] pub plaintiff: Option<String>,
    #[serde(default)] pub defendant: Option<String>,
    #[serde(default)] pub antecedent_guess: Option<String>,
    #[serde(default)] pub pin_cite: Option<String>,
    #[serde(default)] pub name_values: Vec<usize>,
    #[serde(default)] pub metadata_values: Vec<usize>,
    #[serde(default)] pub ambiguous: bool,
}

impl From<&Citation> for ReferenceRecord {
    fn from(citation: &Citation) -> Self {
        let source = citation.fields.source_case_name.as_ref();
        Self {
            form: citation.form, authority: citation.authority, format: citation.format,
            jurisdiction: citation.jurisdiction.clone(),
            reporter: citation.fields.reporter_canonical.clone().or_else(|| citation.fields.reporter.clone()),
            volume: citation.fields.volume.clone(), page: citation.fields.page.clone(),
            plaintiff: source.map(|name| name.plaintiff.clone()).unwrap_or_else(|| citation.parties.as_ref().map(|parties| parties.plaintiff.clone())),
            defendant: source.map(|name| name.defendant.clone()).unwrap_or_else(|| citation.parties.as_ref().map(|parties| parties.defendant.clone())),
            antecedent_guess: source.and_then(|name| name.antecedent_guess.clone()),
            pin_cite: source.filter(|name| name.full_span_end.is_some()).map(|name| name.pin_cite.as_ref())
                .unwrap_or(citation.fields.pin_cite.as_ref()).map(|pin| pin.text.clone()),
            name_values: Vec::new(), metadata_values: Vec::new(), ambiguous: citation.is_ambiguous(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ReferenceRequest {
    pub citation: ReferenceRecord,
    #[serde(default)] pub full_citations: Vec<(ReferenceRecord, Option<usize>)>,
    /// The first citation of the resource returned by the preceding callback.
    #[serde(default)] pub previous: Option<(ReferenceRecord, usize)>,
}

fn unique_resource(resources: impl IntoIterator<Item = Option<usize>>) -> Option<usize> {
    let mut resources = resources.into_iter();
    let first = resources.next()?;
    resources.all(|resource| resource == first).then_some(first).flatten()
}

fn strip_antecedent_punctuation(text: &str) -> String {
    static RULES: std::sync::LazyLock<Vec<(regex::Regex, String)>> = std::sync::LazyLock::new(|| {
        let tables = legal_grammar::load_tables().unwrap();
        (0..12).map(|index| {
            let entry = &tables[&format!("ref.antecedent-punctuation.{index}")].entry;
            (regex::Regex::new(&entry.pattern).unwrap(), if matches!(index, 5 | 11) { "${1}" } else { "" }.to_owned())
        }).collect()
    });
    let mut text = text.to_owned();
    for (pattern, replacement) in RULES.iter() {
        text = pattern.replace_all(&text, replacement.as_str()).into_owned();
    }
    text.trim().to_owned()
}

fn antecedent_matches(guess: &str, plaintiff: Option<&str>, defendant: Option<&str>) -> bool {
    [plaintiff, defendant].into_iter().flatten().any(|name| !name.is_empty() && name.contains(guess))
}

use crate::text::decimal;

fn invalid_id_pin(full_case: bool, page: Option<&str>, pin: Option<&str>) -> bool {
    if full_case && page.is_none() { return true; }
    let Some(pin) = pin.filter(|pin| !pin.is_empty()) else { return false };
    let Some(page) = page.and_then(decimal) else { return false };
    static PIN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(&legal_grammar::load_tables().unwrap()["ref.id-page"].entry.pattern).unwrap()
    });
    let Some(pin) = PIN.captures(pin).and_then(|matched| decimal(&matched[1])) else { return true };
    pin < page || pin > page + num_bigint::BigUint::from(150u8)
}

/// Pinned Eyecite resource matching. The caller supplies the actual resources
/// from preceding callbacks, not a resolution computed before those callbacks.
pub fn resolve_reference(request: &ReferenceRequest) -> Option<usize> {
    let citation = &request.citation;
    if citation.ambiguous { return None; }
    let candidates = request.full_citations.iter().collect::<Vec<_>>();
    let named = |candidates: Vec<&(ReferenceRecord, Option<usize>)>| {
        let guess = citation.antecedent_guess.as_deref().filter(|guess| !guess.is_empty())?;
        let guess = strip_antecedent_punctuation(guess);
        unique_resource(candidates.iter().filter(|(candidate, _)| candidate.authority == crate::Authority::Case
            && antecedent_matches(&guess, candidate.plaintiff.as_deref(), candidate.defendant.as_deref()))
            .map(|(_, resource)| *resource))
    };
    let resource = match citation.form {
        Form::Short => {
            let candidates = candidates.into_iter().filter(|(candidate, _)| candidate.authority == crate::Authority::Case
                && citation.reporter == candidate.reporter && citation.volume == candidate.volume).collect::<Vec<_>>();
            unique_resource(candidates.iter().map(|(_, resource)| *resource)).or_else(|| named(candidates))
        }
        Form::Supra => named(candidates),
        Form::Reference => unique_resource(candidates.iter().filter(|(candidate, _)|
            candidate.metadata_values.iter().any(|value| citation.name_values.contains(value))).map(|(_, resource)| *resource)),
        Form::Ibid => request.previous.as_ref().and_then(|(first, resource)| {
            let page_check = first.jurisdiction.as_deref().is_none_or(|jurisdiction| jurisdiction == "us" || jurisdiction.starts_with("us-"));
            let missing_case_page = first.form == Form::Full && first.authority == crate::Authority::Case
                && matches!(first.format, None | Some(crate::Format::Reporter));
            (!first.ambiguous && !(page_check && invalid_id_pin(missing_case_page,
                first.page.as_deref(), citation.pin_cite.as_deref()))).then_some(*resource)
        }),
        _ => None,
    };
    // Ambiguous full citations participate in matching but cannot supply an identity.
    resource.filter(|resource| !request.full_citations.iter().any(|(candidate, assigned)| candidate.ambiguous && *assigned == Some(*resource)))
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
    resolve_with_links(citations, notes, &[])
}

pub(crate) fn resolve_with_links(citations: &[Citation], notes: Option<&[NoteRange]>, links: &[(usize, usize)]) -> Vec<Resolution> {
    Resolver::new(citations, notes, links).run()
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
    full_authority: Vec<usize>,
}

impl<'a> Resolver<'a> {
    fn new(citations: &'a [Citation], notes: Option<&'a [NoteRange]>, links: &[(usize, usize)]) -> Self {
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
        let clustered = authorities_with_links(citations, links).into_iter().flat_map(|cluster| {
            let first = cluster[0];
            cluster.into_iter().map(move |index| (index, first))
        }).collect::<HashMap<_, _>>();
        let full_authority = citations.iter().map(|citation|
            clustered.get(&citation.index).copied().unwrap_or_else(|| authority_of_full(citation))).collect();
        Self {
            citations,
            notes,
            note_of,
            note_rank,
            resolved: vec![None; citations.len()],
            done: vec![false; citations.len()],
            full_authority,
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
            let (mut antecedent, mut reason) = match citation.form {
                Form::Full => {
                    self.resolved[position] = (!citation.is_ambiguous()).then_some(self.full_authority[position]);
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
            if antecedent.is_some_and(|index| citation.is_ambiguous() || self.citations.iter()
                .any(|candidate| candidate.index == index && candidate.is_ambiguous())) {
                antecedent = None;
                reason = "ambiguous_authority";
            }
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
        let opens_note = note.is_some() && previous.is_none_or(|previous| self.note_of[previous] != note);
        let (authority, reason) = if let (Some(note), true) = (note, opens_note) {
            let rank = self.note_rank[note];
            let Some(previous_note) = rank.checked_sub(1)
                .and_then(|wanted| self.note_rank.iter().position(|&rank| rank == wanted))
            else { return (None, "ibid_no_previous"); };
            let authorities = (0..self.citations.len())
                .filter(|&other| self.note_of[other] == Some(previous_note) && self.done[other])
                .map(|other| self.resolved[other]).collect::<Vec<_>>();
            if authorities.is_empty() { return (None, "ibid_no_previous"); }
            if authorities.iter().any(Option::is_none) { return (None, "ibid_after_unresolved"); }
            let mut distinct = authorities.into_iter().flatten().collect::<Vec<_>>();
            distinct.sort_unstable();
            distinct.dedup();
            let [authority] = distinct.as_slice() else { return (None, "ibid_after_multiple"); };
            (*authority, "ibid_previous_note")
        } else {
            let Some(previous) = previous else { return (None, "ibid_no_previous"); };
            let Some(authority) = self.resolved[previous] else { return (None, "ibid_after_unresolved"); };
            (authority, "ibid_previous")
        };
        let target = self.citations.iter().find(|citation| citation.index == authority);
        let citation = &self.citations[position];
        let pin = citation.fields.pin_cite.as_ref().or_else(|| citation.pinpoints.first().map(|pin| &pin.span));
        if target.is_some_and(|target| is_us(target) && invalid_id_pin(
            target.authority == crate::Authority::Case && target.format == Some(crate::Format::Reporter),
            target.fields.page.as_deref(), pin.map(|pin| pin.text.as_str()),
        )) {
            return (None, "ibid_invalid_pinpoint");
        }
        (Some(authority), reason)
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
                let authority = self.full_authority[other];
                candidates.push((authority, other));
            }
        }
        if candidates.is_empty() {
            return (None, "note_without_authority");
        }
        match hint {
            None => {
                let authority = candidates[0].0;
                if candidates.iter().all(|candidate| candidate.0 == authority) {
                    (Some(authority), "note_only")
                } else { (None, "ambiguous_note") }
            },
            Some(hint) => {
                match self.matching(&hint, candidates.into_iter()) {
                    (Some(authority), _) => (Some(authority), "note_and_name"),
                    (None, "no_match") => (None, "note_name_conflict"),
                    (None, reason) => (None, reason),
                }
            }
        }
    }

    fn by_name(&self, position: usize, hint: Option<String>) -> (Option<usize>, &'static str) {
        if let Some(name) = &self.citations[position].fields.source_case_name {
            // Feed the same exact metadata-value matching used by the facade.
            // Resource tokens include ambiguous candidates; the shared resolver
            // keeps them in the pool and vetoes an uncertain identity.
            let values = [name.plaintiff.as_deref(), name.defendant.as_deref()]
                .into_iter().flatten().filter(|value| !value.is_empty()).collect::<Vec<_>>();
            let mut reference = ReferenceRecord::from(&self.citations[position]);
            reference.name_values = (0..values.len()).collect();
            let full_citations = self.earlier_full(position).map(|(authority, other)| {
                let citation = &self.citations[other];
                let mut record = ReferenceRecord::from(citation);
                let fields = &citation.fields;
                let metadata = [record.plaintiff.as_deref(), record.defendant.as_deref(),
                    record.antecedent_guess.as_deref(), record.pin_cite.as_deref(),
                    fields.year.as_deref(), fields.month.as_deref(), fields.day.as_deref(),
                    fields.extra.as_deref(), fields.publisher.as_deref(),
                    citation.court.as_ref().map(|court| court.id.as_str()),
                    fields.source_case_name.as_ref().filter(|name| name.full_span_end.is_some())
                        .map(|name| name.parenthetical.as_deref()).unwrap_or_else(||
                            citation.parentheticals.iter().find(|part| part.kind == crate::ParentheticalKind::Explanatory)
                                .map(|part| part.content.as_str()))];
                record.metadata_values = values.iter().enumerate().filter_map(|(index, value)|
                    metadata.contains(&Some(*value)).then_some(index)).collect();
                (record, Some(authority))
            }).collect::<Vec<_>>();
            let matched = full_citations.iter().any(|(record, _)| !record.metadata_values.is_empty());
            let authority = resolve_reference(&ReferenceRequest { citation: reference, full_citations, previous: None });
            return (authority, if authority.is_some() { "name_only" } else if matched { "ambiguous_name" } else { "no_match" });
        }
        let Some(hint) = hint else {
            return (None, "no_hint");
        };
        self.matching(&hint, self.earlier_full(position))
    }

    /// `(authority, position)` of every full citation read before `position`.
    fn earlier_full(&self, position: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        (0..self.citations.len())
            .filter(move |&other| other != position && self.done[other] && self.citations[other].form == Form::Full)
            .map(|other| (self.full_authority[other], other))
    }

    /// Adapt observed citation records to ALR's existing strict and inferred
    /// tiers. Opaque resource tokens keep ambiguous candidates in the pool;
    /// run() vetoes an ambiguous winner instead of making another look unique.
    fn matching(&self, hint: &str, candidates: impl Iterator<Item = (usize, usize)>) -> (Option<usize>, &'static str) {
        use crate::short_forms::{self, ReferenceSource};
        let mut registry = Vec::new();
        let mut inferred = Vec::new();
        let mut named = Vec::new();
        for (authority, other) in candidates {
            let citation = &self.citations[other];
            named.push((authority, candidate_names(citation)));
            let record = ReferenceSource {
                verbatim: Some(citation.full_span.text.clone()),
                link: Some(authority.to_string()),
                short_form: citation.explicit_short_name.clone().or_else(|| citation.short_name.clone())
                    .or_else(|| citation.style.as_ref().map(|style| style.text.clone())),
                ..ReferenceSource::default()
            };
            for form in short_forms::infer(&citation.full_span.text, inference_kind(citation)) {
                inferred.push(ReferenceSource {
                    short_form_norm: Some(short_forms::normalize(&form.value)),
                    short_form: Some(form.value), rule: Some(form.rule.into()),
                    link: record.link.clone(), ..ReferenceSource::default()
                });
            }
            registry.push(record);
        }
        let (resource, reason) = short_forms::resolve_registry_hint("", hint, &registry);
        if let Ok(authority) = resource.parse() { return (Some(authority), "name_only"); }
        let candidates = short_forms::reference_candidates_for_hint(hint);
        let (resource, _) = short_forms::resolve_inferred_candidates(&candidates, &registry, &inferred);
        if let Ok(authority) = resource.parse() { return (Some(authority), "name_only"); }
        // The structure parser's word tier covers case styles whose spelling
        // differs only by accents or punctuation. It runs after ALR's strict
        // and inferred forms, and still requires a unique authority.
        let hint_words = words(hint);
        if !hint_words.is_empty() && !is_crown(&hint_words) {
            let mut exact = Vec::new();
            let mut loose = Vec::new();
            for (authority, names) in named {
                if names.iter().any(|name| words(name) == hint_words) && !exact.contains(&authority) {
                    exact.push(authority);
                }
                if names.iter().any(|name| contains_words(&words(name), &hint_words))
                    && !loose.contains(&authority) {
                    loose.push(authority);
                }
            }
            let matches = if exact.len() == 1 { exact } else { loose };
            if let [authority] = matches.as_slice() { return (Some(*authority), "name_only"); }
            if matches.len() > 1 { return (None, "ambiguous_name"); }
        }
        (None, if reason.starts_with("abstain_ambiguous") { "ambiguous_name" } else { "no_match" })
    }

    fn short(&self, position: usize) -> (Option<usize>, &'static str) {
        let citation = &self.citations[position];
        let reporters = reporter_readings(citation);
        let Some(volume) = citation.fields.volume.as_deref().map(normal_number) else {
            return (None, "short_no_match");
        };
        let pin = citation.fields.page.as_deref()
            .or_else(|| citation.pinpoints.first().map(|pinpoint| pinpoint.first.as_str()))
            .and_then(decimal);
        let mut candidates = Vec::new();
        for (authority, other) in self.earlier_full(position) {
            let candidate = &self.citations[other];
            if candidate.authority != crate::Authority::Case
                || !reporter_readings(candidate).iter().any(|(id, surface)| reporters.iter().any(|(wanted_id, wanted)| {
                    surface == wanted && (id.is_none() || wanted_id.is_none() || id == wanted_id)
                }))
                || candidate.fields.volume.as_deref().map(normal_number).as_deref() != Some(volume.as_str())
            {
                continue;
            }
            if let (Some(pin), Some(page)) = (&pin, candidate.fields.page.as_deref().and_then(decimal)) {
                if page > *pin { continue; }
            }
            candidates.push((authority, other));
        }
        if let Some(authority) = unique_resource(candidates.iter().map(|&(authority, _)| Some(authority))) {
            return (Some(authority), "short_reporter");
        }
        if candidates.is_empty() { return (None, "short_no_match"); }
        let Some(hint) = reference_name(citation) else { return (None, "short_ambiguous"); };
        if is_us(citation) {
            let hint = strip_antecedent_punctuation(&hint);
            let authority = unique_resource(candidates.iter().filter(|&&(_, other)| {
                let candidate = &self.citations[other];
                let (plaintiff, defendant) = if let Some(name) = &candidate.fields.source_case_name {
                    (name.plaintiff.as_deref(), name.defendant.as_deref())
                } else {
                    let parties = candidate.parties.as_ref();
                    (parties.map(|parties| parties.plaintiff.as_str()), parties.map(|parties| parties.defendant.as_str()))
                };
                antecedent_matches(&hint, plaintiff, defendant)
            }).map(|&(authority, _)| Some(authority)));
            return (authority, if authority.is_some() { "short_name" } else { "short_ambiguous" });
        }
        let (authority, _) = self.matching(&hint, candidates.into_iter());
        (authority, if authority.is_some() { "short_name" } else { "short_ambiguous" })
    }
}

fn is_us(citation: &Citation) -> bool {
    citation.jurisdiction.as_deref().is_some_and(|jurisdiction| jurisdiction == "us" || jurisdiction.starts_with("us-"))
}

fn authority_of_full(citation: &Citation) -> usize {
    citation.parallel_group.unwrap_or(citation.index)
}

/// Keep unresolved reporter alternatives in the candidate pool. Removing one
/// here could make a different authority appear uniquely resolvable.
fn reporter_readings(citation: &Citation) -> Vec<(Option<&str>, String)> {
    let readings = citation.interpretations.iter().filter(|reading| reading.kind == "reporter").collect::<Vec<_>>();
    let selected = readings.iter().any(|reading| reading.selected);
    if !readings.is_empty() {
        return readings.into_iter().filter(|reading| !selected || reading.selected)
            .map(|reading| (Some(reading.id.as_str()), fold(&reading.canonical))).collect();
    }
    let fields = &citation.fields;
    fields.reporter_canonical.as_deref().or(fields.reporter.as_deref()).into_iter()
        .map(|surface| (fields.reporter_id.as_deref(), fold(surface))).collect()
}

fn normal_number(value: &str) -> String {
    let trimmed = value.trim().trim_start_matches('0');
    if trimmed.is_empty() {
        "0".into()
    } else {
        trimmed.to_lowercase()
    }
}

// Established structure-parser name matching: preserve word boundaries while
// folding accents, case and common case-name connectors.
fn words(value: &str) -> Vec<String> {
    let folded = value.nfkd()
        .filter(|character| !unicode_normalization::char::is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect::<String>();
    let mut output = folded.split_whitespace().map(|word| match word {
        "c" | "vs" | "versus" => "v".to_owned(),
        other => other.to_owned(),
    }).collect::<Vec<_>>();
    if output.first().is_some_and(|word| word == "the") { output.remove(0); }
    output
}

fn is_crown(words: &[String]) -> bool {
    matches!(words.join(" ").as_str(),
        "r" | "regina" | "rex" | "reginam" | "queen" | "king" | "her majesty the queen"
        | "his majesty the king" | "her majesty" | "his majesty" | "sa majeste la reine"
        | "sa majeste le roi" | "united states" | "people" | "state" | "commonwealth")
}

fn contains_words(haystack: &[String], needle: &[String]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|window| window == needle)
}

fn inference_kind(citation: &Citation) -> &'static str {
    match citation.authority {
        crate::Authority::Case => "case",
        crate::Authority::Regulation => "regulation",
        authority if authority.is_legislation() => "statute",
        crate::Authority::Journal => "journal", crate::Authority::Book => "book",
        crate::Authority::BookChapter => "essay_collection",
        crate::Authority::GovernmentDocument => "report", crate::Authority::Webpage => "website",
        _ => "other",
    }
}

/// Every name a later reference may use for this full citation.
pub(crate) fn candidate_names(citation: &Citation) -> Vec<String> {
    let mut names = Vec::new();
    names.extend(citation.explicit_short_name.clone());
    names.extend(citation.short_name.clone());
    names.extend(citation.style.as_ref().map(|style| style.text.clone()));
    if let Some(source) = &citation.fields.source_case_name {
        names.extend(source.plaintiff.clone());
        names.extend(source.defendant.clone());
        names.extend(source.antecedent_guess.clone());
    }
    let parties = citation.parties.clone().or_else(|| {
        if citation.authority == crate::Authority::Case {
            citation.style.as_ref().and_then(|style| crate::metadata::parties(&style.text))
        } else { None }
    });
    if let Some(parties) = parties {
        names.push(parties.plaintiff.clone());
        names.push(parties.defendant.clone());
    }
    names.extend(crate::short_forms::infer(&citation.full_span.text, inference_kind(citation)).into_iter().map(|form| form.value));
    if names.is_empty() {
        names.push(citation.full_span.text.clone());
    }
    names
}

/// The name a supra, reference or short form uses: its short name, explicit
/// short name, style or parties, else the text in front of `supra`/`above`/
/// `(n 4)`/`at`.
pub fn reference_name(citation: &Citation) -> Option<String> {
    if citation.form == Form::Short {
        if let Some(name) = &citation.fields.source_case_name {
            return name.antecedent_guess.clone();
        }
    }
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
    authorities_with_links(citations, &[])
}

pub(crate) fn authorities_with_links(citations: &[Citation], links: &[(usize, usize)]) -> Vec<Vec<usize>> {
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
        if citation.form == Form::Full && !citation.is_ambiguous() {
            if let Some(&group) = citation.parallel_group.as_ref().and_then(|group| position_of.get(group))
                .filter(|&&position| !citations[position].is_ambiguous()) {
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
        } else if let Some(&target) = citation.antecedent.as_ref().and_then(|antecedent| position_of.get(antecedent))
            .filter(|&&position| !citations[position].is_ambiguous()) {
            union(&mut parent, position, target);
        }
    }
    for (left, right) in links {
        if let (Some(&left), Some(&right)) = (position_of.get(left), position_of.get(right)) {
            union(&mut parent, left, right);
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
