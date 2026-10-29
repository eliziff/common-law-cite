//! Collects the evidence the alias inventory (registry/aliases.json) is made from: every parallel citation the engine
//! reads, as written, with where it was written.
//!
//! Each decision of the local A2AJ store gives its own citations (A2AJ's record: its first and second citations) and
//! the parallels its text writes; each article of the journal store gives the parallels its text writes. Citations are
//! read from the grammar alone, never through the current alias table, so a run does not depend on the last. Nothing is
//! bound or refused here: the store keeps each parallel group whole, with its source, and each member's structural key,
//! written form, court and rank; which groups bind into identities is decided over this evidence.
//!
//!   cargo run --release --features alias-evidence --bin alias-evidence -- OUT.sqlite
//!       [--sources a2aj,journals,corpus] [--store a2aj.sqlite] [--journals public_endpoint.db]
//!       [--corpus corpus.sqlite] [--collections scc-factums,court-practice,…] [--corpus-kinds KINDS.tsv]
//!       [--recovery research.sqlite] [--threads N] [--limit N]
//!
//! `reporter_recovery` reads the reporter recovery's research store (`--recovery`): the citations a provider prints
//! in a decision's header (a Westlaw or Lexis copy's citation line, `citation_aliases` rows found in a PDF's header)
//! and the identifiers its PDF carries for the decision (`pdf_reporter_identifiers`, with the decision's own citation
//! as its extraction reads it), each decision's one group, kind `provider_header`.
//!
//! `corpus` reads the documents of the local corpus (`OpenLegalData\corpus\corpus.sqlite`) in the chosen collections,
//! with its collection as its dataset: kind `corpus_document_digital` (digital-born) or `corpus_document_ocr` by
//! `--corpus-kinds` (legal-pdf-parser's `text_layer_kinds` example), `corpus_document` where unclassified.
//!
//! Tables: `source` (kind `a2aj_record`, `a2aj_decision`, `provider_header`, `journal_article_digital` (a digital-native final text) or
//! `journal_article_pdf_text` (text extracted from a PDF, an OCR layer for a scan); the decision's first citation or the
//! article's id; the A2AJ dataset and name; whether the engine failed on it), `parallel` (one group of citations written side by side, in its source),
//! `member` (each citation of a group: key, form, court, rank), `meta` (the run).
//!
//! A run commits as it goes and resumes: started again on its store with the same arguments, it skips the sources the
//! store holds (by their place in the run's order) and goes on; a finished store is left as it is.
use legal_citations::{aliases, extract, key, parallel, registry, Authority, Citation, Form, Format, Options};
use rusqlite::{params, Connection, OpenFlags};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct Job {
    /// The job's place in the run's order, which a resumed run skips when its store holds it.
    seq: i64,
    kind: &'static str,
    reference: String,
    dataset: Option<String>,
    name: Option<String>,
    /// A record's citations, each read alone; a text, read whole.
    texts: Vec<String>,
}

struct Member {
    key: String,
    form: String,
    court: Option<String>,
    rank: u8,
}

fn case_like(citation: &Citation) -> bool {
    citation.form == Form::Full && match citation.authority {
        Authority::Case => true,
        Authority::Unknown => matches!(citation.format, Some(Format::Neutral | Format::Reporter | Format::CanLii | Format::Database)),
        _ => false,
    }
}

fn member(citation: &Citation) -> Option<Member> {
    if !case_like(citation) || citation.is_ambiguous() { return None; }
    Some(Member {
        key: key::key_in(citation, registry::registry())?,
        form: citation.span.text.split_whitespace().collect::<Vec<_>>().join(" "),
        court: citation.court.as_ref().map(|court| court.id.clone()),
        rank: parallel::rank(citation),
    })
}

/// The parallel groups of a job: a record's or a provider header's citations are one group; a text's are the groups
/// the engine reads in it.
fn groups(job: &Job) -> Vec<Vec<Member>> {
    let options = Options { resolve: false, parallel: true, ..Options::default() };
    if job.kind == "a2aj_record" || job.kind == "provider_header" {
        let members: Vec<Member> = job.texts.iter().flat_map(|text| extract(text, &options)).filter_map(|citation| member(&citation)).collect();
        return if members.is_empty() { Vec::new() } else { vec![members] };
    }
    let citations = extract(&job.texts[0], &options);
    let mut found: Vec<(usize, Vec<Member>)> = Vec::new();
    for citation in &citations {
        let Some(group) = citation.parallel_group else { continue };
        let Some(member) = member(citation) else { continue };
        match found.iter_mut().find(|(id, _)| *id == group) {
            Some((_, members)) => members.push(member),
            None => found.push((group, vec![member])),
        }
    }
    found.into_iter().map(|(_, members)| members)
        .filter(|members| members.iter().map(|member| &member.key).collect::<HashSet<_>>().len() > 1).collect()
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|arg| arg == name).and_then(|at| args.get(at + 1)).cloned();
    let out = PathBuf::from(args.first().filter(|arg| !arg.starts_with("--")).ok_or("OUT.sqlite")?);
    let home = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default()).join("OpenLegalData").join("providers");
    let store = value("--store").map(PathBuf::from).unwrap_or_else(|| home.join("a2aj").join("a2aj.sqlite"));
    let journals = value("--journals").map(PathBuf::from).unwrap_or_else(|| home.join("journals").join("public_endpoint.db"));
    let threads: usize = value("--threads").and_then(|value| value.parse().ok()).unwrap_or(3);
    let limit = value("--limit").map_or(String::new(), |limit| format!(" LIMIT {limit}"));
    let sources_wanted = value("--sources").unwrap_or_else(|| "a2aj,journals".to_owned());
    let wants = |name: &str| sources_wanted.split(',').any(|source| source.trim() == name);
    let corpus = value("--corpus").map(PathBuf::from).unwrap_or_else(|| home.parent().unwrap_or(&home).join("corpus").join("corpus.sqlite"));
    let collections: Vec<String> = value("--collections").unwrap_or_else(||
        "scc-factums,court-practice,case-shortforms-sources,legal-pdf,reporter-recovery-books,misc,\
         candidates-2026-10-22-fcc,candidates-2026-10-23-jj,candidates-2026-10-23-jj2,candidates-2026-10-28-factums,\
         candidates-2026-10-28-reform-papers,candidates-2026-10-28-school-papers,candidates-2026-10-28-theses,\
         affidavit-association-expansion,affidavit-association-expansion-2,affidavit-association-expansion-3".to_owned())
        .split(',').map(|collection| collection.trim().to_owned()).collect();
    let (want_a2aj, want_journals, want_corpus) = (wants("a2aj"), wants("journals"), wants("corpus"));
    let recovery = if wants("reporter_recovery") { Some(PathBuf::from(value("--recovery").ok_or("--recovery research.sqlite")?)) } else { None };
    // Each corpus PDF's text kind, by its sha256: `id <TAB> digital|ocr|unread …` as legal-pdf-parser's
    // `text_layer_kinds` example writes it from pdf-inspector's detection. A document it does not classify digital is
    // `corpus_document_ocr` where it read as OCR, else `corpus_document` (unread: no file to read, or not a PDF).
    let text_kinds: std::collections::HashMap<String, String> = value("--corpus-kinds").map(|path| std::fs::read_to_string(path))
        .transpose()?.unwrap_or_default().lines()
        .filter_map(|line| { let mut fields = line.split('\t'); Some((fields.next()?.to_owned(), fields.next()?.to_owned())) })
        .collect();
    aliases::use_evidence(false);
    let started = Instant::now();

    let evidence = Connection::open(&out)?;
    evidence.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;
        CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS source (id INTEGER PRIMARY KEY, seq INTEGER NOT NULL UNIQUE, kind TEXT NOT NULL,
          reference TEXT NOT NULL, dataset TEXT, name TEXT, failed INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS parallel (id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL REFERENCES source(id));
        CREATE TABLE IF NOT EXISTS member (parallel_id INTEGER NOT NULL REFERENCES parallel(id), key TEXT NOT NULL,
          form TEXT NOT NULL, court TEXT, rank INTEGER NOT NULL);")?;
    // The run's arguments (its output aside) and the engine say what the store holds; a store made otherwise is not
    // resumed, and a finished one is left as it is.
    let run = format!("{} {} {}", env!("CARGO_PKG_VERSION"), key::KEY_VERSION, args[1..].join(" "));
    let meta = |key: &str| evidence.query_row("SELECT value FROM meta WHERE key = ?", [key], |row| row.get::<_, String>(0)).ok();
    match meta("run") {
        Some(made) if made != run => return Err(format!("{} was made by another run ({made}); give another path", out.display()).into()),
        Some(_) if meta("seconds").is_some() => { eprintln!("{} is finished", out.display()); return Ok(()); }
        Some(_) => {}
        None => { evidence.execute("INSERT INTO meta VALUES ('run', ?)", [&run])?; }
    }
    let done: HashSet<i64> = evidence.prepare("SELECT seq FROM source")?.query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let count = |table: &str| evidence.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get::<_, i64>(0)).map(|count| count as u64);
    let (resumed, mut parallels, mut failures) = (count("source")?, count("parallel")?,
        evidence.query_row("SELECT count(*) FROM source WHERE failed", [], |row| row.get::<_, i64>(0))? as u64);
    let mut sources = resumed;
    if resumed > 0 { eprintln!("resuming after {resumed} sources"); }

    let (jobs, queue) = mpsc::sync_channel::<Job>(threads * 8);
    let queue = Arc::new(Mutex::new(queue));
    let (finished, results) = mpsc::channel::<(Job, Option<Vec<Vec<Member>>>)>();
    std::thread::scope(|scope| -> Result<()> {
        for _ in 0..threads.max(1) {
            let (queue, finished) = (Arc::clone(&queue), finished.clone());
            std::thread::Builder::new().stack_size(64 << 20).spawn_scoped(scope, move || loop {
                let Ok(job) = queue.lock().unwrap().recv() else { break };
                // A text the engine panics on is kept as a source with no groups.
                let found = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| groups(&job))).ok();
                if finished.send((job, found)).is_err() { break; }
            })?;
        }
        drop(finished);
        let producer = scope.spawn(move || -> Result<()> {
            // Every job takes the next place in the run's order; one the store holds is not sent again.
            let mut seq = 0i64;
            let mut send = |job: Job| -> Result<()> {
                seq += 1;
                if !done.contains(&seq) { jobs.send(Job { seq, ..job })?; }
                Ok(())
            };
            if want_a2aj {
            let store = Connection::open_with_flags(&store, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let mut rows = store.prepare(&format!("SELECT dataset, citation_en, citation_fr, citation2_en, citation2_fr,
                COALESCE(name_en, name_fr), unofficial_text_en, unofficial_text_fr FROM document WHERE doc_type = 'cases'
                ORDER BY id{limit}"))?;
            let mut rows = rows.query([])?;
            while let Some(row) = rows.next()? {
                let text = |at: usize| row.get::<_, Option<String>>(at).map(|value| value.filter(|value| !value.trim().is_empty()));
                let citations: Vec<String> = [text(1)?, text(2)?, text(3)?, text(4)?].into_iter().flatten().collect();
                let Some(first) = citations.first().cloned() else { continue };
                let (dataset, name): (Option<String>, Option<String>) = (row.get(0)?, text(5)?);
                send(Job { seq: 0, kind: "a2aj_record", reference: first.clone(), dataset: dataset.clone(), name: name.clone(), texts: citations })?;
                for body in [text(6)?, text(7)?].into_iter().flatten() {
                    send(Job { seq: 0, kind: "a2aj_decision", reference: first.clone(), dataset: dataset.clone(), name: name.clone(), texts: vec![body] })?;
                }
            }
            }
            if want_journals {
            let journals = Connection::open_with_flags(&journals, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let mut rows = journals.prepare(&format!("SELECT article_id, journal_name, text, final_text_source IS NOT NULL FROM articles
                WHERE text IS NOT NULL AND length(text) > 0 ORDER BY article_id{limit}"))?;
            let mut rows = rows.query([])?;
            while let Some(row) = rows.next()? {
                // An article's digital-native final text, or the text extracted from its PDF (an OCR layer for a scan).
                let kind = if row.get::<_, bool>(3)? { "journal_article_digital" } else { "journal_article_pdf_text" };
                send(Job { seq: 0, kind, reference: row.get::<_, i64>(0)?.to_string(), dataset: row.get(1)?, name: None,
                    texts: vec![row.get(2)?] })?;
            }
            }
            if let Some(recovery) = &recovery {
                // Each decision's citations as one provider's header prints them, and the identifiers one PDF carries
                // for it with its own citation; the provider and PDF are the source, the decision its reference.
                let recovery = Connection::open_with_flags(recovery, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                let mut rows = recovery.prepare(&format!("SELECT source_id || ' ' || locator, decision_key, citation FROM citation_aliases
                    WHERE source_id LIKE 'pdf:%'
                    UNION ALL SELECT e.source_id || ' ' || e.id, e.citation, i.citation FROM pdf_reporter_identifiers i
                      JOIN extractions e ON e.id = i.extraction_id
                    UNION ALL SELECT DISTINCT e.source_id || ' ' || e.id, e.citation, e.citation FROM pdf_reporter_identifiers i
                      JOIN extractions e ON e.id = i.extraction_id
                    ORDER BY 1, 2{limit}"))?;
                let mut rows = rows.query([])?;
                let mut headers: Vec<(String, String, Vec<String>)> = Vec::new();
                while let Some(row) = rows.next()? {
                    let (source, decision, citation): (String, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
                    let citation = citation.split_whitespace().collect::<Vec<_>>().join(" ");
                    match headers.last_mut() {
                        Some((at, key, citations)) if *at == source && *key == decision => {
                            if !citations.contains(&citation) { citations.push(citation); }
                        }
                        _ => headers.push((source, decision, vec![citation])),
                    }
                }
                for (source, decision, citations) in headers.into_iter().filter(|(_, _, citations)| citations.len() > 1) {
                    send(Job { seq: 0, kind: "provider_header", reference: decision, dataset: Some(source), name: None, texts: citations })?;
                }
            }
            if want_corpus {
                // Each document of the chosen corpus collections once, its pages' text in order; its collection is
                // its provenance.
                let corpus = Connection::open_with_flags(&corpus, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                let marks = vec!["?"; collections.len()].join(", ");
                let mut documents = corpus.prepare(&format!("SELECT sha256, MIN(collection) FROM documents WHERE collection IN ({marks})
                    GROUP BY sha256 ORDER BY sha256{limit}"))?;
                let mut pages = corpus.prepare("SELECT text FROM pages WHERE sha256 = ? ORDER BY page")?;
                let mut rows = documents.query(rusqlite::params_from_iter(collections.iter()))?;
                while let Some(row) = rows.next()? {
                    let (sha256, collection): (String, String) = (row.get(0)?, row.get(1)?);
                    let text = pages.query_map([&sha256], |page| page.get::<_, Option<String>>(0))?
                        .collect::<rusqlite::Result<Vec<_>>>()?.into_iter().flatten().collect::<Vec<_>>().join("\n\n");
                    if text.trim().is_empty() { continue; }
                    let kind = match text_kinds.get(&sha256).map(String::as_str) {
                        Some("digital") => "corpus_document_digital",
                        Some("ocr") => "corpus_document_ocr",
                        _ => "corpus_document",
                    };
                    send(Job { seq: 0, kind, reference: sha256, dataset: Some(collection), name: None, texts: vec![text] })?;
                }
            }
            Ok(())
        });
        // Committed every ten seconds, so a stopped run loses at most those.
        let mut committed = Instant::now();
        let mut add_source = evidence.prepare("INSERT INTO source (seq, kind, reference, dataset, name, failed) VALUES (?, ?, ?, ?, ?, ?)")?;
        let mut add_parallel = evidence.prepare("INSERT INTO parallel (source_id) VALUES (?)")?;
        let mut add_member = evidence.prepare("INSERT INTO member VALUES (?, ?, ?, ?, ?)")?;
        evidence.execute_batch("BEGIN")?;
        for (job, found) in results {
            let failed = found.is_none();
            failures += u64::from(failed);
            add_source.execute(params![job.seq, job.kind, job.reference, job.dataset, job.name, failed])?;
            let source = evidence.last_insert_rowid();
            sources += 1;
            for members in found.unwrap_or_default() {
                add_parallel.execute([source])?;
                let group = evidence.last_insert_rowid();
                parallels += 1;
                for member in members {
                    add_member.execute(params![group, member.key, member.form, member.court, member.rank])?;
                }
            }
            if committed.elapsed().as_secs() >= 10 {
                evidence.execute_batch("COMMIT; BEGIN")?;
                committed = Instant::now();
            }
            if sources % 20_000 == 0 {
                eprintln!("{sources} sources, {parallels} parallels, {:.0} s", started.elapsed().as_secs_f64());
            }
        }
        evidence.execute_batch("COMMIT")?;
        producer.join().map_err(|_| "the store reader panicked")?
    })?;
    evidence.execute_batch("CREATE INDEX IF NOT EXISTS member_key ON member(key);
        CREATE INDEX IF NOT EXISTS member_parallel ON member(parallel_id); CREATE INDEX IF NOT EXISTS parallel_source ON parallel(source_id);")?;
    let meta = [("engine", env!("CARGO_PKG_VERSION").to_owned()), ("key_version", key::KEY_VERSION.to_owned()),
        ("sources", sources.to_string()), ("parallels", parallels.to_string()), ("failures", failures.to_string()),
        ("seconds", format!("{:.0}", started.elapsed().as_secs_f64()))];
    for (key, value) in meta { evidence.execute("INSERT OR REPLACE INTO meta VALUES (?, ?)", params![key, value])?; }
    evidence.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")?;
    eprintln!("{sources} sources, {parallels} parallels in {:.0} s", started.elapsed().as_secs_f64());
    Ok(())
}
