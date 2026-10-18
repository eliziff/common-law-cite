//! Resolve a citation to the URL of a public source.
//!
//! Every builder abstains (`None`) when the identity is uncertain: an unknown
//! court, a series without a public route, a chapter that does not map to an
//! id deterministically. Nothing here guesses a route from a court prefix.
//!
//! * CanLII decisions ([`canlii_case`]) from neutral and CanLII citations via
//!   the registry court's CanLII route: the
//!   slug is `{year}{code}{number}` with the neutral code as written
//!   lowercased (`2019 FC 123` → `ca/fct/…/2019fc123`), a database id keeps
//!   its registry casing (`nb/NBQB`), and `2004 CanLII 12345 (ON CA)` →
//!   `…/on/onca/doc/2004/2004canlii12345/…`. Only Canadian courts with a
//!   single-word code qualify. A French neutral code (the registry lists
//!   codes English first, then French) uses the court's `canlii_fr` route and
//!   a French page (`2019 NBBR 5` → `fr/nb/NBQB/…/2019nbbr5`); a French page
//!   of an English code of the five bilingual federal courts uses the French
//!   code and database (`SCC`→`csc`, `FCA`→`caf`, `FC`→`cf`, `TCC`→`cci`,
//!   `CMAC`→`cacm`); any other French page keeps
//!   the English route and slug.
//! * CanLII search and noteup ([`canlii_search`]), anchors ([`canlii_anchor`]),
//!   [`is_canlii_url`] and [`canlii_pdf_url`].
//! * CanLII legislation ([`canlii_legislation`]) for series the registry
//!   routes to CanLII, with CanLII's id scheme (`rsc-1985-c-c-46`,
//!   `sor-2002-227`, `o-reg-191-11`, `crc-c-870`).
//! * Justice Laws ([`justice_laws`]) for RSC 1985 acts and SOR/SI/CRC
//!   regulations, whose ids are their citations.
//! * The National Archives' Find Case Law ([`uk_caselaw`]) for UK neutral
//!   citations (ds-caselaw-utils `neutral_url` paths such as `/uksc/2019/5`,
//!   `/ewhc/ch/2019/123`) and legislation.gov.uk ([`legislation_gov_uk`]) for
//!   UK public general Acts by calendar year (from 1963) or regnal year.
//! * CourtListener ([`courtlistener`]) citation lookup for US reporters.
//! Source-part routes are derived from the same court records and their
//! neutral identifiers; there is no separate route inventory.

use crate::format::Language;
use crate::model::{Citation, Form, Format};
use crate::registry::{self, Registry};
use regex::Regex;
use std::sync::LazyLock;

// Each jurisdiction's publishers, compiled with its feature.
#[cfg(feature = "canada")]
mod canada;
#[cfg(feature = "canada")]
pub use canada::*;
#[cfg(feature = "canada")]
pub(crate) use canada::{source_canlii_french_routes, source_fallback, source_first_pinpoint, source_prefers_fallback};
#[cfg(feature = "uk")]
mod uk;
#[cfg(feature = "uk")]
pub use uk::*;
#[cfg(feature = "us")]
mod us;
#[cfg(feature = "us")]
pub use us::*;

/// Whether a page is on CanLII and whether it is a Decisia decision (neither without the `canada` feature).
#[cfg_attr(not(feature = "canada"), allow(unused_variables))]
fn canadian_page(url: &url::Url) -> (bool, bool) {
    #[cfg(feature = "canada")]
    return (canada::on_canlii_org(url), canada::is_decisia_document(url));
    #[cfg(not(feature = "canada"))]
    (false, false)
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

#[cfg(any(feature = "canada", feature = "uk"))]
fn four_digit_year(value: &str) -> Option<String> {
    let digits = value.chars().filter(char::is_ascii_digit).collect::<String>();
    (digits.len() == 4).then_some(digits)
}

/// `#par12`.
pub fn paragraph_anchor(paragraph: &str) -> String {
    format!("#par{}", encode_component(paragraph.trim()))
}

// ---------------------------------------------------------------------------
// A source's page at a pinpoint: the anchor each publisher's page has for it

/// A number as written without its leading zeros (`007` is `7`).
fn plain_number(digits: &str) -> &str {
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() { "0" } else { trimmed }
}

/// The anchor a source's page has for a locator its reading labels (`par12`, `page 3`, `2(1)`,
/// `sec2subsec1`): `par12` on a CanLII decision or a Decisia decision, `page=3` on a PDF, and on a
/// CanLII statute `sec2` or `sec2subsec1`. None where the page has no such anchor.
pub fn source_anchor(page_url: &str, kind: &str, label: &str) -> Option<String> {
    static PARAGRAPH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^par([0-9]+)").unwrap());
    static PAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:page=?)?([0-9]+)").unwrap());
    static NUMBERED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^([0-9]+(?:\.[0-9]+)*)(?:\s*\(([0-9]+)\))?").unwrap());
    static SECTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^sec[A-Za-z0-9_.-]+").unwrap());
    let url = url::Url::parse(page_url).ok()?;
    let (canlii, decisia) = canadian_page(&url);
    match kind {
        "paragraph" => {
            let number = PARAGRAPH.captures(label)?;
            ((canlii && url.path().contains("/doc/")) || decisia)
                .then(|| format!("par{}", plain_number(&number[1])))
        }
        "page" => {
            let number = PAGE.captures(label)?;
            url.path().to_lowercase().ends_with(".pdf").then(|| format!("page={}", plain_number(&number[1])))
        }
        // CanLII anchors a section as sec2 and a subsection as sec2subsec1.
        _ => {
            let section = SECTION.find(label).map(|found| found.as_str().to_owned()).or_else(|| {
                NUMBERED.captures(label).map(|numbered| format!("sec{}{}", &numbered[1],
                    numbered.get(2).map(|subsection| format!("subsec{}", subsection.as_str())).unwrap_or_default()))
            })?;
            (canlii && url.path().contains("/laws/")).then_some(section)
        }
    }
}

/// A legal source's address as it is linked, at `anchor` (else the anchor it has): None for an
/// address that is not http(s) or a host whose pages cannot be linked; a CanLII PDF opens its page
/// unless the anchor is a PDF page; a PDF keeps only a `page=N` anchor; a Decisia decision opens
/// in the viewer its text fragments need.
pub fn source_url(address: &str, anchor: Option<&str>) -> Option<String> {
    static UNLINKABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:^|\.)getcaselaw\.com$").unwrap());
    static DOCUMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)/document\.do$").unwrap());
    static PDF_PAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^page=[0-9]+$").unwrap());
    let mut url = url::Url::parse(address).ok()?;
    if url.host_str().is_some_and(|host| UNLINKABLE.is_match(host)) || !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    #[cfg(feature = "canada")]
    canada::statute_page(&mut url);
    let existing = url.fragment().unwrap_or("").split(":~:").next().unwrap_or("").to_owned();
    let pdf = |url: &url::Url| url.path().to_lowercase().ends_with(".pdf");
    let converted = canadian_page(&url).0 && pdf(&url) && !anchor.unwrap_or(&existing).starts_with("page=");
    if converted {
        let path = url.path().to_owned();
        url.set_path(&format!("{}.html", &path[..path.len() - ".pdf".len()]));
    }
    let mut resolved = anchor.map_or_else(|| if converted { String::new() } else { existing }, str::to_owned);
    if (DOCUMENT.is_match(url.path()) || pdf(&url)) && !PDF_PAGE.is_match(&resolved) {
        resolved.clear();
    }
    url.set_fragment((!resolved.is_empty()).then_some(resolved.as_str()));
    if canadian_page(&url).1 {
        // INTENTIONAL, LOAD-BEARING FEATURE - text fragments cannot work on
        // Decisia without both parameters. This is not a flaw, lint, or
        // cosmetic preference; do not remove or "normalize" either one.
        //
        // iframe=true: the default Decisia document URL is an iframe shell with
        // no document text at all, so neither paragraph anchors nor
        // #:~:text= directives can resolve against it. iframe=true serves the
        // document inline.
        //
        // site_preference=mobile: in the desktop rendering, a successful
        // text-fragment jump locks the viewport onto the matched text and the
        // page cannot be scrolled afterwards. Only the mobile rendering scrolls
        // normally after a fragment jump. Both behaviors were verified in live
        // Chrome; server-side probes and static HTML inspection cannot see
        // them, so an apparent absence of justification here is not evidence
        // the parameters are removable.
        //
        // Side effect, deliberately accepted: Decisia remembers site_preference
        // in a cookie. That only affects visitors who arrive via Beaver's deep
        // links. A regular user browsing the publisher normally clicks plain
        // desktop URLs and sees the identical desktop site - their experience
        // is completely unchanged.
        let kept: Vec<(String, String)> = url.query_pairs()
            .filter(|(name, _)| name != "iframe" && name != "site_preference")
            .map(|(name, value)| (name.into_owned(), value.into_owned())).collect();
        url.query_pairs_mut().clear().extend_pairs(kept)
            .append_pair("iframe", "true")
            .append_pair("site_preference", "mobile");
    }
    Some(url.into())
}

/// The paragraph or provision a link's anchor opens (`par12`; `sec2subsec1` is provision `2(1)`),
/// and for a provision the part of it the citation pinpoints (`sec2(1)(a)` gives `2(1)(a)`).
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct PassageLocator {
    /// "paragraph" or "section".
    pub kind: &'static str,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
}

/// [`PassageLocator`] of `link` (absolute, or a path on this host), from its anchor.
pub fn passage_locator(link: &str, pinpoints: &[String]) -> Option<PassageLocator> {
    static PARAGRAPH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^par([0-9]+)$").unwrap());
    static SECTION: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^sec([A-Za-z0-9_.-]+?)(?:subsec([A-Za-z0-9_]+))?$").unwrap());
    static LOCAL: LazyLock<url::Url> = LazyLock::new(|| url::Url::parse("http://mike.local").unwrap());
    let url = url::Url::options().base_url(Some(&LOCAL)).parse(link).ok()?;
    let anchor = url.fragment().unwrap_or("");
    if let Some(paragraph) = PARAGRAPH.captures(anchor) {
        return Some(PassageLocator { kind: "paragraph", label: paragraph[1].to_owned(), part: None });
    }
    let section = SECTION.captures(anchor)?;
    let label = format!("{}{}", &section[1], section.get(2).map(|sub| format!("({})", sub.as_str())).unwrap_or_default());
    let part = pinpoints.iter().filter_map(|pinpoint| pinpoint.strip_prefix("sec").filter(|rest| !rest.is_empty()))
        .find(|value| value.starts_with(&format!("{label}("))).map(str::to_owned);
    Some(PassageLocator { kind: "section", label, part })
}

#[cfg(any(feature = "canada", feature = "uk"))]
fn chapter_value(chapter: &str) -> Option<String> {
    static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:chapitre|chapter|chap|ch|c)\.?\s+").unwrap());
    let value = LABEL.replace(chapter.trim(), "").trim().to_owned();
    (!value.is_empty()).then_some(value)
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
    #[cfg(feature = "canada")]
    if matches!(citation.format, Some(Format::Neutral | Format::CanLii)) {
        if let Some(url) = canlii_case(citation, language) { return Some(url); }
    }
    url_in(citation, language, registry::registry())
}

/// [`url`] against `registry`: each enabled jurisdiction's publisher, in order.
#[cfg_attr(not(all(feature = "canada", feature = "uk", feature = "us")), allow(unused_variables, unused_mut))]
pub fn url_in(citation: &Citation, language: Language, registry: &Registry) -> Option<String> {
    if citation.form != Form::Full || citation.is_ambiguous() {
        return None;
    }
    let mut found = None;
    match citation.format? {
        Format::Neutral | Format::CanLii => {
            #[cfg(feature = "canada")]
            { found = found.or_else(|| canlii_case_in(citation, language, registry)); }
            #[cfg(feature = "uk")]
            { found = found.or_else(|| uk_caselaw(citation)); }
            found
        }
        Format::Reporter => {
            #[cfg(feature = "us")]
            { found = courtlistener_in(citation, registry); }
            found
        }
        Format::StatuteVolume | Format::RegulationSeries => {
            #[cfg(feature = "canada")]
            { found = canlii_legislation_in(citation, language, registry).or_else(|| justice_laws(citation, language)); }
            #[cfg(feature = "uk")]
            { found = found.or_else(|| legislation_gov_uk(citation)); }
            found
        }
        Format::Url => citation
            .fields
            .url
            .clone()
            .filter(|url| url.starts_with("https://") || url.starts_with("http://")),
        _ => None,
    }
}

/// Build this module's lazily built statics now ([`crate::warm`]).
pub(crate) fn warm() {
    #[cfg(feature = "canada")]
    canada::warm();
}
