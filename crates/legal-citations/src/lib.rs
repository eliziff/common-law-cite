//! Deterministic legal citation extraction, classification, resolution and
//! formatting for Canadian, Commonwealth and US legal text.
//!
//! The pipeline is:
//!
//! 1. [`find`]: discover citation cores and their styled extent from the shared
//!    grammar corpus ([`legal_grammar`]).
//! 2. [`classify`]: decide [`Authority`], [`Format`] and parse [`Fields`] from
//!    the grammar's named groups.
//! 3. [`metadata`]: pinpoints, parentheticals, subsequent history, parties and court.
//! 4. [`parallel`]: group citations to the same decision printed side by side.
//! 5. [`resolve`]: attach short forms, `supra`, `ibid`/`Id.` and case-name
//!    references to the full citation they refer to.
//! 6. [`key`]: compute the versioned identity key for every authority.
//!
//! [`extract`] runs all of them. [`annotate`], [`format`] and [`url`] operate on
//! its output, and [`registry`] exposes the court, reporter and series data.

pub mod annotate;
pub mod authorities;
pub mod aliases;
pub mod api;
pub mod classify;
pub mod clean;
pub mod cues;
pub mod excerpt;
mod extents;
pub mod find;
pub mod format;
pub mod key;
pub mod metadata;
pub mod model;
pub mod parallel;
pub mod reference_names;
pub mod registry;
pub mod resolve;
mod screen;
pub mod short_forms;
pub mod source;
pub mod text;
pub mod url;
#[cfg(feature = "us")]
mod us;

pub use legal_grammar as grammar;

/// Build the named lazily built statics now ([`warm`]): `warm_statics!(A, B; screened C, D)` also compiles
/// each screened grammar.
macro_rules! warm_statics {
    ($($name:path),* $(,)? $(; screened $($screened:path),+ $(,)?)?) => {
        $(std::sync::LazyLock::force(&$name);)*
        $($($screened.warm();)+)?
    };
}
pub(crate) use warm_statics;

/// A short document with notes that cite the common kinds of authority, read by [`warm`]: its
/// first line is the text the notes annotate, each later line one note.
const WARM_SAMPLE: &str = "As the Court held in Oakes and the Act provides, the limit is justified.
R v Oakes, [1986] 1 SCR 103 at para 69, 26 DLR (4th) 200.
Ibid at 138.
Canadian Charter of Rights and Freedoms, s 7, Part I of the Constitution Act, 1982, being Schedule B to the Canada Act 1982 (UK), 1982, c 11.
Criminal Code, RSC 1985, c C-46, s 718.2(e); see also Smith v Jones, 2011 ONCA 123 at paras 4-6, aff'd 2012 SCC 9.
Kent Roach, “The Charter and Criminal Justice” (2010) 48:2 Osgoode Hall LJ 233 at 240.
Peter W Hogg, Constitutional Law of Canada, 5th ed (Toronto: Carswell, 2007) at 36-12.
Oakes, supra note 1 at 140; Haig v Canada (Chief Electoral Officer), [1993] 2 SCR 995.
House of Commons Debates, 41-1, No 12 (15 June 2011) at 1520 (Hon Rob Nicholson).
Brown v Board of Education, 347 US 483 at 495 (1954), online: <https://www.canlii.org/en/ca/scc/doc/1986/1986canlii46/1986canlii46.html>.
Roach, supra note 5 at 241; Hogg, ibid at 36-14; Re Residential Tenancies Act, 1979, [1981] 1 SCR 714.";

/// Build every grammar, automaton and table that extraction, note splitting, resolution, source
/// fields, note references, authorities and quoted titles build the first time they need one, and
/// read a short document through them so that their search caches exist: the first document read
/// afterwards costs about what any other does. The US reporter extractors are the exception: only
/// those whose reporter a text names are compiled, as before.
pub fn warm() {
    find::warm();
    classify::warm();
    metadata::warm();
    aliases::warm();
    key::warm();
    registry::warm();
    resolve::warm();
    short_forms::warm();
    source::warm();
    #[cfg(feature = "us")]
    us::warm();
    text::warm();
    clean::warm();
    url::warm();
    let mut notes = Vec::new();
    let mut start = WARM_SAMPLE.find('\n').map_or(WARM_SAMPLE.len(), |end| end + 1);
    for (number, line) in WARM_SAMPLE[start..].split('\n').enumerate() {
        notes.push(serde_json::json!({"number": number + 1, "start": start, "end": start + line.len()}));
        start += line.len() + 1;
    }
    let call = |method: &str, request: serde_json::Value| api::call_value(method, request).expect("warm-up call");
    let extracted = call("extract", serde_json::json!({"text": WARM_SAMPLE,
        "options": {"notes": notes, "splitTier": "safe"}}));
    for part in extracted["sourceParts"].as_array().into_iter().flatten() {
        call("sourceFields", serde_json::json!({"part": part}));
    }
    call("noteReferences", serde_json::json!({"parts": extracted["sourceParts"], "citations": extracted["citations"]}));
    call("authorities", serde_json::json!({"citations": extracted["citations"], "authorities": extracted["authorities"]}));
    call("quotedTitles", serde_json::json!({"text": WARM_SAMPLE}));
}
pub use model::*;

use serde::{Deserialize, Serialize};

/// The unit [`api`] reports offsets in. The Rust API always uses bytes.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub enum OffsetUnit {
    #[default]
    Byte,
    /// Unicode scalar values (Python string indices).
    Char,
    /// UTF-16 code units (JavaScript string indices).
    Utf16,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub enum SupraMode {
    #[default]
    Safe,
    Aggressive,
    /// Safe linking, except that a supra whose written short name names exactly one earlier
    /// citation by its short form (its bracketed short form, or every word of the name it was
    /// cited by) links to it even when its note number names another note.
    Named,
}

/// How far a note splitter's references are linked where the evidence is incomplete
/// ([`Options::split_tier`]): `safe` links what a reference's own note and words support, `moderate`
/// also reads a supra by its name where the note it numbers cites another work, and `aggressive` also
/// continues an ibid from the last source of a note that names several. Every tier also splits notes
/// into source rows ([`source::split_notes`]): a semicolon inside brackets or quotes,
/// a sentence that cites nothing with the source after it, a supra with its name, a citation whole.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub enum SplitTier {
    Safe,
    Moderate,
    Aggressive,
}

impl SplitTier {
    /// The supra linking each tier resolves with.
    pub(crate) fn supra_mode(self) -> SupraMode {
        match self {
            Self::Safe => SupraMode::Safe,
            Self::Moderate => SupraMode::Named,
            Self::Aggressive => SupraMode::Aggressive,
        }
    }
}

/// A citation guide whose forms the finder recognizes beyond the forms every guide shares.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub enum CitationStyle { Mcgill, Coal, Bluebook, Aglc, Oscola, Nzlsg }

/// Options for [`extract`].
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct Options {
    /// Return the pinned Eyecite token view of the shared discovery result.
    /// Native/Commonwealth occurrence discovery remains available by default.
    pub source_only: bool,
    /// Omit citations with unresolved registry interpretations.
    pub remove_ambiguous: bool,
    /// Attach short forms, supra, ibid and references to their antecedents.
    pub resolve: bool,
    /// Group parallel citations.
    pub parallel: bool,
    /// Treat each `\n\n`-separated block, or each entry of [`Options::notes`],
    /// as a footnote whose number `supra note N` can refer to.
    pub notes: Option<Vec<NoteRange>>,
    /// Ordered jurisdiction preferences for ambiguous abbreviations: `["ca"]` by default, `["us"]` or
    /// `["uk", "ca"]` for another order. An explicit court takes precedence. Empty is neutral.
    pub jurisdiction_priority: Vec<String>,
    /// The jurisdictions whose own citation forms are read (`["ca", "uk"]`); every compiled one when absent. A list
    /// without `us` (or a `us-` place) skips the US reporter and law grammars (eyecite's), which hold no
    /// Commonwealth form the native grammars do not read.
    pub jurisdictions: Option<Vec<String>>,
    /// Read a report the grammars cannot split by the registry's name for its reporter, however the writer spaced and
    /// pointed it ("[1989] 2 S.C.R . 418", "49 N .B .R . 144", "37 Sask. R. 128"), as the US reporters' names are read.
    /// A report the grammars read whole is read as they read it.
    pub reporter_names: bool,
    /// Read a report's year where OCR garbled it: a bracketed year whose closing bracket was read as a 1 ("[19381 2
    /// All E.R. 602"), or whose leading 1 was read as a letter l or I ("(l915)"), where the registry knows the reporter
    /// and it ran that year. For OCR'd text; off by default.
    pub ocr_repairs: bool,
    /// Supra hint capture uses aggressive mode by default.
    pub supra_hint_mode: SupraMode,
    /// Inferred and bare-note linking runs only in aggressive mode.
    pub supra_linking_mode: SupraMode,
    /// The guides whose own forms are recognized; every guide's when absent.
    pub styles: Option<Vec<CitationStyle>>,
    /// Leave out what a document's own list or table of authorities holds: its entries list the
    /// authorities the document cites elsewhere, and its tab numbers cite nothing.
    pub skip_authority_lists: bool,
    /// Read a case's name with a pinpoint written before the case is first cited in full
    /// ("Oakes at 138" … "R v Oakes, [1986] 1 SCR 103") as a reference to it, which has no
    /// earlier citation. Eyecite reads references only after the full citation.
    pub early_references: bool,
    /// Resolve references as a note splitter's rows read them, at this tier ([`SplitTier`]) in place of
    /// `supra_linking_mode`: an ibid continues from the last part that is a
    /// source, past prose, a part inside another part's parentheses and a decision's subsequent
    /// history, or from the part its own name names in the note before; a supra whose numbered note
    /// cites another work, or none, is read by its name. Absent, references resolve as before.
    pub split_tier: Option<SplitTier>,
    /// The linking rules a split tier's references resolve with, each its own mechanism ([`Linking`]);
    /// absent, the tier's own.
    pub linking: Option<Linking>,
    /// What the host knows of works' extents ([`Extent`]), which [`Linking::locators`] reads, whichever
    /// mechanisms resolve; a host without them links as before, refusing less.
    pub extents: Vec<Extent>,
}

/// The mechanisms a split tier links references with, each on its own so that tiers compose them:
/// readers and matchers that find what a reference's words name, generators that link where those
/// words alone leave a reference open, and vetoes that refuse a link its locators rule out.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct Linking {
    /// A reference's name read without the signals, lead-in words and quotation marks around it
    /// ("see generally Gross", "Voir CCE, «Mesures volontaires»", "In Lewis v. Bankole"), and none
    /// read from the sentence before a supra that opens its own ("… flawed.” Supra note 58").
    pub name_reading: bool,
    /// A work's short form read without its McGill wording ("[hereinafter “Caron, SCC”]",
    /// "[Lavoie avec renvois aux N.S.R.]", "[Shelf Holdings ABQB cited to CanLII]"), and a short form
    /// written after a decision's subsequent history or a parenthetical held by the citation it ends
    /// ("Rooney v Galloway, 2023 BCCA 1, aff'g … 2021 BCSC 2344 [Rooney]"), unless it names that history's court.
    pub short_forms: bool,
    /// A name matched by whole words, a work's own short form before words that merely occur in
    /// another's text, never by its publication facts ("Allen & Unwin"), and a part of prose no work
    /// a name can name.
    pub name_matching: bool,
    /// Where a name's words fit several works of the note it numbers, the one work that holds the
    /// most of them ("Renvoi manitobain (C.A.)", "Braidwood Commission Report, Part 2").
    pub distinguishing_words: bool,
    /// A link refused where the reference's pinpoint cannot be in the work: a section of a decision,
    /// a paragraph of an article, a page of a statute, a page before a report's first or after its
    /// last ([`Options::extents`]), a paragraph past a decision's last; and, among a note's works, the
    /// one the pinpoint fits.
    pub locators: bool,
    /// A supra numbering a note whose number several notes carry names the nearest one before it.
    pub nearest_note: bool,
    /// A name one letter off the only work of its note that writes it so ("Campell" for Campbell).
    pub near_names: bool,
    /// A supra whose words name no work anywhere in the document ("The English statute") names the
    /// one work of the note it numbers.
    pub descriptive_names: bool,
    /// A supra whose name is exactly one earlier work's short form names it even where its number
    /// names another note ([`SupraMode::Named`]'s rule, for any tier).
    pub named_override: bool,
    /// A bare supra numbering a note of several works names the note's first.
    pub first_of_note: bool,
    /// A supra its notes leave open names the one earlier decision a party of which is its name
    /// (eyecite's antecedent rule).
    pub party_names: bool,
    /// An ibid after a note of several works continues the one its pinpoint fits.
    pub ibid_locators: bool,
    /// The aggressive tier's ibid does not continue from the last source of a note of several.
    pub no_ibid_last_part: bool,
    /// A work's short form written with a pinpoint after the work ("IBP, at 21") refers to it.
    pub short_form_references: bool,
    /// A work's short form written alone after the work ("the Charter") refers to it.
    pub bare_mentions: bool,
    /// A bare supra whose note holds several works names the one whose short form or party the text says
    /// before the supra's own note marker ("… as the Court held in Vriend.⁹" with "⁹ Supra note 5.").
    pub body_names: bool,
    /// A note part citing a work without its name is also known by the name the text gives before the note's
    /// marker ([`Fields::anchor_title`], or the one style of cause its sentence gives).
    pub body_citation_names: bool,
    /// A supra whose name is found only past the note it writes links there only where the document's supras drift
    /// by that distance (a safe tier's veto on a misnumbered supra).
    pub confirmed_drift: bool,
    /// A named supra whose note cites one work, and that without a name of its own (its style of cause given in the
    /// text before the marker), names that work.
    pub nameless_notes: bool,
}

impl Linking {
    /// The mechanisms a split tier links with where the host names none: each tier's composition as its
    /// measured precision and coverage set it.
    /// Measured 2026-10-10 on the DOCX supra gold (right / wrong links, against the build before them): safe
    /// 9,341 / 195 from 8,512 / 231; moderate 9,399 / 225 from 8,948 / 244; aggressive 9,456 / 229 from 8,658 / 363.
    /// The locator veto pays in safe only; first-of-note and bare mentions cost too many wrong links.
    pub fn for_tier(tier: SplitTier) -> Self {
        let safe = Self {
            name_reading: true, distinguishing_words: true, short_form_references: true, ibid_locators: true,
            named_override: true, near_names: true, body_names: true,
            ..Self::default()
        };
        let moderate = Self {
            name_matching: true, short_forms: true, descriptive_names: true, body_citation_names: true,
            ..safe.clone()
        };
        match tier {
            SplitTier::Safe => Self { locators: true, ..safe },
            SplitTier::Moderate => moderate,
            SplitTier::Aggressive => Self { party_names: true, no_ibid_last_part: true, ..moderate },
        }
    }

    /// Safer than the safe tier's mechanisms: those, and a supra whose name is found only past the note it writes
    /// links there only where the document's supras drift by that distance ([`Linking::confirmed_drift`]).
    /// Measured 2026-10-17 against the safe tier (right / wrong links, build partition): DOCX 9,146 / 171 from
    /// 9,385 / 177; journals 4,621 / 72 from 4,726 / 90 (evaluation partition: journals 1,119 / 30 from 1,142 / 39).
    pub fn safer() -> Self {
        Self { confirmed_drift: true, ..Self::for_tier(SplitTier::Safe) }
    }
}

/// A work's extent as the host knows it, by the work's citation key ([`crate::key`]): a report's or article's last
/// page (an article's own; a report's the page before the next report its volume prints), a decision's last
/// paragraph.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct Extent {
    pub key: String,
    pub last_page: Option<u64>,
    pub paragraphs: Option<u64>,
}

/// `read` of each item, in the items' order, read on the engine's reading threads, each taking the next unread item.
/// The threads live as long as the engine, so a grammar keeps one set of search caches per reading thread rather
/// than one per thread an extraction ever started. A reading thread (a nested read) and a target without threads
/// (WebAssembly) read them in turn.
pub(crate) fn read_each<T: Send + Sync + 'static, R: Send + 'static>(
    items: Vec<T>, read: impl Fn(&T) -> R + Send + Sync + 'static) -> Vec<R> {
    #[cfg(not(target_arch = "wasm32"))]
    thread_local!(static READER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });
    #[cfg(not(target_arch = "wasm32"))]
    if items.len() > 1 && !READER.get() {
        use std::sync::{atomic::{AtomicUsize, Ordering}, mpsc, Arc, Mutex, OnceLock};
        type Job = Box<dyn FnOnce() + Send>;
        static READERS: OnceLock<(Mutex<mpsc::Sender<Job>>, usize)> = OnceLock::new();
        let (jobs, threads) = READERS.get_or_init(|| {
            // Two read as fast as one per core: every thread past the second added a set of caches, not speed.
            let threads = std::thread::available_parallelism().map_or(1, |count| count.get()).min(2);
            let (jobs, waiting) = mpsc::channel::<Job>();
            let waiting = Arc::new(Mutex::new(waiting));
            for _ in 0..threads {
                let waiting = Arc::clone(&waiting);
                std::thread::spawn(move || loop {
                    READER.set(true);
                    let job = waiting.lock().expect("reading jobs").recv();
                    let Ok(job) = job else { break };
                    job();
                });
            }
            (Mutex::new(jobs), threads)
        });
        let shared = Arc::new((items, read, AtomicUsize::new(0)));
        let (sender, results) = mpsc::channel();
        for _ in 0..(*threads).min(shared.0.len()) {
            let (shared, sender) = (Arc::clone(&shared), sender.clone());
            jobs.lock().expect("reading jobs").send(Box::new(move || loop {
                let index = shared.2.fetch_add(1, Ordering::Relaxed);
                let Some(item) = shared.0.get(index) else { break };
                let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (shared.1)(item)));
                if sender.send((index, read)).is_err() { break }
            })).expect("reading threads");
        }
        drop(sender);
        let mut read_by: Vec<Option<R>> = (0..shared.0.len()).map(|_| None).collect();
        for (index, read) in results {
            read_by[index] = Some(read.unwrap_or_else(|panic| std::panic::resume_unwind(panic)));
        }
        return read_by.into_iter().map(|read| read.expect("every item read")).collect();
    }
    items.iter().map(read).collect()
}

impl Default for Options {
    fn default() -> Self {
        Self {
            source_only: false,
            remove_ambiguous: false,
            resolve: true,
            parallel: true,
            notes: None,
            jurisdiction_priority: source::default_jurisdiction_priority(),
            jurisdictions: None,
            reporter_names: true,
            ocr_repairs: false,
            supra_hint_mode: SupraMode::Aggressive,
            supra_linking_mode: SupraMode::Safe,
            styles: None,
            skip_authority_lists: false,
            early_references: false,
            split_tier: None,
            linking: None,
            extents: Vec::new(),
        }
    }
}

impl Options {
    /// Specialized US forms belong to the Bluebook guide; jurisdiction choices
    /// can select them explicitly without a separate US-only switch.
    pub(crate) fn us_forms(&self) -> bool {
        self.styles.as_ref().is_none_or(|styles| styles.contains(&CitationStyle::Bluebook))
            || source::us_jurisdiction(&self.jurisdiction_priority)
    }

    /// Whether the US reporter and law grammars run ([`Options::jurisdictions`]).
    pub(crate) fn reads_us(&self) -> bool {
        self.jurisdictions.as_deref().is_none_or(source::us_jurisdiction)
    }
}

/// A footnote's byte range and number, so `supra note 4` resolves to the
/// authority first cited in note 4.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct NoteRange {
    pub number: u32,
    pub start: usize,
    pub end: usize,
    /// Numbering sequence the note belongs to. Documents that restart note
    /// numbering (per chapter, per part) repeat numbers across sequences; a
    /// reference prefers a note in its own sequence.
    #[serde(default)]
    pub sequence: u32,
    /// Where the note's marker stands in the text it annotates, when the caller knows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<usize>,
}

/// Find, classify and (optionally) resolve every citation in `text`.
pub fn extract(text: &str, options: &Options) -> Vec<Citation> {
    extract_markup(text, None, options)
}

pub fn extract_markup(text: &str, markup: Option<&str>, options: &Options) -> Vec<Citation> {
    extract_markup_with_parts(text, markup, options).0
}

/// Every caller's way in: a text the engine fails on gives no citations rather than stopping its host.
pub(crate) fn extract_markup_with_parts(text: &str, markup: Option<&str>, options: &Options)
    -> (Vec<Citation>, Vec<source::SourcePart>, Vec<resolve::Resolution>) {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_markup_with_parts(text, markup, options)))
        .unwrap_or_default()
}

fn read_markup_with_parts(text: &str, markup: Option<&str>, options: &Options)
    -> (Vec<Citation>, Vec<source::SourcePart>, Vec<resolve::Resolution>) {
    let timing = std::env::var_os("LEGALPDF_PHASE_TIMING").is_some();
    let mut stage_started = std::time::Instant::now();
    let mut stage = |name: &str| {
        if timing {
            eprintln!("phase	citations.{name}	{}", stage_started.elapsed().as_micros());
            stage_started = std::time::Instant::now();
        }
    };
    let markup = markup.map(|source| clean::Markup::new(text, source));
    let mut citations = find::find_styled(text, options, markup.as_ref());
    if options.source_only { citations.retain(|citation| citation.fields.source_case_name.is_some()); }
    stage("find_styled");
    let mut citations = find::filter_citations(citations);
    if options.skip_authority_lists {
        let listed = find::authority_list_citations(text, &citations, options.notes.as_deref().unwrap_or(&[]));
        citations.retain(|citation| !listed.contains(&citation.index));
        for (index, citation) in citations.iter_mut().enumerate() { citation.index = index; }
    }
    stage("filter_and_lists");
    metadata::attach(text, &mut citations);
    stage("metadata");
    find::anchor_titles(text, &mut citations, options.notes.as_deref().unwrap_or(&[]));
    #[cfg(feature = "us")]
    for citation in &mut citations {
        us::finish(citation);
    }
    if options.parallel {
        parallel::group(text, &mut citations);
    }
    for citation in &mut citations {
        citation.alias = aliases::resolve(citation);
        citation.key = citation.alias.as_ref().map(|target| target.key.clone())
            .or_else(|| key::key_in(citation, registry::registry()));
    }
    stage("titles_finish_group_alias");
    find::ocr_twins(&mut citations);
    find::titled_chapters(&mut citations);
    stage("twins_chapters");
    let notes = options.notes.as_deref().unwrap_or(&[]);
    let mut parts = source::split_notes(text, notes, options.split_tier, &citations, &options.jurisdiction_priority);
    if options.split_tier.is_some() { source::merge_split_citations(text, notes, &mut parts, &citations); }
    // A work's own short form written after it refers to it ([`Linking::short_form_references`],
    // [`Linking::bare_mentions`]).
    // The mechanisms the host names, else its split tier's.
    let linking = options.linking.clone().or_else(|| options.split_tier.map(Linking::for_tier));
    if let Some(linking) = linking.as_ref().filter(|linking| linking.short_form_references || linking.bare_mentions) {
        let defined = parts.iter().filter(|part| part.text.contains('[')).filter_map(|part| {
            let short = source::extract_fields(part, part.us_forms()).short_form;
            let short = short_forms::clean_short_form(&short);
            (!short.is_empty() && part.text.contains(short.as_str())).then_some((short, part.end))
        }).collect::<Vec<_>>();
        for mut reference in find::short_form_references(text, &citations, &defined, linking.bare_mentions) {
            reference.index = citations.len();
            citations.push(reference);
        }
    }
    let candidates = source::linked_ibid_candidates(text, notes, &parts);
    if !candidates.is_empty() {
        let preview = resolve::resolve_with_sources(&citations, options.notes.as_deref(), &[], None,
            &parts, options.supra_hint_mode, options.supra_linking_mode, options.split_tier,
            options.linking.as_ref(), &options.extents, Some(text));
        let linked = candidates.into_iter().filter(|&index| {
            let previous = &notes[index - 1];
            citations.iter().filter(|citation| previous.start <= citation.span.start
                && citation.span.end <= previous.end)
                .max_by_key(|citation| citation.span.start)
                .and_then(|citation| preview.iter().find(|resolution| resolution.index == citation.index))
                .is_some_and(|resolution| resolution.antecedent.is_some() || resolution.source_part.is_some())
        }).collect::<Vec<_>>();
        source::merge_linked_ibids(text, notes, &mut parts, &linked);
    }
    let resolutions = if options.resolve {
        let resolutions = resolve::resolve_with_sources(&citations, options.notes.as_deref(), &[], None,
            &parts, options.supra_hint_mode, options.supra_linking_mode, options.split_tier,
            options.linking.as_ref(), &options.extents, Some(text));
        for resolution in &resolutions {
            if let Some(citation) = citations.iter_mut().find(|citation| citation.index == resolution.index) {
                citation.antecedent = resolution.antecedent;
            }
        }
        resolutions
    } else { Vec::new() };
    if options.remove_ambiguous {
        citations.retain(|citation| !citation.is_ambiguous());
    }
    stage("notes_and_short_forms");
    (citations, parts, resolutions)
}

/// Whether `text` contains a citation or a two-party case name.
pub fn has_citation(text: &str) -> bool {
    find::has_citation(text)
}
