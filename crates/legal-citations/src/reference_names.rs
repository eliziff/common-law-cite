//! Where the names of legal references end in a text: statute titles (registry/statute-titles.txt, made by
//! tools/statute-titles.py from A2AJ's statutes), the Charter, and the titles and short forms of the statutes the
//! text itself cites. A publisher's citator links these names in the judgments it shows and sets a no-break space
//! after each, so a text fragment's term that runs on past one finds nothing there.

use crate::{extract, short_forms, Authority, Options};
use aho_corasick::{AhoCorasick, MatchKind};
use std::io::Read;
use std::sync::LazyLock;

/// Names cited without their title: "the Charter" is the Canadian Charter of Rights and Freedoms.
const UNTITLED: &[&str] = &["Charter", "Charte"];

static REGISTRY: LazyLock<AhoCorasick> = LazyLock::new(|| {
    let compressed = include_bytes!(concat!(env!("OUT_DIR"), "/statute-titles.txt.gz"));
    let mut titles = String::new();
    flate2::read::GzDecoder::new(compressed.as_slice()).read_to_string(&mut titles).expect("statute titles");
    names(titles.lines().chain(UNTITLED.iter().copied()))
});

fn names<'a>(names: impl IntoIterator<Item = &'a str>) -> AhoCorasick {
    AhoCorasick::builder().match_kind(MatchKind::LeftmostLongest).ascii_case_insensitive(true)
        .build(names).expect("reference names")
}

/// Byte offsets in `text` where the name of a legal reference ends, each a whole name's end, sorted.
pub fn reference_name_ends(text: &str) -> Vec<usize> {
    // The titles of the statutes the text cites, and the short forms it gives them ("[Charter]", "the CHRA").
    let cited = extract(text, &Options { resolve: false, ..Options::default() }).into_iter()
        .filter(|citation| matches!(citation.authority, Authority::Statute | Authority::Constitution))
        .flat_map(|citation| {
            let inferred = short_forms::infer(&citation.full_span.text, "statute").into_iter().map(|form| form.value);
            citation.style.map(|style| style.text).into_iter()
                .chain(citation.explicit_short_name).chain(citation.short_name).chain(inferred)
                .map(|name| name.trim().trim_end_matches([',', '.', ';', ':']).to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|name| name.chars().any(char::is_alphabetic))
        .collect::<Vec<_>>();
    let own = (!cited.is_empty()).then(|| names(cited.iter().map(String::as_str)));
    let whole = |start: usize, end: usize| {
        !text[..start].chars().next_back().is_some_and(char::is_alphanumeric)
            && !text[end..].chars().next().is_some_and(char::is_alphanumeric)
    };
    let mut ends = REGISTRY.find_iter(text).chain(own.iter().flat_map(|own| own.find_iter(text)))
        .filter(|found| whole(found.start(), found.end()))
        .map(|found| found.end())
        .collect::<Vec<_>>();
    ends.sort_unstable();
    ends.dedup();
    ends
}
