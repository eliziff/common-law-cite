//! Resolve a citation to the URL of a public source.
//!
//! Every builder abstains (`None`) when the identity is uncertain: an unknown
//! court, a series without a public route, a chapter that does not map to an
//! id deterministically. Nothing here guesses a route from a court prefix.
//!
//! * CanLII decisions ([`canlii_case`]) from neutral and CanLII citations via
//!   the registry court's CanLII route, porting Beaver `canliiUrls.ts`: the
//!   slug is `{year}{code}{number}` with the neutral code as written
//!   lowercased (`2019 FC 123` → `ca/fct/…/2019fc123`), a database id keeps
//!   its registry casing (`nb/NBQB`), and `2004 CanLII 12345 (ON CA)` →
//!   `…/on/onca/doc/2004/2004canlii12345/…`. Only Canadian courts with a
//!   single-word code qualify. A French neutral code (the registry lists
//!   codes English first, then French) uses the court's `canlii_fr` route and
//!   a French page (`2019 NBBR 5` → `fr/nb/NBQB/…/2019nbbr5`); a French page
//!   of an English code of the five bilingual federal courts uses the French
//!   code and database (`SCC`→`csc`, `FCA`→`caf`, `FC`→`cf`, `TCC`→`cci`,
//!   `CMAC`→`cacm`), as legal-pinpointer does; any other French page keeps
//!   the English route and slug, as Beaver does.
//! * CanLII search and noteup ([`canlii_search`], legal-pinpointer
//!   `sonar-launcher.js`), anchors ([`canlii_anchor`], pinpointer
//!   `canliiAnchorForLocator`), [`is_canlii_url`] and [`canlii_pdf_url`]
//!   (Beaver `isCanliiUrl`, `buildCanliiPdfUrl`).
//! * CanLII legislation ([`canlii_legislation`]) for series the registry
//!   routes to CanLII, with pinpointer's id scheme (`rsc-1985-c-c-46`,
//!   `sor-2002-227`, `o-reg-191-11`, `crc-c-870`).
//! * Justice Laws ([`justice_laws`]) for RSC 1985 acts and SOR/SI/CRC
//!   regulations, whose ids are their citations.
//! * The National Archives' Find Case Law ([`uk_caselaw`]) for UK neutral
//!   citations (ds-caselaw-utils `neutral_url` paths such as `/uksc/2019/5`,
//!   `/ewhc/ch/2019/123`) and legislation.gov.uk ([`legislation_gov_uk`]) for
//!   UK public general Acts by calendar year (from 1963) or regnal year.
//! * CourtListener ([`courtlistener`]) citation lookup for US reporters.
//! The independent ALR/Beaver court-route inventory supplies a CanLII URL
//! when a written neutral code lacks a route in the curated court registry.

use crate::format::Language;
use crate::key;
use crate::model::{Authority, Citation, Form, Format, PinpointKind};
use crate::registry::{self, fold, Registry, SeriesKind};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

/// Pinpointer's candidate spellings for its external CanLII TSV index. These
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
        serde_json::from_str(include_str!("../registry/canlii-index.json")).expect("CanLII index rules"));
    static PATTERNS: LazyLock<[legal_grammar::CompiledEcmascriptGrammar; 7]> = LazyLock::new(|| {
        ["statute", "regulation", "instrument", "crc", "jurisdiction-statute", "jurisdiction-instrument", "jurisdiction-crc"]
            .map(|name| legal_grammar::compile_ecmascript_table_entry(&format!("lookup.canlii-legislation.{name}"))
                .expect("Pinpointer index lookup grammar"))
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

fn language_segment(language: Language) -> &'static str {
    match language {
        Language::En => "en",
        Language::Fr => "fr",
    }
}

/// `encodeURIComponent`.
pub fn encode_component(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            output.push(char::from(byte));
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

// ---------------------------------------------------------------------------
// CanLII decisions

/// English neutral codes CanLII also publishes under a French code and
/// database (legal-pinpointer `canliiCourtRoute`); the French database comes
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceRoutes {
    routes: HashMap<String, String>,
    french_routes: HashMap<String, String>,
}

static SOURCE_ROUTES: LazyLock<SourceRoutes> = LazyLock::new(||
    serde_json::from_str(include_str!("../registry/source-canlii-routes.json")).expect("source CanLII routes"));

pub(crate) fn source_canlii_french_routes() -> &'static HashMap<String, String> {
    &SOURCE_ROUTES.french_routes
}

/// Pinpointer `canliiUrlForCitation` and `canliiCourtRoute`: the first written
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

/// ALR `_resolve_footnote_part_link_unlocked`'s SCR override.
pub(crate) fn source_prefers_fallback(kind: &str, text: &str) -> bool {
    static SCR: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(||
        legal_grammar::compile_python_table_entry("cite.reporter.scr.source-link").unwrap());
    matches!(kind, "case" | "unreported") && SCR.is_match(text).expect("source SCR citation")
}

/// ALR _append_first_pinpoint_fragment, including its sanitizer's PDF sibling.
/// Source candidates are HTTP(S) URLs or the splitter's bare www/perma/DOI forms.
pub(crate) fn source_first_pinpoint(link: &str, fragments: &[String]) -> String {
    static URL: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(||
        legal_grammar::compile_python_table_entry("url.source-candidate").unwrap());
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
        // ALR substitutes at the end of the base, not before a nonempty query.
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

/// ALR `_generate_fallback_url`'s offline selection order, using the fields
/// already discovered in the source part and its original court-route table.
pub(crate) fn source_fallback(citations: &[&Citation], source: &crate::source::SourceFields,
    part: &crate::source::SourcePart) -> Option<(usize, String)> {
    if let Some(citation) = citations.iter().find(|citation| citation.format == Some(Format::Neutral)
        && citation.fields.series.as_deref().is_some_and(|court|
            source_canlii_routes().contains_key(&court.to_uppercase()))) {
        let route = &source_canlii_routes()[&citation.fields.series.as_deref()?.to_uppercase()];
        let jurisdiction = route.split('/').next().unwrap();
        let language = if matches!(jurisdiction, "qc" | "nb") { Language::Fr } else { Language::En };
        let mut url = source_canlii_case(citation, language)?;
        static PARAGRAPH: LazyLock<Regex> = LazyLock::new(|| {
            let entry = &legal_grammar::load_tables().unwrap()["pinpoint.source-first-paragraph"].entry;
            regex::RegexBuilder::new(&entry.pattern).case_insensitive(true).build().unwrap()
        });
        if let Some(pin) = source.pinpoint_fragments.first().filter(|pin| pin.starts_with("par")) {
            url.push_str(&format!("#{pin}"));
        } else if let Some(pin) = PARAGRAPH.captures(&source.citation_with_style) {
            url.push_str(&format!("#par{}", &pin[1]));
        }
        return Some((citation.index, url));
    }
    static CANLII: LazyLock<legal_grammar::CompiledGrammar> = LazyLock::new(||
        legal_grammar::compile_python_table_entry("cite.canlii.source-link").unwrap());
    // ALR searches the complete source, including its parenthesized court.
    // Keep its first match and select the core at that original-text offset.
    if let Some(matched) = CANLII.find(&source.citation_with_style).expect("source CanLII citation") {
        let start = part.start + part.text.find(&source.citation_with_style)? + matched.start();
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

/// Pinpointer's alias-index target: a citation or jurisdiction/database/caseId.
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
        if court.is_some() { language } else { Language::En }),
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

fn four_digit_year(value: &str) -> Option<String> {
    let digits = value.chars().filter(char::is_ascii_digit).collect::<String>();
    (digits.len() == 4).then_some(digits)
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

/// `#par12`.
pub fn paragraph_anchor(paragraph: &str) -> String {
    format!("#par{}", encode_component(paragraph.trim()))
}

/// The fragment that opens a pinpoint on a CanLII page (legal-pinpointer
/// `canliiAnchorForLocator`): `#par12` for paragraphs, a text fragment for
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
    // Beaver buildCanliiPdfUrl uses the browser URL parser before these checks.
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

fn chapter_value(chapter: &str) -> Option<String> {
    static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:chapitre|chapter|chap|ch|c)\.?\s+").unwrap());
    let value = LABEL.replace(chapter.trim(), "").trim().to_owned();
    (!value.is_empty()).then_some(value)
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

/// legislation.gov.uk for a UK public general Act: `/ukpga/1998/42` by
/// calendar year (1963 on) or `/ukpga/Vict/30-31/3` by regnal year.
pub fn legislation_gov_uk(citation: &Citation) -> Option<String> {
    if citation.form != Form::Full || !matches!(citation.authority, Authority::Statute | Authority::Constitution) {
        return None;
    }
    let jurisdiction = citation.jurisdiction.as_deref()?;
    if jurisdiction != "uk" {
        return None;
    }
    let fields = &citation.fields;
    let chapter = chapter_value(fields.chapter.as_deref()?)?;
    if !chapter.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    if let Some(regnal) = fields.regnal.as_deref() {
        static REGNAL: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"^(\d+)(?:\s*(?:&|and)\s*(\d+))?\s+(Vict|Edw\.?\s*(?:VII|VIII|7|8)|Geo\.?\s*(?:V|VI|5|6)|Eliz\.?\s*(?:II|2))\.?$").unwrap()
        });
        let captures = REGNAL.captures(regnal.trim())?;
        let monarch = fold(&captures[3]);
        let monarch = match monarch.as_str() {
            "vict" => "Vict",
            "edwvii" | "edw7" => "Edw7",
            "edwviii" | "edw8" => "Edw8",
            "geov" | "geo5" => "Geo5",
            "geovi" | "geo6" => "Geo6",
            "elizii" | "eliz2" => "Eliz2",
            _ => return None,
        };
        let years = match captures.get(2) {
            Some(second) => format!("{}-{}", &captures[1], second.as_str()),
            None => captures[1].to_owned(),
        };
        return Some(format!("https://www.legislation.gov.uk/ukpga/{monarch}/{years}/{chapter}"));
    }
    let year = four_digit_year(fields.year.as_deref()?)?;
    if year.parse::<u32>().ok()? < 1963 {
        return None;
    }
    Some(format!("https://www.legislation.gov.uk/ukpga/{year}/{chapter}"))
}

// ---------------------------------------------------------------------------
// UK and US case law

/// Find Case Law paths by neutral court code and division.
const UK_COURTS: &[(&str, Option<&str>, &str)] = &[
    ("UKSC", None, "uksc"),
    ("UKPC", None, "ukpc"),
    ("EWCA Civ", None, "ewca/civ"),
    ("EWCA Crim", None, "ewca/crim"),
    ("EWHC", Some("Admin"), "ewhc/admin"),
    ("EWHC", Some("Admlty"), "ewhc/admlty"),
    ("EWHC", Some("Ch"), "ewhc/ch"),
    ("EWHC", Some("Comm"), "ewhc/comm"),
    ("EWHC", Some("Costs"), "ewhc/costs"),
    ("EWHC", Some("Fam"), "ewhc/fam"),
    ("EWHC", Some("IPEC"), "ewhc/ipec"),
    ("EWHC", Some("KB"), "ewhc/kb"),
    ("EWHC", Some("QB"), "ewhc/qb"),
    ("EWHC", Some("Mercantile"), "ewhc/mercantile"),
    ("EWHC", Some("Pat"), "ewhc/pat"),
    ("EWHC", Some("SCCO"), "ewhc/scco"),
    ("EWHC", Some("TCC"), "ewhc/tcc"),
    ("EWCOP", None, "ewcop"),
    ("EWFC", None, "ewfc"),
    ("UKUT", Some("AAC"), "ukut/aac"),
    ("UKUT", Some("IAC"), "ukut/iac"),
    ("UKUT", Some("LC"), "ukut/lc"),
    ("UKUT", Some("TCC"), "ukut/tcc"),
    ("UKFTT", Some("TC"), "ukftt/tc"),
    ("UKFTT", Some("GRC"), "ukftt/grc"),
];

/// The National Archives' Find Case Law page for a UK neutral citation:
/// `[2019] UKSC 5` → `https://caselaw.nationalarchives.gov.uk/uksc/2019/5`,
/// `[2019] EWHC 123 (Ch)` → `…/ewhc/ch/2019/123`.
pub fn uk_caselaw(citation: &Citation) -> Option<String> {
    static NEUTRAL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\[(\d{4})\]\s+(UKSC|UKPC|EWCA\s+Civ|EWCA\s+Crim|EWHC|EWCOP|EWFC|UKUT|UKFTT)\s+(\d+)(?:\s*\(([A-Za-z]+)\))?").unwrap()
    });
    if citation.form != Form::Full || citation.format != Some(Format::Neutral) {
        return None;
    }
    let text = if citation.full_span.text.contains(citation.span.text.as_str()) && !citation.span.text.is_empty() {
        let start = citation.full_span.text.find(citation.span.text.as_str()).unwrap_or(0);
        &citation.full_span.text[start..]
    } else {
        citation.span.text.as_str()
    };
    let captures = NEUTRAL.captures(text.trim_start())?;
    let code = captures[2].split_whitespace().collect::<Vec<_>>().join(" ");
    let division = captures.get(4).map(|division| division.as_str());
    let path = UK_COURTS.iter().find_map(|(court, wanted, path)| {
        let same_court = court.eq_ignore_ascii_case(&code);
        let same_division = match (wanted, division) {
            (None, _) => true,
            (Some(wanted), Some(division)) => wanted.eq_ignore_ascii_case(division),
            (Some(_), None) => false,
        };
        (same_court && same_division).then_some(*path)
    })?;
    // `[2020] EWFC 12 (B)` series and other lettered divisions are not the plain court.
    if matches!(code.as_str(), "EWFC" | "EWCOP" | "UKSC" | "UKPC") && division.is_some() {
        return None;
    }
    let number = captures[3].trim_start_matches('0');
    Some(format!(
        "https://caselaw.nationalarchives.gov.uk/{path}/{}/{number}",
        &captures[1]
    ))
}

/// CourtListener's citation lookup for a US reporter citation:
/// `410 U.S. 113` → `https://www.courtlistener.com/c/U.S./410/113/`.
pub fn courtlistener(citation: &Citation) -> Option<String> {
    courtlistener_in(citation, registry::registry())
}

/// [`courtlistener`] against `registry`.
pub fn courtlistener_in(citation: &Citation, registry: &Registry) -> Option<String> {
    if citation.form != Form::Full || citation.format != Some(Format::Reporter) || citation.is_ambiguous() {
        return None;
    }
    let fields = &citation.fields;
    let volume = fields.volume.as_deref()?.trim();
    let page = fields.page.as_deref()?.trim();
    let numeric = |value: &str| !value.is_empty() && value.chars().all(|character| character.is_ascii_digit());
    if !numeric(volume) || !numeric(page) {
        return None;
    }
    let (reporter, canonical) = key::selected_reporter(citation, registry)?;
    let american = reporter.source == "reporters-db"
        || reporter
            .jurisdiction
            .as_deref()
            .is_some_and(|jurisdiction| jurisdiction == "us" || jurisdiction.starts_with("us-"));
    if !american {
        return None;
    }
    let reporter = canonical
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-_.~'&".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect::<String>();
    Some(format!("https://www.courtlistener.com/c/{reporter}/{volume}/{page}/"))
}

/// The best public URL for a full citation: CanLII, Find Case Law or
/// CourtListener for decisions; CanLII, Justice Laws or legislation.gov.uk
/// for legislation. `None` when no source is certain.
pub fn url(citation: &Citation, language: Language) -> Option<String> {
    if let Some(target) = crate::aliases::resolve(citation) {
        let options = crate::Options { resolve: false, parallel: false, ..Default::default() };
        let canonical = crate::extract(&target.citation, &options);
        if let [only] = canonical.as_slice() {
            if let Some(url) = url_in(only, language, registry::registry()) {
                return Some(url);
            }
        }
    }
    if matches!(citation.format, Some(Format::Neutral | Format::CanLii)) {
        if let Some(url) = canlii_case(citation, language) { return Some(url); }
    }
    url_in(citation, language, registry::registry())
}

/// [`url`] against `registry`.
pub fn url_in(citation: &Citation, language: Language, registry: &Registry) -> Option<String> {
    if citation.form != Form::Full || citation.is_ambiguous() {
        return None;
    }
    match citation.format? {
        Format::Neutral | Format::CanLii => {
            canlii_case_in(citation, language, registry).or_else(|| uk_caselaw(citation))
        }
        Format::Reporter => courtlistener_in(citation, registry),
        Format::StatuteVolume | Format::RegulationSeries => canlii_legislation_in(citation, language, registry)
            .or_else(|| justice_laws(citation, language))
            .or_else(|| legislation_gov_uk(citation)),
        Format::Url => citation
            .fields
            .url
            .clone()
            .filter(|url| url.starts_with("https://") || url.starts_with("http://")),
        _ => None,
    }
}
