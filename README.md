# legal-citations

A deterministic legal citation engine in Rust for Canadian, Commonwealth and
US legal text, with Python, JavaScript/WebAssembly and command-line bindings.

This engine is under development; the packages described below are not yet
certified for release.

It finds citations, classifies them, parses their parts, groups parallel
citations, resolves short forms, `supra`, `ibid`/`Id.` and case-name
references, gives each authority a stable identity key, formats citations in
McGill style, links them to public sources (CanLII, Justice Laws, Find Case
Law, legislation.gov.uk, CourtListener) and annotates text or HTML.

The US behavior is meant to be a superset of Free Law Project's
[eyecite](https://github.com/freelawproject/eyecite): eyecite's own test suite
is ported into the conformance suite, and a weekly job diffs the engine
against the latest eyecite, reporters-db and courts-db.

## Install

The source repository builds the Rust crates, Python wheel/source distribution,
and JavaScript/WebAssembly tarball. Until registry releases are certified, install
the Python and JavaScript artifacts from the matching
[GitHub release](https://github.com/eliziff/common-law-cite/releases), and pin
Rust consumers to that release commit. All three bindings use the same engine
source and embedded citation data.

Python wheels are abi3 (one wheel per platform for every CPython ≥ 3.9) for
manylinux, musllinux, macOS and Windows.

## Quick start

Rust:

```rust
let citations = legal_citations::extract("R v Jordan, 2016 SCC 27 at para 105.", &Default::default());
assert_eq!(citations[0].span.text, "2016 SCC 27");
```

Python:

```python
import legal_citations

for c in legal_citations.extract("See R v Jordan, 2016 SCC 27 at para 5. Ibid at para 7."):
    print(c["form"], c["authority"], c["span"]["text"], c.get("antecedent"))

legal_citations.key_for_text("R v Jordan, 2016 SCC 27")          # '3:neutral:2016:scc:27'
legal_citations.format(text="R. v. Jordan, 2016 SCC 27, para. 5")  # McGill form
legal_citations.format_pinpoint("paragraph", [("12", "14")])      # 'at paras 12-14'
legal_citations.annotate(text, '<a href="{url}">', "</a>")
```

JavaScript (Node and Deno load the WebAssembly synchronously; browsers and
extensions call `init()` once):

```js
import { init, extract, keyForText } from "legal-citations";
await init(); // no-op under Node/Deno
const [citation] = extract("R v Jordan, 2016 SCC 27");
text.slice(citation.span.start, citation.span.end); // offsets are UTF-16 code units
```

In a Manifest V3 extension, allow WebAssembly compilation in extension pages:
`"content_security_policy": {"extension_pages": "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'"}`,
and pass `init()` the URL of `legal_citations_wasm_bg.wasm` from
`chrome.runtime.getURL(...)` if your bundler moves it. The entry point has no
top-level `await`, so it also loads in a service worker.

CLI (JSON out; reads files or stdin):

```sh
legal-citations extract opinion.txt --pretty           # offsets in chars by default
legal-citations key  < note.txt
legal-citations format --language fr notes/*.txt
legal-citations url --anchor brief.txt
legal-citations annotate page.html --clean-step html --before '<a href="{url}">' --after '</a>'
legal-citations call registry <<< '{"table": "courts", "surface": "FCA"}'
legal-citations batch < requests.jsonl                  # one process, many calls, JSON Lines
```

`--jsonl` treats each input line as `{"text": ...}`; `batch` answers
`{"id", "method", "request"}` lines with `{"id", "result"}` or `{"id", "error"}`
lines, which is the cheapest way to use the engine from a language without a
binding.

## One JSON API

Every binding is a thin layer over
[`legal_citations::api::call(method, request_json)`](crates/legal-citations/src/api.rs),
so all of them accept the same requests and return the same JSON:

| method | request | response |
|---|---|---|
| `extract` | `{text, options?, offsetUnit?}` | `{schemaVersion, offsetUnit, citations}` |
| `key` | `{citation}` | `{keyVersion, key}` |
| `resolve` | `{citations, notes?, aliasGroups?}` | `{citations, resolutions, authorities}` |
| `referenceInfo` | `{text}` | `{kind, notes, normalized}` using ALR's reference-marker and normalization rules; note digits are strings in note/n/nn pattern precedence |
| `bareCitation` | `{text, kind}` | ALR's bare authority text for lookup, retaining its signal, commentary and short-form removal rules |
| `stripCitationTail` | `{text}` | citation text without ALR's administrative book-of-authorities tab suffix |
| `legislationLookup` | `{text}` | `{candidates, jurisdiction}` using Pinpointer's CanLII index-ID spelling rules; candidates require an index match before becoming a URL |
| `correctCitation` | `{text, kind, style?, jurisdiction?, sourceLayout?, reporter?, correctedReporter?, page?, metadata?}` | `{citation, full, page}` using Eyecite's corrected-citation routines; supplied source layout is retained outside an established U.S. jurisdiction |
| `keyForText` | `{text, options?}` | `{keyVersion, key, reason?, keys}` |
| `format` | `{citation}` \| `{text}` \| `{pinpoint}`, `language?`, `rangeDash?` | `{citations: [{index, formatted}]}` \| `{pinpoint}` |
| `url` | `{citation}` \| `{text}`, `language?`, `anchor?` | `{urls: [{index, url}]}` |
| `annotate` | `{text, annotations? \| before/after templates, span?, source?, cleanSteps?, unbalancedTags?}` | `{text}` |
| `clean` | `{text, steps}` | `{text}` |
| `registry` | `{table?, surface?}` | the registry, one table, or the entries a surface names |
| `classifyExcerpt` | `{excerpt}` | prose / authority list / mixed classification |
| `hasCitation` | `{text}` | `{hasCitation}` |
| `version` | `{}` | crate, schema, key and grammar versions, registry counts |

`options` are `resolve`, `parallel`, `extendedUs` (all default `true`),
`removeAmbiguous` (default `false`), `jurisdictionPriority` (default empty), and
`notes` (footnote ranges, so `supra note 4` resolves). Unknown keys are
errors. Errors are `{code, message}` with `code` one of `unknown_method`,
`invalid_request`, `invalid_offset`, `unimplemented`.

## The model: form × authority

eyecite encodes two questions in one class hierarchy. legal-citations answers
them separately, so Commonwealth forms fit without new classes:

* `form` — how the text refers to the authority: `full`, `short`, `supra`,
  `ibid`, `reference`, `unknown`.
* `authority` — what is cited: `case`, `statute`, `regulation`,
  `constitution`, `court_rule`, `treaty`, `bill`, `debate`,
  `parliamentary_paper`, `government_document`, `journal`, `book`,
  `book_chapter`, `webpage`, `unknown`.
* `format` — the shape of the core: `neutral`, `reporter`, `can_lii`,
  `database`, `docket`, `statute_volume`, `regulation_series`, `code`,
  `publication`, `url`.

Each citation carries `span` (the core), `fullSpan` (style through
parentheticals), `style`, `signal`, `parties`, `fields` (year, volume,
reporter, reporterCanonical, page, number, series, chapter, section, note, ...),
`court` (registry id), `jurisdiction`, `language`, `pinpoints` (kind, first,
last), `parentheticals`, `history`, `parallelGroup`, `antecedent` and `key`.
`parties` is the shared parsed name record; either side may be null.
Bindings and resolution use it directly, without reparsing the styled extent.
The JSON Schema is [`conformance/schema/citation.schema.json`](conformance/schema/citation.schema.json)
(`schemaVersion` 2: breaking changes bump it; new optional properties do not).

### eyecite → legal-citations

| eyecite | legal-citations |
|---|---|
| `get_citations(text)` | `extract(text)` |
| `FullCaseCitation` | `form: full`, `authority: case` |
| `ShortCaseCitation` | `form: short`, `authority: case` |
| `FullLawCitation` | `form: full`, `authority: statute` / `regulation` / ... |
| `FullJournalCitation` | `form: full`, `authority: journal` |
| `SupraCitation` | `form: supra` (also `above n 4`, `(n 4)`) |
| `IdCitation` | `form: ibid` (also `Ibid`) |
| `ReferenceCitation` | `form: reference` |
| `UnknownCitation` | `form: unknown` |
| `groups["volume" / "reporter" / "page"]` | `fields.volume` / `fields.reporter` / `fields.page` |
| `corrected_reporter()` | `fields.reporterCanonical` |
| `groups["title" / "chapter" / "section"]` (laws) | `fields.volume` / `fields.chapter` / `fields.section`, series in `fields.series` |
| `metadata.pin_cite` | `pinpoints[]` (`kind`, `first`, `last`, `span`) |
| `metadata.parenthetical` | `parentheticals[]` with `kind: explanatory` |
| `metadata.year`, `metadata.court` | `fields.year`, `court.id` (courts-db ids for US courts) |
| `metadata.plaintiff` / `defendant` | `parties.plaintiff` / `parties.defendant` |
| `span()` / `full_span()` | `span` / `fullSpan` (for `Id.`, `supra` and references eyecite's `span()` is our `fullSpan`) |
| `resolve_citations()` | `antecedent` on each back reference (`resolve: true`) |
| `annotate_citations()` | `annotate` (exact offset map from `clean`, no diffing) |
| `clean_text(text, steps)` | `clean` with the same step names |
| hash of a `Resource` | `key` (versioned, stable across spellings and languages) |

For code already written against eyecite, `legal_citations.eyecite` provides
`get_citations`, `resolve_citations`, `annotate_citations`, `clean_text` and
the eyecite class names with the attributes eyecite users rely on (`groups`,
`metadata.*`, `span()`, `full_span()`, `corrected_citation()`); the full record
is on `.data`. See the module docstring for the known differences.

## Offsets

The Rust API reports UTF-8 byte offsets. The JSON API converts every offset
(spans, pinpoints, parentheticals, history, and `notes`/`annotations` in
requests) to the requested `offsetUnit`: `byte`, `char` (Unicode scalar
values, Python string indices) or `utf16` (JavaScript string indices). The
Python binding and the CLI default to `char`, the npm package to `utf16`.
`span.text` is always the exact substring.

## Registry

Courts, reporters, statute and regulation series, journals and jurisdictions
are JSON data in [`crates/legal-citations/registry/`](crates/legal-citations/registry/):
Canadian, Commonwealth and McGill entries are authored; US entries are
generated from reporters-db and courts-db at the commits pinned in
[`upstream.lock`](upstream.lock). Read it with `registry` (`{table, surface}`
returns every entry a surface names, preferred first — `FCA` is the Federal
Court of Appeal before the Federal Court of Australia).

## Identity keys (v3)

`key` identifies an authority, not a spelling: `2015 SCC 5` and `2015 CSC 5`
share `3:neutral:2015:scc:5`; `RSC 1985, c C-46` and `LRC 1985, ch C-46` share
`3:statute:ca:rsc:1985:c-46`. The grammar is a durable contract that changes
only with `keyVersion`; see the [`key` module documentation](crates/legal-citations/src/key.rs).

Reporter keys include their registry id and edition. They preserve the year
when the report's volume numbering resets annually. Incomplete or ambiguous
citations have no key.

Code keys retain captured chapters, subjects, acts and other identifying fields.
Source-edition alternatives remain available when a date or contextual reading
does not select one; unresolved editions do not acquire keys or authority merges.

Known reporter parallels share their canonical authority's key. `alias` reports
the canonical citation and evidence-record ids from
[`registry/aliases.json`](crates/legal-citations/registry/aliases.json), which
preserves every original ALR/A2AJ record and the source-backed corrections.
Unresolved records remain available for inspection and do not create links.

Applications with installed source indexes can pass their complete alias
closures to `resolve` as `aliasGroups: [{index, keys}]`. The engine applies
Beaver's reciprocal-closure checks before grouping cases and resolving their
references. Conflicting closures or contradictory citation identities remain
separate with a `source_alias_conflict` reason. Retrieval stays with the caller;
the engine performs no database or network access.

For shared abbreviations such as `CLR`, callers can pass
`jurisdictionPriority: ["ca", "au"]` (Python: `jurisdiction_priority`).
Explicit court evidence takes precedence over this order. `interpretations`
retains competing meanings and explains the selected reading; equal-ranked
meanings remain unresolved. A priority never repairs a conflicting source alias.

## Conformance

[`conformance/`](conformance/) holds language-neutral JSON cases run by the
Rust (`cargo test -p legal-citations-cli`), Python (`python conformance/run.py`)
and JavaScript (`node conformance/run.mjs`) runners, so every binding is held
to the same behavior. Cases come from eyecite's test suite (ported by running
real eyecite) and from authored Canadian/Commonwealth gold cases. Each case
is `pass` or `pending`; CI fails on a `pass` case that regresses and on a
`pending` case that starts passing, so the pending list only shrinks.

## Staying current with upstream

`tools/sync-upstream.py` also generates `crates/legal-grammar/data/us-extractors.json`
from the pinned reporters-db templates using Eyecite's extractor construction.
It retains named captures, exact and variant edition candidates, original dates,
and prefilter strings. Runtime discovery compiles only the extractors selected
by the shared whitespace-insensitive prefilter. Citation fields expose these
source records separately from registry corrections.

`.github/workflows/upstream.yml` runs weekly: it moves the reporters-db and
courts-db pins and regenerates the registry (`tools/sync-upstream.py`),
re-ports eyecite's latest tests into conformance cases, runs
`tools/eyecite-diff.py` against the latest eyecite, and opens a pull request
with the results. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT (see [LICENSE](LICENSE)). The registry and grammar include data derived
from Free Law Project's eyecite, reporters-db and courts-db under the BSD
2-Clause License; see [NOTICE](NOTICE), which ships with every package.
