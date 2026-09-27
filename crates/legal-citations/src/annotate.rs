//! Source annotation with two established output policies: typed Rust callers
//! retain structure-aware tag balancing, while [`prepare`] and [`render`]
//! implement the pinned eyecite ordering for the JSON facade. Offsets are
//! bytes here; bindings translate at the boundary.

use crate::clean::{Cleaned, Tag, TagKind};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
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

/// A whole annotation after offset mapping and markup treatment. `index`
/// identifies the input annotation so hosts can invoke callbacks with objects.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct PreparedAnnotation {
    pub index: usize,
    pub start: usize,
    pub end: usize,
    pub before: String,
    pub after: String,
    pub text: String,
}

fn balanced(text: &str) -> bool {
    !text.contains(['<', '>']) || roxmltree::Document::parse(&format!("<div>{text}</div>")).is_ok()
}

/// Pinned eyecite.utils.maybe_balance_style_tags, with character tolerances.
fn balance_style_tags(source: &str, mut start: usize, mut end: usize) -> (usize, usize) {
    let span = source.get(start..end).unwrap_or_default();
    for tag in ["i", "em", "b"] {
        let opening = format!("<{tag}>");
        let closing = format!("</{tag}>");
        let has_opening = span.contains(&opening);
        let has_closing = span.contains(&closing);
        if has_opening && !has_closing {
            let extended_end = source[end..].char_indices().nth(closing.len() + 10)
                .map_or(source.len(), |(offset, _)| end + offset);
            if let Some(offset) = source[start..extended_end].find(&closing) {
                end = start + offset + closing.len();
            }
        }
        if !has_opening && has_closing {
            let extended_start = source[..start].char_indices().rev().nth(opening.len() + 9)
                .map_or(0, |(offset, _)| offset);
            if let Some(offset) = source[extended_start..end].rfind(&opening) {
                start = extended_start + offset;
            }
        }
    }
    (start, end)
}

/// Prepare whole spans in the pinned annotate_citations order. Partial
/// overlaps are clamped; style-tag recovery is performed after that clamp.
pub fn source_annotations(source: &str, cleaned: &Cleaned, annotations: &[Annotation], unbalanced: Unbalanced) -> Vec<PreparedAnnotation> {
    prepare(source, annotations, unbalanced, |_, annotation| cleaned.source_range(annotation.start..annotation.end))
}

pub fn prepare(
    source: &str, annotations: &[Annotation], unbalanced: Unbalanced,
    map: impl Fn(usize, &Annotation) -> Option<std::ops::Range<usize>>,
) -> Vec<PreparedAnnotation> {
    static TAG: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(<[^>]+>)").unwrap());
    let mut sorted = annotations.iter().enumerate().collect::<Vec<_>>();
    sorted.sort_by_key(|(_, annotation)| (annotation.start, annotation.end));
    let mut output = Vec::with_capacity(sorted.len());
    let mut last_end = 0;
    for (index, annotation) in sorted {
        let Some(range) = map(index, annotation) else { continue };
        let (mut start, mut end) = (range.start, range.end);
        if start > source.len() || end > source.len() || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            continue;
        }
        if start < last_end {
            start = last_end;
            if start >= end { continue; }
        }
        let mut text = source.get(start..end).unwrap_or_default().to_owned();
        if unbalanced != Unbalanced::Unchecked && !balanced(&text) {
            match unbalanced {
                Unbalanced::Wrap => {
                    text = TAG.replace_all(&text, |captures: &regex::Captures<'_>| {
                        format!("{}{}{}", annotation.after, &captures[0], annotation.before)
                    }).into_owned();
                }
                Unbalanced::Skip => {
                    (start, end) = balance_style_tags(source, start, end);
                    text = source.get(start..end).unwrap_or_default().to_owned();
                    if !balanced(&text) { continue; }
                }
                Unbalanced::Unchecked => unreachable!(),
            }
        }
        output.push(PreparedAnnotation { index, start, end, before: annotation.before.clone(), after: annotation.after.clone(), text });
        last_end = end;
    }
    output
}

pub fn render(source: &str, annotations: &[PreparedAnnotation]) -> String {
    let mut output = String::new();
    let mut last_end = 0;
    for annotation in annotations {
        // Style-tag recovery can extend a span back across last_end. Python
        // slicing then emits an empty gap, while retaining the recovered span.
        output.push_str(source.get(last_end..annotation.start).unwrap_or_default());
        output.push_str(&annotation.before);
        output.push_str(&annotation.text);
        output.push_str(&annotation.after);
        last_end = annotation.end;
    }
    output.push_str(&source[last_end..]);
    output
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
fn balanced_tags(tags: &[&Tag]) -> bool {
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
        let even = !splits_tag && balanced_tags(&inside);
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
