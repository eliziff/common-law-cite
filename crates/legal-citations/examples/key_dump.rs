//! Every citation of every text, one line each (text index, span, form, authority, key, text), so two builds or two
//! option sets can be diffed key by key. `OPTIONS` takes the options as JSON (`{"jurisdictions": ["ca"]}`), `THREADS`
//! how many texts are read at once (1 by default); the time extraction took goes to stderr.
//!
//!   cargo run --release --example key_dump -- TEXTS > KEYS.tsv
//!
//! TEXTS holds the texts separated by U+001E.
use legal_citations::{extract, Options};
use std::fmt::Write as _;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).expect("TEXTS");
    let text = std::fs::read_to_string(path).expect("readable texts");
    let texts: Vec<&str> = text.split('\u{1e}').collect();
    let options: Options = std::env::var("OPTIONS").ok()
        .map_or_else(Options::default, |json| serde_json::from_str(&json).expect("options JSON"));
    let threads = std::env::var("THREADS").ok().and_then(|count| count.parse::<usize>().ok()).unwrap_or(1).max(1);
    let _ = extract(texts[0], &options);
    let next = AtomicUsize::new(0);
    let elapsed = AtomicUsize::new(0);
    let mut lines = vec![String::new(); texts.len()];
    let slots: Vec<std::sync::Mutex<&mut String>> = lines.iter_mut().map(std::sync::Mutex::new).collect();
    std::thread::scope(|scope| for _ in 0..threads {
        scope.spawn(|| loop {
            let at = next.fetch_add(1, Ordering::Relaxed);
            let Some(text) = texts.get(at) else { break };
            let started = Instant::now();
            let citations = extract(text, &options);
            elapsed.fetch_add(started.elapsed().as_micros() as usize, Ordering::Relaxed);
            let mut out = String::new();
            for citation in citations {
                let _ = writeln!(out, "{at}\t{}\t{}\t{:?}\t{:?}\t{}\t{}", citation.span.start, citation.span.end, citation.form,
                    citation.authority, citation.key.as_deref().unwrap_or("-"), citation.span.text.replace(['\t', '\n', '\r'], " "));
            }
            **slots[at].lock().unwrap() = out;
        });
    });
    drop(slots);
    let stdout = std::io::stdout();
    let mut stdout = std::io::BufWriter::new(stdout.lock());
    for line in &lines { let _ = stdout.write_all(line.as_bytes()); }
    eprintln!("{} texts, extraction {} ms (summed over {threads} threads)", texts.len(), elapsed.load(Ordering::Relaxed) / 1000);
}
