//! Compact read-only tables that build scripts compile from the JSON data: a blob of sections, each a
//! run of little-endian `u32` words or a UTF-8 string arena, read in place from the binary's static
//! data. Nothing is parsed or copied at run time; a lookup is a binary search over sorted rows.

use std::collections::HashMap;

/// Little-endian `u32` words.
#[derive(Clone, Copy)]
pub struct Words(&'static [u8]);

impl Words {
    pub fn len(self) -> usize {
        self.0.len() / 4
    }

    pub fn get(self, index: usize) -> u32 {
        u32::from_le_bytes(self.0[index * 4..index * 4 + 4].try_into().unwrap())
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

/// Rows of `width` words.
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
        self.words.get(row * self.width + column)
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
        let header = Words(bytes);
        let count = header.get(0) as usize;
        let mut at = 4 * (count + 1);
        Self((1..=count).map(|index| {
            let section = &bytes[at..at + header.get(index) as usize];
            at += section.len();
            section
        }).collect())
    }

    pub fn words(&self, section: usize) -> Words {
        Words(self.0[section])
    }

    pub fn rows(&self, section: usize, width: usize) -> Rows {
        Rows { words: self.words(section), width }
    }

    /// The strings [`StringsWriter::sections`] wrote at `section` and the one after it.
    pub fn strings(&self, section: usize) -> Strings {
        Strings {
            offsets: self.words(section),
            arena: std::str::from_utf8(self.0[section + 1]).expect("compiled string arena"),
        }
    }
}

/// A blob of `sections` (build scripts): their count and lengths, then each in turn.
pub fn blob(sections: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = words([sections.len() as u32].into_iter().chain(sections.iter().map(|section| section.len() as u32)));
    for section in sections {
        bytes.extend_from_slice(section);
    }
    bytes
}

/// A section of words (build scripts).
pub fn words(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
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
