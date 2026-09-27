//! ALR's deterministic source splitter. Source boundaries are independent of
//! citation extents: delimiters and prose remain part of the input document.

use legal_grammar::CompiledGrammar;
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, sync::LazyLock};

macro_rules! pattern {
    ($compiler:path; $($name:ident => $id:literal),+ $(,)?) => {$(
        static $name: LazyLock<CompiledGrammar> = LazyLock::new(||
            $compiler($id).expect($id));
    )+};
    ($($name:ident => $id:literal),+ $(,)?) => {
        pattern!(legal_grammar::compile_table_entry; $($name => $id),+);
    };
}
pattern! {
    URL => "cite.url", NEUTRAL => "cite.neutral", REPORTER => "cite.reporter.splitter",
    STATUTE => "cite.statute.splitter", JOURNAL => "cite.journal.splitter", BOOK => "frame.book",
    REFERENCE => "ref.token", PURE_REFERENCE => "ref.pure.splitter", LINK => "attach.link",
    SIGNAL => "signal.prefix.splitter", SOURCE_SIGNAL => "signal.source",
    AUTHOR => "ref.quoted-work-author", SHORT_FORM => "shortform.splitter",
    PARAGRAPH => "pinpoint.para.splitter", SECTION => "pinpoint.section.splitter",
    PAGE => "pinpoint.page.splitter", EDITORIAL => "bracket.editorial",
    SENTENCE => "boundary.sentence.splitter", AGGRESSIVE_SIGNAL => "signal.aggressive",
    CASE_START => "boundary.case.splitter", CROSS_REFERENCE => "ref.cross-reference",
    QUOTED => "cite.quoted", SECONDARY => "cite.secondary", LEGAL_TITLE => "title.legal.splitter",
    NAMED_CODE => "title.named-code", CONJUNCTION => "boundary.conjunction",
    NOTE_START => "ref.note-reference", EMBEDDED => "signal.embedded.splitter",
    ABBREVIATION => "boundary.abbreviation.splitter", COMPANY => "boundary.company.splitter",
    VERSUS => "party.versus-leading", SHORT_SIGNAL => "signal.short-form.splitter",
    CONJOINED_SHORT => "ref.conjoined-short-form", CONJUNCTION_END => "boundary.conjunction-ending",
    PROVISION_START => "provision.source-leading", PIN_SEPARATOR => "pinpoint.list-separator",
    PIN_NUMBER => "pinpoint.number.splitter", PROVISION_VALUE => "pinpoint.provision-value",
    ESSAY => "title.essay-collection", IBID => "ref.ibid.splitter",
    AUTHORS => "ref.author-separator.comma",
}
pattern! { legal_grammar::compile_python_table_entry;
    BARE_ADMIN => "format.bare.administrative-tail", BARE_REFERENCE => "format.bare.reference",
    BARE_CASE => "format.bare.case", BARE_STATUTE => "format.bare.statute",
    BARE_SIGNAL => "format.bare.signal", BARE_SHORT => "format.bare.short-form",
    BARE_COMMENTARY => "format.bare.commentary", BARE_SENTENCE => "format.bare.sentence",
}

/// ALR's administrative-tail removal, also used before retrieval normalization.
pub fn strip_administrative_tail(text: &str) -> String {
    BARE_ADMIN.replace_all(text, "").trim_matches(crate::text::python_whitespace).to_owned()
}

/// ALR's _derive_bare_citation. The splitter's fields retain their original
/// source surfaces; this operation removes commentary for authority lookup.
pub fn derive_bare_citation(text: &str, kind: &str) -> String {
    let trim = |text: &str| text.trim_matches(crate::text::python_whitespace).to_owned();
    let mut value = trim(text);
    for _ in 0..3 {
        let next = BARE_SIGNAL.replace_all(&value, "").into_owned();
        if next == value { break; }
        value = next;
    }
    value = trim(&BARE_SHORT.replace_all(&value, ""));
    for _ in 0..2 {
        let next = trim(&BARE_COMMENTARY.replace_all(&value, ""));
        if next == value { break; }
        value = next;
    }
    let finish = |text: &str| {
        let text = strip_administrative_tail(text);
        let text = BARE_SHORT.replace_all(&text, "");
        trim(text.trim_end_matches('.'))
    };
    if matches(&BARE_REFERENCE, &value) { return finish(&value); }
    if matches!(kind, "case" | "unreported") {
        if let Some(core) = BARE_CASE.find(&value).expect("bare case citation") {
            let mut text = &value[core.start()..];
            if let Some(sentence) = BARE_SENTENCE.find(text).expect("bare citation sentence") {
                let rest = &text[sentence.end()..];
                if rest.chars().count() > 20 && !matches(&BARE_CASE, rest) {
                    text = &text[..sentence.start()];
                }
            }
            return finish(text);
        }
    }
    if matches!(kind, "statute" | "gazette") {
        if let Some(core) = BARE_STATUTE.find(&value).expect("bare statute citation") {
            return finish(&value[core.start()..]);
        }
    }
    finish(&value)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct SourcePart {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub anchors: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct SourceSplit {
    pub status: &'static str,
    pub parts: Vec<SourcePart>,
    pub delimiters: Vec<(usize, usize, String)>,
    pub reasons: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct SourceFields {
    pub status: &'static str,
    pub corrected: String,
    pub kind: &'static str,
    pub link_candidate: String,
    pub pinpoint_fragments: Vec<String>,
    /// Decimal strings retain Python's unbounded integer representation.
    pub page_pinpoints: Vec<String>,
    pub bare_citation: String,
    pub citation_with_style: String,
    pub short_form: String,
    pub reasons: Vec<&'static str>,
}

type Anchor = (usize, usize, &'static str);
type Boundary = (usize, usize, &'static str);

fn matches(pattern: &CompiledGrammar, text: &str) -> bool {
    pattern.is_match(text).expect("source grammar match")
}

fn full_match(pattern: &CompiledGrammar, text: &str) -> bool {
    pattern.find(text).expect("source grammar match")
        .is_some_and(|found| found.start() == 0 && found.end() == text.len())
}

fn us_matches(text: &str) -> Vec<Anchor> {
    crate::us::find(text, true).into_iter().map(|found| {
        // Preserve ALR's source-routing precedence independently of the
        // authority classification: laws, journals, then case reporters.
        let editions = found.fields.exact_editions.iter().chain(&found.fields.variation_editions);
        let kind = if editions.clone().any(|edition| edition.reporter.source == "laws") { "statute" }
            else if editions.clone().any(|edition| edition.reporter.source == "journals") { "journal" }
            else { "reporter" };
        (found.span.start, found.span.end, kind)
    }).collect()
}

fn anchors(text: &str, extended_us: bool) -> Vec<Anchor> {
    let mut found = Vec::new();
    for (kind, pattern) in [("neutral", &*NEUTRAL), ("reporter", &*REPORTER),
        ("statute", &*STATUTE), ("journal", &*JOURNAL), ("book", &*BOOK), ("url", &*URL)] {
        found.extend(pattern.find_iter(text).map(|found| {
            let found = found.expect("source anchor match");
            (found.start(), found.end(), kind)
        }));
    }
    if extended_us {
        found.extend(us_matches(text));
        found.sort_by_key(|&(start, end, kind)| (start, end, match kind {
            "statute" => 0, "journal" => 1, "reporter" => 2, "neutral" => 3, _ => 4,
        }));
    } else { found.sort_unstable(); }
    let mut deduped: Vec<Anchor> = Vec::new();
    for item in found {
        if let Some(previous) = deduped.last_mut().filter(|previous| item.0 < previous.1) {
            if item.1 - item.0 > previous.1 - previous.0 { *previous = item; }
        } else { deduped.push(item); }
    }
    deduped
}

fn part(text: &str, start: usize, end: usize, strict: bool, extended_us: bool) -> Option<SourcePart> {
    let raw = &text[start..end];
    let value = raw.trim();
    if value.is_empty() { return None; }
    let start = start + raw.len() - raw.trim_start().len();
    let kinds = if full_match(&PURE_REFERENCE, value) { vec!["reference".into()] } else {
        let found = anchors(value, extended_us);
        if strict && (found.is_empty() || found.windows(2).any(|pair| {
            let gap = &value[pair[0].1..pair[1].0];
            gap.trim() != "," && !(pair[1].2 == "url" && full_match(&LINK, gap))
        })) { return None; }
        found.into_iter().map(|(_, _, kind)| kind.to_owned()).collect()
    };
    Some(SourcePart { start, end: start + value.len(), text: value.into(), anchors: kinds })
}

fn inside_quotes(text: &str, position: usize) -> bool {
    let mut inside = false;
    for character in text[..position].chars() {
        match character { '“' => inside = true, '”' => inside = false, '"' => inside = !inside, _ => {} }
    }
    inside
}

fn evidence(text: &str, extended_us: bool) -> bool {
    !anchors(text, extended_us).is_empty() || [&*CROSS_REFERENCE, &*QUOTED, &*SECONDARY, &*LEGAL_TITLE,
        &*NAMED_CODE, &*PROVISION_START].into_iter().any(|pattern| matches(pattern, text))
}

fn segment_start(boundaries: &[Boundary], position: usize) -> usize {
    boundaries.iter().filter(|(left, right, _)| *left < position && *right <= position)
        .map(|(_, right, _)| *right).max().unwrap_or(0)
}

fn segment_end(boundaries: &[Boundary], position: usize, length: usize) -> usize {
    boundaries.iter().filter(|(left, _, _)| *left > position).map(|(left, _, _)| *left).min().unwrap_or(length)
}

fn recall_boundaries(text: &str, extended_us: bool, whole_anchors: &[Anchor]) -> Vec<Boundary> {
    let mut boundaries = text.match_indices(';').map(|(index, _)| (index, index + 1, "semicolon")).collect::<Vec<_>>();
    let sentences = SENTENCE.find_iter(text).map(|found| found.expect("sentence boundary"))
        .filter(|found| !inside_quotes(text, found.start()))
        .filter(|found| {
            let prefix = &text[..found.start() + 1];
            !matches(&ABBREVIATION, prefix)
                && !(matches(&COMPANY, prefix) && matches(&VERSUS, &text[found.end()..]))
        }).map(|found| found.end()).collect::<Vec<_>>();
    for (&start, end) in sentences.iter().zip(sentences.iter().copied().skip(1).chain([text.len()])) {
        if extended_us && whole_anchors.iter().any(|&(left, right, _)| left < start && start < right) { continue; }
        if !text[..start].trim().is_empty() && evidence(&text[start..end], extended_us) {
            boundaries.push((start, start, "new_citation_sentence"));
        }
    }
    let hard = boundaries.iter().flat_map(|&(left, right, _)| [left, right])
        .chain([0, text.len()]).collect::<BTreeSet<_>>();
    for found in AGGRESSIVE_SIGNAL.find_iter(text) {
        let found = found.expect("source signal");
        if inside_quotes(text, found.start()) { continue; }
        let left = *hard.range(..=found.start()).next_back().unwrap();
        let right = *hard.range(found.end()..).next().unwrap();
        if evidence(&text[left..found.start()], extended_us) && evidence(&text[found.start()..right], extended_us) {
            boundaries.push((found.start(), found.start(), "source_signal"));
        }
    }
    for (pattern, reason) in [(&*CASE_START, "new_case_frame"), (&*AUTHOR, "new_author_title_frame"),
        (&*NOTE_START, "new_note_reference")] {
        for found in pattern.find_iter(text).skip(1) {
            let position = found.expect("source frame").start();
            if reason == "new_case_frame" {
                let prefix = &text[..position];
                let tail = prefix.char_indices().rev().nth(11).map_or(prefix, |(start, _)| &prefix[start..]);
                if prefix.ends_with('(') || matches(&CONJUNCTION_END, tail) { continue; }
            }
            let start = segment_start(&boundaries, position);
            let end = segment_end(&boundaries, position, text.len());
            if evidence(&text[start..position], extended_us) && (reason == "new_note_reference" || evidence(&text[position..end], extended_us)) {
                boundaries.push((position, position, reason));
            }
        }
    }
    if whole_anchors.is_empty() && matches(&SHORT_SIGNAL, text) {
        if let Some(found) = CONJOINED_SHORT.find(text).expect("conjoined short form") {
            boundaries.push((found.start(), found.start(), "conjoined_short_form"));
        }
    }
    for found in CONJUNCTION.find_iter(text) {
        let position = found.expect("conjoined citation").start();
        if inside_quotes(text, position) { continue; }
        let start = segment_start(&boundaries, position);
        let end = segment_end(&boundaries, position, text.len());
        if evidence(&text[start..position], extended_us) && evidence(&text[position..end], extended_us) {
            boundaries.push((position, position, "conjoined_citation"));
        }
    }
    let legal = LEGAL_TITLE.find_iter(text).map(|found| found.expect("legal title").start())
        .filter(|&position| !inside_quotes(text, position) && text[..position].rfind('[') <= text[..position].rfind(']'))
        .collect::<Vec<_>>();
    for &position in &legal {
        let start = segment_start(&boundaries, position);
        if !legal.iter().any(|&prior| start <= prior && prior < position) { continue; }
        let end = segment_end(&boundaries, position, text.len());
        if evidence(&text[start..position], extended_us) && evidence(&text[position..end], extended_us) {
            boundaries.push((position, position, "new_legal_source_frame"));
        }
    }
    let semicolons = boundaries.iter().filter(|(left, right, _)| right > left)
        .map(|(left, _, _)| *left).collect::<BTreeSet<_>>();
    boundaries.sort_unstable();
    let mut deduped = BTreeMap::new();
    for (left, right, reason) in boundaries {
        if left == right && (semicolons.contains(&left) || left.checked_sub(1).is_some_and(|p| semicolons.contains(&p))) { continue; }
        deduped.entry((left, right)).or_insert(reason);
    }
    deduped.into_iter().map(|((left, right), reason)| (left, right, reason)).collect()
}

pub fn split(text: &str, recall_first: bool, extended_us: bool) -> SourceSplit {
    let abstain = |reason| SourceSplit { status: "abstain", parts: Vec::new(), delimiters: Vec::new(), reasons: vec![reason] };
    if text.trim().is_empty() { return abstain("empty"); }
    let boundaries = if recall_first { recall_boundaries(text, extended_us, &anchors(text, extended_us)) } else {
        let top = crate::find::top_level(text);
        let mut boundaries = text.match_indices(';').filter(|(position, _)| top[*position])
            .map(|(position, _)| (position, position + 1, "top_level_semicolon")).collect::<Vec<_>>();
        for found in SOURCE_SIGNAL.captures_iter(text) {
            let found = found.expect("source signal");
            let position = found.name("sentence").or_else(|| found.name("inline")).unwrap().start();
            if !top[position] { continue; }
            let start = boundaries.iter().filter(|(_, right, _)| *right <= position).map(|(_, right, _)| *right).max().unwrap_or(0);
            let end = boundaries.iter().filter(|(left, _, _)| *left >= position).map(|(left, _, _)| *left).min().unwrap_or(text.len());
            if part(text, start, position, true, extended_us).is_some() && part(text, position, end, true, extended_us).is_some() {
                boundaries.push((position, position, "explicit_source_signal"));
            }
        }
        boundaries.sort_unstable();
        if boundaries.is_empty() {
            return if full_match(&PURE_REFERENCE, text) {
                SourceSplit { status: "deterministic_complete", parts: vec![part(text, 0, text.len(), true, extended_us).unwrap()],
                    delimiters: Vec::new(), reasons: vec!["pure_reference"] }
            } else { abstain("no_supported_boundary") };
        }
        boundaries
    };
    let starts = std::iter::once(0).chain(boundaries.iter().map(|(_, right, _)| *right));
    let ends = boundaries.iter().map(|(left, _, _)| *left).chain([text.len()]);
    let mut parts = Vec::new();
    for (start, end) in starts.zip(ends) {
        match part(text, start, end, !recall_first, extended_us) {
            Some(part) => parts.push(part),
            None if !recall_first => return abstain("unconsumed_or_ambiguous_clause"),
            None => {},
        }
    }
    if parts.is_empty() { return abstain("empty_parts"); }
    let delimiters = parts.windows(2).map(|pair| (pair[0].end, pair[1].start,
        text[pair[0].end..pair[1].start].to_owned())).collect();
    let mut reasons = Vec::new();
    if recall_first {
        for (_, _, reason) in boundaries { if !reasons.contains(&reason) { reasons.push(reason); } }
        if reasons.is_empty() { reasons.push("single_citation_or_prose"); }
    } else {
        for reason in ["top_level_semicolon", "explicit_source_signal"] {
            if boundaries.iter().any(|(_, _, used)| *used == reason) { reasons.push(reason); }
        }
    }
    SourceSplit { status: "deterministic_complete", parts, delimiters, reasons }
}

fn strip_signals(text: &str) -> String {
    let mut value = text.trim().to_owned();
    for _ in 0..3 {
        let stripped = SIGNAL.replace_all(&value, "");
        if stripped == value { break; }
        value = stripped.trim().to_owned();
    }
    value
}

fn short_form(text: &str, kind: &str) -> String {
    if let Some(bracket) = SHORT_FORM.captures(text).expect("source short form") {
        let value = bracket.name("short").unwrap().as_str().trim();
        if !value.chars().all(|c| c.is_numeric()) && !matches(&EDITORIAL, value) { return value.into(); }
    }
    if matches(&REFERENCE, text) {
        if matches(&IBID, text) { return "Ibid".into(); }
        let value = strip_signals(text);
        let end = crate::short_forms::supra_position(&value).unwrap_or(value.len());
        return value[..end].trim_matches([' ', ',', ';', ':', '.']).into();
    }
    if matches!(kind, "journal" | "book" | "essay_collection" | "report") {
        let prefix = if let Some(position) = text.find(['"', '“']) {
            let prefix = text[..position].trim_matches([' ', ',']);
            prefix.rsplit_once(':').map_or(prefix, |(_, tail)| tail.trim())
        } else { text.split(',').next().unwrap_or(text).trim() };
        let prefix = strip_signals(prefix);
        return AUTHORS.split(&prefix).map(|name| name.expect("author separator"))
            .filter_map(crate::short_forms::surname).collect::<Vec<_>>().join(" and ");
    }
    String::new()
}

fn pin_values(value: &str, expand_ranges: bool) -> Vec<String> {
    let mut values = Vec::new();
    for item in PIN_SEPARATOR.split(value) {
        let item = item.expect("pinpoint list separator");
        let numbers = PIN_NUMBER.find_iter(item).map(|found| found.expect("pinpoint number").as_str()).collect::<Vec<_>>();
        let Some(first) = numbers.first() else { continue; };
        values.push((*first).to_owned());
        if expand_ranges && numbers.len() > 1 && !first.contains('.') && !numbers[1].contains('.') {
            let start: BigUint = first.parse().expect("decimal pinpoint");
            let mut end: BigUint = numbers[1].parse().expect("decimal pinpoint");
            if numbers[1].len() < first.len() {
                let magnitude = BigUint::from(10u8).pow(numbers[1].len() as u32);
                end += &start - (&start % &magnitude);
                if end < start { end += magnitude; }
            }
            if end > start && &end - &start <= BigUint::from(100u8) {
                let mut number = start + 1u8;
                while number <= end { values.push(number.to_string()); number += 1u8; }
            }
        }
    }
    values
}

fn pinpoints(text: &str, kind: &str, extended_us: bool) -> (Vec<String>, Vec<String>) {
    let case = matches!(kind, "case" | "unreported");
    let law = matches!(kind, "statute" | "regulation" | "legislation");
    let unresolved = matches!(kind, "" | "other");
    if case || unresolved {
        if let Some(found) = PARAGRAPH.captures(text).expect("paragraph pinpoint") {
            return (pin_values(&found["values"], false).into_iter().map(|value| format!("par{value}")).collect(), Vec::new());
        }
    }
    if law || unresolved {
        let mut reporters = REPORTER.find_iter(text).map(|found| {
            let found = found.expect("reporter span"); found.start()..found.end()
        }).collect::<Vec<_>>();
        if extended_us { reporters.extend(us_matches(text).into_iter().filter(|(_, _, kind)| *kind == "reporter").map(|(start, end, _)| start..end)); }
        let provision = SECTION.captures_iter(text).map(|found| found.expect("provision pinpoint"))
            .find(|found| !reporters.iter().any(|span| span.contains(&found.get(0).unwrap().start())));
        if let Some(provision) = provision {
            let mut values = Vec::new();
            for item in PIN_SEPARATOR.split(&provision["values"]) {
                if let Some(found) = PROVISION_VALUE.find(item.expect("provision separator")).expect("provision value") {
                    values.push(format!("sec{}", found.as_str().chars().filter(|c| !c.is_whitespace()).collect::<String>().replace('–', "-")));
                }
            }
            return (values, Vec::new());
        }
    }
    if !law {
        if let Some(found) = PAGE.captures(text).expect("page pinpoint") {
            return (Vec::new(), pin_values(&found["values"], true).into_iter().filter(|value| value.bytes().all(|c| c.is_ascii_digit()))
                .map(|value| value.parse::<BigUint>().expect("decimal pinpoint").to_string()).collect());
        }
    }
    (Vec::new(), Vec::new())
}

fn kind(text: &str, anchors: &[String], extended_us: bool) -> &'static str {
    if full_match(&PURE_REFERENCE, text) { return "other"; }
    if matches(&BOOK, text) { return if matches(&ESSAY, text) { "essay_collection" } else { "book" }; }
    if (if extended_us { anchors.iter().any(|kind| kind == "journal") } else { matches(&JOURNAL, text) })
        && text.contains(['"', '“']) { return "journal"; }
    if anchors.iter().any(|kind| kind == "statute") { return "statute"; }
    if anchors.iter().any(|kind| matches!(kind.as_str(), "neutral" | "reporter")) { return "case"; }
    if anchors.iter().any(|kind| kind == "journal") { return "journal"; }
    "other"
}

fn bare_citation(text: &str, kind: &str, extended_us: bool) -> String {
    let stripped = SHORT_FORM.replace_all(text, "");
    let value = stripped.trim().trim_end_matches('.').trim();
    if matches(&REFERENCE, value) { return value.into(); }
    let mut start = match kind {
        "case" => [&*NEUTRAL, &*REPORTER].into_iter().flat_map(|pattern| pattern.find_iter(value))
            .map(|found| found.expect("case core").start()).min(),
        "statute" => STATUTE.find(value).expect("statute core").map(|found| found.start()),
        "journal" => JOURNAL.find(value).expect("journal core").map(|found| found.start()),
        _ => None,
    };
    if extended_us {
        let family = if kind == "case" { "reporter" } else { kind };
        let us = us_matches(value).into_iter().find(|(_, _, kind)| *kind == family).map(|(start, _, _)| start);
        start = if kind == "case" { start.into_iter().chain(us).min() } else { start.or(us) };
    }
    value[start.unwrap_or(0)..].into()
}

fn embedded_source(text: &str, extended_us: bool) -> bool {
    for signal in EMBEDDED.find_iter(text) {
        let signal = signal.expect("embedded source signal");
        if text[..signal.start()].chars().count() < 3 { continue; }
        let tail = text[signal.end()..].chars().take(320).collect::<String>();
        if [&*REFERENCE, &*NEUTRAL, &*REPORTER, &*STATUTE, &*JOURNAL, &*BOOK]
            .into_iter().any(|pattern| matches(pattern, &tail)) || (extended_us && !us_matches(&tail).is_empty()) { return true; }
    }
    false
}

pub fn extract_fields(part: &SourcePart, extended_us: bool) -> SourceFields {
    let text = part.text.trim();
    let kind = kind(text, &part.anchors, extended_us);
    let styled = strip_signals(text);
    let (fragments, pages) = pinpoints(&styled, kind, extended_us);
    let link = URL.find(text).expect("source URL").map_or("other", |found| found.as_str().trim_matches(['<', '>', '.', ',', ';', ' ']));
    let mut reasons = Vec::new();
    if embedded_source(&styled, extended_us) { reasons.push("embedded_second_source"); }
    if styled.is_empty() { reasons.push("missing_citation_surface"); }
    let bare = bare_citation(&styled, kind, extended_us);
    if bare.is_empty() { reasons.push("missing_bare_citation"); }
    SourceFields { status: if reasons.is_empty() { "complete" } else { "partial" },
        corrected: text.into(), kind, link_candidate: link.into(), pinpoint_fragments: fragments,
        page_pinpoints: pages, bare_citation: bare, short_form: short_form(&styled, kind),
        citation_with_style: styled, reasons }
}

pub fn extract_text_fields(text: &str, extended_us: bool) -> SourceFields {
    let value = text.trim();
    let start = text.len() - text.trim_start().len();
    extract_fields(&SourcePart { start, end: start + value.len(), text: value.into(),
        anchors: anchors(value, extended_us).into_iter().map(|(_, _, kind)| kind.to_owned()).collect() }, extended_us)
}
