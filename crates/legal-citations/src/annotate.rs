//! Insert markup around citations while preserving the source's own markup
//! (eyecite `annotate_citations`).
//!
//! [`annotate`] inserts `before`/`after` strings at byte ranges of the text
//! the citations were extracted from. When that text was produced by
//! [`crate::clean::clean`] from HTML or other markup, [`annotate_source`]
//! maps every range back through the exact offset map in
//! [`Cleaned`](crate::clean::Cleaned) (no diffing) and inserts into the
//! source. A mapped range that crosses tag boundaries unevenly
//! (`R v Jordan</i>, 2016 SCC 27`) is handled per [`Unbalanced`]:
//!
//! * [`Unbalanced::Unchecked`]: insert anyway (may produce invalid markup);
//! * [`Unbalanced::Skip`]: leave that annotation out;
//! * [`Unbalanced::Wrap`]: close the annotation before each tag inside the
//!   range and reopen it after, so every run of text between tags is wrapped
//!   on its own (`<a>R v Jordan</a></i><a>, 2016 SCC 27</a>`).
//!
//! Overlapping annotations are not nested: the earliest-starting (then
//! longest) one wins and any annotation overlapping it is dropped. Ranges not
//! on character boundaries are dropped.

use crate::clean::{Cleaned, Tag, TagKind};
use crate::model::Citation;

/// Markup to insert around one range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Annotation {
    pub start: usize,
    pub end: usize,
    pub before: String,
    pub after: String,
}

impl Annotation {
    pub fn new(start: usize, end: usize, before: impl Into<String>, after: impl Into<String>) -> Self {
        Self {
            start,
            end,
            before: before.into(),
            after: after.into(),
        }
    }
}

/// How to treat an annotation whose source range has unbalanced tags.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Unbalanced {
    #[default]
    Unchecked,
    Skip,
    Wrap,
}

/// Which extent of a citation to annotate.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Extent {
    /// The core (`2016 SCC 27`), as eyecite annotates.
    #[default]
    Core,
    /// Style, core, pinpoints and parentheticals.
    Full,
}

/// One annotation per citation for which `markup` returns `(before, after)`.
pub fn citation_annotations(
    citations: &[Citation],
    extent: Extent,
    mut markup: impl FnMut(&Citation) -> Option<(String, String)>,
) -> Vec<Annotation> {
    citations
        .iter()
        .filter_map(|citation| {
            let span = match extent {
                Extent::Core => &citation.span,
                Extent::Full => &citation.full_span,
            };
            markup(citation).map(|(before, after)| Annotation::new(span.start, span.end, before, after))
        })
        .collect()
}

/// Sorted, non-overlapping, in-bounds annotations.
fn accepted<'a>(text: &str, annotations: &'a [Annotation]) -> Vec<&'a Annotation> {
    let mut sorted = annotations
        .iter()
        .filter(|annotation| {
            annotation.start <= annotation.end
                && annotation.end <= text.len()
                && text.is_char_boundary(annotation.start)
                && text.is_char_boundary(annotation.end)
        })
        .collect::<Vec<_>>();
    sorted.sort_by_key(|annotation| (annotation.start, std::cmp::Reverse(annotation.end)));
    let mut output: Vec<&Annotation> = Vec::with_capacity(sorted.len());
    for annotation in sorted {
        if output.last().is_some_and(|previous| annotation.start < previous.end) {
            continue;
        }
        output.push(annotation);
    }
    output
}

fn insert(text: &str, mut insertions: Vec<(usize, String)>) -> String {
    // Stable: insertions at the same offset keep their generation order.
    insertions.sort_by_key(|(offset, _)| *offset);
    let extra = insertions.iter().map(|(_, value)| value.len()).sum::<usize>();
    let mut output = String::with_capacity(text.len() + extra);
    let mut cursor = 0;
    for (offset, value) in insertions {
        output.push_str(&text[cursor..offset]);
        output.push_str(&value);
        cursor = offset;
    }
    output.push_str(&text[cursor..]);
    output
}

/// Insert markup at byte ranges of `text`.
pub fn annotate(text: &str, annotations: &[Annotation]) -> String {
    let mut insertions = Vec::new();
    for annotation in accepted(text, annotations) {
        insertions.push((annotation.start, annotation.before.clone()));
        insertions.push((annotation.end, annotation.after.clone()));
    }
    insert(text, insertions)
}

/// Whether the tags inside a source range open and close evenly.
fn balanced(tags: &[&Tag]) -> bool {
    let mut stack: Vec<&str> = Vec::new();
    for tag in tags {
        match tag.kind {
            TagKind::Open => stack.push(&tag.name),
            TagKind::Close => {
                if stack.pop() != Some(tag.name.as_str()) {
                    return false;
                }
            }
            TagKind::SelfClosing | TagKind::Other => {}
        }
    }
    stack.is_empty()
}

/// Insert markup into `source` at ranges of `cleaned.text` (the text the
/// citations were extracted from), mapping offsets with the clean map.
pub fn annotate_source(source: &str, cleaned: &Cleaned, annotations: &[Annotation], unbalanced: Unbalanced) -> String {
    let mut insertions = Vec::new();
    let mut previous_end = 0;
    for annotation in accepted(&cleaned.text, annotations) {
        let Some(range) = cleaned.source_range(annotation.start..annotation.end) else {
            continue;
        };
        if range.start < previous_end
            || range.end > source.len()
            || !source.is_char_boundary(range.start)
            || !source.is_char_boundary(range.end)
        {
            continue;
        }
        // A boundary inside a tag (a block break mapped to its tag) is never
        // a place to insert markup.
        let splits_tag = cleaned.tags.iter().any(|tag| {
            (tag.start < range.start && range.start < tag.end) || (tag.start < range.end && range.end < tag.end)
        });
        let inside = cleaned
            .tags
            .iter()
            .filter(|tag| range.start <= tag.start && tag.end <= range.end)
            .collect::<Vec<_>>();
        let even = !splits_tag && balanced(&inside);
        match (even, unbalanced) {
            (true, _) | (false, Unbalanced::Unchecked) => {
                insertions.push((range.start, annotation.before.clone()));
                insertions.push((range.end, annotation.after.clone()));
            }
            (false, Unbalanced::Skip) => continue,
            (false, Unbalanced::Wrap) => {
                if splits_tag {
                    continue;
                }
                let mut cursor = range.start;
                for tag in inside.iter().copied().chain(std::iter::once(&Tag {
                    start: range.end,
                    end: range.end,
                    name: String::new(),
                    kind: TagKind::Other,
                })) {
                    if tag.start > cursor {
                        insertions.push((cursor, annotation.before.clone()));
                        insertions.push((tag.start, annotation.after.clone()));
                    }
                    cursor = cursor.max(tag.end);
                }
            }
        }
        previous_end = range.end;
    }
    insert(source, insertions)
}
