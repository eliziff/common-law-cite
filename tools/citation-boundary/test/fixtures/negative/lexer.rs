// Lifetimes, chars and raw strings must not confuse the lexer.
use regex::Regex;

pub struct Holder<'a> {
    pub text: &'a str,
}

pub fn quote() -> char {
    '"'
}

pub fn apostrophe() -> char {
    '\''
}

pub fn numbering() -> Regex {
    Regex::new(r"^\s*(\d+)\.\s+").unwrap()
}

pub fn json() -> &'static str {
    r#"{"court": "SCC", "note": "a single mention is not a table"}"#
}
