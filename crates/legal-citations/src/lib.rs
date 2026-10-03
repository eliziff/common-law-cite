//! Deterministic legal citation extraction, classification, resolution and
//! formatting for Canadian, Commonwealth and US legal text.
//!
//! The pipeline is:
//!
//! 1. [`find`]: discover citation cores and their styled extent from the shared
//!    grammar corpus ([`legal_grammar`]).
//! 2. [`classify`]: decide [`Authority`], [`Format`] and parse [`Fields`] from
//!    the grammar's named groups.
//! 3. [`metadata`]: pinpoints, parentheticals, subsequent history, parties and court.
//! 4. [`parallel`]: group citations to the same decision printed side by side.
//! 5. [`resolve`]: attach short forms, `supra`, `ibid`/`Id.` and case-name
//!    references to the full citation they refer to.
//! 6. [`key`]: compute the versioned identity key for every authority.
//!
//! [`extract`] runs all of them. [`annotate`], [`format`] and [`url`] operate on
//! its output, and [`registry`] exposes the court, reporter and series data.

pub mod annotate;
pub mod aliases;
pub mod api;
pub mod classify;
pub mod clean;
pub mod cues;
pub mod excerpt;
pub mod find;
pub mod format;
pub mod key;
pub mod metadata;
pub mod model;
pub mod parallel;
pub mod registry;
pub mod resolve;
pub mod short_forms;
pub mod source;
pub mod text;
pub mod url;
mod us;

pub use legal_grammar as grammar;
pub use model::*;

use serde::{Deserialize, Serialize};

/// The unit [`api`] reports offsets in. The Rust API always uses bytes.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub enum OffsetUnit {
    #[default]
    Byte,
    /// Unicode scalar values (Python string indices).
    Char,
    /// UTF-16 code units (JavaScript string indices).
    Utf16,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub enum SupraMode {
    #[default]
    Safe,
    Aggressive,
}

/// Options for [`extract`].
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct Options {
    /// Return the pinned Eyecite token view of the shared discovery result.
    /// Native/Commonwealth occurrence discovery remains available by default.
    pub source_only: bool,
    /// Omit citations with unresolved registry interpretations.
    pub remove_ambiguous: bool,
    /// Attach short forms, supra, ibid and references to their antecedents.
    pub resolve: bool,
    /// Group parallel citations.
    pub parallel: bool,
    /// Include the pinned source's specialized US citation forms in the shared
    /// extractor. Literal citation cues prefilter their compiled patterns.
    pub extended_us: bool,
    /// Treat each `\n\n`-separated block, or each entry of [`Options::notes`],
    /// as a footnote whose number `supra note N` can refer to.
    pub notes: Option<Vec<NoteRange>>,
    /// Ordered jurisdiction preferences for ambiguous abbreviations, e.g.
    /// `["ca", "uk"]`. An explicit court takes precedence. Empty is neutral.
    pub jurisdiction_priority: Vec<String>,
    /// ALR hint capture uses aggressive mode by default.
    pub supra_hint_mode: SupraMode,
    /// Inferred and bare-note linking runs only in aggressive mode.
    pub supra_linking_mode: SupraMode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            source_only: false,
            remove_ambiguous: false,
            resolve: true,
            parallel: true,
            extended_us: true,
            notes: None,
            jurisdiction_priority: Vec::new(),
            supra_hint_mode: SupraMode::Aggressive,
            supra_linking_mode: SupraMode::Safe,
        }
    }
}

/// A footnote's byte range and number, so `supra note 4` resolves to the
/// authority first cited in note 4.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct NoteRange {
    pub number: u32,
    pub start: usize,
    pub end: usize,
    /// Numbering sequence the note belongs to. Documents that restart note
    /// numbering (per chapter, per part) repeat numbers across sequences; a
    /// reference prefers a note in its own sequence.
    #[serde(default)]
    pub sequence: u32,
}

/// Find, classify and (optionally) resolve every citation in `text`.
pub fn extract(text: &str, options: &Options) -> Vec<Citation> {
    extract_markup(text, None, options)
}

pub fn extract_markup(text: &str, markup: Option<&str>, options: &Options) -> Vec<Citation> {
    extract_markup_with_parts(text, markup, options).0
}

pub(crate) fn extract_markup_with_parts(text: &str, markup: Option<&str>, options: &Options)
    -> (Vec<Citation>, Vec<source::SourcePart>, Vec<resolve::Resolution>) {
    let markup = markup.map(|source| clean::Markup::new(text, source));
    let mut citations = find::find_styled(text, options, markup.as_ref());
    if options.source_only { citations.retain(|citation| citation.fields.source_case_name.is_some()); }
    let mut citations = find::filter_citations(citations);
    metadata::attach(text, &mut citations);
    for citation in &mut citations {
        us::finish(citation);
    }
    if options.parallel {
        parallel::group(text, &mut citations);
    }
    for citation in &mut citations {
        citation.alias = aliases::resolve(citation).cloned();
        citation.key = citation.alias.as_ref().map(|target| target.key.clone())
            .or_else(|| key::key_in(citation, registry::registry()));
    }
    let notes = options.notes.as_deref().unwrap_or(&[]);
    let mut parts = source::split_notes(text, notes);
    let candidates = source::linked_ibid_candidates(text, notes, &parts);
    if !candidates.is_empty() {
        let preview = resolve::resolve_with_sources(&citations, options.notes.as_deref(), &[], None,
            &parts, options.supra_hint_mode, options.supra_linking_mode);
        let linked = candidates.into_iter().filter(|&index| {
            let previous = &notes[index - 1];
            citations.iter().filter(|citation| previous.start <= citation.span.start
                && citation.span.end <= previous.end)
                .max_by_key(|citation| citation.span.start)
                .and_then(|citation| preview.iter().find(|resolution| resolution.index == citation.index))
                .is_some_and(|resolution| resolution.antecedent.is_some() || resolution.source_part.is_some())
        }).collect::<Vec<_>>();
        source::merge_linked_ibids(text, notes, &mut parts, &linked);
    }
    let resolutions = if options.resolve {
        let resolutions = resolve::resolve_with_sources(&citations, options.notes.as_deref(), &[], None,
            &parts, options.supra_hint_mode, options.supra_linking_mode);
        for resolution in &resolutions {
            if let Some(citation) = citations.iter_mut().find(|citation| citation.index == resolution.index) {
                citation.antecedent = resolution.antecedent;
            }
        }
        resolutions
    } else { Vec::new() };
    if options.remove_ambiguous {
        citations.retain(|citation| !citation.is_ambiguous());
    }
    (citations, parts, resolutions)
}

/// Whether `text` contains a citation or a two-party case name.
pub fn has_citation(text: &str) -> bool {
    find::has_citation(text)
}
