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
pub mod text;
pub mod url;

pub use legal_grammar as grammar;
pub use model::*;

use serde::{Deserialize, Serialize};

/// The unit [`api`] reports offsets in. The Rust API always uses bytes.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OffsetUnit {
    #[default]
    Byte,
    /// Unicode scalar values (Python string indices).
    Char,
    /// UTF-16 code units (JavaScript string indices).
    Utf16,
}

/// Options for [`extract`].
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Options {
    /// Attach short forms, supra, ibid and references to their antecedents.
    pub resolve: bool,
    /// Group parallel citations.
    pub parallel: bool,
    /// Scan for US reporter and code citations beyond the common set. Costs a
    /// second pass over lines that carry a US citation cue.
    pub extended_us: bool,
    /// Treat each `\n\n`-separated block, or each entry of [`Options::notes`],
    /// as a footnote whose number `supra note N` can refer to.
    pub notes: Option<Vec<NoteRange>>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            resolve: true,
            parallel: true,
            extended_us: true,
            notes: None,
        }
    }
}

/// A footnote's byte range and number, so `supra note 4` resolves to the
/// authority first cited in note 4.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
    let mut citations = find::find(text, options);
    for citation in &mut citations {
        classify::classify(text, citation);
    }
    metadata::attach(text, &mut citations);
    if options.parallel {
        parallel::group(text, &mut citations);
    }
    if options.resolve {
        resolve::resolve(&mut citations, options.notes.as_deref());
    }
    for citation in &mut citations {
        citation.key = key::key(citation);
    }
    citations
}

/// Whether `text` contains a citation or a two-party case name.
pub fn has_citation(text: &str) -> bool {
    find::has_citation(text)
}
