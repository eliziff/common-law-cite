//! Works' extents a host registers once from a works pack ([`register`]), read for a key a request's own
//! [`crate::Options::extents`] do not give. Kept compact: a key is its text before the last `:` (a court
//! year's neutral citations, a reporter volume) and a number, so the table holds each such prefix once and
//! a row of numbers per extent.

use crate::Extent;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A missing page or paragraph count.
const NONE: u32 = u32::MAX;
/// While registering: a page or paragraph count too large for a row.
const OVERFLOW: u32 = u32::MAX - 1;

struct Table {
    /// The sorted prefixes, as spans of one arena.
    prefixes: Vec<[u32; 2]>,
    arena: String,
    /// `[prefix, number, last page, paragraphs]` by prefix then number.
    rows: Vec<[u32; 4]>,
    /// Extents whose numbers do not fit a row.
    other: HashMap<String, (Option<u64>, Option<u64>)>,
}

static REGISTERED: RwLock<Option<Arc<Table>>> = RwLock::new(None);

impl Table {
    fn get(&self, key: &str) -> Option<(Option<u64>, Option<u64>)> {
        let row = key.rsplit_once(':').and_then(|(prefix, number)| {
            // Only a number written as a key writes it: digits without a leading zero.
            if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit())
                || (number.len() > 1 && number.starts_with('0')) { return None; }
            let number = u32::try_from(number.parse::<u64>().ok()?).ok()?;
            let id = self.prefixes.binary_search_by(|&[start, end]| self.arena[start as usize..end as usize].cmp(prefix)).ok()? as u32;
            self.rows.binary_search_by(|row| (row[0], row[1]).cmp(&(id, number))).ok()
        });
        match row {
            Some(row) => {
                let value = |value: u32| (value != NONE).then_some(u64::from(value));
                Some((value(self.rows[row][2]), value(self.rows[row][3])))
            }
            None => self.other.get(key).copied(),
        }
    }
}

/// Registers the extents a works pack gives (lines of tab-separated fields) for every later extraction,
/// replacing what was registered before. "N" lines give a court year's decisions' last paragraphs, by
/// neutral citation: court, year, then each number's gap from the last and paragraph count. "V" lines give
/// a reporter volume's works by first page: each page's gap from the last, how many pages more the work
/// runs, and its paragraph count. A later line's extent for a key replaces an earlier one's.
pub(crate) fn register(pack: &str) {
    let number = |value: Option<&str>| value.and_then(|value| value.parse::<u64>().ok());
    let mut prefixes: HashMap<String, u32> = HashMap::new();
    // Rows in the pack's order, by each prefix's first appearance; a row whose page or paragraph
    // count does not fit holds OVERFLOW and the extent's place in `overflow`.
    let mut rows: Vec<[u32; 4]> = Vec::with_capacity(pack.bytes().filter(|&byte| byte == b',' || byte == b'\n').count() + 1);
    let mut overflow = Vec::new();
    let mut other = HashMap::new();
    for line in pack.split('\n') {
        let fields = line.split('\t').collect::<Vec<_>>();
        let (prefix, list) = match fields.as_slice() {
            ["N", court, year, list, ..] => (format!("3:neutral:{year}:{court}"), list),
            ["V", volume, list, ..] => (format!("3:{volume}"), list),
            _ => continue,
        };
        let count = prefixes.len() as u32;
        let id = *prefixes.entry(prefix.clone()).or_insert(count);
        let mut at = 0u64;
        for entry in list.split(',') {
            let mut parts = entry.split(':');
            at += number(parts.next()).unwrap_or(0);
            let (last_page, paragraphs) = if fields[0] == "N" {
                // As JavaScript's Number reads it: an empty count is 0.
                (None, parts.next().map(|count| if count.is_empty() { Some(0) } else { number(Some(count)) }).flatten())
            } else {
                let (span, paragraphs) = (parts.next().filter(|span| !span.is_empty()), parts.next().filter(|count| !count.is_empty()));
                (span.map(|span| at + number(Some(span)).unwrap_or(0)), number(paragraphs))
            };
            let Ok(number) = u32::try_from(at) else {
                // No row holds this key: the last extent for it is the one kept.
                other.insert(format!("{prefix}:{at}"), (last_page, paragraphs));
                continue;
            };
            let fits = |value: Option<u64>| value.map_or(Some(NONE), |value| u32::try_from(value).ok().filter(|&value| value < OVERFLOW));
            rows.push(match (fits(last_page), fits(paragraphs)) {
                (Some(last_page), Some(paragraphs)) => [id, number, last_page, paragraphs],
                _ => {
                    overflow.push((last_page, paragraphs));
                    [id, number, OVERFLOW, overflow.len() as u32 - 1]
                }
            });
        }
    }
    let mut sorted: Vec<(String, u32)> = prefixes.into_iter().collect();
    sorted.sort_unstable();
    let mut ids = vec![0u32; sorted.len()];
    let (mut spans, mut arena) = (Vec::with_capacity(sorted.len()), String::new());
    for (id, (prefix, first)) in sorted.iter().enumerate() {
        ids[*first as usize] = id as u32;
        let start = arena.len() as u32;
        arena.push_str(prefix);
        spans.push([start, arena.len() as u32]);
    }
    for row in &mut rows { row[0] = ids[row[0] as usize]; }
    // The last extent for a key wins: the sort is stable.
    rows.sort_by_key(|row| (row[0], row[1]));
    let mut kept = 0;
    for index in 0..rows.len() {
        let row = rows[index];
        if rows.get(index + 1).is_some_and(|next| (next[0], next[1]) == (row[0], row[1])) { continue; }
        if row[2] == OVERFLOW {
            other.insert(format!("{}:{}", sorted[row[0] as usize].0, row[1]), overflow[row[3] as usize]);
        } else {
            rows[kept] = row;
            kept += 1;
        }
    }
    rows.truncate(kept);
    rows.shrink_to_fit();
    *REGISTERED.write().expect("extents lock") = Some(Arc::new(Table { prefixes: spans, arena, rows, other }));
}

pub(crate) fn registered(key: &str) -> Option<Extent> {
    let table = REGISTERED.read().expect("extents lock").clone()?;
    let (last_page, paragraphs) = table.get(key)?;
    Some(Extent { key: key.to_owned(), last_page, paragraphs })
}
