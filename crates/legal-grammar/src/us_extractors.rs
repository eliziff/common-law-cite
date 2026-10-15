//! Pinned reporters-db extractors, retaining Eyecite's captures and editions: `data/us-extractors.json`,
//! compiled by the build script into a [`crate::table`] blob read in place.

use crate::table::{Blob, Rows, Strings, Words};
use std::sync::LazyLock;

// The layout build.rs writes: strings (sections 0 and 1); extractor rows (2) of pattern, exact
// editions, variation editions and reporter names (each a run in the pool, 3: start and length) and
// flags; edition rows (4) of short name, reporter short name, name, cite type and source, start and end.
const NONE: u32 = u32::MAX;
const SHORT: u32 = 1;
const STANDARD: u32 = 2;
const ECMASCRIPT: u32 = 4;

/// The extractors and the editions they name.
pub struct Extractors {
    strings: Strings,
    extractors: Rows,
    pool: Words,
    editions: Rows,
}

/// One extractor: its pattern, the editions it reads exactly and by variation, and the reporter
/// names (any of which a text must write for the extractor to run).
#[derive(Clone, Copy)]
pub struct Extractor {
    row: usize,
    table: &'static Extractors,
}

/// One edition of a reporter.
pub struct Edition {
    pub short_name: &'static str,
    pub reporter_short_name: &'static str,
    pub reporter_name: &'static str,
    pub cite_type: &'static str,
    pub source: &'static str,
    pub start: Option<&'static str>,
    pub end: Option<&'static str>,
}

pub fn extractors() -> &'static Extractors {
    static TABLE: LazyLock<Extractors> = LazyLock::new(|| {
        let blob = Blob::new(include_bytes!(concat!(env!("OUT_DIR"), "/us-extractors.bin")));
        Extractors { strings: blob.strings(0), extractors: blob.rows(2, 8), pool: blob.words(3), editions: blob.rows(4, 7) }
    });
    &TABLE
}

impl Extractors {
    pub fn len(&'static self) -> usize {
        self.extractors.len()
    }

    pub fn get(&'static self, index: usize) -> Extractor {
        Extractor { row: index, table: self }
    }

    pub fn edition(&'static self, index: usize) -> Edition {
        let column = |column| self.editions.get(index, column);
        let date = |column| Some(column).filter(|&id| id != NONE).map(|id| self.strings.get(id));
        Edition {
            short_name: self.strings.get(column(0)),
            reporter_short_name: self.strings.get(column(1)),
            reporter_name: self.strings.get(column(2)),
            cite_type: self.strings.get(column(3)),
            source: self.strings.get(column(4)),
            start: date(column(5)),
            end: date(column(6)),
        }
    }
}

impl Extractor {
    fn column(self, column: usize) -> u32 {
        self.table.extractors.get(self.row, column)
    }

    pub fn pattern(self) -> &'static str {
        self.table.strings.get(self.column(0))
    }

    pub fn exact(self) -> impl Iterator<Item = usize> {
        self.table.pool.run(self.column(1), self.column(2)).map(|index| index as usize)
    }

    pub fn variations(self) -> impl Iterator<Item = usize> {
        self.table.pool.run(self.column(3), self.column(4)).map(|index| index as usize)
    }

    pub fn strings(self) -> impl Iterator<Item = &'static str> {
        let strings = self.table.strings;
        self.table.pool.run(self.column(5), self.column(6)).map(move |id| strings.get(id))
    }

    pub fn short(self) -> bool {
        self.column(7) & SHORT != 0
    }

    pub fn standard(self) -> bool {
        self.column(7) & STANDARD != 0
    }

    pub fn ecmascript(self) -> bool {
        self.column(7) & ECMASCRIPT != 0
    }
}
