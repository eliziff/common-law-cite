//! Whether each named row's shapes read its own examples (`examples`, as reporters-db writes them, which a
//! reporter's editions share): every example the one scan does not read with that row or another edition of its
//! reporter, by row. With `--text TEXT`, what the scan reads in TEXT and the shapes of the rows it names.
//!
//!   cargo run --release --example named_examples [--authored] [--text TEXT]
use legal_citations::named::{self, Row, Selection};

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let selection = Selection { roman_pages: true, ..Selection::default() };
    if let Some(at) = arguments.iter().position(|argument| argument == "--text") {
        let text = &arguments[at + 1];
        for found in named::find(text, &selection) {
            println!("{:?} {:?} rows {:?} groups {:?}", &text[found.span.clone()], &text[found.name.clone()],
                found.rows.iter().chain(&found.variations).map(|row| &row.edition).collect::<Vec<_>>(), found.groups);
        }
        for row in named::rows().iter().filter(|row| text.contains(row.edition.as_str())) {
            println!("row {} {:?}: {:?}", row.edition, row.table, row.shapes().map(|shape| &named::shapes()[shape]).collect::<Vec<_>>());
        }
        return;
    }
    let authored_only = arguments.iter().any(|argument| argument == "--authored");
    let same = |row: &Row, other: &Row| std::ptr::eq(row, other) || row.table == other.table && row.id.is_some() && row.id == other.id;
    let (mut read, mut missed) = (0usize, Vec::new());
    for row in named::rows().iter().filter(|row| !authored_only || row.source != "reporters-db") {
        for example in &row.examples {
            let found = named::find(example, &selection);
            if found.iter().any(|named| named.rows.iter().chain(&named.variations).any(|other| same(row, other))) {
                read += 1;
            } else {
                let what: Vec<_> = found.iter().map(|named| &example[named.span.clone()]).collect();
                missed.push(format!("{:?}\t{}\t{example}\tread: {what:?}", row.table, row.edition));
            }
        }
    }
    println!("{} rows, {} shapes; examples read {read}, missed {}", named::rows().len(), named::shapes().len(), missed.len());
    for line in &missed { println!("{line}"); }
}
