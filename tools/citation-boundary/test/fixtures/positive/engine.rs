// expect: regex/neutral, regex/reporter, data/court-table, code/citation-function
use regex::Regex;

pub fn neutral() -> Regex {
    Regex::new(r"\b(?:19|20)\d{2}\s+(?:SCC|ONCA|BCCA|FCA)\s+\d+\b").unwrap()
}

pub fn reporter() -> Regex {
    Regex::new(r#"\[(?:19|20)\d{2}\]\s+\d+\s+(?:S\.?C\.?R|R\.?C\.?S)\.?\s+\d+"#).unwrap()
}

pub fn level(code: &str) -> u8 {
    match code {
        "SCC" => 5,
        "ONCA" | "BCCA" | "ABCA" | "QCCA" => 4,
        "ONSC" | "BCSC" | "ABKB" => 3,
        _ => 0,
    }
}

pub fn canlii_case_url(route: &str, year: u16, slug: &str) -> String {
    format!("https://www.canlii.org/en/{}/doc/{}/{}/{}.html", route, year, slug, slug)
}
