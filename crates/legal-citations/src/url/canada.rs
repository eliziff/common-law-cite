//! CanLII, Decisia, Justice Laws and the provinces' statute pages: Canada's publishers.

use super::*;
use crate::key;
use crate::model::PinpointKind;
use crate::registry::{fold, SeriesKind};
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

fn language_segment(language: Language) -> &'static str {
    match language {
        Language::En => "en",
        Language::Fr => "fr",
    }
}

/// Candidate spellings for an external CanLII TSV index. These
/// are lookup hints, never authority keys or unverified source URLs.
/// `keys` are the citation's [`canlii_citation_keys`]: an index of CanLII's
/// own printed citations, keyed the same way, identifies the instrument exactly.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct LegislationLookup {
    pub candidates: Vec<String>,
    pub jurisdiction: String,
    pub keys: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LookupRules {
    series: std::collections::HashSet<String>,
    regulation_prefixes: HashMap<String, String>,
    series_jurisdictions: HashMap<String, String>,
    /// Groups of key prefixes that name one CanLII series (French and older names).
    key_aliases: Vec<Vec<String>>,
    /// Key prefixes of the statute and regulation series CanLII publishes, by jurisdiction.
    key_jurisdictions: HashMap<String, String>,
    /// The imperial statutes CanLII's constitutional collection holds, by their citation, and its id for each.
    constitutional_documents: HashMap<String, String>,
}

static LOOKUP_RULES: LazyLock<LookupRules> = LazyLock::new(||
    serde_json::from_str(include_str!("../../registry/canlii-index.json")).expect("CanLII index rules"));

/// CanLII's citation identity for a statute or regulation: the printed
/// citation folded to the spelling of CanLII's own legislation citations, so
/// "R.S.O. 1990, c. H.8" and "RSO 1990, c H.8" are both `rso-1990-c-h.8`.
/// Accents, case and abbreviation periods go; a period inside a number ("H.8",
/// "18.1") stays; every other run of spaces and punctuation is one hyphen.
pub fn canlii_citation_key(value: &str) -> String {
    let folded = value.nfkd().filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>().to_lowercase();
    let mut key = String::with_capacity(folded.len());
    let mut gap = false;
    let mut chars = folded.chars().peekable();
    while let Some(c) = chars.next() {
        let number_point = c == '.' && chars.peek().is_some_and(char::is_ascii_digit);
        if c == '.' && !number_point { continue; }
        if c.is_ascii_lowercase() || c.is_ascii_digit() || number_point {
            if gap && !key.is_empty() { key.push('-'); }
            gap = false;
            key.push(c);
        } else {
            gap = true;
        }
    }
    key
}

/// [`canlii_citation_key`] and its equivalent spellings, most specific first:
/// the citation as written, then without a trailing provision pinpoint or
/// amendment note (a CanLII citation can itself end "s 7", for an act enacted
/// by a section of another; Quebec's "r 0.1" and Ontario's "Sch 24" are always
/// identity), the same series under its French or older name (`rlrq-c-` and
/// `cqlr-c-`, `dors-` and `sor-`) and a numbered regulation's two- or
/// four-digit year (O Reg 191/11 and O Reg 191/2011).
pub fn canlii_citation_keys(value: &str) -> Vec<String> {
    static TAIL: LazyLock<Regex> = LazyLock::new(||
        Regex::new(r"(?i)\s*,\s*(?:as am\b.*|(?:ss?|arts?)\.?\s*\d.*)$").expect("citation tail"));
    let mut keys = Vec::new();
    for written in [value.to_owned(), TAIL.replace(value, "").into_owned()] {
        for key in canlii_citation_spellings(&written) {
            if !keys.contains(&key) { keys.push(key); }
        }
    }
    keys
}

fn canlii_citation_spellings(value: &str) -> Vec<String> {
    static LONG_YEAR: LazyLock<Regex> = LazyLock::new(||
        Regex::new(r"^((?:.*-)?(?:reg|sor|si|yoic|nlr|cnlr)-[^-]+-)(?:19|20)(\d{2})$").expect("regulation year"));
    static SHORT_YEAR: LazyLock<Regex> = LazyLock::new(||
        Regex::new(r"^((?:.*-)?(?:reg|sor|si|yoic|nlr|cnlr)-[^-]+-)(\d{2})$").expect("regulation year"));
    let base = canlii_citation_key(value);
    if base.is_empty() { return Vec::new(); }
    let mut keys = vec![base.clone()];
    let push = |keys: &mut Vec<String>, key: String| if !keys.contains(&key) { keys.push(key) };
    for group in &LOOKUP_RULES.key_aliases {
        if let Some(prefix) = group.iter().find(|prefix| base.starts_with(prefix.as_str())) {
            for other in group { push(&mut keys, format!("{other}{}", &base[prefix.len()..])); }
        }
    }
    for key in keys.clone() {
        if let Some(year) = LONG_YEAR.captures(&key) {
            push(&mut keys, format!("{}{}", &year[1], &year[2]));
        } else if let Some(year) = SHORT_YEAR.captures(&key) {
            let century = if year[2].parse::<u32>().unwrap_or(0) > 40 { "19" } else { "20" };
            push(&mut keys, format!("{}{century}{}", &year[1], &year[2]));
        }
    }
    keys
}

pub fn legislation_lookup(value: &str) -> LegislationLookup {
    let rules = &*LOOKUP_RULES;
    static PATTERNS: LazyLock<[legal_grammar::CompiledEcmascriptGrammar; 7]> = LazyLock::new(|| {
        ["statute", "regulation", "instrument", "crc", "jurisdiction-statute", "jurisdiction-instrument", "jurisdiction-crc"]
            .map(|name| legal_grammar::compile_ecmascript_table_entry(&format!("lookup.canlii-legislation.{name}"))
                .expect("legislation index lookup grammar"))
    });
    let series_key = |value: &str| value.chars().filter(char::is_ascii_alphabetic).collect::<String>().to_ascii_lowercase();
    let citation: String = value.nfkc().map(|c| if ('\u{2010}'..='\u{2015}').contains(&c) { '-' } else { c }).collect();
    let mut candidates = Vec::new();
    let mut add = |value: String| {
        let value = value.to_lowercase();
        if !value.is_empty() && !candidates.contains(&value) { candidates.push(value); }
    };
    for matched in PATTERNS[0].captures_iter(&citation) {
        let series = series_key(&matched[1]);
        if !rules.series.contains(&series) { continue; }
        let chapter = matched[3].to_lowercase();
        for chapter in [chapter.clone(), chapter.replace('.', "-"), chapter.replace(['.', '-'], "")] {
            add(format!("{series}-{}-c-{chapter}", &matched[2]));
        }
    }
    if let Some(matched) = PATTERNS[1].captures(&citation) {
        if let Some(prefix) = rules.regulation_prefixes.get(&series_key(&matched[1])) {
            add(format!("{prefix}-reg-{}-{}", &matched[2], &matched[3]));
        }
    }
    if let Some(matched) = PATTERNS[2].captures(&citation) {
        add(format!("{}-{}-{}", &matched[1], &matched[2], &matched[3]));
    }
    if let Some(matched) = PATTERNS[3].captures(&citation) { add(format!("crc-c-{}", &matched[1])); }
    // Most CanLII identifiers are the citation key itself, or it without the
    // period of a chapter number (RSO 1990, c H.8 is rso-1990-c-h8).
    let keys = canlii_citation_keys(value);
    for key in &keys {
        add(key.clone());
        add(key.replace('.', ""));
        add(key.replace('.', "-"));
    }
    // The original jurisdiction lookup uses the written text, without the
    // candidate spelling's NFKC/dash normalization.
    let mut jurisdiction = if let Some(matched) = PATTERNS[4].captures(value) {
        rules.series_jurisdictions.get(&series_key(&matched[1])).cloned().unwrap_or_default()
    } else if let Some(matched) = PATTERNS[1].captures(value) {
        match rules.regulation_prefixes.get(&series_key(&matched[1])).map(String::as_str).unwrap_or("") {
            "alta" => "ab", "o" => "on", "man" => "mb", "sask" => "sk", prefix => prefix,
        }.to_owned()
    } else if PATTERNS[5].is_match(value) || PATTERNS[6].is_match(value) { "ca".to_owned() }
    else { String::new() };
    if jurisdiction.is_empty() {
        jurisdiction = keys.iter().find_map(|key| rules.key_jurisdictions.iter()
            .filter(|(prefix, _)| key.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len()).map(|(_, value)| value.clone()))
            .unwrap_or_default();
    }
    LegislationLookup { candidates, jurisdiction, keys }
}

// ---------------------------------------------------------------------------
// CanLII decisions

/// English neutral codes CanLII also publishes under a French code and
/// database; the French database comes
/// from the registry's `canlii_fr` route, else from this table.
const FRENCH_CODES: [(&str, &str, &str); 5] = [
    ("scc", "csc", "csc"),
    ("fca", "caf", "caf"),
    ("fc", "cf", "cf"),
    ("tcc", "cci", "cci"),
    ("cmac", "cacm", "cacm"),
];

/// The CanLII decision page for a neutral or CanLII citation.
pub fn canlii_case(citation: &Citation, language: Language) -> Option<String> {
    canlii_case_in(citation, language, registry::registry())
        .or_else(|| source_canlii_case(citation, language))
}

pub fn source_canlii_routes() -> &'static HashMap<String, String> {
    &SOURCE_ROUTES.routes
}

#[derive(Default)]
struct SourceRoutes {
    routes: HashMap<String, String>,
    french_routes: HashMap<String, String>,
}

static SOURCE_ROUTES: LazyLock<SourceRoutes> = LazyLock::new(|| {
    let mut result = SourceRoutes::default();
    for court in &registry::registry().courts {
        let Some(route) = &court.canlii else { continue };
        let path = format!("{}/{}", route.jurisdiction, route.database);
        result.routes.entry(route.database.to_ascii_uppercase()).or_insert_with(|| path.clone());
        for (position, code) in court.neutral.iter().enumerate() {
            if code.contains(char::is_whitespace) { continue; }
            let translated = (position % 2 == 1).then_some(court.canlii_fr.as_ref()).flatten();
            let path = translated.map_or_else(|| path.clone(), |route| format!("{}/{}", route.jurisdiction, route.database));
            if translated.is_some() || court.canlii_french {
                result.french_routes.entry(code.to_ascii_uppercase()).or_insert_with(|| path.clone());
            }
            result.routes.entry(code.to_ascii_uppercase()).or_insert(path);
        }
        // Codes CanLII uses in decision ids, which link but do not identify the court.
        for code in &court.canlii_codes {
            result.routes.entry(code.to_ascii_uppercase()).or_insert_with(|| path.clone());
        }
        let french = court.canlii_fr.as_ref().map_or_else(|| path.clone(), |route| format!("{}/{}", route.jurisdiction, route.database));
        for code in &court.canlii_codes_fr {
            result.french_routes.entry(code.to_ascii_uppercase()).or_insert_with(|| french.clone());
            result.routes.entry(code.to_ascii_uppercase()).or_insert_with(|| french.clone());
        }
    }
    result
});

/// A decision number as CanLII writes it in decision ids: without leading zeros
/// (`2009 PSST 0013` is `2009psst13`).
fn id_number(number: &str) -> &str {
    let trimmed = number.trim_start_matches('0');
    if trimmed.is_empty() && !number.is_empty() { "0" } else { trimmed }
}

/// The court whose CanLII-only decision-id codes include `written`, and whether the code is French.
fn canlii_code_court<'r>(registry: &'r Registry, written: &str) -> Option<(&'r registry::Court, bool)> {
    let folded = fold(written);
    registry.courts.iter().find_map(|court| {
        if court.canlii_codes_fr.iter().any(|code| fold(code) == folded) { return Some((court, true)); }
        court.canlii_codes.iter().any(|code| fold(code) == folded).then_some((court, false))
    })
}

pub(crate) fn source_canlii_french_routes() -> &'static HashMap<String, String> {
    &SOURCE_ROUTES.french_routes
}

/// The CanLII URL for a citation: the first written
/// CanLII form precedes the first neutral form, including original abstentions.
pub fn canlii_citation_url(text: &str, language: &str) -> Option<String> {
    static PATTERNS: LazyLock<[legal_grammar::CompiledGrammar; 2]> = LazyLock::new(||
        ["format.canlii-url", "format.case-neutral"].map(|id|
            legal_grammar::compile_ecmascript_backtracking_table_entry(id).unwrap()));
    let text = crate::text::normalize_javascript_whitespace(text);
    let canlii = PATTERNS[0].captures(&text).unwrap();
    let is_canlii = canlii.is_some();
    let found = canlii.or_else(|| PATTERNS[1].captures(&text).unwrap())?;
    let [year, second, third] = [1, 2, 3].map(|index| found.get(index).unwrap().as_str());
    let code = if is_canlii { third } else { second };
    if !is_canlii && ["CANLII", "CARSWELL"].iter().any(|name| code.eq_ignore_ascii_case(name)) { return None; }
    let code = code.to_uppercase().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>();
    let french = language.to_lowercase().starts_with("fr") || SOURCE_ROUTES.french_routes.contains_key(&code);
    let code = if french {
        FRENCH_CODES.iter().find(|(en, _, _)| code.eq_ignore_ascii_case(en))
            .map_or(code.clone(), |(_, fr, _)| fr.to_uppercase())
    } else { code };
    let route = SOURCE_ROUTES.french_routes.get(&code).or_else(|| SOURCE_ROUTES.routes.get(&code))?;
    let (jurisdiction, database) = route.split_once('/')?;
    let slug = if is_canlii { format!("{year}canlii{second}") }
        else { format!("{year}{code}{}", id_number(third)).to_lowercase().chars().filter(char::is_ascii_alphanumeric).collect() };
    Some(page_url(jurisdiction, database, year, &slug, if french { Language::Fr } else { Language::En }))
}

static SCR: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(||
    legal_grammar::compile_python_table_entry("cite.reporter.scr.source-link").unwrap());
/// A case or unreported part citing the SCR prefers the fallback URL.
pub(crate) fn source_prefers_fallback(kind: &str, text: &str) -> bool {
    matches!(kind, "case" | "unreported") && SCR.is_match(text).expect("source SCR citation")
}

static URL: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(||
    legal_grammar::compile_python_table_entry("url.source-candidate").unwrap());
/// Appends the first pinpoint fragment, including the sanitizer's PDF sibling.
/// Source candidates are HTTP(S) URLs or the splitter's bare www/perma/DOI forms.
pub(crate) fn source_first_pinpoint(link: &str, fragments: &[String]) -> String {
    const PUNCT: &str = ".,;:!?)]}>\"'“”’‘";
    let raw = link.trim_matches(crate::text::python_whitespace);
    let candidate = URL.find(raw).expect("source URL").map(|hit| hit.as_str())
        .unwrap_or_else(|| raw.split(crate::text::python_whitespace).next().unwrap_or(""))
        .trim_end_matches(|c| PUNCT.contains(c));
    let (base, fragment) = candidate.split_once('#').unwrap_or((candidate, ""));
    if base.is_empty() || !fragment.is_empty() || !base.to_lowercase().contains("canlii.org") {
        return link.to_owned();
    }
    // urlsplit/urlunsplit preserve escapes and dot segments, lowercase the
    // scheme, and omit an empty query. Bare www/perma/DOI candidates are paths.
    let mut base = base.to_owned();
    let path_start = base.find("://").filter(|&colon|
        base[..colon].eq_ignore_ascii_case("http") || base[..colon].eq_ignore_ascii_case("https"))
        .map(|colon| {
            base[..colon].make_ascii_lowercase();
            let authority = colon + 3;
            base[authority..].find(['/', '?']).map_or(base.len(), |at| authority + at)
        }).unwrap_or(0);
    let mut path = base[path_start..].split('?').next().unwrap().to_lowercase();
    if base.find('?') == Some(base.len() - 1) { base.pop(); }
    if path.ends_with(".pdf") {
        // Substitute at the end of the base, not before a nonempty query.
        if !base.to_lowercase().ends_with(".pdf") { return link.to_owned(); }
        base.truncate(base.len() - 4);
        base.push_str(".html");
        path.truncate(path.len() - 4);
        path.push_str(".html");
    }
    let first = fragments.iter().find_map(|pin| {
        let pin = pin.trim_matches(crate::text::python_whitespace).trim_start_matches('#')
            .trim_matches(|c| PUNCT.contains(c)).chars()
            .filter(|c| !crate::text::python_whitespace(*c)).collect::<String>();
        (!pin.is_empty()).then_some(pin)
    });
    match first {
        Some(pin) if pin.starts_with("par") && path.contains("/doc/")
            || pin.starts_with("sec") && path.contains("/laws/") => format!("{base}#{pin}"),
        _ => link.to_owned(),
    }
}

static PARAGRAPH: LazyLock<Regex> = LazyLock::new(|| {
    let entry = &legal_grammar::load_tables().unwrap()["pinpoint.source-first-paragraph"].entry;
    regex::RegexBuilder::new(&entry.pattern).case_insensitive(true).build().unwrap()
});
static CANLII: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(||
    legal_grammar::compile_python_table_entry("cite.canlii.source-link").unwrap());
/// The offline fallback URL selection order, using the fields
/// already discovered in the source part and the registry's court routes.
pub(crate) fn source_fallback(citations: &[&Citation], source: &crate::source::SourceFields,
    part: &crate::source::SourcePart) -> Option<(usize, String)> {
    if let Some(citation) = citations.iter().find(|citation| citation.format == Some(Format::Neutral)
        && citation.fields.series.as_deref().is_some_and(|court|
            source_canlii_routes().contains_key(&court.to_uppercase()))) {
        let route = &source_canlii_routes()[&citation.fields.series.as_deref()?.to_uppercase()];
        let jurisdiction = route.split('/').next().unwrap();
        let language = if matches!(jurisdiction, "qc" | "nb") { Language::Fr } else { Language::En };
        let mut url = source_canlii_case(citation, language)?;
        if let Some(pin) = source.pinpoint_fragments.first().filter(|pin| pin.starts_with("par")) {
            url.push_str(&format!("#{pin}"));
        } else if let Some(pin) = PARAGRAPH.captures(&source.citation_with_style) {
            url.push_str(&format!("#par{}", &pin[1]));
        }
        return Some((citation.index, url));
    }
    // Search the complete source, including its parenthesized court.
    // Keep its first match and select the core at that original-text offset.
    if let Some(matched) = CANLII.find(&source.citation_with_style).expect("source CanLII citation") {
        // The fields read typeset spaces as spaces; the part's text keeps them.
        let spaced = crate::source::spaced(&part.text);
        let at = spaced.find(&source.citation_with_style)? + matched.start();
        let start = part.start + crate::source::unspaced_offset(&part.text, at);
        if let Some(citation) = citations.iter().find(|citation| citation.format == Some(Format::CanLii)
            && citation.span.start <= start && start < citation.span.end) {
            if let Some(url) = source_canlii_case(citation, Language::En) { return Some((citation.index, url)); }
        }
    }
    if matches!(source.kind, "statute" | "gazette") {
        if let Some(citation) = citations.iter().find(|citation| citation.format == Some(Format::StatuteVolume)
            && citation.fields.series.as_deref().is_some_and(|series| fold(series) == "rsc")) {
            let year = citation.fields.year.as_deref()?;
            let chapter = citation.fields.chapter.as_deref()?.to_lowercase().replace('.', "-");
            let id = format!("rsc-{year}-c-{}", chapter.trim_matches('-'));
            return Some((citation.index, format!("https://www.canlii.org/en/ca/laws/stat/{id}/latest/{id}.html")));
        }
    }
    None
}

fn source_canlii_case(citation: &Citation, language: Language) -> Option<String> {
    if citation.form != Form::Full || citation.is_ambiguous()
        || !matches!(citation.format, Some(Format::Neutral | Format::CanLii)) {
        return None;
    }
    let canadian = |jurisdiction: &str| jurisdiction == "ca" || jurisdiction.starts_with("ca-");
    if citation.jurisdiction.as_deref().is_some_and(|jurisdiction| !canadian(jurisdiction))
        || citation.court.as_ref().is_some_and(|court| key::court(registry::registry(), &court.id)
            .is_none_or(|court| !canadian(&court.jurisdiction))) {
        return None;
    }
    let fields = &citation.fields;
    let year = four_digit_year(fields.year.as_deref()?)?;
    let number = fields.number.as_deref()?;
    if number.is_empty() || !number.chars().all(|character| character.is_ascii_digit()) { return None; }
    let surface = if citation.format == Some(Format::CanLii) {
        citation.court.as_ref()?.text.replace(' ', "")
    } else { fields.series.as_deref()?.trim().to_owned() };
    let written = surface.as_str();
    if written.is_empty() || written.contains(char::is_whitespace) { return None; }
    let route = source_canlii_routes().get(&written.to_ascii_uppercase())?;
    let (jurisdiction, database) = route.split_once('/').unwrap_or(("", route));
    let code = if citation.format == Some(Format::CanLii) { "canlii".to_owned() } else { written.to_ascii_lowercase() };
    let slug = format!("{year}{code}{}", id_number(number));
    Some(page_url(jurisdiction, database, &year, &slug, language))
}

/// [`canlii_case`] against `registry`.
pub fn canlii_case_in(citation: &Citation, language: Language, registry: &Registry) -> Option<String> {
    if citation.form != Form::Full || citation.is_ambiguous() {
        return None;
    }
    let fields = &citation.fields;
    let year = four_digit_year(fields.year.as_deref()?)?;
    let number = fields.number.as_deref().or(fields.page.as_deref())?.trim();
    if number.is_empty() || !number.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let number = id_number(number);
    match citation.format? {
        Format::Neutral => {
            let written = fields.series.as_deref()?.trim();
            // A multi-word code (`Comp Trib`) has no known slug spelling.
            if written.is_empty() || written.contains(char::is_whitespace) {
                return None;
            }
            let court = citation
                .court
                .as_ref()
                .and_then(|court| key::court(registry, &court.id))
                .or_else(|| key::court_by_surface(registry, written));
            // The code as written, lowercased: `SCC-L` → `scc-l`.
            let code = written
                .chars()
                .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
                .collect::<String>()
                .to_ascii_lowercase();
            // A code CanLII uses in the court's decision ids without identifying the court by it
            // (`SSTAD`), or a French one, though it is also a neutral code (`CRTFP`): the court's route,
            // under /fr/ for a French code.
            let neutral_position = court.and_then(|court| court.neutral.iter().position(|value| fold(value) == fold(written)));
            let french_code = court.is_some_and(|court| court.canlii_codes_fr.iter().any(|value| fold(value) == fold(written)));
            if neutral_position.is_none() || french_code {
                let (court, french) = court
                    .and_then(|court| {
                        let folded = fold(written);
                        if court.canlii_codes_fr.iter().any(|value| fold(value) == folded) { return Some((court, true)); }
                        court.canlii_codes.iter().any(|value| fold(value) == folded).then_some((court, false))
                    })
                    .or_else(|| canlii_code_court(registry, written))?;
                if !(court.jurisdiction == "ca" || court.jurisdiction.starts_with("ca-")) {
                    return None;
                }
                let route = if french { court.canlii_fr.as_ref().or(court.canlii.as_ref())? } else { court.canlii.as_ref()? };
                return Some(page_url(
                    &route.jurisdiction,
                    &route.database,
                    &year,
                    &format!("{year}{code}{number}"),
                    if french { Language::Fr } else { language },
                ));
            }
            let court = court?;
            if !(court.jurisdiction == "ca" || court.jurisdiction.starts_with("ca-")) {
                return None;
            }
            // The written code must be one of the court's own neutral codes.
            let position = neutral_position?;
            // Neutral codes are listed English first, then French (`SCC`, `CSC`).
            if position % 2 == 1 {
                if let Some(route) = &court.canlii_fr {
                    return Some(page_url(
                        &route.jurisdiction,
                        &route.database,
                        &year,
                        &format!("{year}{code}{number}"),
                        Language::Fr,
                    ));
                }
            }
            let route = court.canlii.as_ref()?;
            if language == Language::Fr {
                if let Some((_, french_code, french_database)) =
                    FRENCH_CODES.iter().find(|(english, _, _)| *english == code)
                {
                    let (jurisdiction, database) = court.canlii_fr.as_ref().map_or_else(
                        || (route.jurisdiction.clone(), (*french_database).to_owned()),
                        |french| (french.jurisdiction.clone(), french.database.clone()),
                    );
                    return Some(page_url(
                        &jurisdiction,
                        &database,
                        &year,
                        &format!("{year}{french_code}{number}"),
                        Language::Fr,
                    ));
                }
            }
            Some(page_url(
                &route.jurisdiction,
                &route.database,
                &year,
                &format!("{year}{code}{number}"),
                language,
            ))
        }
        Format::CanLii => {
            let court = citation.court.as_ref().and_then(|court| key::court(registry, &court.id))?;
            let route = court.canlii.as_ref()?;
            let slug = format!("{year}canlii{number}");
            Some(page_url(&route.jurisdiction, &route.database, &year, &slug, language))
        }
        _ => None,
    }
}

/// A CanLII alias-index target: a citation or jurisdiction/database/caseId.
/// The path is evidence supplied by the owning CanLII index, not inferred from
/// a case name. Keep the original slug while selecting the published language.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct AliasTargetInfo {
    pub url: String,
    pub year: String,
    pub court_id: Option<String>,
}

fn citation_alias_target_info(citation: &Citation, language: Language) -> Option<AliasTargetInfo> {
    Some(AliasTargetInfo {
        url: canlii_case(citation, language)?,
        year: four_digit_year(citation.fields.year.as_deref()?)?,
        court_id: citation.court.as_ref().map(|court| court.id.clone()),
    })
}

pub fn canlii_alias_target_info(target: &str, language: Language) -> Option<AliasTargetInfo> {
    static PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:([a-z]{2})/)?([A-Za-z0-9-]+)/((\d{4})[a-z0-9]+)$").unwrap());
    let value = target.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some(path) = PATH.captures(&value) else {
        // A CanLII alias target carries Canadian source provenance. Resolve an
        // observed parallel to its canonical citation before reading route metadata.
        let options = crate::Options { resolve: false, parallel: false,
            jurisdiction_priority: vec!["ca".to_owned()], ..Default::default() };
        return crate::extract(&value, &options).iter().find_map(|citation| {
            if let Some(target) = crate::aliases::resolve(citation) {
                let canonical = crate::extract(&target.citation, &options);
                if let [only] = canonical.as_slice() {
                    if let Some(info) = citation_alias_target_info(only, language) { return Some(info); }
                }
            }
            citation_alias_target_info(citation, language)
        });
    };
    let jurisdiction = path.get(1).map_or("", |part| part.as_str());
    let court = registry::registry().courts.iter().find(|court| court.canlii.iter().chain(court.canlii_fr.iter())
        .any(|route| route.jurisdiction == jurisdiction && route.database.eq_ignore_ascii_case(&path[2])));
    let route = court.and_then(|court| if language == Language::Fr { court.canlii_fr.as_ref().or(court.canlii.as_ref()) } else { court.canlii.as_ref() });
    Some(AliasTargetInfo {
        url: page_url(route.map_or(jurisdiction, |route| route.jurisdiction.as_str()),
        route.map_or(&path[2], |route| route.database.as_str()), &path[4], &path[3],
        match court { Some(court) if court.canlii_french => Language::Fr, Some(_) => language, None => Language::En }),
        year: path[4].to_owned(),
        court_id: court.map(|court| court.id.clone()),
    })
}

pub fn canlii_alias_target(target: &str, language: Language) -> Option<String> {
    canlii_alias_target_info(target, language).map(|info| info.url)
}

fn page_url(jurisdiction: &str, database: &str, year: &str, slug: &str, language: Language) -> String {
    let route = if jurisdiction.is_empty() {
        database.to_owned()
    } else {
        format!("{jurisdiction}/{database}")
    };
    format!(
        "https://www.canlii.org/{}/{route}/doc/{year}/{slug}/{slug}.html",
        language_segment(language)
    )
}

// ---------------------------------------------------------------------------
// CanLII search, noteup and anchors

/// Which CanLII search box a query goes to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchField {
    /// Document text (`text=`).
    Text,
    /// Case name, title or citation (`id=`).
    Id,
}

/// `https://www.canlii.org/en/#search/text=…`; `None` for an empty query or
/// one over 4,096 characters.
pub fn canlii_search(query: &str, field: SearchField) -> Option<String> {
    let value = query.trim();
    if value.is_empty() || query.chars().count() > 4096 {
        return None;
    }
    let name = match field {
        SearchField::Text => "text",
        SearchField::Id => "id",
    };
    Some(format!("https://www.canlii.org/en/#search/{name}={}", encode_component(value)))
}

/// A CanLII noteup of the decision `citation` resolves to, keeping `query` as
/// the typed text: `…#search/origin1={path}&nquery1={query}`.
pub fn canlii_noteup(citation: &Citation, query: &str) -> Option<String> {
    let value = query.trim();
    if value.is_empty() || query.chars().count() > 4096 {
        return None;
    }
    let cited = canlii_case(citation, Language::En)?;
    let path = cited.strip_prefix("https://www.canlii.org")?;
    Some(format!(
        "https://www.canlii.org/en/#search/origin1={}&nquery1={}",
        encode_component(path),
        encode_component(value)
    ))
}

/// `page_url` with the anchor for `citation`'s first pinpoint, when there is exactly one pinpoint and
/// it has an anchor ([`super::source_anchor`]; a page as a text fragment for its `[page 5]` marker).
pub fn with_pinpoint(page_url: &str, citation: &Citation) -> String {
    match citation.pinpoints.as_slice() {
        [only] if only.last.is_none() && !page_url.contains('#') => {
            let value = crate::text::normalize_javascript_whitespace(&only.first);
            let anchor = match only.kind {
                PinpointKind::Page => Some(format!(":~:text={}", encode_component(&format!("[page {value}]")))),
                PinpointKind::Paragraph => super::source_anchor(page_url, "paragraph", &format!("par{value}")),
                PinpointKind::Section | PinpointKind::Subsection => super::source_anchor_within(page_url, "section", &value, Some(1)),
                PinpointKind::Article => super::source_anchor_within(page_url, "article", &value, Some(1)),
                PinpointKind::Rule => super::source_anchor_within(page_url, "rule", &value, Some(1)),
                _ => None,
            };
            anchor.map_or_else(|| page_url.to_owned(), |anchor| format!("{page_url}#{anchor}"))
        }
        _ => page_url.to_owned(),
    }
}

/// The hostname of an absolute URL, lowercased, without a trailing root dot
/// (the WHATWG parser's answer for the inputs the apps see).
fn hostname(value: &str) -> Option<String> {
    Some(url::Url::parse(value).ok()?.host_str().unwrap_or("")
        .to_lowercase().trim_end_matches('.').to_owned())
}

/// Whether `value` is a URL on CanLII (`canlii.org`, `canlii.ca` or any
/// subdomain). A value that is not a URL is not CanLII.
pub fn is_canlii_url(value: &str) -> bool {
    hostname(value).is_some_and(|host| {
        ["canlii.ca", "canlii.org"]
            .iter()
            .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
    })
}

/// The PDF sibling of a canonical CanLII decision page, and nothing else.
pub fn canlii_pdf_url(page_url: &str) -> Option<String> {
    static PATH: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^/(?:en|fr)/(?:[A-Za-z0-9-]+/){1,2}doc/(\d{4})/([a-z0-9-]+)/([a-z0-9-]+)\.html$").unwrap()
    });
    // Parse as a WHATWG URL before these checks.
    let mut parsed = url::Url::parse(page_url).ok()?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("www.canlii.org")
        || parsed.port().is_some() || !parsed.username().is_empty()
        || !parsed.password().unwrap_or("").is_empty()
        || !parsed.query().unwrap_or("").is_empty() || !parsed.fragment().unwrap_or("").is_empty() {
        return None;
    }
    let path = parsed.path();
    let captures = PATH.captures(path)?;
    if captures[2] != captures[3] || !captures[2].starts_with(&captures[1]) {
        return None;
    }
    let path = format!("{}.pdf", &path[..path.len() - ".html".len()]);
    parsed.set_path(&path);
    Some(parsed.into())
}

/// Whether a publisher's page may print a legal reference inside the text it quotes: BC Laws annotates them, and the
/// Supreme Court's site links each one with an icon and a no-break space after it (334 of 474 cached pages, 2026-10-18).
pub fn publisher_annotates_legal_references(address: &str) -> bool {
    url::Url::options().base_url(Some(&url::Url::parse("http://mike.local").unwrap())).parse(address)
        .is_ok_and(|url| matches!(url.host_str(), Some("www.bclaws.gov.bc.ca" | "decisions.scc-csc.ca")))
}

/// Whether a publisher's page may print a passage again where the document holds it once: CanLII prepends a reporter's
/// headnote to a reported decision and sets "[See Note n below]" lines, so a quotation's words recur before their
/// paragraph (measured 2026-10-18: a bare quotation painted the headnote's copy on 14 cached pages). A court's or
/// legislature's own page is the document.
pub fn publisher_may_repeat_passages(address: &str) -> bool {
    url::Url::options().base_url(Some(&url::Url::parse("http://mike.local").unwrap())).parse(address)
        .is_ok_and(|url| url.host_str().is_some_and(|host| host == "canlii.org" || host.ends_with(".canlii.org")))
}

/// Whether a publisher's page prints a section's number against its first word with nothing between ("57An
/// application"): New Brunswick's does, so a browser finds no word boundary before that word (measured 2026-10-18:
/// 16 of 29 section openings painted nothing there, every subsection's "(1)" opening painted).
pub fn publisher_glues_section_labels(address: &str) -> bool {
    url::Url::options().base_url(Some(&url::Url::parse("http://mike.local").unwrap())).parse(address)
        .is_ok_and(|url| matches!(url.host_str(), Some("laws.gnb.ca")))
}

/// A2AJ Law Web's reader of a document (`https://law.a2aj.ca/document?citation=…&doc_type=…`), which draws the
/// A2AJ text itself.
pub fn law_web_document(citation: &str, doc_type: &str) -> String {
    let mut url = url::Url::parse("https://law.a2aj.ca/document").unwrap();
    url.query_pairs_mut().append_pair("citation", citation).append_pair("doc_type", doc_type);
    url.into()
}

pub(super) fn on_canlii_org(url: &url::Url) -> bool {
    url.host_str().is_some_and(|host| host == "canlii.org" || host.ends_with(".canlii.org"))
}

/// A Decisia (Lexum Norma) decision page: SCC, FCA, FC, TCC, ONCA, NSCA, the tribunals and
/// decisia.lexum.com's tenants, `…/item/N/index.do`. Its plain address is an iframe shell with no
/// text and no anchors; `iframe=true` serves the decision inline with `parN` anchors (live-verified
/// on 14 hosts, 2026-07-27). Known by host and path alone: some older decisions have no anchors.
pub(super) fn is_decisia_document(url: &url::Url) -> bool {
    static HOST: LazyLock<Regex> = LazyLock::new(|| Regex::new(
        r"(?i)^(?:decisions?\.[A-Za-z0-9_-]+\.(?:gc\.)?ca|decisia\.lexum\.com|coadecisions\.ontariocourts\.ca)$").unwrap());
    static ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)/item/[0-9]+/index\.do$").unwrap());
    url.host_str().is_some_and(|host| HOST.is_match(host)) && ITEM.is_match(url.path())
}

/// A statute's page where its address is the publisher's machine-readable copy (BC Laws `…/xml`, Justice Laws
/// `/eng/XML/C-46.xml`, Ontario e-Laws' API): a browser shows that copy as data, where no paragraph anchor or text
/// fragment lands, so the link opens the page that renders the same statute.
pub(super) fn statute_page(url: &mut url::Url) {
    static JUSTICE_XML: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^/(eng|fra)/XML/([^/]+)\.xml$").unwrap());
    static REGULATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:SOR|SI|C\.?R\.?C\.?|DORS|TR)\b").unwrap());
    static ELAWS_API: LazyLock<Regex> = LazyLock::new(||
        Regex::new(r"(?i)^/laws/api/v2/legislation/en/doc-search/(statute|regulation)/([^/]+)$").unwrap());
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    let path = url.path().to_owned();
    if (host == "bclaws.gov.bc.ca" || host.ends_with(".bclaws.gov.bc.ca")) && path.ends_with("/xml") {
        url.set_path(&path[..path.len() - "/xml".len()]);
    } else if host == "laws-lois.justice.gc.ca" {
        if let Some(found) = JUSTICE_XML.captures(&path) {
            let language = found[1].to_ascii_lowercase();
            let id = &found[2];
            let collection = match (language.as_str(), REGULATION.is_match(id)) {
                ("fra", true) => "reglements",
                ("fra", false) => "lois",
                (_, true) => "regulations",
                (_, false) => "acts",
            };
            url.set_path(&format!("/{language}/{collection}/{id}/FullText.html"));
        }
    } else if host == "ontario.ca" || host == "www.ontario.ca" {
        if let Some(found) = ELAWS_API.captures(&path) {
            let path = format!("/laws/{}/{}", found[1].to_ascii_lowercase(), &found[2]);
            let _ = url.set_host(Some("www.ontario.ca"));
            url.set_path(&path);
        }
    }
}

/// The id a federal or provincial statute page prints for a provision (`5`, `5(2)`, `sec5subsec2`), at the deepest
/// level that publisher's ids reach, read from the publishers' pages (experiments/text-fragment-fidelity/
/// provider-anchors.md). Each id sits where its unit's text starts and is unique to it, except where a page prints the
/// sections of its amending instruments after its own (Justice Laws `s-1` repeats there; the first is the text's).
///
/// * Justice Laws, `…/eng/acts/C-46/FullText.html`: the section, `s-5`, `s-5.01`; no id for a subsection.
/// * BC Laws, `…/complete/statreg/…`: the section, `section5`, `section7.1`; its subsection ids are opaque.
/// * Manitoba, `web2.gov.mb.ca/laws/…`: the section `5` or subsection `5(2)`, `5.3(4)`; none for a paragraph.
/// * New Brunswick, `laws.gnb.ca/en/document/…`: the whole chain, `se:5`, `se:5-ss:2`, `se:5-ss:2-p1:b-p2:ii`, a
///   decimal's point an underscore (`se:36_1`).
/// * Newfoundland and Labrador, `assembly.nl.ca/Legislation/sr/…`: the section, `5_`, `6_1`, ahead of its heading.
///
/// Ontario e-Laws prints `s5` and `s5s2` on about one page in ten and an ordinal bookmark (`BK26`) on the rest;
/// Alberta's King's Printer, Nova Scotia's and the PDF sites print none.
pub(super) fn statute_anchor(url: &url::Url, label: &str) -> Option<String> {
    let value = match label.get(..3) {
        Some(prefix) if prefix.eq_ignore_ascii_case("sec") => match label[3..].split_once("subsec") {
            Some((section, subsection)) => format!("{section}({subsection})"),
            None => label[3..].to_owned(),
        },
        _ => label.to_owned(),
    };
    let provision = crate::format::parse_locator(&value)?;
    let (root, parts) = (provision.root.as_str(), &provision.suffixes);
    let host = url.host_str()?.to_ascii_lowercase();
    let path = url.path().to_ascii_lowercase();
    let subsection = parts.first().filter(|part| part.starts_with(|digit: char| digit.is_ascii_digit())
        && part.chars().all(|digit| digit.is_ascii_digit() || digit == '.'));
    match host.as_str() {
        "laws-lois.justice.gc.ca" => (path.starts_with("/eng/") && path.ends_with("/fulltext.html"))
            .then(|| format!("s-{root}")),
        "www.bclaws.gov.bc.ca" => path.starts_with("/civix/document/id/complete/statreg/")
            .then(|| format!("section{root}")),
        "web2.gov.mb.ca" => (path.starts_with("/laws/statutes/") || path.starts_with("/laws/regs/"))
            .then(|| subsection.map_or_else(|| root.to_owned(), |part| format!("{root}({part})"))),
        "laws.gnb.ca" => path.starts_with("/en/document/").then(|| {
            let mut anchor = format!("se:{}", root.replace('.', "_"));
            let mut levels = ["p1", "p2", "c1"].into_iter();
            for (index, part) in parts.iter().enumerate() {
                let level = if index == 0 && subsection.is_some() { Some("ss") } else { levels.next() };
                match level {
                    Some(level) if part.chars().all(|character| character.is_ascii_alphanumeric() || character == '.') =>
                        anchor.push_str(&format!("-{level}:{}", part.replace('.', "_"))),
                    _ => break,
                }
            }
            anchor
        }),
        "www.assembly.nl.ca" => path.starts_with("/legislation/sr/")
            .then(|| if root.contains('.') { root.replace('.', "_") } else { format!("{root}_") }),
        _ => None,
    }
}

/// A CanLII decision or statute page at a citation's first pinpoint when that is a paragraph or a
/// section (`par12`, `sec4`, as [`crate::source::SourceFields`] names them), as the ALR Quote
/// Verifier anchors a citation's link. Any other address, a PDF, or one with an anchor stays as is.
pub fn canlii_pinpoint_url(page_url: &str, pinpoints: &[String]) -> String {
    let first = pinpoints.first().map_or("", String::as_str);
    let Ok(mut url) = url::Url::parse(page_url) else { return page_url.to_owned() };
    if !on_canlii_org(&url) || !url.fragment().unwrap_or("").is_empty() || url.path().to_lowercase().ends_with(".pdf")
        || !(first.starts_with("par") || first.starts_with("sec")) {
        return page_url.to_owned();
    }
    let Some(anchor) = source_anchor(page_url, if first.starts_with("par") { "paragraph" } else { "section" }, first)
        else { return page_url.to_owned() };
    url.set_fragment(Some(&anchor));
    url.into()
}

/// A CanLII PDF link as its page, its anchor kept, unless the anchor names a PDF page (paragraph
/// and section anchors are the page's): alr_quote_verifier.py `_canlii_pdf_to_html_sibling`.
pub fn canlii_pdf_page(link: &str) -> String {
    static CANLII: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^https?://(?:[^/]+\.)?canlii\.org/").unwrap());
    static PDF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\.pdf$").unwrap());
    static PDF_PAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^page=-?[0-9]+$").unwrap());
    let (base, anchor) = link.split_once('#').map_or((link, None), |(base, anchor)| (base, Some(anchor)));
    if !CANLII.is_match(base) || !PDF.is_match(base)
        || PDF_PAGE.is_match(crate::text::trim_javascript_whitespace(anchor.unwrap_or(""))) {
        return link.to_owned();
    }
    format!("{}.html{}", &base[..base.len() - ".pdf".len()], anchor.map(|anchor| format!("#{anchor}")).unwrap_or_default())
}

/// The statute or regulation a CanLII legislation page is of, at any version
/// (`/en/on/laws/stat/rso-1990-c-c43/latest/…`).
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CanliiLawPage {
    pub language: String,
    /// CanLII's jurisdiction segment: `ca`, `on`, `bc`, ….
    pub jurisdiction: String,
    /// `stat`, `astat`, `regu`, ….
    pub kind: String,
    /// The statute's or regulation's id: `rso-1990-c-c43`.
    pub id: String,
}

pub fn canlii_law_page(page_url: &str) -> Option<CanliiLawPage> {
    static LAW: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^/(en|fr)/([a-z]+)/(?:laws|legis)/([a-z]+)/([^/]+)/").unwrap());
    if !is_canlii_url(page_url) {
        return None;
    }
    let url = url::Url::parse(page_url).ok()?;
    let law = LAW.captures(url.path())?;
    // The French interface names the kinds in French (lois, loisa, regl).
    let kind = match &law[3] { "lois" => "stat", "loisa" => "astat", "regl" => "regu", kind => kind };
    // An id as CanLII writes it, not as an address escapes it (`c-c%E2%80%9113.2` is `c-c\u{2011}13.2`).
    let id = law[4].as_bytes();
    let mut bytes = Vec::with_capacity(id.len());
    let mut at = 0;
    while at < id.len() {
        let hex = |at: usize| id.get(at).and_then(|byte| (*byte as char).to_digit(16));
        match (id[at], hex(at + 1), hex(at + 2)) {
            (b'%', Some(high), Some(low)) => { bytes.push((high * 16 + low) as u8); at += 3; }
            (byte, _, _) => { bytes.push(byte); at += 1; }
        }
    }
    Some(CanliiLawPage { language: law[1].to_owned(), jurisdiction: law[2].to_owned(), kind: kind.to_owned(),
        id: String::from_utf8_lossy(&bytes).into_owned() })
}

// ---------------------------------------------------------------------------
// Legislation

/// The CanLII id and path segment (`stat`, `astat`, `regu`) of a statute or
/// regulation whose registry series CanLII publishes; its French page's id (the citation written in French slugged,
/// `lrc-1985-c-c-46`) where `french`.
fn canlii_legislation_id(citation: &Citation, registry: &Registry, french: bool)
    -> Option<(String, String, &'static str)> {
    let fields = &citation.fields;
    let series = key::selected_series(citation, registry, fields.series.as_deref()?)?;
    let route = series.canlii.as_ref()?;
    // A regulation is in CanLII's regulations whatever its series' own database (CQLR c C-26, r 1).
    let path = if citation.format == Some(Format::RegulationSeries) { "regu" } else { match route.database.as_str() {
        "stat" => "stat",
        "astat" => "astat",
        "regu" => "regu",
        "hstat" => "hstat",
        database if database.len() == 3 => match database.as_bytes()[2] {
            b's' => "stat",
            b'a' => "astat",
            b'r' => "regu",
            b'h' => "hstat",
            _ => return None,
        },
        _ => match series.kind {
            SeriesKind::RevisedStatutes => "stat",
            SeriesKind::AnnualStatutes => "astat",
            SeriesKind::Regulations => "regu",
            SeriesKind::Code => return None,
        },
    } };
    let year = fields.year.as_deref().unwrap_or("");
    let path = match path {
        // A series CanLII files statutes' schedules among its regulations (Newfoundland and Labrador's).
        _ if fields.schedule.is_some() && route.schedules.as_deref() == Some("regu") => "regu",
        "astat" if fields.schedule.is_some() || route.as_enacted_from.as_deref().is_some_and(|from| year < from) => "stat",
        "stat" if route.historical_before.as_deref().is_some_and(|before| !year.is_empty() && year < before) => "hstat",
        path => path,
    };
    // CanLII files some series under another's name (SOR (NT) under SOR); a French page by its French name.
    let series_id = match fields.series.as_deref() {
        Some(written) if french => canlii_slug(written),
        _ => canlii_slug(route.series.as_deref().unwrap_or(&series.abbreviation)),
    };
    let id = match citation.format? {
        Format::StatuteVolume => {
            let chapter = chapter_value(fields.chapter.as_deref()?)?;
            // Ontario's 1990 consolidation numbers a chapter with a dot (c D.16, id d16); it is often written c D-16.
            let chapter = if series_id == "rso" && fields.year.as_deref() == Some("1990") {
                chapter.replace('-', ".")
            } else { chapter };
            // A revision whose own chapters are lettered numbers its one supplement's (RSA 1980, c 1 is its Supp's).
            let chapter = match route.numbered_supplement.as_deref() {
                Some(year) if fields.year.as_deref() == Some(year) && fields.volume.is_none()
                    && chapter.chars().all(|character| character.is_ascii_digit()) => format!("{chapter} (Supp)"),
                _ => chapter,
            };
            let mut id = match fields.volume.as_deref().or(fields.year.as_deref()) {
                Some(volume) => format!("{series_id}-{}-c-{}", canlii_slug(volume), canlii_slug(&chapter)),
                None => format!("{series_id}-c-{}", canlii_slug(&chapter)),
            };
            if let Some(schedule) = fields.schedule.as_deref() {
                id.push_str("-sch");
                if !schedule.is_empty() {
                    id = format!("{id}-{}", canlii_slug(schedule));
                }
            }
            id
        }
        Format::RegulationSeries => {
            let regulation = fields.regulation.as_deref();
            if series_id == "goq" {
                // A regulation by its Gazette officielle page: 2005 GOQ 2, 1581 [2].
                format!("{}-goq-{}", fields.year.as_deref()?, canlii_slug(regulation?))
            } else if let Some(chapter) = fields.chapter.as_deref() {
                // A chapter's regulation: CQLR c T-15.01, r 4; RRQ 1981, c M-20, r 1; RRS c E-9.1 Reg 2.
                let chapter = canlii_slug(&chapter_value(chapter)?);
                // The revision's year written in the citation (RRQ 1981), never a date after it (CQLR c C-26, r 1 (2015)).
                let year = fields.year.as_deref().filter(|year| citation.span.text.contains(year))
                    .map(|year| format!("-{year}")).unwrap_or_default();
                let label = if series_id == "rrs" { "reg" } else { "r" };
                match regulation {
                    Some(regulation) => format!("{series_id}{year}-c-{chapter}-{label}-{}", canlii_slug(regulation)),
                    None => format!("{series_id}{year}-c-{chapter}"),
                }
            } else if let (Some(year), Some(number)) = (fields.year.as_deref().filter(|year| citation.span.text.contains(year)),
                regulation.filter(|number| !number.contains(['/', '-'])))
            {
                // A revision's regulation: RRO 1990, Reg 194.
                format!("{series_id}-{year}-reg-{}", canlii_slug(number))
            } else {
                // A numbered regulation in its written order (NB Reg 2009-140, NWT Reg 085-95, YOIC 2020/68), the year
                // after a slash written as CanLII writes the series' years (Alta Reg 115/93 -> alta-reg-115-1993).
                let written = regulation?;
                let written = match written.split_once('/') {
                    Some((number, after)) => {
                        let digits = after.find(|character: char| !character.is_ascii_digit()).unwrap_or(after.len());
                        format!("{number}/{}{}", canlii_year(&after[..digits], route.year.as_deref())?, &after[digits..])
                    }
                    None => written.to_owned(),
                };
                format!("{series_id}-{}", canlii_slug(&written))
            }
        }
        _ => return None,
    };
    if id.is_empty() || id.contains("--") {
        return None;
    }
    Some((route.jurisdiction.clone(), id, path))
}

/// CanLII's id for a citation as CanLII writes it: accents folded, lowercased, parentheses, brackets, commas and
/// apostrophes dropped, a dot closing a letter dropped (`c A.1` -> `c-a1`, `Reg.` -> `reg`), a non-breaking hyphen
/// kept, and every other mark a dash, a run of spaces one (`SS 1980-81, c E-0.02` -> `ss-1980-81-c-e-0.02`, `30 & 31
/// Vict` -> `30---31-vict`). Measured against CanLII's legislation list (2026-10-23).
pub fn canlii_slug(citation: &str) -> String {
    let characters = citation.nfd().filter(|character| !unicode_normalization::char::is_combining_mark(*character))
        .flat_map(char::to_lowercase).filter(|character| !"()[],'\u{2019}".contains(*character)).collect::<Vec<_>>();
    let mut id = String::with_capacity(characters.len());
    for (at, &character) in characters.iter().enumerate() {
        let closes_letter = character == '.' && at > 0 && characters[at - 1].is_alphabetic();
        if character.is_alphanumeric() || character == '\u{2011}' || character == '.' && !closes_letter {
            id.push(character);
        } else if character.is_whitespace() {
            // A run of spaces is one separator; each other mark is its own (`30 & 31 Vict` -> `30---31-vict`).
            if !characters[..at].last().is_some_and(|previous| previous.is_whitespace()) {
                id.push('-');
            }
        } else if !closes_letter {
            id.push('-');
        }
    }
    id.trim_matches(['-', '.']).to_owned()
}

/// A regulation's year (`93`, `1993`) as CanLII writes it for a series: `four` digits, `two`, as `written`, or by
/// default two before 2000 and four from 2000 (measured per series against CanLII's legislation list, 2026-10-23).
/// Two digits up to 30 are this century's: CanLII's regulations written so reach back to the 1930s.
fn canlii_year(year: &str, form: Option<&str>) -> Option<String> {
    if form == Some("written") {
        return Some(year.to_owned());
    }
    let full = match year.len() {
        2 => format!("{}{year}", if year <= "30" { "20" } else { "19" }),
        4 => year.to_owned(),
        _ => return None,
    };
    Some(match form {
        Some("written") => year.to_owned(),
        Some("four") => full,
        Some("two") => full[2..].to_owned(),
        _ if full.as_str() < "2000" => full[2..].to_owned(),
        _ => full,
    })
}

/// `(year as written, number)` of a numbered regulation.
fn regulation_parts(citation: &Citation) -> Option<(String, String)> {
    let fields = &citation.fields;
    let written = fields.regulation.as_deref().or(fields.number.as_deref())?.trim();
    let digits = |part: &str| !part.is_empty() && part.chars().all(|character| character.is_ascii_digit());
    if let Some((number, year)) = written.split_once('/') {
        if digits(number.trim()) && digits(year.trim()) {
            return Some((year.trim().to_owned(), number.trim().to_owned()));
        }
    }
    if let Some((year, number)) = written.split_once('-') {
        if digits(year.trim()) && digits(number.trim()) {
            return Some((year.trim().to_owned(), number.trim().to_owned()));
        }
    }
    let year = fields.year.as_deref()?.trim();
    (digits(written) && digits(year)).then(|| (year.to_owned(), written.to_owned()))
}

/// The CanLII page of a statute or regulation whose series CanLII publishes:
/// `https://www.canlii.org/en/ca/laws/stat/rsc-1985-c-c-46/latest/rsc-1985-c-c-46.html`.
pub fn canlii_legislation(citation: &Citation, language: Language) -> Option<String> {
    canlii_legislation_in(citation, language, registry::registry())
}

/// The CanLII page of a consolidated statute's own citation, as an A2AJ legislation record carries it: an annual act at
/// its consolidation (`/stat/`), not as enacted, and the sections after its chapter part of what it is (`SC 2018, c 27,
/// s 675` is the act that section enacted, `sc-2018-c-27-s-675`).
pub fn canlii_consolidated_legislation(citation: &Citation, language: Language) -> Option<String> {
    if citation.form != Form::Full {
        return None;
    }
    let (jurisdiction, id, path, french) = canlii_legislation_page(citation, registry::registry())?;
    let path = if path == "astat" { "stat" } else { path };
    let after = citation.full_span.text.get(citation.span.end.saturating_sub(citation.full_span.start)..)
        .filter(|_| citation.format == Some(Format::StatuteVolume)).unwrap_or("");
    let id = match canlii_slug(after) {
        sections if sections.is_empty() => id,
        sections => format!("{id}-{sections}"),
    };
    Some(canlii_law_address(&jurisdiction, &id, path, french, language))
}

/// [`canlii_legislation`] against `registry`.
pub fn canlii_legislation_in(citation: &Citation, language: Language, registry: &Registry) -> Option<String> {
    if citation.form != Form::Full {
        return None;
    }
    if let Some(page) = canlii_constitutional_page(citation, language) {
        return Some(page);
    }
    let (jurisdiction, id, path, french) = canlii_legislation_page(citation, registry)?;
    if jurisdiction.is_empty() {
        return None;
    }
    Some(canlii_law_address(&jurisdiction, &id, path, french, language))
}

/// The CanLII page of an imperial statute CanLII's constitutional collection holds, which the citation names by its
/// regnal year or its United Kingdom citation (`30 & 31 Vict, c 3`; `… Schedule B to the Canada Act 1982 (UK), 1982,
/// c 11`).
pub fn canlii_constitutional_page(citation: &Citation, language: Language) -> Option<String> {
    if citation.form != Form::Full {
        return None;
    }
    canlii_constitutional_document(citation).map(|id| canlii_law_address("ca", id, "stat", false, language))
}

fn canlii_constitutional_document(citation: &Citation) -> Option<&'static str> {
    if citation.fields.series.is_some() {
        return None;
    }
    let written = canlii_citation_key(&citation.full_span.text);
    LOOKUP_RULES.constitutional_documents.iter().find_map(|(document, id)| {
        let key = canlii_citation_key(document);
        written.match_indices(&key).any(|(at, _)| (at == 0 || written[..at].ends_with('-'))
            && written[at + key.len()..].chars().next().is_none_or(|next| next == '-')).then_some(id.as_str())
    })
}

/// A citation's CanLII page: its French page where it is written in French (`LRC 1985, c C-46`, `RLRQ c C-12`,
/// `DORS/98-106` are not the series' English names), as a decision cited by its French neutral identifier is; else
/// its English page. CanLII's French interface holds no historical statutes, which stay on the English page.
fn canlii_legislation_page(citation: &Citation, registry: &Registry) -> Option<(String, String, &'static str, bool)> {
    let french = citation.language.as_deref() == Some("fr") && citation.fields.series.as_deref().is_some_and(|written|
        key::selected_series(citation, registry, written).is_some_and(|series| fold(written) != fold(&series.abbreviation)));
    match canlii_legislation_id(citation, registry, french)? {
        (_, _, "hstat") if french => canlii_legislation_id(citation, registry, false).map(|(j, id, path)| (j, id, path, false)),
        (jurisdiction, id, path) => Some((jurisdiction, id, path, french)),
    }
}

/// CanLII's address of a statute or regulation page: the English interface's (`/en/on/laws/stat/…/latest/`), whatever
/// the reader's language, which CanLII has for every statute; or, for a citation written in French, the French
/// interface's (`/fr/on/legis/lois/lro-1990-c-c43/derniere/`), which names the kinds in French.
fn canlii_law_address(jurisdiction: &str, id: &str, path: &str, french: bool, _language: Language) -> String {
    if french {
        let kind = match path { "stat" => "lois", "astat" => "loisa", _ => "regl" };
        format!("https://www.canlii.org/fr/{jurisdiction}/legis/{kind}/{id}/derniere/{id}.html")
    } else {
        format!("https://www.canlii.org/en/{jurisdiction}/laws/{path}/{id}/latest/{id}.html")
    }
}

/// Justice Laws for RSC 1985 acts (`/eng/acts/C-46/`) and SOR, SI and CRC
/// regulations (`/eng/regulations/SOR-2002-227/`, `/eng/regulations/C.R.C.,_c._870/`).
pub fn justice_laws(citation: &Citation, language: Language) -> Option<String> {
    if citation.form != Form::Full {
        return None;
    }
    static ACT_CHAPTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z]-\d+(?:\.\d+)?$").unwrap());
    let fields = &citation.fields;
    let series = fold(fields.series.as_deref()?);
    let base = "https://laws-lois.justice.gc.ca";
    let french = language == Language::Fr;
    match citation.format? {
        Format::StatuteVolume if matches!(series.as_str(), "rsc" | "lrc") => {
            if four_digit_year(fields.year.as_deref()?)? != "1985" {
                return None;
            }
            let chapter = chapter_value(fields.chapter.as_deref()?)?;
            if !ACT_CHAPTER.is_match(&chapter) {
                return None;
            }
            let chapter = chapter.to_uppercase();
            Some(if french {
                format!("{base}/fra/lois/{chapter}/")
            } else {
                format!("{base}/eng/acts/{chapter}/")
            })
        }
        Format::RegulationSeries => match series.as_str() {
            "crc" => {
                let chapter = chapter_value(fields.chapter.as_deref().or(fields.regulation.as_deref())?)?;
                if !chapter.chars().all(|character| character.is_ascii_digit()) {
                    return None;
                }
                Some(if french {
                    format!("{base}/fra/reglements/C.R.C.,_ch._{chapter}/")
                } else {
                    format!("{base}/eng/regulations/C.R.C.,_c._{chapter}/")
                })
            }
            "sor" | "dors" | "si" | "tr" => {
                let (year, number) = regulation_parts(citation)?;
                if !matches!(year.len(), 2 | 4) {
                    return None;
                }
                let statutory = matches!(series.as_str(), "sor" | "dors");
                let prefix = match (statutory, french) {
                    (true, false) => "SOR",
                    (true, true) => "DORS",
                    (false, false) => "SI",
                    (false, true) => "TR",
                };
                Some(if french {
                    format!("{base}/fra/reglements/{prefix}-{year}-{number}/")
                } else {
                    format!("{base}/eng/regulations/{prefix}-{year}-{number}/")
                })
            }
            _ => None,
        },
        _ => None,
    }
}

/// Build this module's lazily built statics now ([`crate::warm`]).
pub(super) fn warm() {
    crate::warm_statics!(SOURCE_ROUTES, SCR, URL, PARAGRAPH, CANLII);
}
