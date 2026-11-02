//! The grammar's literal screens, worked out by the build script (build_registry.rs) from the same functions
//! ([`legal_grammar::table_entry_literals`] and its kin) so that no start parses every pattern again to find them.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Deserialize)]
pub(crate) struct Entry {
    pub linear: Option<Vec<String>>,
    pub backtracking: Option<Vec<String>>,
    pub python: Option<Vec<String>>,
    pub ascii: Option<Vec<String>>,
    pub leading: Option<String>,
}

#[derive(Deserialize)]
struct Literals {
    entries: HashMap<String, Entry>,
    #[cfg_attr(not(feature = "us"), allow(dead_code))]
    extractors: Vec<Option<Vec<String>>>,
}

static LITERALS: LazyLock<Literals> = LazyLock::new(|| {
    let compressed = include_bytes!(concat!(env!("OUT_DIR"), "/grammar-literals.json.gz"));
    serde_json::from_reader(flate2::read::GzDecoder::new(compressed.as_slice())).expect("compiled grammar literals")
});

/// A grammar entry's screens.
pub(crate) fn entry(id: &str) -> &'static Entry {
    LITERALS.entries.get(id).unwrap_or_else(|| panic!("unknown grammar entry: {id}"))
}

/// A US extractor's screen.
#[cfg(feature = "us")]
pub(crate) fn extractor(index: usize) -> Option<&'static [String]> {
    LITERALS.extractors[index].as_deref()
}

pub(crate) fn warm() {
    LazyLock::force(&LITERALS);
}
