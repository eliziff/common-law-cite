//! Pinned Eyecite reporter extractors and whitespace-insensitive prefilter.
use crate::model::{Fields, SourceEdition};
use aho_corasick::AhoCorasick;
use chrono::Datelike;
use legal_grammar::CompiledGrammar;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use crate::screen::Screened;
use std::sync::{LazyLock, OnceLock};

mod names {
    use super::*;
    use crate::SourceCaseName;

    macro_rules! pattern {
        ($name:ident, $id:literal) => {
            static $name: LazyLock<CompiledGrammar> = LazyLock::new(||
                legal_grammar::compile_python_table_entry($id).expect($id));
        };
    }
    pattern!(ID, "name.us.id");
    pattern!(SUPRA, "name.us.supra");
    pattern!(SUPRA_ANTECEDENT, "name.us.supra-antecedent");
    pattern!(PARAGRAPH, "name.us.paragraph");
    pattern!(STOP, "name.us.stop");
    pattern!(PLACEHOLDER, "cite.us.placeholder");
    pattern!(SECTION, "name.us.section");
    pattern!(YEAR, "name.us.year");
    pattern!(VERSUS, "name.us.versus");
    pattern!(HTML_VERSUS, "name.us.html-versus");
    pattern!(LOWER, "name.us.lower-word");
    pattern!(IN, "name.us.leading-in");
    pattern!(ARTICLE, "name.us.leading-article");
    pattern!(INITIAL_ARTICLE, "name.us.initial-article");
    pattern!(CAPITAL, "name.us.capital");
    pattern!(NUMBER, "name.us.ending-number");
    pattern!(PRE, "name.us.pre-full");
    static STOP_CASE: LazyLock<CompiledGrammar> = LazyLock::new(|| {
        let tables = legal_grammar::load_tables().unwrap();
        legal_grammar::compile_python_pattern(&tables["name.us.stop"].entry.pattern, "").unwrap()
    });

    #[derive(Clone, Copy)]
    enum Kind<'a> { Text, Citation(Option<&'a str>), Stop(bool), Supra, Id, Placeholder, Section, Other }
    struct Word<'a> { text: &'a str, start: usize, end: usize, kind: Kind<'a> }

    // Python str.isalpha uses Unicode Letter, not Other_Alphabetic.
    static LETTER: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\p{L}").unwrap());
    fn alphabetic(character: char) -> bool {
        LETTER.is_match(character.encode_utf8(&mut [0; 4]))
    }

    // Eyecite Tokenizer.append_text splits on literal spaces, retaining each
    // space as a token. Citation tokens are the already recognized cores.
    fn append<'a>(words: &mut Vec<Word<'a>>, text: &'a str, start: usize, end: usize) {
        let mut at = start;
        for (offset, _) in text[start..end].match_indices(' ') {
            let space = start + offset;
            if at < space { words.push(Word { text: &text[at..space], start: at, end: space, kind: Kind::Text }); }
            words.push(Word { text: &text[space..space + 1], start: space, end: space + 1, kind: Kind::Text });
            at = space + 1;
        }
        if at < end { words.push(Word { text: &text[at..end], start: at, end, kind: Kind::Text }); }
    }

    fn slice(words: &[Word<'_>], start: isize, end: isize) -> String {
        let bound = |index: isize| (if index < 0 { words.len() as isize + index } else { index })
            .clamp(0, words.len() as isize) as usize;
        let (start, end) = (bound(start), bound(end));
        words.iter().skip(start).take(end.saturating_sub(start)).map(|word| word.text).collect()
    }

    fn strip_stop_words(text: &str) -> String {
        let cleaned = STOP_CASE.replace_all(text, " ");
        let cleaned = IN.replace_all(&cleaned, "");
        let cleaned = cleaned.trim_matches(crate::text::python_whitespace).trim_start_matches('(').trim_end_matches(')');
        let cleaned = cleaned.split(';').nth(1).unwrap_or(cleaned).trim_matches([',', ' ']);
        STOP.replace_all(cleaned, "").trim_matches([',', ' ']).trim_matches(crate::text::python_whitespace).to_owned()
    }

    // Port of helpers._scan_for_case_boundaries and _process_case_name.
    // Keep the source's 28-token search and slice boundaries, including its
    // treatment of preceding citations, placeholders, punctuation and years.
    fn name(text: &str, words: &[Word<'_>], cite: usize, short: bool) -> SourceCaseName {
        let mut result = SourceCaseName { full_span_start: words[cite].start, ..Default::default() };
        let (mut versus, mut start, mut title, mut length, mut plaintiff_length) =
            (false, 0isize, cite as isize - 1, 0usize, 0usize);
        let mut candidate = None;
        for index in (cite.saturating_sub(27)..cite).rev() {
            let word = &words[index];
            let value = word.text;
            if value == "," { continue; }
            length += 1;
            let nonempty = !value.trim_matches(crate::text::python_whitespace).is_empty();
            if versus && nonempty { plaintiff_length += 1; }
            if matches!(word.kind, Kind::Citation(_)) { title = index as isize - 1; continue; }
            if value.ends_with([';', '\u{201d}', '"']) {
                start = index as isize + 2; candidate = Some(slice(words, start, title)); break;
            }
            if YEAR.find(value).expect("source year").is_some_and(|matched| matched.start() == 0) {
                title = index as isize - 1;
                result.year = Some(value.chars().skip(1).take(4).collect());
                continue;
            }
            let first = value.chars().next().unwrap();
            if first == '(' && length > 3 {
                start = index as isize;
                if value == "(" || value.chars().nth(1).is_some_and(|c| alphabetic(c) && c.is_lowercase()) { start += 2; }
                candidate = Some(slice(words, start, title)); break;
            }
            let article = matches!(value, "of" | "the" | "an" | "and");
            if versus && !first.is_uppercase() && nonempty && !article {
                start = index as isize + 2;
                candidate = Some(ARTICLE.replace_all(&slice(words, start, title), "").into_owned()); break;
            }
            if matches!(word.kind, Kind::Placeholder) { title = index as isize - 1; continue; }
            if matches!(word.kind, Kind::Stop(true)) {
                versus = true; start = index as isize - 2;
                candidate = Some(slice(words, start, title)); continue;
            }
            if (versus && first.is_uppercase() && value.chars().count() > 4 && value.ends_with('.') && plaintiff_length > 1)
                || matches!(word.kind, Kind::Stop(_)) {
                start = index as isize + 2; candidate = Some(slice(words, start, title)); break;
            }
            if !versus && !first.is_uppercase() && nonempty && alphabetic(first) && !article {
                if matches!(value, "ex" | "rel.") { continue; }
                if matches!(word.kind, Kind::Supra) { title = index as isize - 1; continue; }
                start = index as isize + 2;
                let value = slice(words, start, title);
                candidate = CAPITAL.find(&value).expect("source capitalized name").map(|matched| value[matched.start()..].to_owned());
                break;
            }
            if index == 0 {
                start = 0;
                let value = INITIAL_ARTICLE.replace_all(&slice(words, 0, title), "").into_owned();
                candidate = (!NUMBER.is_match(&value).expect("source name ending")).then_some(value);
            }
        }
        let Some(candidate) = candidate.filter(|value| !value.is_empty()) else { result.year = None; return result; };
        let defendant = if versus {
            let (plaintiff, defendant) = VERSUS.find(&candidate).expect("source versus").map_or(("", candidate.as_str()),
                |matched| (&candidate[..matched.start()], &candidate[matched.end()..]));
            let plaintiff = plaintiff.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}' | ',' | '('));
            result.plaintiff = Some(strip_stop_words(&LOWER.replace_all(plaintiff, "")));
            defendant
        } else { &candidate };
        let defendant = strip_stop_words(defendant);
        if !defendant.is_empty() {
            if short { result.antecedent_guess = Some(defendant); } else { result.defendant = Some(defendant); }
            let offset = slice(words, start, cite as isize - 1).chars().count() + 1;
            result.full_span_start = text[..words[cite].start].char_indices().rev().nth(offset - 1).map_or(0, |(at, _)| at);
        }
        result
    }

    // helpers.find_case_name_in_html, using Document's original tag lookup
    // and both source/plain SpanUpdaters.
    fn html_name(text: &str, words: &[Word<'_>], cite: usize, short: bool, markup: &crate::clean::Markup<'_>) -> Option<SourceCaseName> {
        let tag = |position| markup.tag_at(position);
        let before_core = |range: &Range<usize>| &text[range.start..range.end.min(words[cite].start).max(range.start)];
        for index in (cite.saturating_sub(28)..cite).rev() {
            let word = &words[index];
            if word.text.trim_matches([',', ' ']).is_empty() { continue; }
            if short {
                let (_, range) = tag(word.start)?;
                return Some(SourceCaseName { full_span_start: range.start,
                    antecedent_guess: Some(strip_stop_words(before_core(range))), ..Default::default() });
            }
            match word.kind {
                Kind::Stop(true) => {
                    let (left_tag, left) = tag(word.start.checked_sub(slice(words, index as isize - 2, index as isize).len())?)?;
                    let (right_tag, right) = tag(word.start + slice(words, index as isize, index as isize + 2).len())?;
                    let (plaintiff, defendant) = if left_tag == right_tag {
                        let value = &text[left.clone()];
                        HTML_VERSUS.find(value).expect("source HTML versus").map_or(("", value),
                            |matched| (&value[..matched.start()], &value[matched.end()..]))
                    } else { (&text[left.clone()], &text[right.clone()]) };
                    let clean = strip_stop_words(plaintiff);
                    let shift = plaintiff.chars().count() - clean.chars().count();
                    let start = text[left.start..].char_indices().nth(shift).map_or(text.len(), |(at, _)| left.start + at);
                    return Some(SourceCaseName { full_span_start: start,
                        plaintiff: Some(clean.trim_matches(crate::text::python_whitespace).trim_matches(',').trim_matches('(').to_owned()),
                        defendant: Some(strip_stop_words(defendant).trim_matches(crate::text::python_whitespace).trim_matches(',').to_owned()),
                        ..Default::default() });
                }
                Kind::Stop(false) => {
                    let mut shift = 3;
                    while words.get(index + shift).is_some_and(|word| word.text == " ") { shift += 1; }
                    let end = word.start + slice(words, index as isize, (index + shift) as isize).len();
                    let position = text[..end].char_indices().next_back()?.0;
                    let (_, range) = tag(position)?;
                    return Some(SourceCaseName { full_span_start: range.start,
                        defendant: Some(strip_stop_words(before_core(range)).trim_matches([',', ' ']).to_owned()),
                        ..Default::default() });
                }
                _ => {},
            }
        }
        None
    }

    // helpers.match_on_tokens: stop at a non-string token and retain at most
    // 300 Unicode scalars when scanning backward.
    fn before(words: &[Word<'_>], cite: usize) -> String {
        let mut window = String::new();
        for word in words[..cite].iter().rev().take_while(|word| matches!(word.kind, Kind::Text)) {
            window.insert_str(0, word.text);
            let length = window.chars().count();
            if length >= 300 {
                window.drain(..window.char_indices().nth(length - 300).unwrap().0);
                break;
            }
        }
        window
    }

    fn preceding_offset(text: &str, end: usize, suffix: &str) -> usize {
        let length = suffix.chars().count();
        if length == 0 { end } else { text[..end].char_indices().rev().nth(length - 1).map_or(0, |(at, _)| at) }
    }

    // helpers.add_pre_citation / match_on_tokens: stop at the first non-string
    // token and keep the last 300 Unicode scalars, not 300 bytes.
    fn pre_citation(text: &str, words: &[Word<'_>], cite: usize, name: &mut SourceCaseName) {
        if name.plaintiff.as_ref().is_some_and(|value| !value.is_empty())
            || name.defendant.as_ref().is_some_and(|value| !value.is_empty()) { return; }
        let window = before(words, cite);
        let end = words[cite].start;
        let Some(captures) = PRE.captures(&window).expect("source pre-citation") else { return; };
        let matched = captures.get(0).unwrap();
        name.full_span_start = preceding_offset(text, end, &window[matched.start()..]);
        name.pre_citation = Some(crate::find::span(text, name.full_span_start..end));
        name.antecedent_guess = captures.name("antecedent").map(|value| value.as_str().to_owned());
        name.pin_cite = captures.name("pin_cite").and_then(|pin|
            crate::metadata::clean_pin_cite(&window, pin.start()..pin.end())).map(|mut pin| {
                pin.start = preceding_offset(text, end, &window[pin.start..]);
                pin.end = preceding_offset(text, end, &window[pin.end..]);
                pin
            });
    }

    // find._extract_id_citation / _extract_supra_citation. The existing tail
    // parser carries the pinned POST_SHORT_CITATION_REGEX and parenthetical rules.
    fn reference(text: &str, words: &[Word<'_>], index: usize, prefix: &str) -> SourceCaseName {
        let token = &words[index];
        let limit = words[index + 1..].iter().take_while(|word| matches!(word.kind, Kind::Text))
            .last().map_or(token.end, |word| word.end);
        let mut name = crate::metadata::short_reference(text, token.start..token.end, limit, prefix);
        name.reference_form = match token.kind {
            Kind::Supra => Some(crate::Form::Supra), Kind::Id => Some(crate::Form::Ibid), _ => None,
        };
        if matches!(token.kind, Kind::Supra) {
            let window = before(words, index);
            if let Some(captures) = SUPRA_ANTECEDENT.captures(&window).expect("source supra antecedent") {
                name.full_span_start = preceding_offset(text, token.start, &window[captures.get(0).unwrap().start()..]);
                name.antecedent_guess = captures.name("antecedent").or_else(|| captures.name("antecedent_only"))
                    .map(|value| value.as_str().to_owned());
                name.volume = captures.name("volume").or_else(|| captures.name("volume_only"))
                    .map(|value| value.as_str().to_owned());
            }
        }
        name
    }

    pub(super) fn extract(text: &str, citations: &[(Range<usize>, Option<&str>, Option<usize>)], markup: Option<&crate::clean::Markup<'_>>) -> BTreeMap<usize, SourceCaseName> {
        let mut tokens: Vec<_> = citations.iter().map(|(span, prefix, _)| Word {
            text: &text[span.clone()], start: span.start, end: span.end, kind: Kind::Citation(*prefix),
        }).collect();
        for (pattern, kind) in [(&*ID, Kind::Id), (&*SUPRA, Kind::Supra), (&*PARAGRAPH, Kind::Other),
            (&*STOP, Kind::Stop(false)), (&*PLACEHOLDER, Kind::Placeholder), (&*SECTION, Kind::Section)] {
            for captures in pattern.captures_iter(text) {
                let captures = captures.expect("source name token");
                let matched = captures.get(1).expect("source token capture");
                let kind = if matches!(kind, Kind::Stop(_)) {
                    Kind::Stop(captures.name("stop_word").is_some_and(|value| value.as_str() == "v"))
                } else { kind };
                tokens.push(Word { text: matched.as_str(), start: matched.start(), end: matched.end(), kind });
            }
        }
        let omitted: Vec<_> = citations.iter().filter_map(|(span, _, end)|
            end.map(|end| end..span.start)).collect();
        tokens.retain(|word| !omitted.iter().any(|range| range.contains(&word.start)));
        tokens.sort_by_key(|word| (word.start, std::cmp::Reverse(word.end)));
        let (mut words, mut end) = (Vec::new(), 0);
        for token in tokens {
            if token.start < end { continue; }
            let before = omitted.iter().find(|range| range.end == token.start)
                .map_or(token.start, |range| range.start);
            if end < before { append(&mut words, text, end, before); }
            end = token.end; words.push(token);
        }
        append(&mut words, text, end, text.len());
        words.iter().enumerate().filter_map(|(index, word)| match word.kind {
            Kind::Citation(prefix) => {
                let short = prefix.is_some();
                let mut found = markup.and_then(|markup| html_name(text, &words, index, short, markup))
                    .unwrap_or_else(|| name(text, &words, index, short));
                found.token_span = Some(crate::find::span(text, word.start..word.end));
                if let Some(prefix) = prefix {
                    let pin = reference(text, &words, index, prefix);
                    found.full_span_end = pin.full_span_end;
                    found.reference_span = pin.reference_span;
                    found.pin_cite = pin.pin_cite;
                    found.parenthetical = pin.parenthetical;
                } else { pre_citation(text, &words, index, &mut found); }
                Some((word.start, found))
            },
            Kind::Id | Kind::Supra => Some((word.start, reference(text, &words, index, ""))),
            Kind::Section => Some((word.start, SourceCaseName {
                full_span_start: word.start, full_span_end: Some(word.end),
                reference_span: Some(crate::find::span(text, word.start..word.end)),
                token_span: Some(crate::find::span(text, word.start..word.end)), ..Default::default()
            })),
            _ => None,
        }).collect()
    }

    /// Build this module's lazily built statics now ([`crate::warm`]).
    pub(super) fn warm() {
        crate::warm_statics!(
            ID, SUPRA, SUPRA_ANTECEDENT, PARAGRAPH, STOP, PLACEHOLDER, SECTION, YEAR, VERSUS,
            HTML_VERSUS, LOWER, IN, ARTICLE, INITIAL_ARTICLE, CAPITAL, NUMBER, PRE, STOP_CASE,
            LETTER
        );
    }
}

pub(crate) fn case_names(text: &str, citations: &[(Range<usize>, Option<&str>, Option<usize>)], markup: Option<&crate::clean::Markup<'_>>) -> BTreeMap<usize, crate::SourceCaseName> {
    names::extract(text, citations, markup)
}

#[derive(Deserialize)]
struct Extractor {
    pattern: String,
    exact: Vec<usize>,
    variations: Vec<usize>,
    strings: Vec<String>,
    short: bool,
    standard: bool,
    #[serde(default)]
    ecmascript: bool,
    #[serde(skip)]
    compiled: OnceLock<CompiledGrammar>,
}

#[derive(Deserialize)]
struct Data {
    editions: Vec<SourceEdition>,
    extractors: Vec<Extractor>,
}

static DATA: LazyLock<Data> = LazyLock::new(|| {
    serde_json::from_str(legal_grammar::US_EXTRACTORS_JSON).expect("pinned US extractors")
});

fn compact(text: &str) -> String {
    text.chars().filter(|c| !crate::text::python_whitespace(*c)).collect()
}

static FILTER: LazyLock<(AhoCorasick, Vec<Vec<usize>>, Vec<usize>)> = LazyLock::new(|| {
    let mut strings: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut unfiltered = Vec::new();
    for (index, extractor) in DATA.extractors.iter().enumerate() {
        if extractor.strings.is_empty() { unfiltered.push(index); }
        for string in &extractor.strings {
            strings.entry(compact(string)).or_default().push(index);
        }
    }
    let automaton = AhoCorasick::new(strings.keys()).expect("reporter prefilter");
    (automaton, strings.into_values().collect(), unfiltered)
});

pub(crate) struct Match {
    pub span: Range<usize>,
    pub fields: Fields,
    pub short_at: Option<usize>,
    pub preceding_text_end: Option<usize>,
    short: bool,
    ecmascript: bool,
}

// Eyecite token_is_from_nominative_reporter: source tagging is incomplete.
pub(crate) fn nominative(fields: &Fields) -> bool {
    fields.exact_editions.first().or(fields.variation_editions.first())
        .is_some_and(|edition| matches!(edition.reporter.short_name.as_str(),
            "Thompson" | "Cooke" | "Holmes" | "Olcott" | "Chase" |
            "Gilmer" | "Bee" | "Deady" | "Taney"))
}

impl Match {
    fn merge(&mut self, other: &Self) -> bool {
        if self.span != other.span || self.short != other.short
            || self.fields.source_groups != other.fields.source_groups { return false; }
        for edition in &other.fields.exact_editions {
            if !self.fields.exact_editions.contains(edition) {
                self.fields.exact_editions.push(edition.clone());
            }
        }
        for edition in &other.fields.variation_editions {
            if !self.fields.variation_editions.contains(edition) {
                self.fields.variation_editions.push(edition.clone());
            }
        }
        self.fields.variation_editions.retain(|edition| !self.fields.exact_editions.contains(edition));
        true
    }
}

static SURFACES: LazyLock<[regex::Regex; 2]> = LazyLock::new(|| {
    let tables = legal_grammar::load_tables().unwrap();
    let defs = &tables["cite.us.reporter.standard.full"].defs;
    ["us_reporters", "us_journals"].map(|name| regex::Regex::new(
        &format!("^(?:{})$", defs[name]).replace(r"\s*", "").replace(' ', "")).unwrap())
});
// One catalogue lookup for discovery and classification. The original LSP
// gives a shared reporter/journal spelling to reporters first.
fn standard_source(surface: &str) -> Option<&'static str> {
    ["reporters", "journals"].into_iter().zip(SURFACES.iter())
        .find_map(|(source, pattern)| pattern.is_match(surface).then_some(source))
}

static CANDIDATE: LazyLock<regex::Regex> = LazyLock::new(||
    legal_grammar::compile_ecmascript_table_entry("cite.us.reporter.candidate").unwrap());
pub(crate) fn is_journal(core: &str) -> bool {
    CANDIDATE.captures(core).is_some_and(|captures| {
        let citation = captures.name("citation").unwrap();
        citation.start() == 0 && citation.end() == core.len()
            && standard_source(&captures["reporter"].chars()
                .filter(|c| !crate::text::javascript_whitespace(*c)).collect::<String>()) == Some("journals")
    })
}

fn screened(source: &str) -> Screened<regex::Regex> {
    let literals = legal_grammar::linear_pattern_literals(source);
    let source = source.to_owned();
    Screened::new(move || regex::Regex::new(&source).unwrap(), literals)
}
static PATTERN: LazyLock<Screened<regex::Regex>> = LazyLock::new(|| screened(
    &legal_grammar::load_tables().unwrap()["cite.us.standard-candidate"].entry.pattern));
static EXTENDED: LazyLock<Screened<regex::Regex>> = LazyLock::new(|| {
    let tables = legal_grammar::load_tables().unwrap();
    screened(&tables["cite.us.standard-candidate"].entry.pattern
        .replace("[0-9]+|_+", &tables["cite.us.reporter.full"].defs["us_page"]))
});
static EDITIONS: LazyLock<BTreeMap<String, (BTreeSet<usize>, BTreeSet<usize>)>> = LazyLock::new(|| {
    let mut map = BTreeMap::<String, (BTreeSet<usize>, BTreeSet<usize>)>::new();
    for extractor in &DATA.extractors {
        for surface in &extractor.strings {
            let entry = map.entry(compact(surface)).or_default();
            entry.0.extend(extractor.exact.iter().copied().filter(|&index|
                matches!(DATA.editions[index].reporter.source.as_str(), "reporters" | "journals")));
            entry.1.extend(extractor.variations.iter().copied().filter(|&index|
                matches!(DATA.editions[index].reporter.source.as_str(), "reporters" | "journals")));
        }
    }
    map
});
// Frozen LSP citator.rs::standard_us_matches. Its catalogue lookup ignores
// whitespace anywhere in the reporter surface, including dotless variants.
fn standard_matches(text: &str, extended: bool) -> Vec<Match> {
    // Frozen LSP us_fallback_ranges: extended pages are scanned only on cue lines.
    let mut ranges = vec![(0, text.len(), &*PATTERN)];
    if extended {
        ranges.extend(fallback_ranges(text).into_iter().map(|(start, end)| (start, end, &*EXTENDED)));
    }
    let mut found = Vec::new();
    for (start, end, pattern) in ranges {
        let value = &text[start..end];
        if !pattern.may_match(value) { continue; }
        let mut cursor = 0;
        while let Some(captures) = pattern.captures_at(value, cursor) {
            let core = captures.name("citation").unwrap();
            let reporter = captures.name("reporter").unwrap().as_str().chars()
                .filter(|character| !crate::text::javascript_whitespace(*character)).collect::<String>();
            if standard_source(&reporter).is_some() {
                let (exact, variations) = &EDITIONS[&reporter];
                let short_at = captures.name("__short_at").map(|matched| matched.start() - core.start());
                let fields = Fields {
                    source_groups: ["volume", "reporter", "page"].into_iter().map(|name|
                        (name.to_owned(), captures.name(name).map(|matched| matched.as_str().to_owned()))).collect(),
                    exact_editions: exact.iter().map(|&index| DATA.editions[index].clone()).collect(),
                    variation_editions: variations.difference(exact).map(|&index| DATA.editions[index].clone()).collect(),
                    ..Fields::default()
                };
                found.push(Match { span: start + core.start()..start + core.end(), fields, short_at,
                    short: short_at.is_some(), ecmascript: true, preceding_text_end: None });
            }
            cursor = core.start() + 1;
        }
    }
    found
}

static CUE: LazyLock<Screened<regex::Regex>> = LazyLock::new(|| Screened::linear("cite.us.fallback-cue"));
fn fallback_ranges(text: &str) -> BTreeSet<(usize, usize)> {
    CUE.find_all(text).map(|cue| (
        text[..cue.start()].rfind('\n').map_or(0, |at| at + 1),
        text[cue.end()..].find('\n').map_or(text.len(), |at| cue.end() + at),
    )).collect()
}

static COMMON: LazyLock<Screened<legal_grammar::AsciiBoundedGrammar>> = LazyLock::new(|| Screened::new(
        || legal_grammar::compile_ascii_bounded_table_entry("cite.us.law.common").unwrap(),
        legal_grammar::ascii_bounded_table_entry_literals("cite.us.law.common").unwrap()));
// The custom reporters are read only for extended US citations; a statute is read as one by the
// laws whatever the reader asks for.
static CUSTOM: LazyLock<[(&str, legal_grammar::AsciiBoundedGrammar); 2]> = LazyLock::new(||
        ["cite.us.reporter.custom.full", "cite.us.reporter.custom.short"].map(|id|
            (id, legal_grammar::compile_ascii_bounded_table_entry(id).unwrap())));
static LAWS: LazyLock<[(&str, legal_grammar::AsciiBoundedGrammar); 2]> = LazyLock::new(||
        ["cite.us.law.full", "cite.us.law.short"].map(|id|
            (id, legal_grammar::compile_ascii_bounded_table_entry(id).unwrap())));

pub(crate) fn is_law(core: &str) -> bool {
    let whole = |span: &Range<usize>| span.start == 0 && span.end == core.len();
    COMMON.find_spans(core).iter().any(whole) || LAWS.iter().any(|(_, grammar)|
        grammar.find_spans(core).iter().any(whole))
}

pub(crate) fn common_law_spans(text: &str) -> Vec<Range<usize>> {
    if COMMON.may_match(text) { COMMON.find_spans(text) } else { Vec::new() }
}

/// LSP's citation-hit evidence for excerpt scoring, before identity or metadata.
/// Its four extended grammars only run on the original cue-selected lines.
pub(crate) fn citation_spans(text: &str, extended: bool) -> Vec<Range<usize>> {
    let mut spans = standard_matches(text, extended).into_iter().map(|hit| hit.span).collect::<Vec<_>>();
    extend_native_spans(text, extended, &mut spans);
    spans
}

fn extend_native_spans(text: &str, extended: bool, spans: &mut Vec<Range<usize>>) {
    spans.extend(common_law_spans(text));
    if extended {
        for (start, end) in fallback_ranges(text) {
            let candidate = &text[start..end];
            for (id, grammar) in CUSTOM.iter().chain(LAWS.iter()) {
                if id.ends_with(".short") && !candidate.contains(" at") { continue; }
                spans.extend(grammar.find_spans(candidate).into_iter()
                    .map(|hit| start + hit.start..start + hit.end));
            }
        }
    }
}

pub(crate) fn find(text: &str, extended: bool, native: Option<&mut Vec<Range<usize>>>) -> Vec<Match> {
    let (automaton, indices, unfiltered) = &*FILTER;
    let mut selected: BTreeSet<usize> = unfiltered.iter().copied().collect();
    for matched in automaton.find_overlapping_iter(&compact(text)) {
        selected.extend(indices[matched.pattern().as_usize()].iter().copied());
    }
    let mut found = Vec::new();
    for index in selected {
        let extractor = &DATA.extractors[index];
        if !extended && !extractor.standard { continue; }
        let pattern = extractor.compiled.get_or_init(|| {
            let compiled = if extractor.ecmascript {
                legal_grammar::compile_ecmascript_backtracking_pattern("source extractor", &extractor.pattern, "")
            } else {
                legal_grammar::compile_python_pattern(&extractor.pattern, "")
            };
            compiled.unwrap_or_else(|error| panic!("reporter extractor {index}: {error}"))
        });
        for captures in pattern.captures_iter(text).flatten() {
            if !extended && captures.name("page").is_some_and(|page|
                !page.as_str().chars().all(|c| c.is_ascii_digit() || c == '_')) { continue; }
            let core = captures.name("__citation").expect("source citation span");
            let editions = |indices: &[usize]| {
                let mut editions = Vec::new();
                for index in indices {
                    let edition = &DATA.editions[*index];
                    if !editions.contains(edition) { editions.push(edition.clone()); }
                }
                editions
            };
            let exact_editions = editions(&extractor.exact);
            let variation_editions = editions(&extractor.variations).into_iter()
                .filter(|edition| !exact_editions.contains(edition)).collect();
            let fields = Fields {
                source_groups: pattern.capture_names().flatten()
                    .filter(|name| !name.starts_with("__"))
                    .map(|name| (name.to_owned(), captures.name(name).map(|m| m.as_str().to_owned())))
                    .collect(),
                exact_editions,
                variation_editions,
                ..Fields::default()
            };
            found.push(Match { span: core.start()..core.end(), fields, short: extractor.short, ecmascript: extractor.ecmascript, preceding_text_end: None,
                short_at: captures.name("__short_at").map(|at| at.start() - core.start()) });
        }
    }
    let standard = standard_matches(text, extended);
    if let Some(native) = native {
        native.extend(standard.iter().map(|hit| hit.span.clone()));
        extend_native_spans(text, extended, native);
    }
    found.extend(standard);
    // Preserve the pinned source token when a broader standard form also matches.
    found.sort_by_key(|m| (m.span.start, m.ecmascript, std::cmp::Reverse(m.span.end)));
    let mut kept: Vec<Match> = Vec::new();
    for mut candidate in found {
        if let Some(previous) = kept.last_mut() {
            if previous.merge(&candidate) { continue; }
            if previous.span.end > candidate.span.start {
                if nominative(&previous.fields) {
                    // Tokenizer.tokenize pops the nominative token without
                    // putting its covered text back into the word stream.
                    candidate.preceding_text_end = Some(previous.preceding_text_end.unwrap_or(previous.span.start));
                    kept.pop();
                } else { continue; }
            }
        }
        kept.push(candidate);
    }
    kept
}

/// Eyecite ResourceCitation.guess_edition, after contextual registry resolution.
pub(crate) fn finish(citation: &mut crate::Citation) {
    let current_year = citation.fields.year.as_ref().map(|_| chrono::Utc::now().year());
    citation.fields.year_number = citation.fields.year.as_deref()
        .and_then(|year| crate::text::decimal(year.trim_matches(crate::text::python_whitespace)))
        .and_then(|year| year.to_string().parse::<i32>().ok())
        .filter(|year| (1600..=current_year.unwrap() + 1).contains(year));
    if citation.is_ambiguous() { return; }
    let registry = crate::registry::registry();
    let selected_name = citation.fields.reporter_id.as_deref()
        .and_then(|id| registry.reporters.iter().find(|reporter| reporter.id == id))
        .map(|reporter| reporter.name.en.as_str())
        .or_else(|| {
            if citation.authority != crate::Authority::Journal { return None; }
            let surface = citation.fields.reporter_canonical.as_deref().or(citation.fields.reporter.as_deref())?;
            crate::key::selected_journal(citation, registry, surface)?.name.as_deref()
        });
    let fields = &mut citation.fields;
    let selected_edition = |edition: &&SourceEdition| {
        fields.reporter_canonical.as_deref().is_none_or(|canonical| canonical == edition.short_name)
            && selected_name.is_none_or(|name| name == edition.reporter.name)
    };
    let mut editions: Vec<_> = fields.exact_editions.iter().filter(selected_edition).collect();
    if editions.is_empty() {
        editions = fields.variation_editions.iter().filter(selected_edition).collect();
    }
    if editions.is_empty() { return; }
    let year = fields.year_number;
    let includes_year = |edition: &SourceEdition| {
        if let Some(year) = year.filter(|_| editions.len() > 1) {
            let bound = |value: &Option<String>| value.as_deref().and_then(|v| v.get(..4))
                .and_then(|v| v.parse::<i32>().ok());
            year <= current_year.unwrap() && bound(&edition.start).is_none_or(|start| start <= year)
                && bound(&edition.end).is_none_or(|end| year <= end)
        } else { true }
    };
    let viable: Vec<_> = editions.iter().copied().filter(|edition| includes_year(edition)).collect();
    if let [edition] = viable.as_slice() {
        fields.source_edition = Some((*edition).clone());
    } else {
        // Preserve the original candidates when date filtering yields no
        // unique edition. The ordinary ambiguity guard then applies equally
        // to keys, URLs, parallel grouping and short-reference resolution.
        citation.interpretations.extend(editions.iter().map(|edition| crate::Interpretation {
            kind: "source_edition".into(),
            id: format!("{}:{}:{}:{}:{}", edition.reporter.source, edition.reporter.name,
                edition.short_name, edition.start.as_deref().unwrap_or(""), edition.end.as_deref().unwrap_or("")),
            canonical: edition.short_name.clone(),
            jurisdiction: citation.jurisdiction.clone(),
            selected: false,
            reason: if includes_year(edition) { "alternative" } else { "reporter_period_conflict" }.into(),
        }));
        citation.reasons.push("ambiguous_source_edition".into());
    }
}

/// Build this module's lazily built statics now ([`crate::warm`]).
pub(crate) fn warm() {
    names::warm();
    crate::warm_statics!(DATA, FILTER, CUSTOM, LAWS, SURFACES, CANDIDATE, EDITIONS; screened COMMON, PATTERN, EXTENDED, CUE);
}
