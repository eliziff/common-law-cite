//! Text as the pipeline reads it: JavaScript's coordinates and white space ([`legal_text`]), and the Python readings
//! the citation grammar was written against.

pub use legal_text::*;

/// Python's `\s` and `str.isspace`: Unicode whitespace and ASCII information separators.
pub(crate) fn python_whitespace(character: char) -> bool {
    character.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&character)
}

static DIGIT: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"^\p{Nd}$").unwrap());
/// Unicode decimal digits are stored in contiguous runs of ten in Unicode.
/// Convert their numeric representation without limiting Python's integer size.
pub(crate) fn decimal(value: &str) -> Option<num_bigint::BigUint> {
    let digit = |character: char| DIGIT.is_match(character.encode_utf8(&mut [0; 4]));
    let ascii = value.chars().map(|character| {
        if character.is_ascii_digit() { return Some(character); }
        if !digit(character) { return None; }
        let mut start = character as u32;
        while start > 0 && char::from_u32(start - 1).is_some_and(digit) { start -= 1; }
        Some(char::from(b'0' + ((character as u32 - start) % 10) as u8))
    }).collect::<Option<String>>()?;
    ascii.parse().ok()
}

/// Build this module's lazily built statics now ([`crate::warm`]).
pub(crate) fn warm() {
    crate::warm_statics!(DIGIT);
}
