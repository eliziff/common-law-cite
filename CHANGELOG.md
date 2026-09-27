# Changelog

All notable changes to the `legal-grammar`, `legal-citations` and
`legal-citations-cli` crates, the `legal-citations` Python package and the
`legal-citations` npm package, which are released together with one version.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project uses [Semantic Versioning](https://semver.org/). The JSON
citation schema (`schemaVersion`) and identity keys (`keyVersion`) are
versioned separately and noted here when they change.

## [Unreleased]

- Extraction now returns split footnote source parts alongside citation spans;
  separate resolution reuses those parts and offers safe or aggressive supra
  linking. A part without a proven authority remains unlinked.
- The ALR source splitter and short-form registry now share one Rust resolution
  pass, including explicit names, inferred forms, numbered notes and ibid.
- Pinned U.S. extractor captures and edition candidates now travel through the
  shared pipeline, including source post-citation metadata and markup references.
  Code identities retain the source's additional hierarchy fields.
- Pinpointer's CanLII legislation-index spelling rules are shared through
  `legislationLookup`; external TSV retrieval remains in the application.
- All 26,546 direct ALR/A2AJ reporter-alias records are preserved with their
  evidence. Reviewed parallels supply a canonical authority key and URL;
  disputed, incomplete and invalid records cannot supply a guessed identity.
- Reporter-series grammar accepts French ordinals and New Series spellings.
- Identity keys are now **v3**. Reporter keys include the registry reporter id
  and edition, preventing collisions between jurisdictions. Historical DLR,
  CCC and WWR numbering regimes preserve essential years; CTC and CNLR require
  their report year. Ambiguous numbering produces no identity.
- `jurisdictionPriority` ranks ambiguous court and reporter abbreviations.
  Written court evidence takes precedence, alternatives remain in
  `interpretations`, and unresolved ties produce no identity. Keys, reporter
  ranking and URLs use the same selected reporter.

## [0.1.0]

### Added

- Engine: citation finding, classification, metadata (pinpoints,
  parentheticals, history, parties, courts), parallel groups, resolution of
  short forms, `supra`/`above n`/`(n N)`, `ibid`/`Id.` and references, identity
  keys v2, McGill formatting, public-source URLs, annotation and cleaning.
- `api`: one JSON-in/JSON-out surface (`extract`, `key`, `keyForText`,
  `format`, `url`, `annotate`, `clean`, `registry`, `classifyExcerpt`,
  `hasCitation`, `version`) with byte/char/UTF-16 offsets. Citation schema
  version 1 (`conformance/schema/citation.schema.json`).
- Python package `legal_citations` (abi3 wheels, CPython ≥ 3.9) with the
  eyecite-compatible `legal_citations.eyecite` facade.
- npm package `legal-citations` (WebAssembly) for Node, Deno, browsers and MV3
  extensions, with TypeScript types.
- `legal-citations` CLI: `extract`, `key`, `format`, `url`, `annotate`,
  `clean`, `registry`, `version`, `call` and JSON Lines `batch`.
- Conformance suite shared by the Rust, Python and JS runners: eyecite's test
  suite ported from real eyecite output plus Canadian/Commonwealth gold cases.
- `tools/eyecite-diff.py` differential harness and weekly upstream sync.

[Unreleased]: https://github.com/eliziff/legal-citations/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/eliziff/legal-citations/releases/tag/v0.1.0
