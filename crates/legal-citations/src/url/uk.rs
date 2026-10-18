//! The National Archives' Find Case Law and legislation.gov.uk: the United Kingdom's publishers.

use super::*;
use crate::model::Authority;
use crate::registry::fold;

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
