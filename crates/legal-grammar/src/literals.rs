//! The literal strings a grammar's every match must contain: a text holding none of them holds no
//! match, so a caller can pass over it without running the grammar. Read from the same source the
//! grammar compiles from; anything the reading cannot follow (a back reference, a large class, an
//! optional part) only weakens the set or leaves the grammar without one, never wrongs it.

use fancy_regex::{Expr, LookAround};
use regex_syntax::hir::{Class, Hir, HirKind};

/// The most strings one part's exact reading may hold before it is read as "contains" instead.
const EXACT_LIMIT: usize = 64;
/// The most strings a set may hold at all.
const SET_LIMIT: usize = 512;
/// The most characters a class may hold for its characters to be read as literals.
const CLASS_LIMIT: u32 = 12;

#[derive(Clone, Debug)]
enum Reading {
    /// Every match of the part is exactly one of these strings.
    Exact(Vec<String>),
    /// Every match of the part contains one of these strings.
    Contains(Vec<String>),
    /// The part matches nothing but holds only where the text it looks at contains one of these
    /// strings: a positive look-ahead or look-behind.
    Beside(Vec<String>),
    /// Nothing is known.
    Unknown,
}

use Reading::{Beside, Contains, Exact, Unknown};

fn usable(strings: &[String]) -> bool {
    !strings.is_empty() && strings.iter().all(|string| !string.is_empty())
}

/// How selective a "contains" set is: longer shortest strings first, then fewer strings.
fn quality(strings: &[String]) -> (usize, std::cmp::Reverse<usize>) {
    (strings.iter().map(String::len).min().unwrap_or(0), std::cmp::Reverse(strings.len()))
}

fn dedup(mut strings: Vec<String>) -> Vec<String> {
    strings.sort();
    strings.dedup();
    strings
}

fn concat(parts: impl IntoIterator<Item = Reading>) -> Reading {
    let mut best: Option<Vec<String>> = None;
    let consider = |strings: Vec<String>, best: &mut Option<Vec<String>>| {
        if usable(&strings) && strings.len() <= SET_LIMIT
            && best.as_ref().is_none_or(|current| quality(&strings) > quality(current)) {
            *best = Some(strings);
        }
    };
    let mut run: Option<Vec<String>> = Some(vec![String::new()]);
    let mut whole = true;
    for part in parts {
        match part {
            Exact(strings) => {
                let current = run.take().unwrap_or_else(|| vec![String::new()]);
                if current.len() * strings.len() <= EXACT_LIMIT {
                    run = Some(dedup(current.iter().flat_map(|left| strings.iter().map(move |right| format!("{left}{right}"))).collect()));
                } else {
                    whole = false;
                    consider(current, &mut best);
                    run = Some(strings);
                }
            }
            Contains(strings) => {
                whole = false;
                if let Some(current) = run.take() { consider(current, &mut best); }
                consider(strings, &mut best);
            }
            // It matches nothing, so the strings on either side of it stay one run.
            Beside(strings) => consider(strings, &mut best),
            Unknown => {
                whole = false;
                if let Some(current) = run.take() { consider(current, &mut best); }
            }
        }
    }
    if whole { return Exact(run.unwrap_or_default()); }
    if let Some(current) = run { consider(current, &mut best); }
    best.map_or(Unknown, Contains)
}

fn alternation(parts: impl IntoIterator<Item = Reading>) -> Reading {
    let mut strings = Vec::new();
    let mut exact = true;
    for part in parts {
        match part {
            Exact(part) => strings.extend(part),
            Contains(part) | Beside(part) => { exact = false; strings.extend(part); }
            Unknown => return Unknown,
        }
    }
    let strings = dedup(strings);
    if exact && strings.len() <= EXACT_LIMIT { return Exact(strings); }
    if usable(&strings) && strings.len() <= SET_LIMIT { Contains(strings) } else { Unknown }
}

fn repetition(child: Reading, min: usize, max: Option<usize>) -> Reading {
    match (min, max, child) {
        (_, Some(0), _) => Exact(vec![String::new()]),
        (1, Some(1), child) => child,
        (0, Some(1), Exact(mut strings)) => { strings.push(String::new()); Exact(dedup(strings)) }
        (0, _, _) => Unknown,
        (_, _, Exact(strings) | Contains(strings) | Beside(strings)) if usable(&strings) => Contains(strings),
        _ => Unknown,
    }
}

fn hir(hir: &Hir) -> Reading {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => Exact(vec![String::new()]),
        HirKind::Literal(literal) => std::str::from_utf8(&literal.0).map_or(Unknown, |text| Exact(vec![text.to_owned()])),
        HirKind::Class(Class::Unicode(class)) => {
            let count = class.ranges().iter().map(|range| range.end() as u32 - range.start() as u32 + 1).sum::<u32>();
            if count == 0 || count > CLASS_LIMIT { return Unknown; }
            Exact(class.ranges().iter().flat_map(|range| (range.start()..=range.end()).map(String::from)).collect())
        }
        HirKind::Class(Class::Bytes(class)) => {
            let count = class.ranges().iter().map(|range| range.end() as u32 - range.start() as u32 + 1).sum::<u32>();
            if count == 0 || count > CLASS_LIMIT || class.ranges().iter().any(|range| !range.end().is_ascii()) { return Unknown; }
            Exact(class.ranges().iter().flat_map(|range| (range.start()..=range.end()).map(|byte| char::from(byte).to_string())).collect())
        }
        HirKind::Repetition(repeat) => repetition(self::hir(&repeat.sub), repeat.min as usize, repeat.max.map(|max| max as usize)),
        HirKind::Capture(capture) => self::hir(&capture.sub),
        HirKind::Concat(parts) => concat(parts.iter().map(self::hir)),
        HirKind::Alternation(parts) => alternation(parts.iter().map(self::hir)),
    }
}

fn expr(expr: &Expr) -> Reading {
    match expr {
        // What a positive look-around reads must be in the text, though not in the match.
        Expr::LookAround(child, LookAround::LookAhead | LookAround::LookBehind) => match self::expr(child) {
            Exact(strings) | Contains(strings) | Beside(strings) if usable(&strings) => Beside(strings),
            _ => Exact(vec![String::new()]),
        },
        Expr::Empty | Expr::Assertion(_) | Expr::LookAround(..) | Expr::KeepOut
        | Expr::ContinueFromPreviousMatchEnd => Exact(vec![String::new()]),
        Expr::Literal { val, casei: false } => Exact(vec![val.clone()]),
        Expr::Delegate { inner, casei } => regex_syntax::ParserBuilder::new().case_insensitive(*casei).build()
            .parse(inner).map_or(Unknown, |parsed| hir(&parsed)),
        Expr::Concat(parts) => concat(parts.iter().map(self::expr)),
        Expr::Alt(parts) => alternation(parts.iter().map(self::expr)),
        Expr::Group(child) => self::expr(child),
        Expr::AtomicGroup(child) => self::expr(child),
        Expr::Repeat { child, lo, hi, .. } =>
            repetition(self::expr(child), *lo, (*hi != usize::MAX).then_some(*hi)),
        _ => Unknown,
    }
}

fn finish(reading: Reading) -> Option<Vec<String>> {
    match reading {
        Exact(strings) | Contains(strings) | Beside(strings) if usable(&strings) && strings.len() <= SET_LIMIT => Some(strings),
        _ => None,
    }
}

/// The literals every match of a backtracking grammar's compiled `source` contains.
pub(crate) fn backtracking(source: &str, case_insensitive: bool) -> Option<Vec<String>> {
    // fancy-regex's parse flags: case-insensitive (1) and Unicode (1 << 5), as the grammar compiles.
    let flags = if case_insensitive { 1 | 1 << 5 } else { 1 << 5 };
    finish(expr(&Expr::parse_tree_with_flags(source, flags).ok()?.expr))
}

fn parse_linear(source: &str, case_insensitive: bool, multi_line: bool, dot_matches_new_line: bool) -> Option<Hir> {
    regex_syntax::ParserBuilder::new().case_insensitive(case_insensitive).multi_line(multi_line)
        .dot_matches_new_line(dot_matches_new_line).build().parse(source).ok()
}

/// The literals every match of a linear grammar's compiled `source` contains.
pub(crate) fn linear(source: &str, case_insensitive: bool, multi_line: bool, dot_matches_new_line: bool) -> Option<Vec<String>> {
    finish(hir(&parse_linear(source, case_insensitive, multi_line, dot_matches_new_line)?))
}

/// The named group a linear grammar's every match opens with, so that the group starts where the
/// match does.
pub(crate) fn leading_group(source: &str, case_insensitive: bool, multi_line: bool, dot_matches_new_line: bool) -> Option<String> {
    let parsed = parse_linear(source, case_insensitive, multi_line, dot_matches_new_line)?;
    let first = match parsed.kind() {
        HirKind::Concat(parts) => parts.first()?,
        _ => &parsed,
    };
    match first.kind() {
        HirKind::Capture(capture) => capture.name.as_deref().map(str::to_owned),
        _ => None,
    }
}
