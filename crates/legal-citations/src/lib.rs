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
pub mod authorities;
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
mod screen;
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
    /// Safe linking, except that a supra whose written short name names exactly one earlier
    /// citation by its short form (its bracketed short form, or every word of the name it was
    /// cited by) links to it even when its note number names another note.
    Named,
}

/// How far a note splitter's references are linked where the evidence is incomplete
/// ([`Options::split_tier`]): `safe` links what a reference's own note and words support, `moderate`
/// also reads a supra by its name where the note it numbers cites another work, and `aggressive` also
/// continues an ibid from the last source of a note that names several. Every tier also splits notes
/// into rows as ALR's rows keep them ([`source::split_notes`]): a semicolon inside brackets or quotes,
/// a sentence that cites nothing with the source after it, a supra with its name, a citation whole.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub enum SplitTier {
    Safe,
    Moderate,
    Aggressive,
}

impl SplitTier {
    /// The supra linking each tier resolves with.
    pub(crate) fn supra_mode(self) -> SupraMode {
        match self {
            Self::Safe => SupraMode::Safe,
            Self::Moderate => SupraMode::Named,
            Self::Aggressive => SupraMode::Aggressive,
        }
    }
}

/// A citation guide whose forms the finder recognizes beyond the forms every guide shares.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub enum CitationStyle { Mcgill, Coal, Bluebook, Aglc, Oscola, Nzlsg }

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
    /// The guides whose own forms are recognized; every guide's when absent.
    pub styles: Option<Vec<CitationStyle>>,
    /// Leave out what a document's own list or table of authorities holds: its entries list the
    /// authorities the document cites elsewhere, and its tab numbers cite nothing.
    pub skip_authority_lists: bool,
    /// Read a case's name with a pinpoint written before the case is first cited in full
    /// ("Oakes at 138" … "R v Oakes, [1986] 1 SCR 103") as a reference to it, which has no
    /// earlier citation. Eyecite reads references only after the full citation.
    pub early_references: bool,
    /// Resolve references as a note splitter's rows read them, at this tier ([`SplitTier`]) in place of
    /// `supra_linking_mode`: an ibid continues from the last part that is a
    /// source, past prose, a part inside another part's parentheses and a decision's subsequent
    /// history, or from the part its own name names in the note before; a supra whose numbered note
    /// cites another work, or none, is read by its name. Absent, references resolve as before.
    pub split_tier: Option<SplitTier>,
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
            styles: None,
            skip_authority_lists: false,
            early_references: false,
            split_tier: None,
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
    /// Where the note's marker stands in the text it annotates, when the caller knows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<usize>,
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
    if options.skip_authority_lists {
        let listed = find::authority_list_citations(text, &citations, options.notes.as_deref().unwrap_or(&[]));
        citations.retain(|citation| !listed.contains(&citation.index));
        for (index, citation) in citations.iter_mut().enumerate() { citation.index = index; }
    }
    metadata::attach(text, &mut citations);
    find::anchor_titles(text, &mut citations, options.notes.as_deref().unwrap_or(&[]));
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
    find::ocr_twins(&mut citations);
    find::titled_chapters(&mut citations);
    let notes = options.notes.as_deref().unwrap_or(&[]);
    let mut parts = source::split_notes(text, notes, options.split_tier);
    if options.split_tier.is_some() { source::merge_split_citations(text, notes, &mut parts, &citations); }
    let candidates = source::linked_ibid_candidates(text, notes, &parts);
    if !candidates.is_empty() {
        let preview = resolve::resolve_with_sources(&citations, options.notes.as_deref(), &[], None,
            &parts, options.supra_hint_mode, options.supra_linking_mode, options.split_tier);
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
            &parts, options.supra_hint_mode, options.supra_linking_mode, options.split_tier);
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
