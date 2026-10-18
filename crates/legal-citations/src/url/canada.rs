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
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct LegislationLookup {
    pub candidates: Vec<String>,
    pub jurisdiction: String,
}

pub fn legislation_lookup(value: &str) -> LegislationLookup {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Rules {
        series: std::collections::HashSet<String>,
        regulation_prefixes: std::collections::HashMap<String, String>,
        series_jurisdictions: std::collections::HashMap<String, String>,
    }
    static RULES: LazyLock<Rules> = LazyLock::new(||
        serde_json::from_str(include_str!("../../registry/canlii-index.json")).expect("CanLII index rules"));
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
        if !RULES.series.contains(&series) { continue; }
        let chapter = matched[3].to_lowercase();
        for chapter in [chapter.clone(), chapter.replace('.', "-"), chapter.replace(['.', '-'], "")] {
            add(format!("{series}-{}-c-{chapter}", &matched[2]));
        }
    }
    if let Some(matched) = PATTERNS[1].captures(&citation) {
        if let Some(prefix) = RULES.regulation_prefixes.get(&series_key(&matched[1])) {
            add(format!("{prefix}-reg-{}-{}", &matched[2], &matched[3]));
        }
    }
    if let Some(matched) = PATTERNS[2].captures(&citation) {
        add(format!("{}-{}-{}", &matched[1], &matched[2], &matched[3]));
    }
    if let Some(matched) = PATTERNS[3].captures(&citation) { add(format!("crc-c-{}", &matched[1])); }
    // The original jurisdiction lookup uses the written text, without the
    // candidate spelling's NFKC/dash normalization.
    let jurisdiction = if let Some(matched) = PATTERNS[4].captures(value) {
        RULES.series_jurisdictions.get(&series_key(&matched[1])).cloned().unwrap_or_default()
    } else if let Some(matched) = PATTERNS[1].captures(value) {
        match RULES.regulation_prefixes.get(&series_key(&matched[1])).map(String::as_str).unwrap_or("") {
            "alta" => "ab", "o" => "on", "man" => "mb", "sask" => "sk", prefix => prefix,
        }.to_owned()
    } else if PATTERNS[5].is_match(value) || PATTERNS[6].is_match(value) { "ca".to_owned() }
    else { String::new() };
    LegislationLookup { candidates, jurisdiction }
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
    }
    result
});

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
        else { format!("{year}{code}{third}").to_lowercase().chars().filter(char::is_ascii_alphanumeric).collect() };
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
    let slug = format!("{year}{code}{number}");
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
                .or_else(|| key::court_by_surface(registry, written))?;
            if !(court.jurisdiction == "ca" || court.jurisdiction.starts_with("ca-")) {
                return None;
            }
            // The written code must be one of the court's own neutral codes.
            let position = court.neutral.iter().position(|code| fold(code) == fold(written))?;
            // The code as written, lowercased: `SCC-L` → `scc-l`.
            let code = written
                .chars()
                .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
                .collect::<String>()
                .to_ascii_lowercase();
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

/// The fragment that opens a pinpoint on a CanLII page: `#par12` for paragraphs, a text fragment for
/// `[page 5]` markers, `#sec7`, `#sec7subsec2`, `#art1457`, `#rule3` for
/// provisions, and LégisQuébec's `#se:18_1` on Quebec statutes. Deeper
/// provisions (`7(2)(a)`) have no stable id.
pub fn canlii_anchor(kind: PinpointKind, locator: &str, page_url: &str) -> Option<String> {
    let value = crate::text::normalize_javascript_whitespace(locator);
    match kind {
        PinpointKind::Page => Some(format!("#:~:text={}", encode_component(&format!("[page {value}]")))),
        PinpointKind::Paragraph => Some(paragraph_anchor(&value)),
        PinpointKind::Section | PinpointKind::Subsection | PinpointKind::Rule | PinpointKind::Article => {
            static QUEBEC: LazyLock<Regex> =
                LazyLock::new(|| Regex::new(r"//[^/]+/(?:en|fr)/qc/laws/").unwrap());
            let parsed = crate::format::parse_locator(&value)?;
            let (root, suffixes) = (&parsed.root, &parsed.suffixes);
            if QUEBEC.is_match(page_url) {
                return Some(format!("#se:{}", root.replace('.', "_")));
            }
            if suffixes.len() > 1 {
                return None;
            }
            let prefix = match kind {
                PinpointKind::Article => "art",
                PinpointKind::Rule => "rule",
                _ => "sec",
            };
            let tail = suffixes.first().map(|suffix| format!("subsec{suffix}")).unwrap_or_default();
            Some(format!("#{prefix}{root}{tail}"))
        }
        _ => None,
    }
}

/// `page_url` with the anchor for `citation`'s first pinpoint, when there is
/// exactly one pinpoint and it has an anchor.
pub fn with_pinpoint(page_url: &str, citation: &Citation) -> String {
    match citation.pinpoints.as_slice() {
        [only] if only.last.is_none() => match canlii_anchor(only.kind, &only.first, page_url) {
            Some(anchor) if !page_url.contains('#') => format!("{page_url}{anchor}"),
            _ => page_url.to_owned(),
        },
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

/// Whether `url` is on canlii.org (any subdomain).
/// Whether a publisher's page may print a legal reference inside the text it quotes (BC Laws annotates them).
pub fn publisher_annotates_legal_references(address: &str) -> bool {
    url::Url::options().base_url(Some(&url::Url::parse("http://mike.local").unwrap())).parse(address)
        .is_ok_and(|url| url.host_str() == Some("www.bclaws.gov.bc.ca"))
}

/// The CanLII page of an A2AJ legislation record (`LEGISLATION-ON`, `REGULATIONS-FED`) by its citation slugged as CanLII
/// slugs it: `RSO 1990, c C.43` in LEGISLATION-ON is `…/on/laws/stat/rso-1990-c-c.43/latest/rso-1990-c-c.43.html`.
pub fn canlii_a2aj_law_url(dataset: &str, citation: &str, language: &str) -> Option<String> {
    static DATASET: LazyLock<Regex> = LazyLock::new(||
        Regex::new(r"^(LEGISLATION|REGULATIONS)-(FED|AB|BC|MB|NB|NL|NS|NT|NU|ON|PE|QC|SK|YT)$").unwrap());
    static SEPARATORS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9.]+").unwrap());
    let dataset = DATASET.captures(dataset.trim())?;
    let lowered = citation.trim().to_lowercase();
    let slug = SEPARATORS.replace_all(&lowered, "-");
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        return None;
    }
    let jurisdiction = if &dataset[2] == "FED" { "ca".to_owned() } else { dataset[2].to_lowercase() };
    let kind = if &dataset[1] == "LEGISLATION" { "stat" } else { "regu" };
    Some(format!("https://www.canlii.org/{language}/{jurisdiction}/laws/{kind}/{slug}/latest/{slug}.html"))
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
        LazyLock::new(|| Regex::new(r"^/(en|fr)/([a-z]+)/laws/([a-z]+)/([^/]+)/").unwrap());
    if !is_canlii_url(page_url) {
        return None;
    }
    let url = url::Url::parse(page_url).ok()?;
    let law = LAW.captures(url.path())?;
    Some(CanliiLawPage { language: law[1].to_owned(), jurisdiction: law[2].to_owned(), kind: law[3].to_owned(),
        id: law[4].to_owned() })
}

// ---------------------------------------------------------------------------
// Legislation

/// The CanLII id and path segment (`stat`, `astat`, `regu`) of a statute or
/// regulation whose registry series CanLII publishes.
fn canlii_legislation_id(citation: &Citation, registry: &Registry) -> Option<(String, String, &'static str)> {
    let fields = &citation.fields;
    let series = key::selected_series(citation, registry, fields.series.as_deref()?)?;
    let route = series.canlii.as_ref()?;
    let path = match route.database.as_str() {
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
    };
    let slug = |value: &str| key::ident(value);
    let series_id = slug(&series.abbreviation);
    let id = match citation.format? {
        Format::StatuteVolume => {
            let chapter = chapter_value(fields.chapter.as_deref()?)?;
            // The Ontario 1990 consolidation's published ids close the
            // letter-number chapter (D-16 -> d16); the other series retain
            // the written separator (for example RSC C-46 -> c-46).
            let chapter = if series_id == "rso" && fields.year.as_deref() == Some("1990") {
                chapter.replace('-', "")
            } else { chapter };
            match fields.year.as_deref() {
                Some(year) => format!("{series_id}-{}-c-{}", slug(year), slug(&chapter)),
                None => format!("{series_id}-c-{}", slug(&chapter)),
            }
        }
        Format::RegulationSeries => {
            if let Some(chapter) = fields.chapter.as_deref() {
                let chapter = slug(&chapter_value(chapter)?);
                match fields.regulation.as_deref() {
                    Some(regulation) => format!("{series_id}-c-{chapter}-r-{}", slug(regulation)),
                    None => format!("{series_id}-c-{chapter}"),
                }
            } else {
                let (year, number) = regulation_parts(citation)?;
                if series_id == "sor" || series_id == "si" {
                    format!("{series_id}-{year}-{number}")
                } else {
                    format!("{series_id}-{number}-{year}")
                }
            }
        }
        _ => return None,
    };
    if id.is_empty() || id.contains("--") {
        return None;
    }
    Some((route.jurisdiction.clone(), id, path))
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

/// [`canlii_legislation`] against `registry`.
pub fn canlii_legislation_in(citation: &Citation, language: Language, registry: &Registry) -> Option<String> {
    if citation.form != Form::Full {
        return None;
    }
    let (jurisdiction, id, path) = canlii_legislation_id(citation, registry)?;
    if jurisdiction.is_empty() {
        return None;
    }
    Some(format!(
        "https://www.canlii.org/{}/{jurisdiction}/laws/{path}/{id}/latest/{id}.html",
        language_segment(language)
    ))
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
