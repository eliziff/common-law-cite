// Rule ids, what each one catches, and the engine API to call instead.
//
// Rule ids are `<family>/<topic>` so an allow entry can name one topic
// (`regex/provision`) or a family (`regex/*`).

// Method names are the JSON API in crates/legal-citations/src/api.rs, the same
// in Rust (api::call), Python, WASM and the CLI.
const EXTRACT = 'extract { text, options } -> Citation { form, authority, fields, court, pinpoints, history, key }';

export const RULES = {
  'regex/neutral': {
    title: 'neutral / CanLII citation grammar',
    explain: 'A regex matches neutral citations (2016 SCC 27, 2019 CanLII 123) or year + court-code shapes.',
    use: `${EXTRACT}; Citation.fields (year/court/number); hasCitation for a yes/no test.`,
  },
  'regex/court': {
    title: 'court-code grammar',
    explain: 'A regex alternation or literal encodes court identifiers (SCC, ONCA, EWCA ...).',
    use: 'extract -> Citation.court; registry { table: "courts", surface } for a bare code.',
  },
  'regex/reporter': {
    title: 'law-report grammar',
    explain: 'A regex encodes reporter abbreviations or [year] volume REPORTER page shapes (S.C.R., D.L.R., F.3d).',
    use: `${EXTRACT}; registry { table: "reporters", surface } for canonical editions (R.C.S. -> SCR).`,
  },
  'regex/legislation': {
    title: 'statute / regulation grammar',
    explain: 'A regex encodes statute or regulation series (RSC, RSO, SOR, O Reg, U.S.C.) or "year, c N" chapter shapes.',
    use: `${EXTRACT} (authority statute/regulation); registry { table: "series", surface }.`,
  },
  'regex/short-form': {
    title: 'supra / ibid / Id. grammar',
    explain: 'A regex recognises short forms (supra, ibid, Id., above note N).',
    use: 'extract with options.resolve (and options.notes) -> Citation.form supra|ibid|short|reference and Citation.antecedent.',
  },
  'regex/pinpoint': {
    title: 'pinpoint grammar',
    explain: 'A regex parses pinpoints (at para 12, paras 3-5, at p 4, ¶ 7).',
    use: 'extract -> Citation.pinpoints; format { pinpoint } to render one.',
  },
  'regex/provision': {
    title: 'provision-label grammar',
    explain: 'A regex parses provision labels with numbers (s 5(1), ss 2-3, art 12, r 4). Legitimate in document-structure parsers that number their own units; allowlist it there with a reason.',
    use: 'extract -> Citation.pinpoints (section/article/rule) for citations; the corpus provisions table through the engine for document structure.',
  },
  'regex/history': {
    title: 'subsequent-history grammar',
    explain: "A regex recognises subsequent history (aff'd, rev'd, leave to appeal refused).",
    use: 'extract -> Citation.history.',
  },
  'regex/case-name': {
    title: 'style-of-cause grammar',
    explain: 'A regex parses case names (X v Y, R v, Reference re, Attorney General, Her Majesty).',
    use: 'extract -> Citation.style / parties / shortName; format for McGill party abbreviations.',
  },
  'regex/signal': {
    title: 'introductory-signal grammar',
    explain: 'A regex recognises introductory signals (see also, cf, contra, citing, quoting, but see).',
    use: 'extract -> Citation.signal.',
  },
  'regex/secondary': {
    title: 'journal / book / parliamentary grammar',
    explain: 'A regex parses secondary-source citations (journal volume/page, edition and imprint, Hansard sessions).',
    use: `${EXTRACT} (authority journal/book/book_chapter/debate).`,
  },
  'data/court-table': {
    title: 'court table',
    explain: 'A literal table/list keyed by court codes (levels, names, routes).',
    use: 'registry { table: "courts" } / { surface } -> Court { level, name, jurisdiction, canlii, neutral, aliases }. Add missing courts to crates/legal-citations/registry/courts.json.',
  },
  'data/reporter-table': {
    title: 'reporter table',
    explain: 'A literal table/list of reporter abbreviations or variations.',
    use: 'registry { table: "reporters" } / { surface } -> reporter + canonical edition. Add missing reporters or journals to registry/reporters.json or journals.json.',
  },
  'data/series-table': {
    title: 'statute-series table',
    explain: 'A literal table/list of statute or regulation series abbreviations.',
    use: 'registry { table: "series" } / { surface } -> Series { jurisdiction, kind, canlii }. Add missing series to registry/series.json.',
  },
  'data/canlii-route': {
    title: 'CanLII route table',
    explain: 'Literal CanLII database routes ("on/onca", "nb/NBQB").',
    use: 'Court.canlii / Series.canlii from registry; url for the full link.',
  },
  'code/canlii-url': {
    title: 'CanLII URL builder/parser',
    explain: 'Code builds or parses canlii.org decision/legislation URLs.',
    use: 'url { citation | text, language, anchor } -> public-source URL (CanLII, Justice Laws, ...).',
  },
  'code/citation-function': {
    title: 'citation function defined outside the engine',
    explain: 'A function whose name says it keys, normalizes, resolves, formats, routes or parses citations is defined here.',
    use: 'The engine method for the job: keyForText / key (identity keys), extract with resolve (supra/ibid), format (citations, short forms, pinpoints), url (links), registry (courts/reporters/series).',
  },
  'code/lookup-key': {
    title: 'citation lookup-key normalizer',
    explain: 'Citation/reporter/court strings are folded to a lookup key (lowercase + strip non-alphanumerics).',
    use: 'keyForText { text } -> key (versioned by keyVersion) or Citation.key from extract; registry { surface } folds surface forms.',
  },
  'code/short-form-logic': {
    title: 'ibid/supra resolution or rendering',
    explain: 'Code branches on or renders ibid/supra/Id. forms (resolving antecedents or choosing the short form to print).',
    use: 'extract with options.resolve/notes -> Citation.antecedent; format to render full/supra/ibid forms.',
  },
  'code/pinpoint-format': {
    title: 'pinpoint formatting',
    explain: 'Code renders pinpoint labels (para/paras, s/ss, at p).',
    use: 'format { pinpoint } (McGill pinpoint rendering); Citation.pinpoints for parsed values.',
  },
  'vendored/corpus': {
    title: 'vendored grammar corpus',
    explain: 'A copy of grammar-corpus.json/manifest, code that loads the corpus directly, or code embedding corpus patterns verbatim.',
    use: 'Depend on legal-citations at a pinned tag and call the engine; never copy crates/legal-grammar/data.',
  },
  'vendored/registry': {
    title: 'vendored registry data',
    explain: 'A data file shaped like the engine registry (courts/reporters/series) or reporters-db/courts-db.',
    use: 'registry() from legal-citations; contribute data to crates/legal-citations/registry/*.json.',
  },
  'vendored/engine': {
    title: 'copied engine source',
    explain: 'Source lines copied from legal-citations crates.',
    use: 'Depend on the legal-citations crate / bindings instead of copying source.',
  },
  'suppression/invalid': {
    title: 'invalid inline suppression',
    explain: 'A citation-boundary-allow comment without a rule or without a "-- reason".',
    use: 'Write `citation-boundary-allow: <rule> -- <reason>`.',
  },
};

export function describe(rule) {
  return RULES[rule] ?? { title: rule, explain: '', use: '' };
}
