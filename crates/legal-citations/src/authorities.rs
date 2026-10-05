//! An authority as a document cites it: the citation that represents it among those a resolution
//! groups together, the name the document gives it, how its citation is written, and the authority
//! it is subsequent history of.

use crate::model::{Citation, Form, Format, ParentheticalKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct AuthoritiesRequest {
    pub citations: Vec<Citation>,
    /// Each authority's citations, by index, as `resolve` groups them.
    pub authorities: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityReading {
    /// The full citation that represents the authority; none where it is cited only by reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub representative: Option<usize>,
    /// The name the document gives it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Its citation as the document writes it, with the court a core that does not name one is
    /// written with ("1961 CanLII 7 (SCC)").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation: Option<String>,
    /// The authority (by position in `authorities`) whose subsequent history it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_of: Option<usize>,
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A citation written with a recognizer's confusion ("ocr_twin") or without the chapter it takes from
/// another citation of the same Act ("titled_chapter") does not represent its authority.
fn borrowed(citation: &Citation) -> bool {
    citation.reasons.iter().any(|reason| reason == "ocr_twin" || reason == "titled_chapter")
}

/// The citation that represents an authority among its full citations: a decision's report or
/// neutral citation before the court file it was made under, one written as it is before one a
/// recognizer misread, and one that writes where the authority is found ("…, 1982, c 11") before
/// one that names it alone ("the Canadian Charter of Rights and Freedoms").
pub fn representative<'a>(full: &[&'a Citation]) -> Option<&'a Citation> {
    let docket = |citation: &Citation| citation.format == Some(Format::Docket);
    full.iter().find(|citation| citation.key.is_some() && citation.format.is_some() && !docket(citation) && !borrowed(citation))
        .or_else(|| full.iter().find(|citation| citation.key.is_some() && !docket(citation) && !borrowed(citation)))
        .or_else(|| full.iter().find(|citation| citation.key.is_some() && !docket(citation)))
        .or_else(|| full.iter().find(|citation| citation.key.is_some()))
        .or_else(|| full.first())
        .copied()
}

/// The name a document gives an authority: the representative citation's style of cause or title,
/// the title the sentence a note hangs from gives it, an instrument's own name, or else the style
/// another citation of the same authority writes (a citation in a heading without its Act's title, a
/// decision whose first citation a recognizer garbled before its style of cause).
fn name(representative: &Citation, full: &[&Citation]) -> Option<String> {
    let written = |citation: &Citation| citation.style.as_ref().map(|style| one_line(&style.text)).filter(|text| !text.is_empty());
    written(representative)
        .or_else(|| representative.fields.anchor_title.as_ref().map(|title| one_line(&title.text)))
        .or_else(|| representative.fields.instrument_title.clone())
        .or_else(|| full.iter().find_map(|citation| written(citation)))
        .filter(|text| !text.is_empty())
}

/// Each authority's representative citation, name and citation text, and the authority whose
/// subsequent history it is ("…, 2010 ABQB 242, aff'd 2010 ABCA 191"); a decision of the history
/// that the document gives no name of its own is known by its case's.
pub fn authorities(request: &AuthoritiesRequest) -> Vec<AuthorityReading> {
    let by_index = |index: usize| request.citations.iter().find(|citation| citation.index == index);
    let mut owner = std::collections::HashMap::new();
    let mut readings = request.authorities.iter().enumerate().map(|(position, group)| {
        let full = group.iter().filter_map(|&index| by_index(index)).filter(|citation| citation.form == Form::Full).collect::<Vec<_>>();
        for &index in group { owner.insert(index, position); }
        let Some(chosen) = representative(&full) else { return AuthorityReading::default() };
        // A core that does not name its court (a CanLII ID, a reporter) keeps the court written after it.
        let court = (chosen.format != Some(Format::Neutral)).then(|| chosen.parentheticals.iter()
            .find(|parenthetical| parenthetical.kind == ParentheticalKind::Court)).flatten();
        let citation = one_line(&[Some(chosen.span.text.as_str()), court.map(|court| court.span.text.as_str())]
            .into_iter().flatten().collect::<Vec<_>>().join(" "));
        AuthorityReading { representative: Some(chosen.index), name: name(chosen, &full), citation: Some(citation), history_of: None }
    }).collect::<Vec<_>>();
    for citation in &request.citations {
        let Some(&parent) = owner.get(&citation.index) else { continue };
        for target in citation.history.iter().filter_map(|history| history.target) {
            let Some(&child) = owner.get(&target) else { continue };
            if child == parent || readings[child].history_of.is_some() { continue; }
            readings[child].history_of = Some(parent);
            if readings[child].name.is_none() { readings[child].name = readings[parent].name.clone(); }
        }
    }
    readings
}

#[derive(Clone, Debug, Deserialize)]
pub struct CaptionRequest {
    pub text: String,
    /// The identity keys that name the decision.
    pub keys: Vec<String>,
}

/// The style of cause a decision prints before its own citation, as a CanLII PDF opens: "Citation:
/// Pell v Marlow Holdings, 2030 ABKB 12" gives "Pell v Marlow Holdings". A caption that is not a plain
/// "Name, citation" (a label such as "Neutral citation:", or a heading's bracket read into the name)
/// names nothing. The text is read as one run, as a PDF's page is: a label on the line above stays out of
/// the name.
pub fn caption_style_of_cause(request: &CaptionRequest) -> Option<String> {
    let balanced = |style: &str, open: char, close: char| style.matches(open).count() == style.matches(close).count();
    let text = one_line(&request.text);
    crate::extract(&text, &crate::Options::default()).into_iter().find_map(|citation| {
        let style = citation.style.as_ref()?;
        let key = crate::key::key_for_text(&citation.span.text).ok().or(citation.key.clone())?;
        (citation.authority == crate::model::Authority::Case && request.keys.contains(&key)).then(|| {
            let written = text.get(style.start..citation.span.start).unwrap_or(&style.text);
            written.trim_end_matches(|character: char| character.is_whitespace() || character == ',').to_owned()
        })
    }).filter(|style| !style.is_empty() && !style.ends_with(':') && balanced(style, '(', ')') && balanced(style, '[', ']'))
}

#[derive(Clone, Debug, Deserialize)]
pub struct PinpointsRequest {
    pub text: String,
    /// The selection, in UTF-16 units.
    pub start: usize,
    pub end: usize,
    /// No pinpoint where the selection has no locator words; else a bare value is a page.
    #[serde(default)]
    pub strict: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinpointValue {
    pub kind: crate::model::PinpointKind,
    /// The value, or the range "first-last".
    pub text: String,
    pub first: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
    /// In UTF-16 units.
    pub start: usize,
    pub end: usize,
}

/// The pinpoints written in a selection of a text, each with its kind and place, read as an ibid's are:
/// the locator words before them ("at para", "s", "pp"), up to three words back, give their kind.
pub fn pinpoints_at(request: &PinpointsRequest) -> Vec<PinpointValue> {
    let text = request.text.as_str();
    let byte = |utf16: usize| {
        let mut units = 0;
        for (at, character) in text.char_indices() {
            if units >= utf16 { return at; }
            units += character.len_utf16();
        }
        text.len()
    };
    let utf16 = |byte: usize| text[..byte].encode_utf16().count();
    let (mut start, mut end) = (byte(request.start), byte(request.end).min(text.len()));
    while start < end && text[start..].starts_with(char::is_whitespace) { start += text[start..].chars().next().unwrap().len_utf8(); }
    while end > start && text[..end].ends_with(char::is_whitespace) { end -= text[..end].chars().next_back().unwrap().len_utf8(); }
    if start >= end { return Vec::new(); }
    let mut from = start;
    for _ in 0..=3 {
        for lead in ["Ibid ", "Ibid, "] {
            let read = format!("{lead}{}", &text[from..end]);
            let found = crate::find::find_references(&read).into_iter().flat_map(|citation| citation.pinpoints)
                .filter(|pinpoint| from + pinpoint.span.end > start + lead.len()).collect::<Vec<_>>();
            if !found.is_empty() {
                return found.into_iter().map(|pinpoint| {
                    let (at, to) = (from + pinpoint.span.start - lead.len(), from + pinpoint.span.end - lead.len());
                    PinpointValue { kind: pinpoint.kind, text: pinpoint.last.as_ref()
                        .map_or_else(|| pinpoint.first.clone(), |last| format!("{}-{last}", pinpoint.first)),
                        first: pinpoint.first, last: pinpoint.last, start: utf16(at), end: utf16(to) }
                }).collect();
            }
        }
        if from == 0 { break; }
        // One more word back, to the space before it.
        from = text[..from.saturating_sub(1)].rfind(' ').map_or(0, |space| space + 1);
    }
    if request.strict { return Vec::new(); }
    let value = text[start..end].split(['-', '\u{2013}', '\u{2014}']).map(str::trim).collect::<Vec<_>>();
    vec![PinpointValue { kind: crate::model::PinpointKind::Page, text: value.join("-"), first: value[0].to_owned(),
        last: value.get(1).map(|last| (*last).to_owned()), start: utf16(start), end: utf16(end) }]
}
