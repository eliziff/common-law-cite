//! Text cleaning with an offset map back to the source (eyecite `clean_text`).
//!
//! Steps, applied in order:
//!
//! * [`Step::Html`] — strip tags, comments, doctypes and the contents of
//!   `script`, `style`, `title` and `template`; decode character references;
//!   collapse HTML whitespace (except inside `pre`); turn block boundaries
//!   into newlines (`\n\n` after paragraphs, headings, lists, tables and
//!   quotes, `\n` after other blocks and for every `br`; table cells are
//!   separated by a space). The tags are kept in [`Cleaned::tags`], in source
//!   offsets, for [`crate::annotate`].
//! * [`Step::InlineWhitespace`] — every run of whitespace other than `\n`
//!   becomes one space (eyecite `inline_whitespace`).
//! * [`Step::AllWhitespace`] — every run of whitespace becomes one space
//!   (eyecite `all_whitespace`).
//! * [`Step::Underscores`] — runs of two or more underscores are removed
//!   (eyecite `underscores`, for blank signature and page lines).
//! * [`Step::ZeroWidth`] — zero-width spaces and joiners, word joiners, byte
//!   order marks and soft hyphens are removed.
//!
//! Every cleaned character remembers the source byte range it came from (an
//! entity maps to the whole `&amp;`, a collapsed run to the whole run), so a
//! cleaned span maps back exactly with [`Cleaned::source_range`]. No diff is
//! ever needed.

use crate::model::Span;
use std::ops::Range;

/// One cleaning step.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Step {
    Html,
    InlineWhitespace,
    AllWhitespace,
    Underscores,
    ZeroWidth,
}

impl Step {
    /// eyecite's step names: `html` (and `xml`), `inline_whitespace`,
    /// `all_whitespace`, `underscores`, plus `zero_width`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "html" | "xml" => Some(Self::Html),
            "inline_whitespace" => Some(Self::InlineWhitespace),
            "all_whitespace" => Some(Self::AllWhitespace),
            "underscores" => Some(Self::Underscores),
            "zero_width" => Some(Self::ZeroWidth),
            _ => None,
        }
    }
}

/// What a markup token is.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TagKind {
    Open,
    Close,
    /// `<br>`, `<img>`, `<x/>`: opens nothing.
    SelfClosing,
    /// Comments, doctypes, processing instructions.
    Other,
}

/// A markup token of the source, in source byte offsets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tag {
    pub start: usize,
    pub end: usize,
    /// Lowercase element name; empty for [`TagKind::Other`].
    pub name: String,
    pub kind: TagKind,
}

/// Cleaned text with its exact map back to the source.
#[derive(Clone, Debug, Default)]
pub struct Cleaned {
    pub text: String,
    /// Markup tokens found by [`Step::Html`], in source offsets and order.
    pub tags: Vec<Tag>,
    /// Source range of the character each cleaned byte belongs to.
    starts: Vec<usize>,
    ends: Vec<usize>,
    source_len: usize,
}

#[derive(Clone, Copy, Debug)]
struct Unit {
    character: char,
    start: usize,
    end: usize,
}

impl Cleaned {
    /// The source unchanged, with an identity map.
    pub fn identity(source: &str) -> Self {
        Self::from_units(units(source), source.len(), Vec::new())
    }

    fn from_units(units: Vec<Unit>, source_len: usize, tags: Vec<Tag>) -> Self {
        let mut text = String::with_capacity(units.len());
        let mut starts = Vec::with_capacity(units.len());
        let mut ends = Vec::with_capacity(units.len());
        for unit in &units {
            text.push(unit.character);
            for _ in 0..unit.character.len_utf8() {
                starts.push(unit.start);
                ends.push(unit.end);
            }
        }
        Self {
            text,
            tags,
            starts,
            ends,
            source_len,
        }
    }

    fn units(&self) -> Vec<Unit> {
        self.text
            .char_indices()
            .map(|(offset, character)| Unit {
                character,
                start: self.starts[offset],
                end: self.ends[offset],
            })
            .collect()
    }

    /// Length of the source the map points into.
    pub fn source_len(&self) -> usize {
        self.source_len
    }

    /// The source offset where the cleaned character at `offset` starts;
    /// `text.len()` maps to the end of the last character's source.
    pub fn source_offset(&self, offset: usize) -> Option<usize> {
        if offset == self.text.len() {
            return Some(self.ends.last().copied().unwrap_or(self.source_len));
        }
        self.text.is_char_boundary(offset).then(|| self.starts[offset])
    }

    /// The source range a cleaned range came from. `None` for a range out of
    /// bounds or not on character boundaries.
    pub fn source_range(&self, range: Range<usize>) -> Option<Range<usize>> {
        if range.start > range.end
            || range.end > self.text.len()
            || !self.text.is_char_boundary(range.start)
            || !self.text.is_char_boundary(range.end)
        {
            return None;
        }
        if range.is_empty() {
            let at = self.source_offset(range.start)?;
            return Some(at..at);
        }
        Some(self.starts[range.start]..self.ends[range.end - 1])
    }

    /// A span of the cleaned text re-expressed in `source`.
    pub fn source_span(&self, span: &Span, source: &str) -> Option<Span> {
        let range = self.source_range(span.start..span.end)?;
        Some(Span {
            text: source.get(range.clone())?.to_owned(),
            start: range.start,
            end: range.end,
        })
    }
}

fn units(text: &str) -> Vec<Unit> {
    text.char_indices()
        .map(|(start, character)| Unit {
            character,
            start,
            end: start + character.len_utf8(),
        })
        .collect()
}

/// Clean `source` with `steps` in order.
pub fn clean(source: &str, steps: &[Step]) -> Cleaned {
    apply(units(source), Vec::new(), source.len(), steps)
}

impl Cleaned {
    /// Apply more steps to already cleaned text, keeping the map to the
    /// original source.
    pub fn then(&self, steps: &[Step]) -> Cleaned {
        apply(self.units(), self.tags.clone(), self.source_len, steps)
    }
}

fn apply(mut current: Vec<Unit>, mut tags: Vec<Tag>, source_len: usize, steps: &[Step]) -> Cleaned {
    for step in steps {
        current = match step {
            Step::Html => html(&current, &mut tags),
            Step::InlineWhitespace => collapse(&current, |character| character != '\n' && is_space(character)),
            Step::AllWhitespace => collapse(&current, is_space),
            Step::Underscores => underscores(&current),
            Step::ZeroWidth => current
                .into_iter()
                .filter(|unit| !is_zero_width(unit.character))
                .collect(),
        };
    }
    tags.sort_by_key(|tag: &Tag| tag.start);
    Cleaned::from_units(current, source_len, tags)
}

/// Python's `\s`: Unicode whitespace plus the ASCII information separators.
fn is_space(character: char) -> bool {
    character.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&character)
}

fn is_zero_width(character: char) -> bool {
    matches!(
        character,
        '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}' | '\u{ad}'
    )
}

fn collapse(input: &[Unit], matches: impl Fn(char) -> bool) -> Vec<Unit> {
    let mut output: Vec<Unit> = Vec::with_capacity(input.len());
    let mut in_run = false;
    for unit in input {
        if matches(unit.character) {
            if in_run {
                let last = output.last_mut().expect("run has a unit");
                last.end = last.end.max(unit.end);
            } else {
                output.push(Unit {
                    character: ' ',
                    ..*unit
                });
                in_run = true;
            }
        } else {
            output.push(*unit);
            in_run = false;
        }
    }
    output
}

fn underscores(input: &[Unit]) -> Vec<Unit> {
    let mut output = Vec::with_capacity(input.len());
    let mut position = 0;
    while position < input.len() {
        if input[position].character == '_' {
            let mut end = position;
            while end < input.len() && input[end].character == '_' {
                end += 1;
            }
            if end - position == 1 {
                output.push(input[position]);
            }
            position = end;
        } else {
            output.push(input[position]);
            position += 1;
        }
    }
    output
}

// ---------------------------------------------------------------------------
// HTML

const RAW_TEXT: [&str; 4] = ["script", "style", "title", "template"];
const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr",
];
const PARAGRAPH_BLOCKS: [&str; 17] = [
    "p", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "pre", "table", "ul", "ol", "dl", "hr", "section",
    "article", "aside",
];
const LINE_BLOCKS: [&str; 20] = [
    "address", "dd", "div", "dt", "fieldset", "figcaption", "figure", "footer", "form", "header", "li", "main",
    "nav", "tr", "caption", "body", "html", "tbody", "thead", "tfoot",
];

struct Html<'a> {
    input: &'a [Unit],
    output: Vec<Unit>,
    pre_depth: usize,
}

impl Html<'_> {
    fn char_at(&self, position: usize) -> Option<char> {
        self.input.get(position).map(|unit| unit.character)
    }

    fn starts_with_ignore_case(&self, position: usize, literal: &str) -> bool {
        literal
            .chars()
            .enumerate()
            .all(|(offset, expected)| {
                self.char_at(position + offset)
                    .is_some_and(|character| character.eq_ignore_ascii_case(&expected))
            })
    }

    fn range(&self, start: usize, end: usize) -> (usize, usize) {
        (self.input[start].start, self.input[end - 1].end)
    }

    fn trim_trailing_spaces(&mut self) {
        while self.output.last().is_some_and(|unit| unit.character == ' ') {
            self.output.pop();
        }
    }

    /// Make the output end with `count` newlines (never at its start).
    fn block_break(&mut self, count: usize, source: (usize, usize)) {
        self.trim_trailing_spaces();
        if self.output.is_empty() {
            return;
        }
        let existing = self
            .output
            .iter()
            .rev()
            .take_while(|unit| unit.character == '\n')
            .count();
        for _ in existing..count {
            self.output.push(Unit {
                character: '\n',
                start: source.0,
                end: source.1,
            });
        }
    }

    fn space(&mut self, unit: Unit) {
        match self.output.last() {
            None => {}
            Some(last) if last.character == ' ' || last.character == '\n' => {}
            Some(_) => self.output.push(Unit { character: ' ', ..unit }),
        }
    }

    /// End (exclusive) of the tag opening at `start`, honouring quoted
    /// attribute values.
    fn tag_end(&self, start: usize) -> Option<usize> {
        let mut position = start + 1;
        let mut previous_significant = '<';
        while let Some(character) = self.char_at(position) {
            match character {
                '>' => return Some(position + 1),
                '"' | '\'' if previous_significant == '=' => {
                    let close = (position + 1..self.input.len()).find(|&at| self.input[at].character == character)?;
                    position = close + 1;
                    previous_significant = character;
                    continue;
                }
                _ => {}
            }
            if !character.is_whitespace() {
                previous_significant = character;
            }
            position += 1;
        }
        None
    }

    fn find_ignore_case(&self, from: usize, literal: &str) -> Option<usize> {
        (from..self.input.len()).find(|&position| self.starts_with_ignore_case(position, literal))
    }

    fn entity(&self, start: usize) -> Option<(char, usize)> {
        let mut name = String::new();
        let mut position = start + 1;
        while let Some(character) = self.char_at(position) {
            if character == ';' {
                break;
            }
            if !(character.is_ascii_alphanumeric() || character == '#') || name.len() > 32 {
                return None;
            }
            name.push(character);
            position += 1;
        }
        self.char_at(position).filter(|&character| character == ';')?;
        let decoded = if let Some(number) = name.strip_prefix('#') {
            let value = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse::<u32>().ok()?,
            };
            char::from_u32(value).filter(|&character| character != '\0').unwrap_or('\u{fffd}')
        } else {
            named_entity(&name)?
        };
        Some((decoded, position + 1))
    }

    fn run(mut self, tags: &mut Vec<Tag>) -> Vec<Unit> {
        let mut position = 0;
        while position < self.input.len() {
            let unit = self.input[position];
            match unit.character {
                '<' if self.starts_with_ignore_case(position, "<!--") => {
                    let end = self
                        .find_ignore_case(position + 4, "-->")
                        .map_or(self.input.len(), |close| close + 3);
                    let (start, stop) = self.range(position, end);
                    tags.push(Tag {
                        start,
                        end: stop,
                        name: String::new(),
                        kind: TagKind::Other,
                    });
                    position = end;
                }
                '<' if self
                    .char_at(position + 1)
                    .is_some_and(|next| next.is_ascii_alphabetic() || matches!(next, '/' | '!' | '?')) =>
                {
                    let Some(end) = self.tag_end(position) else {
                        self.output.push(unit);
                        position += 1;
                        continue;
                    };
                    position = self.tag(position, end, tags);
                }
                '&' => match self.entity(position) {
                    Some((character, end)) => {
                        let (start, stop) = self.range(position, end);
                        self.output.push(Unit {
                            character,
                            start,
                            end: stop,
                        });
                        position = end;
                    }
                    None => {
                        self.output.push(unit);
                        position += 1;
                    }
                },
                character if self.pre_depth == 0 && matches!(character, ' ' | '\t' | '\n' | '\r' | '\u{c}') => {
                    self.space(unit);
                    position += 1;
                }
                _ => {
                    self.output.push(unit);
                    position += 1;
                }
            }
        }
        self.output
    }

    /// Handle the tag `start..end`; returns where scanning resumes.
    fn tag(&mut self, start: usize, end: usize, tags: &mut Vec<Tag>) -> usize {
        let inner = self.input[start + 1..end - 1]
            .iter()
            .map(|unit| unit.character)
            .collect::<String>();
        let source = self.range(start, end);
        let (kind, name) = if inner.starts_with(['!', '?']) {
            (TagKind::Other, String::new())
        } else {
            let closing = inner.starts_with('/');
            let name = inner
                .trim_start_matches('/')
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | ':' | '_'))
                .collect::<String>()
                .to_ascii_lowercase();
            let kind = if closing {
                TagKind::Close
            } else if inner.trim_end().ends_with('/') || VOID.contains(&name.as_str()) {
                TagKind::SelfClosing
            } else {
                TagKind::Open
            };
            (kind, name)
        };
        tags.push(Tag {
            start: source.0,
            end: source.1,
            name: name.clone(),
            kind,
        });
        let name = name.as_str();
        if name == "br" && kind != TagKind::Close {
            self.trim_trailing_spaces();
            self.output.push(Unit {
                character: '\n',
                start: source.0,
                end: source.1,
            });
        } else if PARAGRAPH_BLOCKS.contains(&name) {
            self.block_break(2, source);
        } else if LINE_BLOCKS.contains(&name) {
            self.block_break(1, source);
        } else if matches!(name, "td" | "th") && kind == TagKind::Close {
            self.space(Unit {
                character: ' ',
                start: source.0,
                end: source.1,
            });
        }
        if name == "pre" {
            match kind {
                TagKind::Open => self.pre_depth += 1,
                TagKind::Close => self.pre_depth = self.pre_depth.saturating_sub(1),
                _ => {}
            }
        }
        if kind == TagKind::Open && RAW_TEXT.contains(&name) {
            let closing = format!("</{name}");
            if let Some(close) = self.find_ignore_case(end, &closing) {
                if let Some(close_end) = self.tag_end(close) {
                    let (close_start, close_stop) = self.range(close, close_end);
                    tags.push(Tag {
                        start: close_start,
                        end: close_stop,
                        name: name.to_owned(),
                        kind: TagKind::Close,
                    });
                    return close_end;
                }
            }
            return self.input.len();
        }
        end
    }
}

fn html(input: &[Unit], tags: &mut Vec<Tag>) -> Vec<Unit> {
    Html {
        input,
        output: Vec::with_capacity(input.len()),
        pre_depth: 0,
    }
    .run(tags)
}

fn named_entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" | "AMP" => '&',
        "lt" | "LT" => '<',
        "gt" | "GT" => '>',
        "quot" | "QUOT" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "ensp" => '\u{2002}',
        "emsp" => '\u{2003}',
        "thinsp" => '\u{2009}',
        "zwsp" => '\u{200b}',
        "zwnj" => '\u{200c}',
        "zwj" => '\u{200d}',
        "shy" => '\u{ad}',
        "ndash" => '\u{2013}',
        "mdash" => '\u{2014}',
        "hyphen" | "dash" => '\u{2010}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "sbquo" => '\u{201a}',
        "ldquo" => '\u{201c}',
        "rdquo" => '\u{201d}',
        "bdquo" => '\u{201e}',
        "laquo" => '\u{ab}',
        "raquo" => '\u{bb}',
        "lsaquo" => '\u{2039}',
        "rsaquo" => '\u{203a}',
        "hellip" => '\u{2026}',
        "bull" => '\u{2022}',
        "middot" => '\u{b7}',
        "sect" => '\u{a7}',
        "para" => '\u{b6}',
        "copy" => '\u{a9}',
        "reg" => '\u{ae}',
        "trade" => '\u{2122}',
        "deg" => '\u{b0}',
        "plusmn" => '\u{b1}',
        "times" => '\u{d7}',
        "divide" => '\u{f7}',
        "frac12" => '\u{bd}',
        "frac14" => '\u{bc}',
        "frac34" => '\u{be}',
        "sup1" => '\u{b9}',
        "sup2" => '\u{b2}',
        "sup3" => '\u{b3}',
        "ordf" => '\u{aa}',
        "ordm" => '\u{ba}',
        "euro" => '\u{20ac}',
        "pound" => '\u{a3}',
        "cent" => '\u{a2}',
        "yen" => '\u{a5}',
        "dagger" => '\u{2020}',
        "Dagger" => '\u{2021}',
        "prime" => '\u{2032}',
        "iexcl" => '\u{a1}',
        "iquest" => '\u{bf}',
        "Agrave" => '\u{c0}',
        "Aacute" => '\u{c1}',
        "Acirc" => '\u{c2}',
        "Atilde" => '\u{c3}',
        "Auml" => '\u{c4}',
        "Aring" => '\u{c5}',
        "AElig" => '\u{c6}',
        "Ccedil" => '\u{c7}',
        "Egrave" => '\u{c8}',
        "Eacute" => '\u{c9}',
        "Ecirc" => '\u{ca}',
        "Euml" => '\u{cb}',
        "Igrave" => '\u{cc}',
        "Iacute" => '\u{cd}',
        "Icirc" => '\u{ce}',
        "Iuml" => '\u{cf}',
        "Ntilde" => '\u{d1}',
        "Ograve" => '\u{d2}',
        "Oacute" => '\u{d3}',
        "Ocirc" => '\u{d4}',
        "Otilde" => '\u{d5}',
        "Ouml" => '\u{d6}',
        "Oslash" => '\u{d8}',
        "Ugrave" => '\u{d9}',
        "Uacute" => '\u{da}',
        "Ucirc" => '\u{db}',
        "Uuml" => '\u{dc}',
        "Yacute" => '\u{dd}',
        "szlig" => '\u{df}',
        "agrave" => '\u{e0}',
        "aacute" => '\u{e1}',
        "acirc" => '\u{e2}',
        "atilde" => '\u{e3}',
        "auml" => '\u{e4}',
        "aring" => '\u{e5}',
        "aelig" => '\u{e6}',
        "ccedil" => '\u{e7}',
        "egrave" => '\u{e8}',
        "eacute" => '\u{e9}',
        "ecirc" => '\u{ea}',
        "euml" => '\u{eb}',
        "igrave" => '\u{ec}',
        "iacute" => '\u{ed}',
        "icirc" => '\u{ee}',
        "iuml" => '\u{ef}',
        "ntilde" => '\u{f1}',
        "ograve" => '\u{f2}',
        "oacute" => '\u{f3}',
        "ocirc" => '\u{f4}',
        "otilde" => '\u{f5}',
        "ouml" => '\u{f6}',
        "oslash" => '\u{f8}',
        "ugrave" => '\u{f9}',
        "uacute" => '\u{fa}',
        "ucirc" => '\u{fb}',
        "uuml" => '\u{fc}',
        "yacute" => '\u{fd}',
        "yuml" => '\u{ff}',
        "OElig" => '\u{152}',
        "oelig" => '\u{153}',
        _ => return None,
    })
}
