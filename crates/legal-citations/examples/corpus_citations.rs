//! Extracts the citations of every ExtractedPdf document (`*.json.gz`, pages[].lines[].text) in a
//! directory and prints, per document, its name, its length, the time extraction took (the last of
//! `CORPUS_REPEAT` runs), the number of
//! citations and a digest of their serialized form, so two builds can be compared document by
//! document. `CORPUS_DUMP=<dir>` also writes each document's citations as JSON there.
//!
//! cargo run --release --example corpus_citations -- <inputs dir> [name filter]
use std::io::Read;

fn text(path: &std::path::Path) -> String {
    let mut json = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).expect("input")).read_to_string(&mut json).expect("gzip");
    let document: serde_json::Value = serde_json::from_str(&json).expect("json");
    let mut text = String::new();
    for page in document["pages"].as_array().into_iter().flatten() {
        for line in page["lines"].as_array().into_iter().flatten() {
            text.push_str(line["text"].as_str().unwrap_or(""));
            text.push('\n');
        }
    }
    text
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("inputs dir");
    let filter = args.next().unwrap_or_default();
    let repeat = std::env::var("CORPUS_REPEAT").ok().and_then(|count| count.parse::<usize>().ok()).unwrap_or(1).max(1);
    let dump = std::env::var_os("CORPUS_DUMP").map(std::path::PathBuf::from);
    let mut paths = std::fs::read_dir(&dir).expect("dir").flatten().map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".json.gz") && path.to_string_lossy().contains(&filter))
        .collect::<Vec<_>>();
    paths.sort();
    // The grammars compile on first use; that is not the documents' time.
    legal_citations::extract("R v White, 2020 NSCA 33 at para 4; Smith v Jones, [1999] 1 SCR 5", &legal_citations::Options::default());
    let mut total = 0u128;
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy().trim_end_matches(".json.gz").to_owned();
        let text = text(&path);
        let mut started = std::time::Instant::now();
        let mut citations = legal_citations::extract(&text, &legal_citations::Options::default());
        for _ in 1..repeat {
            started = std::time::Instant::now();
            citations = legal_citations::extract(&text, &legal_citations::Options::default());
        }
        let micros = started.elapsed().as_micros();
        total += micros;
        let serialized = serde_json::to_string(&citations).expect("serialize");
        let digest = sha2::Digest::finalize(<sha2::Sha256 as sha2::Digest>::new_with_prefix(&serialized));
        let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        println!("{name}\t{}\t{micros}\t{}\t{hex}", text.len(), citations.len());
        if let Some(dump) = &dump { std::fs::write(dump.join(format!("{name}.json")), &serialized).expect("dump"); }
    }
    eprintln!("total extraction {} ms", total / 1000);
}
