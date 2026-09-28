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
