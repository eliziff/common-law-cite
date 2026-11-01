//! Compact read-only tables that build scripts compile from the JSON data: a blob of sections, each a
//! run of `u32` words or a UTF-8 string arena, read from the binary's static data. Words are stored as
//! byte planes (every word's low byte, then every word's second byte, and so on) and rows column by
//! column, so that counting and slowly changing columns compress to little where the binary is
//! compressed; a section of words is read back into words once, when the table is first used, and a
//! lookup is a binary search over sorted rows. String arenas are read in place.

use std::collections::HashMap;

/// `u32` words, read once from their four byte planes (low byte first).
#[derive(Clone, Copy)]
pub struct Words(&'static [u32]);

impl Words {
    /// The words of a section written by [`words`].
    fn read(planes: &'static [u8]) -> Self {
        let len = planes.len() / 4;
        let words: Vec<u32> = (0..len)
            .map(|index| u32::from_le_bytes([0, 1, 2, 3].map(|plane| planes[plane * len + index])))
            .collect();
        Self(Box::leak(words.into_boxed_slice()))
    }

    pub fn len(self) -> usize {
        self.0.len()
    }

    pub fn get(self, index: usize) -> u32 {
        self.0[index]
    }

    /// `len` words from `start`.
    pub fn run(self, start: u32, len: u32) -> impl Iterator<Item = u32> {
        (start..start + len).map(move |index| self.get(index as usize))
    }
}

/// Interned strings, by id: one offset more than there are strings, into one arena.
#[derive(Clone, Copy)]
pub struct Strings {
    offsets: Words,
    arena: &'static str,
}

impl Strings {
    pub fn get(self, id: u32) -> &'static str {
        &self.arena[self.offsets.get(id as usize) as usize..self.offsets.get(id as usize + 1) as usize]
    }
}

/// Rows of `width` words, stored column by column.
#[derive(Clone, Copy)]
pub struct Rows {
    words: Words,
    width: usize,
}

impl Rows {
    pub fn len(self) -> usize {
        self.words.len() / self.width
    }

    pub fn get(self, row: usize, column: usize) -> u32 {
        self.words.get(column * self.len() + row)
    }

    /// The row whose `column` names `key`, the rows being sorted by that string.
    pub fn find(self, strings: Strings, column: usize, key: &str) -> Option<usize> {
        let at = partition_point(self.len(), |row| strings.get(self.get(row, column)) < key);
        (at < self.len() && strings.get(self.get(at, column)) == key).then_some(at)
    }
}

/// The first index in `0..len` for which `before` is false; `before` holds for a prefix.
fn partition_point(len: usize, before: impl Fn(usize) -> bool) -> usize {
    let (mut low, mut high) = (0, len);
    while low < high {
        let middle = low + (high - low) / 2;
        if before(middle) { low = middle + 1 } else { high = middle }
    }
    low
}

/// A compiled blob's sections, in the order [`blob`] wrote them.
pub struct Blob(Vec<&'static [u8]>);

impl Blob {
    pub fn new(bytes: &'static [u8]) -> Self {
        let header = |index: usize| u32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap()) as usize;
        let count = header(0);
        let mut at = 4 * (count + 1);
        Self((1..=count).map(|index| {
            let section = &bytes[at..at + header(index)];
            at += section.len();
            section
        }).collect())
    }

    pub fn words(&self, section: usize) -> Words {
        Words::read(self.0[section])
    }

    pub fn rows(&self, section: usize, width: usize) -> Rows {
        Rows { words: self.words(section), width }
    }

    /// The strings [`StringsWriter::sections`] wrote at `section` and the one after it.
    pub fn strings(&self, section: usize) -> Strings {
        let arena = self.0[section + 1];
        // A debug build checks what a release build relies on (reading it all would page in the whole arena).
        debug_assert!(std::str::from_utf8(arena).is_ok(), "compiled string arena");
        Strings {
            offsets: self.words(section),
            // SAFETY: the arena is [`StringsWriter::sections`]' bytes, a `String`'s, compiled into the binary as written.
            arena: unsafe { std::str::from_utf8_unchecked(arena) },
        }
    }
}

/// A blob of `sections` (build scripts): their count and lengths (little-endian), then each in turn.
pub fn blob(sections: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes: Vec<u8> = [sections.len() as u32].into_iter().chain(sections.iter().map(|section| section.len() as u32))
        .flat_map(u32::to_le_bytes).collect();
    for section in sections {
        bytes.extend_from_slice(section);
    }
    bytes
}

/// A section of words (build scripts).
pub fn words(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    let values: Vec<[u8; 4]> = values.into_iter().map(u32::to_le_bytes).collect();
    (0..4).flat_map(|plane| values.iter().map(move |bytes| bytes[plane])).collect()
}

/// A section of rows of `width` words, given row by row (build scripts).
pub fn rows(values: &[u32], width: usize) -> Vec<u8> {
    assert_eq!(values.len() % width, 0, "whole rows");
    let len = values.len() / width;
    words((0..width).flat_map(|column| (0..len).map(move |row| values[row * width + column])))
}

/// Interns strings for a blob (build scripts).
#[derive(Default)]
pub struct StringsWriter {
    ids: HashMap<String, u32>,
    offsets: Vec<u32>,
    arena: String,
}

impl StringsWriter {
    pub fn id(&mut self, value: &str) -> u32 {
        if let Some(&id) = self.ids.get(value) {
            return id;
        }
        if self.offsets.is_empty() {
            self.offsets.push(0);
        }
        let id = self.offsets.len() as u32 - 1;
        self.arena.push_str(value);
        self.offsets.push(u32::try_from(self.arena.len()).expect("string arena fits u32 offsets"));
        self.ids.insert(value.to_owned(), id);
        id
    }

    /// The two sections [`Blob::strings`] reads: offsets, then the arena.
    pub fn sections(mut self) -> [Vec<u8>; 2] {
        if self.offsets.is_empty() {
            self.offsets.push(0);
        }
        [words(self.offsets), self.arena.into_bytes()]
    }
}
