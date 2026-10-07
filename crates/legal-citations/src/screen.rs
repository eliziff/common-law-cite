//! Which of a set of grammars can match a text at all, from one pass over it: a grammar whose every
//! match contains one of a few literal strings ([`legal_grammar::table_entry_literals`]) cannot
//! match a text that holds none of them, so it need not run there.

use aho_corasick::AhoCorasick;
use legal_grammar::{CompiledEcmascriptGrammar, CompiledGrammar};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub(crate) enum Dialect {
    /// [`legal_grammar::compile_ecmascript_table_entry`].
    Linear,
    /// [`legal_grammar::compile_table_entry`].
    Backtracking,
}

impl Dialect {
    fn literals(self, id: &str) -> Option<Vec<String>> {
        match self {
            Self::Linear => legal_grammar::ecmascript_table_entry_literals(id),
            Self::Backtracking => legal_grammar::table_entry_literals(id),
        }.unwrap_or_else(|error| panic!("{error}"))
    }
}

/// A grammar with the literals one of which its every match contains, compiled the first time a
/// text that holds one of them is searched.
pub(crate) struct Screened<G> {
    grammar: OnceLock<G>,
    compile: Box<dyn Fn() -> G + Send + Sync>,
    literals: Option<AhoCorasick>,
    /// The named group every match opens with.
    leading: Option<String>,
}

impl<G> Screened<G> {
    pub(crate) fn new(compile: impl Fn() -> G + Send + Sync + 'static, literals: Option<Vec<String>>) -> Self {
        Self { grammar: OnceLock::new(), compile: Box::new(compile),
            literals: literals.map(|literals| AhoCorasick::new(literals).expect("grammar literals")), leading: None }
    }

    /// Whether the grammar can match `text`: false when `text` holds none of its literals.
    pub(crate) fn may_match(&self, text: &str) -> bool {
        self.literals.as_ref().is_none_or(|literals| literals.is_match(text))
    }

    fn grammar(&self) -> &G {
        self.grammar.get_or_init(|| (self.compile)())
    }

    /// Compile the grammar now ([`crate::warm`]).
    pub(crate) fn warm(&self) {
        self.grammar();
    }
}

impl<G> Deref for Screened<G> {
    type Target = G;
    fn deref(&self) -> &G { self.grammar() }
}

fn or_panic<T>(result: legal_grammar::Result<T>) -> T {
    result.unwrap_or_else(|error| panic!("{error}"))
}

impl Screened<CompiledGrammar> {
    /// [`legal_grammar::compile_table_entry`] with its literals.
    pub(crate) fn backtracking(id: &'static str) -> Self {
        Self::new(move || or_panic(legal_grammar::compile_table_entry(id)), Dialect::Backtracking.literals(id))
    }

    /// [`legal_grammar::compile_python_table_entry`] with its literals.
    pub(crate) fn python(id: &'static str) -> Self {
        Self::new(move || or_panic(legal_grammar::compile_python_table_entry(id)),
            or_panic(legal_grammar::python_table_entry_literals(id)))
    }

    /// Every match in `text`, as [`fancy_regex::Regex::find_iter`] finds them.
    pub(crate) fn find_all<'t>(&'t self, text: &'t str) -> impl Iterator<Item = fancy_regex::Result<fancy_regex::Match<'t>>> + 't {
        self.may_match(text).then(|| self.grammar().find_iter(text)).into_iter().flatten()
    }

    pub(crate) fn captures_all<'t>(&'t self, text: &'t str) -> impl Iterator<Item = fancy_regex::Result<legal_grammar::GrammarCaptures<'t>>> + 't {
        self.may_match(text).then(|| self.grammar().captures_iter(text)).into_iter().flatten()
    }

    pub(crate) fn find_screened<'t>(&self, text: &'t str) -> fancy_regex::Result<Option<fancy_regex::Match<'t>>> {
        if self.may_match(text) { self.grammar().find(text) } else { Ok(None) }
    }

    pub(crate) fn is_match_screened(&self, text: &str) -> fancy_regex::Result<bool> {
        if self.may_match(text) { self.grammar().is_match(text) } else { Ok(false) }
    }
}

impl Screened<CompiledEcmascriptGrammar> {
    /// [`legal_grammar::compile_ecmascript_table_entry`] with its literals.
    pub(crate) fn linear(id: &'static str) -> Self {
        let mut screened = Self::new(move || or_panic(legal_grammar::compile_ecmascript_table_entry(id)),
            Dialect::Linear.literals(id));
        screened.leading = or_panic(legal_grammar::ecmascript_table_entry_leading_group(id));
        screened
    }

    /// [`Self::linear`] with `bytes` of room for its lazily built automaton
    /// ([`legal_grammar::compile_ecmascript_table_entry_with_dfa_size`]).
    pub(crate) fn linear_with_dfa_size(id: &'static str, bytes: usize) -> Self {
        let mut screened = Self::linear(id);
        screened.compile = Box::new(move || or_panic(legal_grammar::compile_ecmascript_table_entry_with_dfa_size(id, bytes)));
        screened
    }

    /// Where `group` starts in the first match in `text`. A group every match opens with starts
    /// where the match does, which needs no capture engine.
    pub(crate) fn group_start(&self, group: &str, text: &str) -> Option<usize> {
        if self.leading.as_deref() == Some(group) {
            self.find_screened(text).map(|found| found.start())
        } else {
            self.captures_screened(text).and_then(|captures| captures.name(group)).map(|found| found.start())
        }
    }

    pub(crate) fn find_all<'t>(&'t self, text: &'t str) -> impl Iterator<Item = regex::Match<'t>> + 't {
        self.may_match(text).then(|| self.grammar().find_iter(text)).into_iter().flatten()
    }

    pub(crate) fn captures_all<'t>(&'t self, text: &'t str) -> impl Iterator<Item = regex::Captures<'t>> + 't {
        self.may_match(text).then(|| self.grammar().captures_iter(text)).into_iter().flatten()
    }

    pub(crate) fn captures_screened<'t>(&self, text: &'t str) -> Option<regex::Captures<'t>> {
        if self.may_match(text) { self.grammar().captures(text) } else { None }
    }

    pub(crate) fn find_screened<'t>(&self, text: &'t str) -> Option<regex::Match<'t>> {
        if self.may_match(text) { self.grammar().find(text) } else { None }
    }

    pub(crate) fn is_match_screened(&self, text: &str) -> bool {
        self.may_match(text) && self.grammar().is_match(text)
    }
}

pub(crate) struct Screen {
    automaton: AhoCorasick,
    /// The grammars each literal belongs to.
    owners: Vec<Vec<usize>>,
    /// The grammars that have no literals and so may match any text.
    open: Vec<bool>,
}

impl Screen {
    pub(crate) fn new(entries: &[(&str, Dialect)]) -> Self {
        let mut literals = Vec::<String>::new();
        let mut owners = Vec::<Vec<usize>>::new();
        let mut index = HashMap::<String, usize>::new();
        let mut open = Vec::with_capacity(entries.len());
        for (slot, (id, dialect)) in entries.iter().enumerate() {
            let found = dialect.literals(id);
            open.push(found.is_none());
            for literal in found.into_iter().flatten() {
                let at = *index.entry(literal.clone()).or_insert_with(|| {
                    literals.push(literal);
                    owners.push(Vec::new());
                    owners.len() - 1
                });
                owners[at].push(slot);
            }
        }
        let automaton = AhoCorasick::new(&literals).expect("grammar literals");
        Self { automaton, owners, open }
    }

    /// For each grammar, whether `text` holds one of its literals (or it has none).
    pub(crate) fn present(&self, text: &str) -> Vec<bool> {
        let mut present = self.open.clone();
        let mut left = present.iter().filter(|present| !**present).count();
        if left == 0 { return present; }
        for found in self.automaton.find_overlapping_iter(text) {
            for &slot in &self.owners[found.pattern().as_usize()] {
                if !present[slot] {
                    present[slot] = true;
                    left -= 1;
                }
            }
            if left == 0 { break; }
        }
        present
    }
}
