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

Association is independent of retrieval. A source with no URL can supply an
antecedent or source-part target. Numbered references stay within the indicated
note and numbering sequence; ambiguous sources remain unresolved. URLs are
optional navigation metadata applied to a resolved source afterward.

The registry reference methods take opaque source identifiers in
`ReferenceSource.target` and return the matched identifier with a reason.
Identifiers are compared exactly; they are not interpreted as URLs, lowercased,
or stripped of fragments. The caller supplies the same identifier for repeated
citations of one authority and distinct identifiers for distinct sources.

Grammar entries may carry descriptive `conventions` (for example `mcgill`).
These identify the citation convention illustrated by a rule, not an engine
mode or a jurisdiction filter. McGill Canadian note, publication and unreported
decision forms coexist with U.S. and other styles; mixed-style conformance
cases guard against collisions. URL-independent identity and association are
general engine contracts.

A newly detected unreported decision retains its date, court and file number.
It has no file-level identity key: different decisions in one court file must
not become one authority merely because their docket matches.

Application *policy* stays in the application. Examples: which citation of a
parallel group to show (the engine groups them and `parallel::preferred` names the
member to keep),
whether to drop parallels, when a UI shows `Ibid`, which courts a search is
limited to.
