# Deviations ledger

Every omission, guess, correction, unverified item and scope deviation reported
during the consolidation, recorded verbatim in substance, with its status. An
item closes only when a test or recorded evidence covers it.

| # | Area | What happened | Source | Status |
|---|------|---------------|--------|--------|
| 1 | Registry | UK reporter `P` (Probate) omitted as "ambiguous" (clashes with US `P.`) | registry agent report | Open: coverage agent restoring; both entries kept |
| 2 | Registry | `Ex`/`Eq` tokens omitted | registry agent report | Open: restoring |
| 3 | Registry | `ILR` omitted | registry agent report | Open: restoring |
| 4 | Registry | Session Cases `SC` omitted (clashes with Canadian `SC` statutes) | registry agent report | Open: restoring; both kept |
| 5 | Registry | UK Acts chapter series omitted | registry agent report | Open: restoring |
| 6 | Registry | courts-db `flaindcommn` dates dropped (start after end upstream) | registry agent report | Open: restore dates as published, flag upstream contradiction |
| 7 | Registry | Coordinator instruction "prefer fewer, correct entries over many guessed ones" invited omissions | coordinator | Closed: superseded by no-drop rule + coverage test |
| 8 | Reporting | Items 1-6 not relayed to the user when the registry was reported done | coordinator | Closed: this ledger |
| 9 | Registry | 730 McGill strings of uncertain type initially placed in journals | registry agent | Open: 4 reviewers classifying every entry with evidence |
| 10 | Registry | 331 McGill entries moved to reporters "from memory of the McGill appendix" | registry agent | Open: re-verified by the same 4 reviewers |
| 11 | Registry | 399 McGill entries left as `verified: false` journals | registry agent | Open: being classified, not left unverified |
| 12 | Registry | 535 McGill entries classified as journals by name pattern only | registry agent | Open: verification pending after shards |
| 13 | Registry | `QCCQLC` has no known name; code used as its name | registry agent | Open: courts reviewers |
| 14 | Registry | `FOREP` is a CanLII database, not a court; typed as tribunal | registry agent | Open: courts reviewers |
| 15 | Registry | `RLLR` recorded as alias of `rpd` | registry agent | Open: courts reviewers |
| 16 | Registry | Every CanLII database code from Beaver's route table (e.g. `ONLTB`, `ABAER`) was recorded as a neutral-citation court identifier. Many of these bodies issue no neutral citations; their decisions are cited only as `2021 CanLII 12345 (ON LTB)`. As recorded, the engine would accept and key a neutral citation like `2021 ONLTB 5` that does not exist | registry agent | Open: courts reviewers confirm per body whether it issues neutral citations; codes that are only CanLII database codes move out of `neutral` |
| 17 | Registry | Yukon CanLII segment changed `yt` -> `yk`; UKJCPC routed `ca/ukjcpc` (evidence: web search results; canlii.org blocked from this environment) | registry agent | Open: needs corrections.json with evidence; not directly fetched |
| 18 | Review | Coordinator's McGill review prompts permitted `drop` | coordinator | Closed: corrected to no-drop before any results |
| 19 | Upstream | reporters-db / courts-db entries are taken as published; checked for conversion fidelity only, not independently verified | coordinator | Open: stated limitation |
| 20 | Audit | Coordinator initially called parallel-citation dropping a bug; it is AuthoritiesHelper policy | coordinator | Closed: engine groups, apps choose (`parallel::preferred`) |
| 21 | Engine/key | `key::v1` (port of the retired v1 normalizer) kept "for migration"; user chose a new versioned key | downstream agent | Open: decide at review gate (remove, or keep only in DB rebuild scripts) |
| 22 | Engine/key | `canlii:{year}:{num}` omits the court on the assumption CanLII numbers are unique per year across all courts | downstream agent | Open: needs evidence |
| 23 | Engine/key | No keys for constitutions, treaties, or titled acts without a chapter | downstream agent | Open |
| 24 | Engine/parallel | Grouping refuses pairs eyecite would group: different neutral identities, same reporter at different volume/page, different courts, years >2 apart | downstream agent | Open: deliberate stricter rule; needs conformance cases and user sign-off |
| 25 | Engine/parallel | `preferred` ranks CanLII below specialty reporters and above databases | downstream agent | Open: confirm against AuthoritiesHelper keep-one policy |
| 26 | Engine/url | CanLII legislation URLs are built from the registry alone and are not checked against an index of existing pages; a URL may point to a page that does not exist | downstream agent | Open |
| 27 | Engine/url | French legislation URL shape `/fr/.../laws/...` copied from legal-pinpointer, not confirmed against CanLII | downstream agent | Open |
| 28 | Engine/url | UK Find Case Law coverage is a fixed table; EWHC without division, UKHL, EAT abstain | downstream agent | Open |
| 29 | Engine/format | Beaver renders a section range as `s 7(2)–(4)`; engine emits McGill `ss 7(2)-(4)` (user-visible change in Beaver) | downstream agent | Open: user decision at review gate |
| 30 | Engine/annotate | Overlapping annotations are dropped, not nested | downstream agent | Open |
| 31 | Engine/clean | HTML entity table covers Latin-1 + common typographic entities only; `<head>` content other than title/style/script/template kept | downstream agent | Open |
| 32 | Engine | `cargo clippy` fails on other agents' code (find.rs:391, a text.rs test) | downstream agent | Open: fix at integration |
| 33 | Boundary | Claude Code hook fails open: if the engine clone is unavailable (offline), edits are not blocked | boundary agent | Open: decide fail-open vs fail-closed at review gate |
| 34 | Boundary | Many Beaver findings are in `benchmarks/` and `experiments/`; agent suggests ignoring them. Beaver AGENTS.md forbids consolidating experiments in ordinary refactors | boundary agent | Open: user decision (ignore, allowlist with reasons, or migrate) |
| 35 | Boundary | ~3% false positives in Beaver (an `O. Reg` keyword list, benchmark report f-strings, test fixtures), ~1 each in legal-pinpointer and legal-pdf-parser | boundary agent | Open: tune or allowlist with reasons during migration |
| 36 | Boundary | 75 of 92 engine corpus citation/pinpoint/reference entries detected if copied; misses are generic URL/boundary/conjunction fragments | boundary agent | Open: accepted limitation unless user objects |
| 37 | Boundary | `npx` distribution needed a root `package.json` bin | boundary agent | Closed: root `package.json` exposes `citation-boundary` and `citation-boundary-hook`; `npm run test:boundary` 55/55 |
| 38 | Boundary | Audit paths moved: `courtlistenerLocalBulk.ts` now `backend/src/lib/`, `folderSources.ts` now `frontend/src/app/authorities/`; AuthoritiesHelper `domain.mjs` `key()` at line 2 | boundary agent | Closed: informational |
| 39 | Boundary | Baseline findings to eliminate in migration: Beaver 186, legal-pinpointer 68, AuthoritiesHelper 16, legal-pdf-parser 28, legal-structure-parser 28 | boundary agent | Open: migration target is zero non-allowlisted |
| 40 | Conformance | Engine is NOT yet an eyecite superset: 149 pass / 197 pending overall. eyecite-find 62/216, resolve 14/27, annotate 13/21, clean 9/11, models 1/13; Commonwealth authored 50/58. Top eyecite failure causes: fullSpan (35), court id (35), citation count (25), form (18) | bindings agent | Open: engine work until pending is empty |
| 41 | Conformance | 2 regressions currently failing: subsection pulled into the core span for `Kan. Stat. Ann. § 21-3516(a)(2)` and `Ohio Rev. Code Ann. § 5739.02(B)(7)` | bindings agent | Open: extraction fix |
| 42 | Engine | `300 S.C. 1` canonicalized as `S. Ct.` instead of South Carolina Reports | bindings agent | Open |
| 43 | Engine | `supra note N` and `(n N)` do not resolve with `notes`; AGLC `2 Ibid.` read as a full citation | bindings agent | Open |
| 44 | Engine | Bracketed short name `[Jordan]` does not replace `shortName` | bindings agent | Open |
| 45 | Engine | Leave-refused history case finds only one of its two citations | bindings agent | Open |
| 46 | Engine | Book citations (e.g. Hogg) not extracted | bindings agent | Open |
| 47 | Engine | `(1990). Id.` read as a journal citation | bindings agent | Open |
| 48 | Bindings | eyecite facade gives eyecite's class sequence on 171 of 229 inputs | bindings agent | Open |
| 49 | Bindings | WASM module is 3.57 MB (~800 KB gzipped); ~1.8 MB is embedded registry + grammar data | bindings agent | Open: acceptable for MV3? decide at review gate |
| 50 | Bindings | `version` reported `null` for grammar sha256 and upstream pins | bindings agent | Closed: sha256 computed from the embedded corpus; pins from `registry/upstream/pins.json`, written by `sync-upstream.py` |
| 51 | CI | GitHub workflows not executed (YAML parse only); no real browser/extension run | bindings agent | Open: runs after push |
| 52 | CI | Reported flag mismatch between `upstream.yml` and `sync-upstream.py` | bindings agent | Closed: not a defect. Without `--check` the script writes; `--update` moves pins then writes |
| 53 | Aliases | 26,546 A2AJ-observed reporter aliases from ALR-Verifier `data/a2aj_reporter_aliases.json` being added to the engine; completeness test required | coordinator | Open: alias agent |
| 54 | Scope | ALR-Verifier (AlbertaLawReview) added to the migration; push needs the Claude GitHub App on that account | coordinator | Open: waiting on access; migration prepared locally meanwhile |
| 55 | Process | Commit 1dc6bd5 ("Report corpus hash…") used `git add -A` and also captured the coverage agent's in-progress fixture edits (tests/fixtures/sources/*), which its message does not describe | coordinator | Closed: explicit path staging from here on |
| 56 | AuthoritiesHelper | Python app removal prepared as a local commit (not pushed; awaiting review gate). 17,320 lines removed; last commit with it is 864ebde | coordinator | Open: review gate |
| 57 | AuthoritiesHelper | Two court-profile items exist only in the deleted `web/court-profiles.json`: the Federal Courts Rules source (SOR/98-106) for the FC/FCA profiles, and the Rule 348 paper-filing colour guidance. Must be carried into Beaver's profile sources before the removal is pushed | coordinator | Open: Beaver migration |
| 58 | AuthoritiesHelper | `tests/test_toa_maker.py` (1,801 lines of regression cases) is deleted with the app; its cases must become engine conformance cases (Beaver `benchmarks/grammar_vectors/harvest.py` already harvests part of it) | coordinator | Open: conformance port before push |
