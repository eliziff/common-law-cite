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

use crate::format::Language;
use crate::key;
use crate::model::{Authority, Citation, Form, Format, PinpointKind};
use crate::registry::{self, fold, Registry, SeriesKind};
use regex::Regex;
use std::sync::LazyLock;

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
}

/// [`canlii_case`] against `registry`.
pub fn canlii_case_in(citation: &Citation, language: Language, registry: &Registry) -> Option<String> {
    if citation.form != Form::Full {
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
    let value = locator.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        return None;
    }
    match kind {
        PinpointKind::Page => Some(format!("#:~:text={}", encode_component(&format!("[page {value}]")))),
        PinpointKind::Paragraph => Some(paragraph_anchor(&value)),
        PinpointKind::Section | PinpointKind::Subsection | PinpointKind::Rule | PinpointKind::Article => {
            static QUEBEC: LazyLock<Regex> =
                LazyLock::new(|| Regex::new(r"//[^/]+/(?:en|fr)/qc/laws/").unwrap());
            static LOCATOR: LazyLock<Regex> =
                LazyLock::new(|| Regex::new(r"^(\d+(?:\.\d+)*)((?:\([^()]+\))*)$").unwrap());
            let captures = LOCATOR.captures(&value)?;
            let root = &captures[1];
            let suffixes = captures[2]
                .split(['(', ')'])
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
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
    let value = value.trim_matches(|character: char| character <= ' ');
    let colon = value.find(':')?;
    let scheme = &value[..colon];
    if scheme.is_empty()
        || !scheme.starts_with(|character: char| character.is_ascii_alphabetic())
        || !scheme
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "+-.".contains(character))
    {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    let special = matches!(scheme.as_str(), "http" | "https" | "ftp" | "ws" | "wss" | "file");
    let rest = &value[colon + 1..];
    let authority_start = if special {
        rest.trim_start_matches(['/', '\\'])
    } else if let Some(stripped) = rest.strip_prefix("//") {
        stripped
    } else {
        return Some(String::new());
    };
    let end = authority_start
        .find(|character: char| character == '/' || character == '?' || character == '#' || (special && character == '\\'))
        .unwrap_or(authority_start.len());
    let authority = &authority_start[..end];
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = if host_port.starts_with('[') {
        host_port
    } else {
        host_port.rsplit_once(':').map_or(host_port, |(host, _)| host)
    };
    if special && host.is_empty() && scheme != "file" {
        return None;
    }
    Some(host.to_lowercase().trim_end_matches('.').to_owned())
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
    let rest = page_url.trim().strip_prefix("https://")?;
    let slash = rest.find('/')?;
    let (authority, path) = rest.split_at(slash);
    if !authority.eq_ignore_ascii_case("www.canlii.org") || path.contains(['?', '#']) {
        return None;
    }
    let captures = PATH.captures(path)?;
    if captures[2] != captures[3] || !captures[2].starts_with(&captures[1]) {
        return None;
    }
    Some(format!("https://www.canlii.org{}.pdf", &path[..path.len() - ".html".len()]))
}

// ---------------------------------------------------------------------------
// Legislation

/// The CanLII id and path segment (`stat`, `astat`, `regu`) of a statute or
/// regulation whose registry series CanLII publishes.
fn canlii_legislation_id(citation: &Citation, registry: &Registry) -> Option<(String, String, &'static str)> {
    let fields = &citation.fields;
    let series = key::series_by_surface(registry, fields.series.as_deref()?)?;
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
    if citation.form != Form::Full || citation.format != Some(Format::Reporter) {
        return None;
    }
    let fields = &citation.fields;
    let volume = fields.volume.as_deref()?.trim();
    let page = fields.page.as_deref()?.trim();
    let numeric = |value: &str| !value.is_empty() && value.chars().all(|character| character.is_ascii_digit());
    if !numeric(volume) || !numeric(page) {
        return None;
    }
    let surface = fields.reporter_canonical.as_deref().or(fields.reporter.as_deref())?;
    let (reporter, canonical) = key::reporter_by_surface(registry, surface)?;
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
    url_in(citation, language, registry::registry())
}

/// [`url`] against `registry`.
pub fn url_in(citation: &Citation, language: Language, registry: &Registry) -> Option<String> {
    if citation.form != Form::Full {
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
