//! Citations that name a registry row: a reporter, a statute or regulation series, a code, a journal. Each row
//! carries the shapes its citations take, written in reporters-db's template language: its own `regexes`, or its
//! kind's (`build_registry.rs`), over reporters-db's variables and the authored ones beside them
//! (`registry/regexes.json`). A row an upstream sync adds is read with no change here.
//!
//! One scan finds every name the selected rows give in the text. Each shape is read around a name in two parts:
//! what precedes the name (its head, anchored where the name starts) and what follows it (its tail, anchored where it
//! ends). A row whose name is itself a pattern ("Tex. $law_subject Code Ann.") has a shape with no slot, read whole
//! where the text holds the literals every match of it contains.
use aho_corasick::{AhoCorasick, MatchKind};
use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, LazyLock, Mutex};

/// Where the row's names go in an expanded shape: reporters-db's `$reporter`, whose `$edition` is left for them.
const SLOT: &str = "(?P<reporter>$edition)";
/// How far before a name its head is looked for.
const HEAD_WINDOW: usize = 160;

/// The registry table a named row comes from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Table {
    Reporters,
    Laws,
    Journals,
    /// A court a neutral citation names.
    Courts,
}

/// A registry row as a citation names it: one edition of a reporter, a law series or code, a journal.
#[derive(Debug, Deserialize)]
pub struct Row {
    pub table: Table,
    /// The registry row's id, where the registry indexes it.
    #[serde(default)]
    pub id: Option<String>,
    /// The edition's or series' own name (`F.3d`, `RSC`, `U.S.C.`).
    pub edition: String,
    /// `authored`, `mcgill` or `reporters-db`.
    pub source: String,
    pub jurisdiction: String,
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub end: Option<String>,
    /// Other names the row is written by (`S.C.R.`, `R.C.S.`, `LRC`).
    pub variations: Vec<String>,
    /// Citations the row's shapes must read.
    pub examples: Vec<String>,
    shapes: Vec<usize>,
}

impl Row {
    /// The row's shapes, by position in [`shapes`].
    pub fn shapes(&self) -> impl Iterator<Item = usize> + '_ {
        self.shapes.iter().copied()
    }
}

#[derive(Deserialize)]
struct Data {
    shapes: Vec<String>,
    rows: Vec<Row>,
    conventions: Conventions,
}

/// What a writer adds around a volume, expanded: the year before it (`commonwealth_year_before`) and the issue after
/// it (`commonwealth_issue`), and the volume they are read around (reporters-db's `volume`).
#[derive(Deserialize)]
struct Conventions {
    year_before: String,
    issue: String,
    volume: String,
}

static DATA: LazyLock<Data> = LazyLock::new(|| {
    let compressed = include_bytes!(concat!(env!("OUT_DIR"), "/named.json.gz"));
    serde_json::from_reader(flate2::read::GzDecoder::new(compressed.as_slice())).expect("compiled named rows")
});

/// Every named row.
pub fn rows() -> &'static [Row] {
    &DATA.rows
}

/// The expanded shapes, in reporters-db's (Python) dialect, `SLOT` marking where a row's name goes.
pub fn shapes() -> &'static [String] {
    &DATA.shapes
}

/// Which rows a reading loads and how loosely it reads them. Rows left out are never scanned for.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Selection {
    /// The jurisdictions whose rows are read: a row whose jurisdiction is one of these, or within one (`ca-on` within
    /// `ca`). Every row when `None`.
    pub jurisdictions: Option<Vec<String>>,
    pub reporters: bool,
    pub laws: bool,
    pub journals: bool,
    pub courts: bool,
    /// Rows written for this engine (`authored`, `mcgill`).
    pub authored: bool,
    /// Rows from reporters-db.
    pub upstream: bool,
    /// Whitespace anywhere inside a name ("S C R", "S.C.R .", "Wis. L.\nRev.") and any run of whitespace where a shape
    /// writes one space. Without it whitespace between two letters or digits of a name must be written, and a shape's
    /// space is one space, as eyecite reads them. Punctuation inside a name is never required: a name matches where
    /// its letters and digits do, as the registry folds it ("C.R.N.S." for CR (NS)).
    pub spacing: bool,
    /// A name's letters after its first in either case, for a name of more than three ("FCJ no", "EWCA CIV").
    pub case: bool,
    /// The short form's "at" before its page ("123 F.3d at 456").
    pub short: bool,
    /// The year a writer puts before any volume: "[1987] 36 CCC (3d) 417", "(1989), 49 NBR 144", "(1981), 401 U.S. 424".
    pub year_before: bool,
    /// The issue a writer puts after any volume: "43:2 McGill LJ 287".
    pub issue: bool,
    /// A Roman page ("1 S.C.R. ix", "5 P. xi"). An Arabic page and a slip opinion's blank ("556 U.S. ____") are always read.
    pub roman_pages: bool,
}

impl Default for Selection {
    fn default() -> Self {
        Self { jurisdictions: None, reporters: true, laws: true, journals: true, courts: true, authored: true, upstream: true,
            spacing: true, case: true, short: true, year_before: true, issue: true, roman_pages: false }
    }
}

impl Selection {
    fn selects(&self, row: &Row) -> bool {
        let table = match row.table {
            Table::Reporters => self.reporters, Table::Laws => self.laws, Table::Journals => self.journals, Table::Courts => self.courts,
        };
        let source = if row.source == "reporters-db" { self.upstream } else { self.authored };
        table && source && self.jurisdictions.as_ref().is_none_or(|jurisdictions| jurisdictions.iter().any(|jurisdiction|
            row.jurisdiction == *jurisdiction || row.jurisdiction.strip_prefix(jurisdiction.as_str()).is_some_and(|rest| rest.starts_with('-'))))
    }
}

/// A citation one shape of one or more rows reads.
#[derive(Clone, Debug)]
pub struct Named {
    /// The citation core.
    pub span: Range<usize>,
    /// The name inside it.
    pub name: Range<usize>,
    /// The rows it names by a name of theirs, then those it names by a variation.
    pub rows: Vec<&'static Row>,
    pub variations: Vec<&'static Row>,
    /// Each named group the shape reads, `reporter` being the name as written.
    pub groups: Vec<(String, String)>,
    /// Where the short form's "at" starts, from the core's start.
    pub short_at: Option<usize>,
}

/// One shape compiled for a reading: around a name, or whole.
enum Shape {
    Slot { head: Option<Regex>, tail: Regex, short: Option<Regex> },
    Whole { pattern: Regex, literals: Option<AhoCorasick> },
}

/// A selection's rows, ready to scan for.
struct Index {
    /// Every name and variation, compacted ([`compact`]), and the automaton finding them.
    names: Vec<String>,
    automaton: Option<AhoCorasick>,
    /// For each name, the punctuation its surfaces end with past what [`name_end`] reads ("," of "Chase,").
    endings: Vec<Vec<String>>,
    /// For each name, the marks other than a period its surfaces hold within them ("," of "Holt, Eq", "&" of "Nfld &
    /// PEIR"): no name is read across another's comma, nor across another mark in another case ("Act, SC" and "Act
    /// (S.C." are no ACTSC).
    marks: Vec<String>,
    /// For each name: the rows it names, and whether by a name of theirs rather than a variation.
    rows: Vec<Vec<(usize, bool)>>,
    /// The shapes the selected rows use, by position in [`shapes`].
    shapes: HashMap<usize, Shape>,
    /// For each whole shape, the rows giving it.
    whole: Vec<(usize, Vec<usize>)>,
}

static INDEXES: LazyLock<Mutex<HashMap<Selection, Arc<Index>>>> = LazyLock::new(Default::default);

fn index(selection: &Selection) -> Arc<Index> {
    let mut indexes = INDEXES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    indexes.entry(selection.clone()).or_insert_with(|| Arc::new(Index::new(selection))).clone()
}

impl Index {
    fn new(selection: &Selection) -> Self {
        let mut positions: HashMap<String, usize> = HashMap::new();
        let mut names: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<(usize, bool)>> = Vec::new();
        let mut endings: Vec<Vec<String>> = Vec::new();
        let mut marks: Vec<String> = Vec::new();
        let mut shapes = HashMap::new();
        let mut whole: HashMap<usize, Vec<usize>> = HashMap::new();
        for (at, row) in DATA.rows.iter().enumerate().filter(|(_, row)| selection.selects(row)) {
            for &shape in &row.shapes {
                if !DATA.shapes[shape].contains(SLOT) { whole.entry(shape).or_default().push(at); }
                shapes.entry(shape).or_insert_with(|| compile(&DATA.shapes[shape], selection));
            }
            if row.shapes.iter().all(|&shape| !DATA.shapes[shape].contains(SLOT)) { continue; }
            for (surface, exact) in std::iter::once((&row.edition, true)).chain(row.variations.iter().map(|variation| (variation, false))) {
                let name = compact(surface, selection.spacing).0;
                if name.is_empty() { continue; }
                let position = *positions.entry(name.clone()).or_insert_with(|| {
                    names.push(name);
                    rows.push(Vec::new());
                    endings.push(Vec::new());
                    marks.push(String::new());
                    rows.len() - 1
                });
                if !rows[position].contains(&(at, exact)) { rows[position].push((at, exact)); }
                let inner = surface.trim().trim_start_matches(|character: char| !character.is_alphanumeric())
                    .trim_end_matches(|character: char| !character.is_alphanumeric());
                for mark in inner.chars().filter(|character| !character.is_alphanumeric() && !character.is_whitespace() && *character != '.') {
                    if !marks[position].contains(mark) { marks[position].push(mark); }
                }
                let ending = surface.trim_end().trim_end_matches(['.', ')', ']']);
                let ending = &ending[ending.trim_end_matches(|character: char| !character.is_alphanumeric()).len()..];
                if !ending.trim().is_empty() && !endings[position].iter().any(|known| known == ending.trim()) {
                    endings[position].push(ending.trim().to_owned());
                }
            }
        }
        let automaton = (!names.is_empty()).then(|| AhoCorasick::builder().match_kind(MatchKind::Standard)
            .ascii_case_insensitive(selection.case).build(&names).expect("named row names"));
        let mut whole: Vec<_> = whole.into_iter().collect();
        whole.sort();
        Self { names, automaton, endings, marks, rows, shapes, whole }
    }
}

/// A shape in reporters-db's dialect as this reading reads it: with `spacing`, each space outside a character class
/// is whitespace, a run of it where the space is not itself quantified (`" ?"` is any whitespace or none).
fn spaced(pattern: &str, spacing: bool) -> String {
    if !spacing { return pattern.to_owned(); }
    let mut output = String::with_capacity(pattern.len());
    let (mut class, mut characters) = (false, pattern.chars().peekable());
    while let Some(character) = characters.next() {
        match character {
            '\\' => { output.push(character); output.extend(characters.next()); }
            '[' if !class => { class = true; output.push(character); }
            ']' if class => { class = false; output.push(character); }
            ' ' if !class => match characters.peek() {
                Some('?' | '*') => { characters.next(); output.push_str(r"\s*"); }
                Some('+' | '{') => output.push_str(r"\s"),
                _ => output.push_str(r"\s+"),
            },
            _ => output.push(character),
        }
    }
    output
}

fn python(pattern: &str) -> Regex {
    legal_grammar::compile_python_pattern(pattern, "").unwrap_or_else(|error| panic!("named shape {pattern}: {error}"))
}

fn compile(shape: &str, selection: &Selection) -> Shape {
    let (spacing, short) = (selection.spacing, selection.short);
    let Some((head, tail)) = shape.split_once(SLOT) else {
        let pattern = spaced(shape, spacing);
        let literals = legal_grammar::python_pattern_literals(&pattern, "").ok().flatten()
            .map(|literals| AhoCorasick::new(literals).expect("named shape literals"));
        return Shape::Whole { pattern: python(&format!(r"(?:^|[^A-Za-z0-9])(?P<__core>{pattern})(?:[^A-Za-z0-9]|$)")), literals };
    };
    // A citation by volume that states no year of its own may follow the year its writer gives, and its volume may
    // carry an issue.
    let conventions = &DATA.conventions;
    let mut head = head.to_owned();
    if let Some(rest) = head.strip_prefix(conventions.volume.as_str()) {
        if selection.issue { head = format!("{}{}{rest}", conventions.volume, conventions.issue); }
        if selection.year_before && !head.contains("(?P<year") { head = format!("(?:{})?{head}", conventions.year_before); }
    }
    let (head, tail) = (spaced(&head, spacing), spaced(tail, spacing));
    // Eyecite's short form: "at" (and "p.", "page") before the page.
    let short_tail = tail.replace("(?P<page>", r"(?P<__short_at>at)\s?(?:p(?:\.|age)?)? (?P<page>");
    let tail_of = |tail: &str| python(&format!(r"^(?P<__tail>{tail})(?:[^A-Za-z0-9]|$)"));
    Shape::Slot {
        // A head that opens with a bracket opens at it whatever precedes it ("1[1942] 1 D.L.R. 81", a note's number).
        head: (!head.is_empty()).then(|| python(&format!(r"(?:^|[^A-Za-z0-9]|(?=[\[(]))(?P<__head>{head})$"))),
        short: (short && short_tail != tail).then(|| tail_of(&short_tail)),
        tail: tail_of(&tail),
    }
}

/// The text a name is matched in: its letters and digits, as the registry folds a surface, and without `spacing` one
/// space for whitespace between two of them. With it, where each of its bytes' characters starts and ends in `text`.
fn compact(text: &str, spacing: bool) -> (String, Vec<usize>, Vec<usize>) {
    let (mut compacted, mut starts, mut ends) = (String::with_capacity(text.len()), Vec::new(), Vec::new());
    let mut characters = text.char_indices().peekable();
    let mut previous: Option<char> = None;
    while let Some((at, character)) = characters.next() {
        if character.is_whitespace() {
            while characters.next_if(|(_, next)| next.is_whitespace()).is_some() {}
            let between = previous.is_some_and(char::is_alphanumeric) && characters.peek().is_some_and(|(_, next)| next.is_alphanumeric());
            if !spacing && between {
                compacted.push(' ');
                starts.push(at);
                ends.push(at + character.len_utf8());
            }
        } else if character.is_alphanumeric() {
            for _ in 0..character.len_utf8() { starts.push(at); ends.push(at + character.len_utf8()); }
            compacted.push(character);
        }
        previous = Some(character);
    }
    (compacted, starts, ends)
}

/// Where a name a scan found ends: after its last letter or digit, and the periods that close it ("S.C.R.", with
/// `spacing` "S.C.R ."), and the bracket closing one it opened ("D.L.R. (4th)", "Empl Prac Dec [CCH]").
fn name_end(text: &str, start: usize, mut end: usize, spacing: bool) -> usize {
    loop {
        let rest = &text[end..];
        let space = if spacing { rest.len() - rest.trim_start().len() } else { 0 };
        let open = |opening: char, closing: char| text[start..end].matches(opening).count() > text[start..end].matches(closing).count();
        match rest[space..].chars().next() {
            Some('.') => end += space + 1,
            Some(')') if open('(', ')') => end += space + 1,
            Some(']') if open('[', ']') => end += space + 1,
            _ => return end,
        }
    }
}

fn groups(captures: &fancy_regex::Captures<'_, str>, names: fancy_regex::CaptureNames<'_>, into: &mut Vec<(String, String)>) {
    for name in names.flatten().filter(|name| !name.starts_with("__")) {
        if let Some(value) = captures.name(name) { into.push((name.to_owned(), value.as_str().to_owned())); }
    }
}

fn arabic(groups: &[(String, String)]) -> bool {
    groups.iter().filter(|(name, _)| name == "page").all(|(_, page)| page.chars().all(|character| character.is_ascii_digit() || character == '_'))
}

/// Every citation the selected rows name in `text`, overlapping ones resolved as [`resolve`] does.
pub fn find(text: &str, selection: &Selection) -> Vec<Named> {
    let index = index(selection);
    let mut found = Vec::new();
    if let Some(automaton) = &index.automaton {
        let (compacted, starts, ends) = compact(text, selection.spacing);
        for hit in automaton.find_overlapping_iter(&compacted) {
            let name = &index.names[hit.pattern().as_usize()];
            // Only a name of more than three letters and digits may differ in case, and only after its first: "FCJ no",
            // "EWCA CIV", but not "Le" for L.E., "In" for I.N. or "so" for So.
            if selection.case && (compacted[hit.start()..].chars().next() != name.chars().next()
                || name.chars().count() <= 3 && compacted[hit.start()..hit.end()] != *name) { continue; }
            let start = starts[hit.start()];
            // A comma or semicolon parts two names; any other mark does too where the name is read in another case.
            let (marks, cased) = (&index.marks[hit.pattern().as_usize()], compacted[hit.start()..hit.end()] == *name);
            if text[start..ends[hit.end() - 1]].chars().any(|character| !character.is_alphanumeric() && !character.is_whitespace()
                && character != '.' && !marks.contains(character) && (!cased || matches!(character, ',' | ';'))) { continue; }
            let end = name_end(text, start, ends[hit.end() - 1], selection.spacing);
            let rows = &index.rows[hit.pattern().as_usize()];
            read_name(text, start..end, rows, &index, selection, &mut found);
            for ending in &index.endings[hit.pattern().as_usize()] {
                if text[end..].starts_with(ending.as_str()) {
                    read_name(text, start..end + ending.len(), rows, &index, selection, &mut found);
                }
            }
        }
    }
    for (shape, rows) in &index.whole {
        let Some(Shape::Whole { pattern, literals }) = index.shapes.get(shape) else { continue };
        if literals.as_ref().is_some_and(|literals| !literals.is_match(text)) { continue; }
        for captures in pattern.captures_iter(text).flatten() {
            let core = captures.name("__core").unwrap();
            let mut read = Vec::new();
            groups(&captures, pattern.capture_names(), &mut read);
            if !selection.roman_pages && !arabic(&read) { continue; }
            let name = captures.name("reporter").map_or(core.start()..core.end(), |name| name.start()..name.end());
            found.push(Named { span: core.start()..core.end(), name, rows: rows.iter().map(|&at| &DATA.rows[at]).collect(),
                variations: Vec::new(), groups: read, short_at: None });
        }
    }
    resolve(found, text)
}

/// The citations each shape of a name's rows reads around it.
fn read_name(text: &str, name: Range<usize>, named: &[(usize, bool)], index: &Index, selection: &Selection, found: &mut Vec<Named>) {
    let mut by_shape: Vec<(usize, Vec<&'static Row>, Vec<&'static Row>)> = Vec::new();
    for &(at, exact) in named {
        let row = &DATA.rows[at];
        for &shape in &row.shapes {
            let position = by_shape.iter().position(|(other, _, _)| *other == shape).unwrap_or_else(|| {
                by_shape.push((shape, Vec::new(), Vec::new()));
                by_shape.len() - 1
            });
            let (_, rows, variations) = &mut by_shape[position];
            if exact { rows.push(row) } else { variations.push(row) }
        }
    }
    for (shape, rows, variations) in by_shape {
        let Some(Shape::Slot { head, tail, short }) = index.shapes.get(&shape) else { continue };
        // With `spacing`, a name of two letters or more its period closes may run into a page of two digits or more
        // ("[1974] 1 W.W.R.481"), read as if spaced; less is too little to tell from garbled text or a chapter ("65 L.7 .
        // Ch . 521", "R.S.O. 1990 C. F.3").
        let digits = text[name.end..].chars().take_while(char::is_ascii_digit).count();
        let joined = selection.spacing && text[..name.end].ends_with('.') && digits >= 2
            && text[name.clone()].chars().filter(|character| character.is_alphanumeric()).count() >= 2;
        let padded = if joined { format!(" {}", &text[name.end..]) } else { String::new() };
        let (rest, shift) = if joined { (padded.as_str(), 1) } else { (&text[name.end..], 0) };
        let (tail_captures, tail) = match tail.captures(rest).ok().flatten() {
            Some(captures) => (captures, tail),
            None => match short.as_ref().and_then(|short| short.captures(rest).ok().flatten().map(|captures| (captures, short))) {
                Some(found) => found,
                None => continue,
            },
        };
        let mut read = Vec::new();
        let start = match head {
            // A citation that opens with its name opens at a boundary, as a head does.
            None if text[..name.start].ends_with(|character: char| character.is_ascii_alphanumeric()) => continue,
            None => name.start,
            Some(head) => {
                let mut window = name.start.saturating_sub(HEAD_WINDOW);
                while !text.is_char_boundary(window) { window += 1; }
                let Some(captures) = head.captures(&text[window..name.start]).ok().flatten() else { continue };
                let matched = captures.name("__head").unwrap();
                // A head the window's edge, not a boundary, opens.
                if matched.start() == 0 && text[..window].ends_with(|character: char| character.is_ascii_alphanumeric()) { continue; }
                groups(&captures, head.capture_names(), &mut read);
                window + matched.start()
            }
        };
        read.push(("reporter".to_owned(), text[name.clone()].to_owned()));
        groups(&tail_captures, tail.capture_names(), &mut read);
        if !selection.roman_pages && !arabic(&read) { continue; }
        let end = name.end + tail_captures.name("__tail").unwrap().end() - shift;
        let short_at = tail_captures.name("__short_at").map(|at| name.end + at.start() - shift - start);
        found.push(Named { span: start..end, name: name.clone(), rows, variations, groups: read, short_at });
    }
}

/// One reading per extent, the rows read by the same extent merged. Of two that overlap, one whose end the other
/// begins with nothing but its digits gives way (its page is the next citation's volume or year: "S.A., 2016 ONSC
/// 2810"); otherwise the longer is kept, the earlier of two as long.
fn resolve(mut found: Vec<Named>, text: &str) -> Vec<Named> {
    found.sort_by_key(|named| (named.span.start, std::cmp::Reverse(named.span.end)));
    let mut merged: Vec<Named> = Vec::new();
    for named in found {
        match merged.last_mut() {
            Some(previous) if previous.span == named.span => {
                for row in named.rows { if !previous.rows.iter().any(|kept| std::ptr::eq(*kept, row)) { previous.rows.push(row); } }
                for row in named.variations {
                    if !previous.rows.iter().chain(&previous.variations).any(|kept| std::ptr::eq(*kept, row)) { previous.variations.push(row); }
                }
            }
            _ => merged.push(named),
        }
    }
    let mut kept: Vec<Named> = Vec::new();
    for named in merged {
        match kept.last() {
            Some(previous) if named.span.start < previous.span.end => {
                let shared = &text[named.span.start..previous.span.end.min(named.span.end)];
                let yields = named.span.start > previous.span.start && previous.span.end <= named.span.end
                    && shared.bytes().all(|byte| byte.is_ascii_digit());
                if yields || named.span.len() > previous.span.len() {
                    kept.pop();
                    kept.push(named);
                }
            }
            _ => kept.push(named),
        }
    }
    kept
}

/// Build this module's lazily built statics now ([`crate::warm`]).
pub(crate) fn warm() {
    let _ = &*DATA;
    index(&Selection::default());
}
