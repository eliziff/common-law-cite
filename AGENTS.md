# legal-citations agent guide

legal-citations is the only place where legal citation logic lives: grammar,
court/reporter/series data, identity keys, short-form resolution, parallel
grouping, formatting and public-source URLs. Beaver, AuthoritiesHelper,
legal-pinpointer, legal-pdf-parser and legal-structure-parser call it
([docs/boundary.md](docs/boundary.md)). A change that an application needs is
made and tested here. Local applications use this checkout directly; tagging,
publishing bindings and updating consumer pins are only required when requested.

## Layout

- `crates/legal-grammar`: the grammar corpus (`data/grammar-corpus.json`, its
  `manifest.json`) and the code that expands and compiles it.
- `crates/legal-citations`: the pipeline (`find` → `classify` → `metadata` →
  `parallel` → `resolve` → `key`), `format`, `url`, `annotate`, `clean`,
  `excerpt`, the registry (`registry/*.json`) and `api.rs`, the JSON surface.
- `crates/legal-citations-{py,wasm,cli}`: bindings over `api.rs`.
- `conformance/`: language-neutral cases (`cases/*.json`) and the Python/JS
  runners; the Rust runner is `crates/legal-citations-cli/tests/conformance.rs`.
- `tools/`: `sync-upstream.py` (reporters-db/courts-db), `eyecite-diff.py`.

## Rules

**Grammar lives only in the corpus, with vectors.** Every citation, pinpoint,
reference, provision or footnote-label pattern is a corpus entry: an `id`, the
`pattern` (JS-style named groups, `{{def}}` references to shared `defs`),
`flags`, `provenance` saying where it came from, and `vectors` with inputs that
must match (with the expected groups) and inputs that must not (`groups: null`).
Stage code compiles entries by id; it does not carry its own citation regexes.
Change a pattern by changing its entry and its vectors in the same commit, keep
dialects that differ side by side instead of merging them, and update
`manifest.json` (sha256, entry and vector counts). The corpus must keep
compiling under both the Rust (`fancy-regex`/`regex`) and ECMAScript dialects.

**Registry data is JSON, not code.** Courts, reporters, series, journals and
jurisdictions are rows in `crates/legal-citations/registry/*.json`, read by
every binding through `registry`. No court, reporter or series tables in Rust
source, and no per-binding copies. Authored rows (Canadian, Commonwealth,
international) are edited by hand; `registry/upstream/*.json` is generated and
never hand-edited. An authored row wins over an upstream one with the same
surface form.

**Every behaviour change carries conformance cases.** Add or update cases in
`conformance/cases/*.json` in the same change: the input, the expected
citations (only the fields the case is about), and `"status": "pass"`. A known
gap is recorded as `"status": "pending"`. The runners fail when a `pass` case
stops matching. Pending cases are backlog and skipped outside explicit review, so:

- the pending list only shrinks: add a pending case only to document a gap you
  are not fixing now, never to park a regression;
- never turn a `pass` case back into `pending`, and never weaken an expectation
  to make a case pass;
- after a fix, `python conformance/run.py --cli target/release/legal-citations --promote`
  flips the cases that now pass; review that diff.

**No application policy in the engine.** The engine reports what the text
says and offers choices; applications decide what to show. Examples: parallel
citations are grouped (`parallelGroup`) and `parallel::preferred` names the
member a one-citation display keeps, but dropping parallels is the
application's decision; `resolve` links `ibid`/`supra` to antecedents, but
whether a screen or document shows `Ibid` is the application's choice.
Options are named for what they do, never for a consumer, and default to the
neutral behaviour.

**Bindings stay thin over `api.rs`.** A capability exists once, as a Rust
function plus a method in `api.rs` (`call`/`call_value` dispatch, camelCase
JSON, `deny_unknown_fields`, errors as `ApiError`). The Python, WASM and CLI
crates only marshal JSON and offsets (`OffsetUnit`: bytes, chars, UTF-16); no
logic, defaults or post-processing in a binding. A new method gets a
conformance case so all three runners exercise it.

**Keys are versioned.** When the output of `key` changes for any input, bump
`key::KEY_VERSION` and say so in the release notes; consumers store keys.

**Syncing upstream.**

- reporters-db and courts-db: `python3 tools/sync-upstream.py --update` moves
  the pins in `upstream.lock` and regenerates `registry/upstream/*.json`;
  `--check` (CI) fails on drift. Update in its own change, run the Rust tests
  and all conformance runners, and read the registry diff for surface forms
  that now shadow or collide with authored rows. Keep NOTICE current (both
  projects are BSD-2-Clause).
- eyecite: port its test suite with
  `python conformance/tools/port-eyecite.py --eyecite /path/to/eyecite` at the
  commit pinned in `conformance/tools/eyecite.lock.json`; new cases arrive
  `pending` and `run.py --promote` marks those already satisfied. Use
  `tools/eyecite-diff.py` for a differential run over a text corpus. Port
  behaviour into the corpus and stages; do not vendor eyecite code.

## Validation

- `cargo test -p legal-grammar -p legal-citations` for corpus vectors, stages
  and registry validation; `cargo test -p legal-citations-cli` runs the Rust
  conformance runner.
- `python conformance/run.py --cli target/release/legal-citations` and, after
  `crates/legal-citations-wasm/build.sh`, `node conformance/run.mjs`.
- `python3 tools/sync-upstream.py --check` when registry or pins change.
