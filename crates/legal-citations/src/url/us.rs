//! CourtListener's citation lookup: the United States' publisher.

use super::*;
use crate::key;

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
