// Types for the `legal-citations` npm package. The citation shape follows
// conformance/schema/citation.schema.json (schemaVersion 1).

export type OffsetUnit = "byte" | "char" | "utf16";

export type Form = "full" | "short" | "supra" | "ibid" | "reference" | "unknown";

export type Authority =
  | "case" | "statute" | "regulation" | "constitution" | "court_rule" | "treaty" | "bill"
  | "debate" | "parliamentary_paper" | "government_document" | "journal" | "book"
  | "book_chapter" | "webpage" | "unknown";

export type CitationFormat =
  | "neutral" | "reporter" | "can_lii" | "database" | "docket" | "statute_volume"
  | "regulation_series" | "code" | "publication" | "url";

export interface Span { start: number; end: number; text: string }

export type PinpointKind =
  | "paragraph" | "page" | "section" | "subsection" | "rule" | "article" | "schedule" | "footnote" | "clause";

export interface Pinpoint { kind: PinpointKind; span: Span; first: string; last?: string }

export interface Parenthetical { kind: "court" | "date" | "explanatory" | "source"; span: Span; content: string }

export interface History { relation: string; span: Span; target?: number }

export interface Fields {
  year?: string; volume?: string; reporter?: string; reporterCanonical?: string; page?: string;
  number?: string; series?: string; chapter?: string; schedule?: string; section?: string;
  regnal?: string; regulation?: string; edition?: string; place?: string; publisher?: string;
  bill?: string; session?: string; body?: string; url?: string; docket?: string; note?: number;
}

export interface Citation {
  index: number;
  form: Form;
  authority: Authority;
  format?: CitationFormat;
  span: Span;
  signal?: Span;
  fullSpan: Span;
  style?: Span;
  parties?: { plaintiff: string; defendant: string };
  fields: Fields;
  court?: { id: string; text: string };
  jurisdiction?: string;
  language?: string;
  pinpoints?: Pinpoint[];
  parentheticals?: Parenthetical[];
  history?: History[];
  shortName?: string;
  explicitShortName?: string;
  parallelGroup?: number;
  antecedent?: number;
  key?: string;
  reasons: string[];
}

export interface NoteRange { number: number; start: number; end: number; sequence?: number }

export interface EngineOptions {
  /** Attach short forms, supra, ibid and references to their antecedents (default true). */
  resolve?: boolean;
  /** Group parallel citations (default true). */
  parallel?: boolean;
  /** Extended US reporter/code pass (default true). */
  extendedUs?: boolean;
  /** Footnote ranges, in the call's offset unit, so `supra note N` resolves. */
  notes?: NoteRange[];
}

export interface ExtractOptions extends EngineOptions {
  /** Default "utf16" (JavaScript string indices). */
  offsetUnit?: OffsetUnit;
}

export interface Annotation { start: number; end: number; before?: string; after?: string }

export interface AnnotateOptions extends EngineOptions {
  before?: string;
  after?: string;
  annotations?: Annotation[];
  span?: "span" | "fullSpan";
  /** Markup `text` was cleaned from with `cleanSteps`; the result is that markup, annotated. */
  source?: string;
  cleanSteps?: string[];
  unbalancedTags?: "unchecked" | "skip" | "wrap";
  offsetUnit?: OffsetUnit;
}

export interface VersionInfo {
  version: string;
  schemaVersion: number;
  keyVersion: string;
  grammar: { format: string; entries: number; sha256: string | null };
  registry: { jurisdictions: number; courts: number; reporters: number; series: number; journals: number; upstream: Record<string, unknown> };
}

export class LegalCitationsError extends Error {
  code: "unknown_method" | "invalid_request" | "invalid_offset" | "unimplemented" | string;
}

/** Load the WebAssembly module (browser/bundler/extension entry; a no-op under Node). */
export function init(input?: string | URL | Request | Response | BufferSource | WebAssembly.Module): Promise<void>;
export function initSync(module: BufferSource | WebAssembly.Module): void;

export function call(method: string, request?: object): any;
export function extract(text: string, options?: ExtractOptions): Citation[];
export function key(citation: Citation): string | null;
export function keyForText(text: string, options?: EngineOptions): {
  keyVersion: string; key: string | null; reason?: "no_citation" | "multiple_citations" | "no_identity"; message?: string;
  keys: { index: number; text: string; key: string | null }[];
};
export interface FormatOptions extends EngineOptions {
  /** Only "mcgill" today. */
  style?: string;
  language?: "en" | "fr";
  rangeDash?: "-" | "\u2013";
}
export function format(input: Citation | string, options?: FormatOptions): { index: number; formatted: string | null }[];
export function formatPinpoint(
  kind: PinpointKind, locators: { first: string; last?: string }[], options?: Omit<FormatOptions, keyof EngineOptions>,
): string;
export function url(input: Citation | string, options?: EngineOptions & { language?: "en" | "fr"; anchor?: boolean }): { index: number; url: string | null }[];
export function annotate(text: string, options?: AnnotateOptions): string;
export function clean(text: string, steps: string[]): string;
export type RegistryTable = "jurisdictions" | "courts" | "reporters" | "series" | "journals";
/** The whole registry, one table, or (with `surface`) the entries of `table` a surface form names. */
export function registry(table?: RegistryTable, surface?: string): any;
export function classifyExcerpt(excerpt: string): {
  kind: string; citeTokens: number; citeRuns: number; citeCharCoverage: number;
  functionWords: number; proseWindow: string | null; rule: string;
};
export function hasCitation(text: string): boolean;
export function version(): VersionInfo;
