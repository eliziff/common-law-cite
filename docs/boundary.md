# The citation boundary

**Citation grammar, court/reporter/series data, citation keys, resolution and
citation formatting live only in legal-citations. Applications call the engine.**

That covers:

- recognising citations: neutral, CanLII, reporter, statute and regulation
  citations, journals and books, signals, subsequent history, case names,
  pinpoints, `supra` / `ibid` / `Id.`
- the data behind them: courts (codes, names, levels, CanLII databases),
  reporters and their variations, statute and regulation series, journals
- identity: citation lookup keys and their normalisation
- resolution: attaching short forms, `supra note N`, `ibid` and bare case
  names to the citation they refer to, and grouping parallel citations
- output: formatting citations, short forms and pinpoints, and building
  public-source URLs (CanLII, Justice Laws, ...)

Application *policy* stays in the application. Examples: which citation of a
parallel group to show (the engine groups them and `parallel::preferred` names the
member to keep),
whether to drop parallels, when a UI shows `Ibid`, which courts a search is
limited to.

`tools/citation-boundary/` enforces the rule in every consumer repository, in CI
and in Claude Code, so that nobody (people or agents) re-implements engine logic
in application code.

## What the checker catches

`node tools/citation-boundary/check.mjs --list-rules` prints the same list.
Every finding names the engine method to use instead. The methods are the
JSON API in `crates/legal-citations/src/api.rs` (`extract`, `key`,
`keyForText`, `format`, `url`, `annotate`, `clean`, `registry`,
`classifyExcerpt`, `hasCitation`, `version`), which the Rust, Python, WASM and
CLI bindings all expose.

### Regex grammar (`regex/*`)

Regex literals, `RegExp(...)`, Python `re.*(...)`, Rust `Regex::new(...)` /
`RegexBuilder` / `regex!`, and string fragments that read as regex source
(raw strings, `String.raw`, strings with `\d`, `\s`, `(?:`, classes) are
classified by what they encode. One regex can raise several rules.

| Rule | Catches | Use instead |
| --- | --- | --- |
| `regex/neutral` | year + court code + number shapes, `CanLII` + number | `extract` → `Citation.fields`; `hasCitation` |
| `regex/court` | court codes: `SCC`, `ONCA`, `S\.?C\.?C`, `EWCA`, and every neutral code in the registry | `extract` → `Citation.court`; `registry { surface }` |
| `regex/reporter` | reporter abbreviations (`S\.?C\.?R`, `R\.?C\.?S`, `DLR`, `WWR`, `F\.\s?\dd`, `CarswellOnt`), `[year] volume` and `(year) volume Title page` shapes, `(?<volume>)` + `(?<page>)` | `extract`; `registry { table: "reporters", surface }` |
| `regex/legislation` | series (`RSC`, `R\.?S\.?O`, `SOR`, `DORS`, `O\. ?Reg`, `U\.?S\.?C`, `CCSM`, ...), `year, c N` chapter shapes, `Act\|Code\|Regulations` title alternations | `extract`; `registry { table: "series", surface }` |
| `regex/short-form` | `supra`, `ibid`, `Id\.`, `infra`, `above note` | `extract` with `options.resolve` → `Citation.form`, `Citation.antecedent` |
| `regex/pinpoint` | `at\s+para`, `paras?\.?` + number, `¶` + number, `(?<pinpoint>)` | `extract` → `Citation.pinpoints`; `format { pinpoint }` |
| `regex/provision` | two or more provision labels (`s`, `ss`, `section`, `art`, `rule`, `part`, `§`) + a number | `Citation.pinpoints`; the corpus `provisions` table through the engine |
| `regex/history` | `aff'd`, `rev'd`, `leave to appeal`, `affirmed by`, `cert denied` | `Citation.history` |
| `regex/case-name` | `\s+v\.?\s+`, `R\.? v`, `Reference re`, `Regina`, `Attorney General`, `(?<style>)` | `Citation.style` / `parties` / `shortName`; `format` |
| `regex/signal` | two or more signals: `see also`, `but see`, `cf`, `contra`, `citing`, `quoting` | `Citation.signal` |
| `regex/secondary` | journal, book and Hansard groups: `(?<journal>)`, `(?<edition>)`, `(?<session>)` | `extract` (authority journal/book/debate) |

Not grammar, and not reported: literal needles (`/2019 SCC 65/` in a test),
file-name and URL regexes (`..._page-(\d+)\.png$`, `^https?://...`), date and
version regexes, abbreviation/stop-word lists (`^(?:v|vs|no|para|ibid)$` in a
sentence splitter), months that happen to spell reporter codes (`APR`), and
upstream US data forms that read as English (`Day`, `At`).

### Data (`data/*`)

| Rule | Catches | Use instead |
| --- | --- | --- |
| `data/court-table` | a cluster of 5+ distinct court codes (at least 3 unambiguous) in object keys, string lists, match arms or space-packed strings (`"ABCA ABKB ABPC ..."`); JSON objects keyed by codes whose values are records | `registry { table: "courts" }` / `{ surface }` → `Court { level, name, jurisdiction, canlii, neutral, aliases }` |
| `data/reporter-table` | the same for reporters, and JSON lists that are mostly reporter/journal abbreviations | `registry { table: "reporters" }`, `journals` |
| `data/series-table` | the same for statute/regulation series (`'rsc', 'rso', 'rsbc'`) | `registry { table: "series" }` |
| `data/canlii-route` | two or more CanLII database routes (`"on/onca"`, `"nb/NBQB"`) | `Court.canlii` / `Series.canlii`; `url` |

The code lists come from `crates/legal-citations/registry/*.json` of the
checkout the checker runs from, merged with a built-in seed list. Upstream
(reporters-db / courts-db) forms only ever count as weak evidence. JSON lists of
codes (a search scope) and JSON maps of counts are not tables.

### Code (`code/*`)

| Rule | Catches | Use instead |
| --- | --- | --- |
| `code/citation-function` | a *definition* (not a call) of a function named like `citationKey`, `citation_lookup_key`, `reporter_key`, `normalizeCitation`, `resolveIbid`, `pinpointLabel`, `formatPinpoint`, `canliiUrl`, `buildCanliiCaseUrl`, `neutralCitations`, `courtLevel`, `mcgillParty` ... A thin wrapper whose body calls a name imported from legal-citations is fine. | `keyForText` / `key`, `extract` with `resolve`, `format`, `url`, `registry` |
| `code/lookup-key` | a lowercase + strip-non-alphanumerics fold applied to a citation/reporter/court value, or inside a `key`/`citationKey`-style function | `keyForText` → `key` (versioned by `keyVersion`) |
| `code/short-form-logic` | comparisons or ternaries on `"ibid"`/`"supra"`/`"Id."` literals, `in ("ibid", "supra")`, templates rendering `, supra note ${n}` | `extract` with `options.resolve`/`notes` → `Citation.antecedent`; `format` |
| `code/pinpoint-format` | literals choosing pinpoint labels (`"para"`/`"paras"`, `'at para '`, `'s '`/`'ss '`) and templates rendering `at para ${n}` | `format { pinpoint }` |
| `code/canlii-url` | interpolated or concatenated `canlii.org/.../doc/` or `/laws/` URLs, and regexes parsing them | `url { citation, language, anchor }` |

### Vendored copies (`vendored/*`)

| Rule | Catches |
| --- | --- |
| `vendored/corpus` | a copy of `grammar-corpus.json` or its manifest (by name or the `legal-grammar-corpus:v1` format marker), code that loads the corpus directly, a file embedding 3+ corpus patterns verbatim. A regex that *is* a corpus entry says so: `(verbatim copy of corpus entry cite.statute.toa)` |
| `vendored/registry` | JSON records shaped like the registry or reporters-db/courts-db (`neutral`, `canlii`, `editions`, `variations`, `cite_type`, ...) |
| `vendored/engine` | a source file where 20+ lines and a quarter of its lines are copied from `crates/` |

`suppression/invalid` reports an inline suppression without a rule or reason;
`allow/stale` and `allow/expired` report allow entries (see below).

## Allowlisting

The goal is an empty allow list. Allow entries exist for code that is
genuinely not citation logic (a document-structure parser numbering its own
sections) and, temporarily, for code being migrated.

`.citation-boundary.json` at the repository root:

```json
{
  "engine": "eliziff/legal-citations@v0.1.0",
  "ignore": ["benchmarks/**"],
  "allow": [
    { "path": "src/instrument*.rs", "rule": "regex/provision", "reason": "numbers the instrument's own provisions; citations go through legal-citations" },
    { "path": "backend/src/lib/canliiUrls.ts", "rule": "*", "reason": "replaced by legal-citations url in #123", "until": "2026-11-30" }
  ]
}
```

- `engine` pins the legal-citations tag or revision. CI and the hook run the
  checker from that same pin, so moving the pin moves the rules with the engine.
- `path` is a glob (`**`, `*`, `?`, `{a,b}`; a pattern without `/` matches a
  file name anywhere). `rule` is a rule id, a family (`regex/*`) or `*`.
- `reason` is required. `until` (ISO date) makes the entry fail once it passes.
- `ignore` removes paths from the scan entirely (generated code, third-party
  data); `include` restricts it. `tableThreshold` (default 5) and
  `engineModules` (names that count as the engine for the wrapper exemption)
  are optional.

Inline, on the line or the line above the finding:

```ts
// citation-boundary-allow: regex/provision -- matches the contract's own "Section 4.2" headings
const HEADING = /^(?:section|article)\s+(\d+(?:\.\d+)*)/i;
```

`citation-boundary-allow-file: <rule> -- <reason>` anywhere in a file covers
the whole file. The reason after ` -- ` is mandatory.

**Allowlists only shrink.** An allow entry or inline suppression that matches
nothing is reported as `allow/stale` and fails the check (use `--allow-stale`
only while iterating locally). Delete it in the change that removed the code.

**Baseline** (staged migration only):

```sh
node <legal-citations>/tools/citation-boundary/check.mjs --baseline
```

writes one allow entry per offending file and rule, keeps entries still in use
and drops stale ones, and prints a loud warning. Commit it, then burn it down:
every later change may only delete entries.

## Consuming the checker

Wiring a repository is one file plus one CI step, with an optional Claude Code
hook and an AGENTS.md paragraph.

### 1. `.citation-boundary.json`

```json
{ "engine": "eliziff/legal-citations@<tag>", "allow": [] }
```

then `--baseline` if the repository has existing violations.

### 2. CI

The checker is a single zero-dependency Node (>= 18) script in this repository;
fetch it at the pinned tag and run it. `npx --yes github:eliziff/legal-citations#<tag> citation-boundary`
does **not** work: the repository has no root `package.json` exposing a bin.

GitHub Actions (after `actions/checkout`):

```yaml
      - name: Citation boundary
        run: |
          ref="$(node -p "require('./.citation-boundary.json').engine.split('@')[1]")"
          git clone --quiet --depth 1 --branch "$ref" https://github.com/eliziff/legal-citations "$RUNNER_TEMP/legal-citations"
          node "$RUNNER_TEMP/legal-citations/tools/citation-boundary/check.mjs" --format github
```

`--format github` prints `::error file=...,line=...::` annotations. For a
private repository use `https://x-access-token:${{ secrets.LEGAL_CITATIONS_TOKEN }}@github.com/eliziff/legal-citations`.
To pin a commit instead of a tag, `git init` + `git fetch --depth 1 origin <sha>` + `git checkout FETCH_HEAD`.
Other CI systems run the same three lines. Exit status: 0 clean, 1 violations
or stale/expired allow entries, 2 usage or config error.

Node consumers can instead depend on the engine once legal-citations publishes a
root `package.json` with `"bin": { "citation-boundary": "tools/citation-boundary/check.mjs" }`:
`"devDependencies": { "legal-citations": "github:eliziff/legal-citations#<tag>" }` and
`npx citation-boundary --format github`. Until then use the clone.

Locally: `node ../legal-citations/tools/citation-boundary/check.mjs` from the
repository root (`--format json`, `--verbose` for snippets, paths to narrow the
scan). The checker respects `.gitignore` through `git ls-files` (tracked and
untracked, not ignored) and walks the tree outside git. It skips files over
1.5 MB and minified bundles.

### 3. Claude Code hook

`tools/citation-boundary/claude-hook.mjs` is a `PreToolUse` hook for `Edit`,
`Write` and `MultiEdit`. It reads the hook JSON on stdin, applies the edit to
the current file in memory, runs the detectors before and after, and blocks
(exit 2, reason on stderr, which Claude reads) only when the edit **introduces**
a finding that is not allowlisted. Unrelated edits to a file with existing,
baselined findings pass. Edits inside legal-citations itself, ignored paths,
non-source files and other tools pass. A malformed input or config is a
non-blocking hook error (exit 1); CI remains the backstop.

`.claude/settings.json` in the consumer repository (it clones the pinned tag
once into `~/.cache/legal-citations/<tag>`, or uses `$LEGAL_CITATIONS_DIR` if
you keep a checkout, for example the sibling `../legal-citations`):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write|MultiEdit",
        "hooks": [
          {
            "type": "command",
            "command": "ref=$(node -p \"require(require('path').join(process.env.CLAUDE_PROJECT_DIR,'.citation-boundary.json')).engine.split('@')[1]\") && d=\"${LEGAL_CITATIONS_DIR:-$HOME/.cache/legal-citations/$ref}\" && { [ -f \"$d/tools/citation-boundary/claude-hook.mjs\" ] || git clone -q --depth 1 --branch \"$ref\" https://github.com/eliziff/legal-citations \"$d\" >&2; } && exec node \"$d/tools/citation-boundary/claude-hook.mjs\"",
            "timeout": 120,
            "statusMessage": "citation-boundary"
          }
        ]
      }
    ]
  }
}
```

With a fixed checkout the command is just
`node "$LEGAL_CITATIONS_DIR/tools/citation-boundary/claude-hook.mjs"`. The
command is shell form, so on Windows it runs under Git Bash. If the clone
fails (offline) the hook exits non-zero without blocking.

### 4. The AGENTS.md paragraph

Paste this verbatim into each consumer's AGENTS.md:

```markdown
## Legal citations

Citation grammar, court/reporter/series data, citation keys, ibid/supra/Id. resolution and citation or pinpoint formatting live only in legal-citations (eliziff/legal-citations, pinned in `.citation-boundary.json`). Call its API (`extract`, `keyForText`, `format`, `url`, `registry`, `hasCitation`, `annotate`, `clean`) through the Rust, Python, WASM or CLI binding. Do not write citation regexes, court/reporter/series tables, CanLII route or URL builders, lookup-key normalizers, ibid/supra resolvers or pinpoint formatters here, and never copy the grammar corpus, registry or engine source. If the engine cannot do what you need, stop and propose the change to legal-citations (grammar in the corpus with vectors, data in the registry, a conformance case), then move the pin. `citation-boundary` enforces this in CI and in a Claude Code hook; do not add allow entries or `citation-boundary-allow` comments to get past it without the owner's agreement, and delete allow entries that no longer match: the allow list only shrinks.
```

## When the engine is missing something

Do not work around the engine in the application, and do not allowlist the
workaround. Instead:

1. Write the case down as a conformance case in `conformance/cases/*.json`
   (input text, expected citations) with `"status": "pending"`, or reproduce it
   with `legal-citations extract`.
2. Add the capability here, following [AGENTS.md](../AGENTS.md): grammar goes
   in `crates/legal-grammar/data/grammar-corpus.json` with vectors, data in
   `crates/legal-citations/registry/*.json`, behaviour in the stage module with
   a conformance case flipped to `pass`, surface in `api.rs` so every binding
   gets it.
3. Tag a release and move the consumer's `engine` pin; the application change
   then calls the new method.

If the application needs a policy on top of engine output (show only the
preferred parallel, hide `Ibid` in a UI), that policy is application code and
does not trip the checker: it reads `Citation` fields, it does not parse text.

## Calibration

Read-only scans of the consumer repositories (2026-09-26, default config, no
allow entries):

| Repository | Findings | Notes |
| --- | --- | --- |
| Beaver | 186 | every audited offender found; ~5 clear false positives (a keyword list containing `O. Reg`, benchmark report strings, test fixtures building citation strings); 9 `regex/provision` hits on contract/document numbering to allowlist or ignore with `benchmarks/**`, `experiments/**` |
| legal-pinpointer | 68 | every audited offender found (core.js, canlii-courts.js, canlii-legislation.js, a2aj-index build, lens structure) |
| AuthoritiesHelper | 16 | toa_maker.py, grammar_tables.py, the corpus copy, domain.mjs `key()` |
| legal-pdf-parser | 28 | current tree: pairing/support grammar and `data/mcgill_reporters.json`; the audited `deterministic_citations.rs`, `grammar_tables.rs`, corpus copy and `legalpdf.grammar_tables` are deleted from HEAD and are all caught at `db82486^` |
| legal-structure-parser | 28 | the corpus copy, the `grammar/` crate and `citator.rs`/`text.rs` (copied engine source), citation regexes; 5 `regex/provision` hits in instrument/document code are the expected allowlist entries |

Recall on the engine's own grammar corpus (every `citations`, `pinpoints` and
`references` entry, expanded) is 75 of 92 entries (82%); the misses are generic fragments
(URLs, sentence boundaries, conjunctions) that are not citation-specific on
their own. The test suite asserts at least 70%.

## Development

```sh
cd tools/citation-boundary
node --test test/*.test.mjs
```

Fixtures of true positives and true negatives in JS, TS, Python, Rust and JSON
live in `test/fixtures/{positive,negative}`; a positive fixture's first line
lists the rules it must raise. When tuning a detector, run it over the consumer
repositories and read every new finding before and after.
