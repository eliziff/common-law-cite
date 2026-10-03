//! Attach short forms, supra, ibid and references to their antecedents.
//!
//! Semantics follow eyecite's `resolve_citations`, extended for McGill and
//! OSCOLA footnote practice. A citation's *authority* is the full citation it
//! ultimately names; a parallel group counts as one authority whose index is
//! the group's first member ([`Citation::parallel_group`]). Every antecedent
//! this module sets is such an index, never another short form.
//!
//! * [`Form::Ibid`] (`Ibid`, `Id.`) follows the preceding source part or
//!   citation in reading order when it supplies one unambiguous authority.
//!   An unresolved or mixed source part cannot supply an antecedent.
//! * [`Form::Supra`] with a note number (`Jordan, supra note 4`, `(n 4)`)
//!   uses ALR's ordered source-part registry and its short-form matching tiers.
//!   The reference's numbering sequence constrains note matches. Safe linking
//!   abstains on conflicting or missing evidence; aggressive linking may use
//!   ALR's separate inferred-name and bare-note fallback.
//! * [`Form::Reference`] (`Jordan at para 12`) matches its name against
//!   earlier authorities; only a unique match resolves.
//! * [`Form::Short`] (`123 F.3d at 456`) matches an earlier full citation
//!   with the same reporter and volume;
//!   several candidates are narrowed by name, else it stays unresolved.
//!
//! A full citation of a record document (`(Transcript of hearing at 14)`)
//! names its case, so it keys and clusters with it, but a reference to it
//! locates that document: it never resolves to the decision.
//!
//! Name matching reuses ALR's exact-short-form, token-short-form, verbatim
//! and bracket-definition tiers, followed by its inferred-short-form lookup.
//! U.S. reporter short forms use Eyecite's party-name matching primitive.
//!
//! Note numbering and reading order are the caller's: this module honours
//! [`NoteRange::number`] and [`NoteRange::sequence`] and never infers numbering.

use crate::key;
use crate::model::{Citation, Form};
use crate::registry::fold;
use crate::{NoteRange, SupraMode};
use crate::source::SourcePart;
use crate::short_forms::{self, ReferenceSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

/// How one non-full citation was (or was not) resolved.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct Resolution {
    /// [`Citation::index`] of the reference.
    pub index: usize,
    /// The authority it resolves to (a full citation's index, or its parallel
    /// group's first index).
    pub antecedent: Option<usize>,
    /// Optional navigation URL of the matched source. Its absence has no
    /// effect on association or authority identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Matched source's origin, indexed into ExtractResponse.sourceParts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_part: Option<usize>,
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
            plaintiff: citation.parties.as_ref().and_then(|parties| parties.plaintiff.clone()),
            defendant: citation.parties.as_ref().and_then(|parties| parties.defendant.clone()),
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

fn unique_resource<T: PartialEq>(resources: impl IntoIterator<Item = Option<T>>) -> Option<T> {
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

/// Resolve references against ordered citation and source-part identities.
/// URLs enrich the result after an antecedent has been identified.
pub(crate) fn resolve_with_sources(
    citations: &[Citation], notes: Option<&[NoteRange]>, links: &[(usize, usize)],
    order: Option<Vec<usize>>, parts: &[SourcePart], hint_mode: SupraMode,
    linking_mode: SupraMode,
) -> Vec<Resolution> {
    let mut resolver = Resolver::new(citations, notes, links);
    resolver.reading_order = order;
    resolver.source_parts = parts;
    resolver.supra_hint_mode = hint_mode;
    resolver.supra_linking_mode = linking_mode;
    resolver.run()
}

struct Resolver<'a> {
    citations: &'a [Citation],
    notes: Option<&'a [NoteRange]>,
    /// Note position (into `notes`) holding each citation.
    note_of: Vec<Option<usize>>,
    /// Resolved authority of each citation position.
    resolved: Vec<Option<Target>>,
    done: Vec<bool>,
    full_authority: Vec<usize>,
    reading_order: Option<Vec<usize>>,
    source_parts: &'a [SourcePart],
    supra_hint_mode: SupraMode,
    supra_linking_mode: SupraMode,
    source_urls: HashMap<Target, String>,
    source_origins: HashMap<Target, usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Target {
    Authority(usize),
    Source(usize),
}

impl Target {
    fn from_registry(id: &str) -> Option<Self> {
        let (kind, index) = id.split_once(':')?;
        let index = index.parse().ok()?;
        match kind {
            "citation" => Some(Self::Authority(index)),
            "source" => Some(Self::Source(index)),
            _ => None,
        }
    }

    fn registry_id(&self) -> String {
        match self {
            Self::Authority(index) => format!("citation:{index}"),
            Self::Source(index) => format!("source:{index}"),
        }
    }
}

#[derive(Default)]
struct History {
    records: Vec<ReferenceSource>,
    inferred: Vec<ReferenceSource>,
    last_part_target: Option<Option<Target>>,
}

impl History {
    fn push(&mut self, record: ReferenceSource, kind: &str,
        linking_mode: SupraMode) {
        if linking_mode == SupraMode::Aggressive
            && record.target.as_deref().is_some_and(|target| !target.is_empty()) {
            for form in short_forms::infer(record.verbatim.as_deref().unwrap_or(""), kind) {
                self.inferred.push(ReferenceSource {
                    note: record.note.clone(), sequence: record.sequence,
                    target: record.target.clone(), short_form: Some(form.value.clone()),
                    short_form_norm: Some(short_forms::normalize(&form.value)),
                    rule: Some(form.rule.into()), ..ReferenceSource::default()
                });
            }
        }
        self.records.push(record);
    }
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
            resolved: vec![None; citations.len()],
            done: vec![false; citations.len()],
            full_authority,
            reading_order: None,
            source_parts: &[],
            supra_hint_mode: SupraMode::Aggressive,
            supra_linking_mode: SupraMode::Safe,
            source_urls: HashMap::new(),
            source_origins: HashMap::new(),
        }
    }

    /// Citations in reading order: notes in their supplied order, and a
    /// citation outside every note after the last note that starts before it.
    fn order(&self) -> Vec<usize> {
        if let Some(order) = &self.reading_order { return order.clone(); }
        let mut order = (0..self.citations.len()).collect::<Vec<_>>();
        if let Some(notes) = self.notes {
            let rank = |position: usize| -> (u32, i64, usize) {
                let citation = &self.citations[position];
                let note_rank = match self.note_of[position] {
                    Some(note) => Some(note),
                    None => notes
                        .iter()
                        .enumerate()
                        .filter(|(_, note)| note.end <= citation.span.start)
                        .max_by_key(|(_, note)| note.end)
                        .map(|(note, _)| note),
                };
                let (sequence, number) = note_rank.map_or((0, -1), |note|
                    (notes[note].sequence, notes[note].number as i64));
                (sequence, number, citation.span.start)
            };
            order.sort_by_key(|&position| rank(position));
        }
        order
    }

    fn run(mut self) -> Vec<Resolution> {
        let mut history = History::default();
        let mut output = Vec::new();
        let mut previous = None;
        let mut seen = vec![false; self.citations.len()];
        let note_count = self.notes.map_or(0, |notes| notes.len());
        let has_citations = (0..note_count).map(|note|
            self.note_of.iter().any(|&owner| owner == Some(note))).collect::<Vec<_>>();
        let mut handled_notes = vec![false; note_count];
        for position in self.order() {
            if seen[position] { continue; }
            if let Some(note) = self.note_of[position].filter(|_| !self.source_parts.is_empty()) {
                let missing = (0..note_count).filter(|&other|
                    !handled_notes[other] && !has_citations[other]
                        && if self.reading_order.is_none() {
                            let notes = self.notes.unwrap();
                            (notes[other].sequence, notes[other].number) < (notes[note].sequence, notes[note].number)
                        } else { other < note }).collect::<Vec<_>>();
                for other in missing {
                    self.process_note(other, &mut history, &mut previous, &mut seen, &mut output);
                    handled_notes[other] = true;
                }
                if !handled_notes[note] {
                    self.process_note(note, &mut history, &mut previous, &mut seen, &mut output);
                    handled_notes[note] = true;
                }
            } else {
                self.process_citation(position, &mut history, &mut previous, &mut output, None,
                    self.source_parts.is_empty() || self.note_of[position].is_none());
                seen[position] = true;
            }
        }
        if !self.source_parts.is_empty() {
            let remaining = (0..note_count).filter(|&note| !handled_notes[note] && !has_citations[note])
                .collect::<Vec<_>>();
            for note in remaining {
                self.process_note(note, &mut history, &mut previous, &mut seen, &mut output);
            }
        }
        output.sort_by_key(|resolution| resolution.index);
        output
    }

    fn process_note(&mut self, note: usize, history: &mut History, previous: &mut Option<usize>,
        seen: &mut [bool], output: &mut Vec<Resolution>) {
        let range = self.notes.expect("owned note")[note].clone();
        let mut parts = self.source_parts.iter().enumerate().filter(|(_, part)|
            range.start <= part.start && part.end <= range.end).map(|(index, part)|
            (part.start, 0u8, index)).collect::<Vec<_>>();
        let part_indices = parts.iter().map(|(_, _, index)| *index).collect::<Vec<_>>();
        parts.extend(self.citations.iter().enumerate().filter(|(position, citation)|
            self.note_of[*position] == Some(note)
                && !part_indices.iter().any(|&part| {
                    let part = &self.source_parts[part];
                    part.start <= citation.span.start && citation.span.end <= part.end
                })).map(|(position, citation)| (citation.span.start, 1u8, position)));
        parts.sort_unstable();
        let prior_records = history.records.len();
        let prior_inferred = history.inferred.len();
        let mut sibling: Option<Option<Target>> = None;
        for (_, event, index) in parts {
            if event == 0 {
                self.process_part(note, index, prior_records, prior_inferred, history,
                    previous, seen, output, &mut sibling);
            } else if !seen[index] {
                self.process_citation(index, history, previous, output, None, true);
                seen[index] = true;
            }
        }
        let note_targets = history.records[prior_records..].iter()
            .map(|record| record.target.as_deref()).collect::<std::collections::HashSet<_>>();
        history.last_part_target = Some(if note_targets.len() == 1 {
            note_targets.into_iter().next().flatten().and_then(Target::from_registry)
        } else { None });
    }

    fn process_part(&mut self, note: usize, part_index: usize, prior_records: usize,
        prior_inferred: usize, history: &mut History, previous: &mut Option<usize>,
        seen: &mut [bool], output: &mut Vec<Resolution>, sibling: &mut Option<Option<Target>>) {
        let part = &self.source_parts[part_index];
        let range = &self.notes.expect("owned note")[note];
        let fields = crate::source::extract_fields(part, part.extended_us);
        let mut positions = self.citations.iter().enumerate().filter(|(position, citation)|
            !seen[*position] && self.note_of[*position] == Some(note)
                && part.start <= citation.span.start && citation.span.end <= part.end)
            .map(|(position, citation)| (citation.span.start, position)).collect::<Vec<_>>();
        positions.sort_unstable();
        let reference = short_forms::reference_info(&part.text);
        let full = positions.iter().map(|&(_, position)| &self.citations[position])
            .filter(|citation| citation.form == Form::Full && !cites_record(citation)).collect::<Vec<_>>();
        let fallback = crate::url::source_fallback(&full, &fields, part)
            .filter(|(index, _)| full.iter().any(|citation| citation.index == *index && !citation.is_ambiguous()));
        // ALR _deterministic_footnote_parts clears explicit URLs on reference
        // parts before calling _resolve_footnote_part_link_unlocked.
        let explicit = reference.kind.is_empty() && !fields.link_candidate.is_empty()
            && fields.link_candidate.to_lowercase() != "other"
            && !(fallback.is_some() && crate::url::source_prefers_fallback(fields.kind, &part.text));
        let mut own_url = part.resolved_url.clone().or_else(|| if explicit { Some(fields.link_candidate.clone()) }
            else { fallback.as_ref().map(|(_, url)| url.clone()) });
        if let Some(url) = &mut own_url {
            if part.resolved_url.is_none() {
                *url = crate::url::source_first_pinpoint(url, &fields.pinpoint_fragments);
            }
        }
        let own_target = if reference.kind.is_empty() {
            if full.is_empty() {
                // An opaque source can be associated without inventing a
                // citation identity. A record citation remains a barrier.
                (positions.is_empty()
                    && !fields.reasons.contains(&"embedded_second_source"))
                    .then_some(Target::Source(part_index))
            } else {
                unique_resource(positions.iter().filter(|&&(_, position)| self.citations[position].form == Form::Full)
                    .map(|&(_, position)| (!self.citations[position].is_ambiguous() && !cites_record(&self.citations[position]))
                        .then_some(self.full_authority[position])))
                    .map(Target::Authority)
            }
        } else { None };
        if let Some(target) = &own_target {
            self.source_origins.entry(target.clone()).or_insert(part_index);
            if let Some(url) = own_url.filter(|url| !url.is_empty() && !url.eq_ignore_ascii_case("other")) {
                self.source_urls.entry(target.clone()).or_insert(url);
            }
        };
        // ALR _resolve_footnote_reference_links resolves each whole part once.
        let part_result = match reference.kind {
            "supra" => Some(self.resolve_supra_text(&part.text,
                &history.records[..prior_records], &history.inferred[..prior_inferred],
                Some(range.sequence))),
            "ibid" => {
                let prior = sibling.clone().or_else(|| history.last_part_target.clone());
                Some(if let Some(target) = prior {
                    let reason = if target.is_none() { "ibid_after_unresolved" }
                        else if sibling.is_some() { "ibid_previous" } else { "ibid_previous_note" };
                    (target, reason)
                } else if self.reading_order.is_some()
                    && previous.is_some_and(|position| self.note_of[position].is_none()) {
                    positions.first().map_or((None, "ibid_no_previous"), |&(_, position)|
                        self.previous_citation(position, *previous))
                } else { (None, "ibid_no_previous") })
            }
            _ => None,
        };
        for &(_, position) in &positions {
            let override_result = matches!(self.citations[position].form, Form::Supra | Form::Ibid)
                .then(|| part_result.clone()).flatten()
                .map(|(target, reason)| (target, reason, Some(part_index)));
            self.process_citation(position, history, previous, output, override_result, false);
            seen[position] = true;
        }
        let part_target = part_result.map_or_else(|| own_target.or_else(||
            unique_resource(positions.iter().map(|&(_, position)| self.resolved[position].clone()))),
            |(target, _)| target);
        // ALR registers one emitted source part, after resolving every reference
        // in its note. Discovered cores remain separate Citation identities, but
        // must not multiply the source-part registry or make a numbered supra
        // look ambiguous by counting one part more than once.
        let short_form = (!fields.short_form.is_empty()).then_some(fields.short_form);
        let kind = full.first().map_or(fields.kind, |citation| inference_kind(citation));
        history.push(self.source_record(Some(&range), part.text.clone(), part_target.clone(), short_form),
            kind, self.supra_linking_mode);
        *sibling = Some(part_target.clone());
        history.last_part_target = Some(part_target);
    }

    fn resolve_supra_text(&self, text: &str, registry: &[ReferenceSource],
        inferred: &[ReferenceSource], sequence: Option<u32>) -> (Option<Target>, &'static str) {
        // ALR's original note identifiers are globally unique. A host may have
        // restarted numbering, so a numbered reference can only consult the
        // matching numbering sequence, including aggressive fallbacks.
        let reference = short_forms::reference_info(text);
        let numbered = !reference.notes.is_empty();
        if let (Some(number), Some(notes)) = (reference.notes.first().and_then(|number| decimal(number)), self.notes) {
            let number = number.to_string();
            let count = notes.iter().filter(|note| note.number.to_string() == number && sequence.is_none_or(|sequence| note.sequence == sequence)).count();
            if count != 1 { return (None, if count == 0 { "note_out_of_range" } else { "ambiguous_note" }); }
        }
        let scoped_registry = registry.iter().filter(|entry|
            !numbered || sequence.is_none_or(|sequence| entry.sequence.is_none_or(|owner| owner == sequence)))
            .cloned().collect::<Vec<_>>();
        let scoped_inferred = inferred.iter().filter(|entry|
            !numbered || sequence.is_none_or(|sequence| entry.sequence.is_none_or(|owner| owner == sequence)))
            .cloned().collect::<Vec<_>>();
        let (strict, reason) = short_forms::resolve_registry_scoped(text, &scoped_registry,
            self.supra_hint_mode == SupraMode::Aggressive, sequence);
        if let Some(target) = Target::from_registry(&strict) { return (Some(target), reason); }
        if self.supra_linking_mode == SupraMode::Aggressive {
            let (fallback, method) = short_forms::resolve_after_strict_abstention(text,
                &scoped_registry, &scoped_inferred);
            if let Some(target) = Target::from_registry(&fallback) {
                return (Some(target), if method == "bare_note_unique_citation" {
                    "bare_note_unique_citation"
                } else { "inferred_short_form" });
            }
        }
        (None, reason)
    }

    fn process_citation(&mut self, position: usize, history: &mut History,
        previous: &mut Option<usize>, output: &mut Vec<Resolution>,
        source_result: Option<(Option<Target>, &'static str, Option<usize>)>,
        record_full: bool) {
        let citation = &self.citations[position];
        if citation.form == Form::Full {
            self.resolved[position] = (!citation.is_ambiguous() && !cites_record(citation))
                .then_some(Target::Authority(self.full_authority[position]));
            self.done[position] = true;
            if record_full {
                let range = self.note_of[position].and_then(|index| self.notes.map(|notes| &notes[index]));
                history.push(self.source_record(range, citation.full_span.text.clone(),
                    self.resolved[position].clone(),
                    citation.explicit_short_name.clone().or_else(|| citation.short_name.clone())
                        .or_else(|| citation.style.as_ref().map(|style| style.text.clone()))),
                    inference_kind(citation), self.supra_linking_mode);
                if self.source_parts.is_empty() {
                    history.last_part_target = Some(self.resolved[position].clone());
                }
            }
            *previous = Some(position);
            return;
        }
        let (mut target, mut reason, _) = match source_result {
            Some((target, reason, part)) => (target, reason, part),
            None => match citation.form {
                Form::Ibid if self.source_parts.is_empty() => {
                    let (authority, reason) = self.ibid_from_citations(position, *previous);
                    (authority, reason, None)
                }
                Form::Supra if self.source_parts.is_empty() => {
                    let (authority, reason) = self.supra_from_citations(position, history);
                    (authority.map(Target::Authority), reason, None)
                }
                Form::Ibid => {
                    let (authority, reason) = self.previous_citation(position, *previous);
                    (authority, reason, None)
                }
                Form::Supra => {
                    let (authority, reason) = self.resolve_supra_text(
                        if citation.full_span.text.is_empty() { &citation.span.text }
                        else { &citation.full_span.text },
                        &history.records, &history.inferred,
                        self.note_of[position].map(|note| self.notes.unwrap()[note].sequence));
                    (authority, reason, None)
                }
                Form::Reference => {
                    let (authority, reason) = self.by_name(position, reference_name(citation),
                        &history.records);
                    (authority.map(Target::Authority), reason, None)
                }
                Form::Short => {
                    let (authority, reason) = self.short(position, &history.records);
                    (authority.map(Target::Authority), reason, None)
                }
                Form::Unknown => (None, "unknown_form", None),
                Form::Full => unreachable!(),
            },
        };
        if target.as_ref().is_some_and(|target| citation.is_ambiguous() || match target {
            Target::Authority(index) => self.citations.iter()
                .any(|candidate| candidate.index == *index && candidate.is_ambiguous()),
            Target::Source(_) => false,
        }) {
            target = None;
            reason = "ambiguous_authority";
        }
        self.resolved[position] = target.clone();
        self.done[position] = true;
        *previous = Some(position);
        let source_part = target.as_ref().and_then(|target| self.source_origins.get(target).copied());
        let url = target.as_ref().and_then(|target| self.source_urls.get(target))
            .map(|url| short_forms::reanchor_reference(url, &citation.full_span.text));
        let antecedent = match target {
            Some(Target::Authority(index)) => Some(index),
            _ => None,
        };
        output.push(Resolution { index: citation.index, antecedent, url, source_part, reason });
        if record_full {
            let range = self.note_of[position].and_then(|index| self.notes.map(|notes| &notes[index]));
            history.push(self.source_record(range, citation.full_span.text.clone(), self.resolved[position].clone(),
                citation.short_name.clone()), inference_kind(citation), self.supra_linking_mode);
        }
        if record_full && self.source_parts.is_empty() {
            history.last_part_target = Some(self.resolved[position].clone());
        }
    }

    fn previous_citation(&self, position: usize, previous: Option<usize>) -> (Option<Target>, &'static str) {
        let Some(previous) = previous else { return (None, "ibid_no_previous"); };
        let Some(link) = self.resolved[previous].clone() else { return (None, "ibid_after_unresolved"); };
        let target = match &link {
            Target::Authority(index) => self.citations.iter().find(|citation| citation.index == *index),
            Target::Source(_) => None,
        };
        let citation = &self.citations[position];
        let pin = citation.fields.source_case_name.as_ref()
            .filter(|source| source.full_span_end.is_some()).map(|source| source.pin_cite.as_ref())
            .unwrap_or_else(|| citation.fields.pin_cite.as_ref()
                .or_else(|| citation.pinpoints.first().map(|pin| &pin.span)));
        if target.is_some_and(|target| is_us(target) && invalid_id_pin(
            target.authority == crate::Authority::Case && target.format == Some(crate::Format::Reporter),
            target.fields.page.as_deref(), pin.map(|pin| pin.text.as_str()),
        )) {
            return (None, "ibid_invalid_pinpoint");
        }
        (Some(link), "ibid_previous")
    }

    /// The same citation history supplies separate resolve calls when the host
    /// has note ranges but did not supply splitter parts. At a note boundary,
    /// all citations in the preceding note must agree before ibid can link.
    fn ibid_from_citations(&self, position: usize, previous: Option<usize>) -> (Option<Target>, &'static str) {
        let Some(note) = self.note_of[position] else { return self.previous_citation(position, previous); };
        let from_body = self.reading_order.is_some()
            && previous.is_some_and(|previous| self.note_of[previous].is_none());
        if from_body || previous.is_some_and(|previous| self.note_of[previous] == Some(note)) {
            return self.previous_citation(position, previous);
        }
        let notes = self.notes.unwrap();
        let Some(prior) = notes.iter().enumerate().filter(|(_, other)|
            other.sequence == notes[note].sequence && other.number < notes[note].number)
            .max_by_key(|(_, other)| other.number).map(|(index, _)| index)
        else { return (None, "ibid_no_previous"); };
        let positions = (0..self.citations.len()).filter(|&other|
            self.note_of[other] == Some(prior) && self.done[other]).collect::<Vec<_>>();
        if positions.is_empty() { return (None, "ibid_no_previous"); }
        if positions.iter().any(|&other| self.resolved[other].is_none()) {
            return (None, "ibid_after_unresolved");
        }
        let authority = self.resolved[positions[0]].as_ref().unwrap();
        if positions.iter().any(|&other| self.resolved[other].as_ref() != Some(authority)) {
            return (None, "ibid_after_multiple");
        }
        let (linked, reason) = self.previous_citation(position, Some(positions[0]));
        let reason = if linked.is_some() { "ibid_previous_note" } else { reason };
        (linked, reason)
    }

    fn supra_from_citations(&self, position: usize, history: &History) -> (Option<usize>, &'static str) {
        let citation = &self.citations[position];
        if self.notes.is_none() {
            return self.by_name(position, reference_name(citation), &history.records);
        }
        let (Some(number), Some(notes), Some(own)) =
            (citation.fields.note, self.notes, self.note_of[position]) else {
            let (target, reason) = self.resolve_supra_text(if citation.full_span.text.is_empty() {
                &citation.span.text
            } else { &citation.full_span.text }, &history.records, &history.inferred,
                self.note_of[position].map(|note| self.notes.unwrap()[note].sequence));
            return (target.and_then(|target| match target {
                Target::Authority(index) => Some(index), Target::Source(_) => None,
            }), reason);
        };
        let matches = notes.iter().enumerate().filter(|(_, note)|
            note.sequence == notes[own].sequence && note.number == number)
            .map(|(index, _)| index).collect::<Vec<_>>();
        let [target] = matches.as_slice() else {
            return (None, if matches.is_empty() { "note_out_of_range" } else { "ambiguous_note" });
        };
        let candidates = (0..self.citations.len()).filter(|&other|
            self.done[other] && self.note_of[other] == Some(*target))
            .filter_map(|other| match self.resolved[other] {
                Some(Target::Authority(authority)) => Some((authority, other)),
                _ => None,
            }).collect::<Vec<_>>();
        if candidates.is_empty() { return (None, "note_without_authority"); }
        let Some(hint) = reference_name(citation) else {
            let authority = candidates[0].0;
            let unresolved = (0..self.citations.len()).any(|other| self.done[other]
                && self.note_of[other] == Some(*target) && self.resolved[other].is_none());
            return if !unresolved && candidates.iter().all(|&(candidate, _)| candidate == authority) {
                (Some(authority), "note_only")
            } else { (None, "ambiguous_note") };
        };
        let target_number = number.to_string();
        let in_note = history.records.iter().filter(|record|
            record.note.as_str() == Some(target_number.as_str())
                && record.sequence == Some(notes[own].sequence)).cloned().collect::<Vec<_>>();
        match self.name_in_registry(&hint, &in_note, &candidates, true) {
            (Some(authority), _) => (Some(authority), "note_and_name"),
            (None, "no_match") => (None, "note_name_conflict"),
            result => result,
        }
    }

    fn by_name(&self, position: usize, hint: Option<String>, registry: &[ReferenceSource]) -> (Option<usize>, &'static str) {
        if self.citations[position].form == Form::Reference && self.citations[position].fields.source_case_name.is_some() {
            // Feed the same exact metadata-value matching used by the facade.
            // Resource tokens include ambiguous candidates; the shared resolver
            // keeps them in the pool and vetoes an uncertain identity.
            let parties = self.citations[position].parties.as_ref();
            let values = [parties.and_then(|p| p.plaintiff.as_deref()), parties.and_then(|p| p.defendant.as_deref())]
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
        let candidates = self.earlier_full(position).collect::<Vec<_>>();
        self.name_in_registry(&hint, registry, &candidates, true)
    }

    /// `(authority, position)` of every full citation read before `position`
    /// that a reference can name: a record document's citation is not one.
    fn earlier_full(&self, position: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        (0..self.citations.len())
            .filter(move |&other| other != position && self.done[other] && self.citations[other].form == Form::Full
                && !cites_record(&self.citations[other]))
            .map(|other| (self.full_authority[other], other))
    }

    /// ALR's registry owns name tiers; LSP's established word comparison only
    /// handles actual case-name references after a genuine ALR no-match.
    fn name_in_registry(&self, hint: &str, registry: &[ReferenceSource],
        candidates: &[(usize, usize)], lsp_reference: bool) -> (Option<usize>, &'static str) {
        let allowed = candidates.iter().map(|(authority, _)| *authority).collect::<std::collections::HashSet<_>>();
        let pool = registry.iter().filter(|entry| entry.target.as_deref()
            .and_then(|target| self.authority_for_target(target)).is_none_or(|authority| allowed.contains(&authority)))
            .cloned().collect::<Vec<_>>();
        let (resource, reason) = short_forms::resolve_registry_hint("", hint, &pool);
        if let Some(authority) = self.authority_for_target(&resource) { return (Some(authority), "name_only"); }
        if reason != "abstain_no_match" || !lsp_reference {
            return (None, if reason.starts_with("abstain_ambiguous") { "ambiguous_name" } else { "no_match" });
        }
        let hint_words = words(hint);
        if !hint_words.is_empty() && !is_crown(&hint_words) {
            let mut exact = Vec::new();
            let mut loose = Vec::new();
            for &(authority, other) in candidates {
                let names = candidate_names(&self.citations[other]);
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
        (None, "no_match")
    }

    fn authority_for_target(&self, id: &str) -> Option<usize> {
        match Target::from_registry(id) {
            Some(Target::Authority(index)) => Some(index),
            _ => None,
        }
    }

    fn source_record(&self, note: Option<&NoteRange>, verbatim: String, target: Option<Target>,
        short_form: Option<String>) -> ReferenceSource {
        let names = match &target {
            Some(Target::Authority(index)) => self.citations.iter().find(|citation| citation.index == *index)
                .map(candidate_names).unwrap_or_default(),
            Some(Target::Source(index)) => vec![self.source_parts[*index].text.clone()],
            None => Vec::new(),
        };
        ReferenceSource {
            note: note.map(|note| note.number.to_string()).unwrap_or_default().into(),
            sequence: note.map(|note| note.sequence), verbatim: Some(verbatim), names,
            target: target.map(|target| target.registry_id()), short_form,
            ..ReferenceSource::default()
        }
    }

    fn short(&self, position: usize, registry: &[ReferenceSource]) -> (Option<usize>, &'static str) {
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
                let parties = candidate.parties.as_ref();
                let (plaintiff, defendant) = (parties.and_then(|p| p.plaintiff.as_deref()), parties.and_then(|p| p.defendant.as_deref()));
                antecedent_matches(&hint, plaintiff, defendant)
            }).map(|&(authority, _)| Some(authority)));
            return (authority, if authority.is_some() { "short_name" } else { "short_ambiguous" });
        }
        let (authority, _) = self.name_in_registry(&hint, registry, &candidates, false);
        (authority, if authority.is_some() { "short_name" } else { "short_ambiguous" })
    }
}

/// A citation of a document of the proceeding's record, not of its decision.
fn cites_record(citation: &Citation) -> bool {
    citation.parentheticals.iter().any(|part| part.kind == crate::ParentheticalKind::Record)
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
        names.extend(source.antecedent_guess.clone());
    }
    if let Some(parties) = &citation.parties {
        names.extend(parties.plaintiff.clone());
        names.extend(parties.defendant.clone());
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
        .or_else(|| citation.parties.as_ref().and_then(|parties| parties.plaintiff.clone()));
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
    authorities_with_resolutions(citations, links, &[])
}

pub(crate) fn authorities_with_resolutions(citations: &[Citation], links: &[(usize, usize)], resolutions: &[Resolution]) -> Vec<Vec<usize>> {
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
    let source_parts = resolutions.iter().filter_map(|resolution|
        resolution.source_part.map(|source| (resolution.index, source)))
        .collect::<HashMap<_, _>>();
    let member = |citation: &Citation| match citation.form {
        Form::Full => true,
        Form::Unknown => false,
        _ => citation.antecedent.is_some() || source_parts.contains_key(&citation.index),
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
    let mut by_source: HashMap<usize, usize> = HashMap::new();
    for (index, source) in &source_parts {
        if let Some(&position) = position_of.get(index).filter(|&&position| !citations[position].is_ambiguous()) {
            match by_source.get(source) {
                Some(&other) => union(&mut parent, position, other),
                None => { by_source.insert(*source, position); }
            }
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
