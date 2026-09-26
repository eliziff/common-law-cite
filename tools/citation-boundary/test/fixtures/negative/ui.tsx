// Ordinary application code that mentions citations without re-implementing them.
// A commented-out regex is not code: const NEUTRAL = /\d{4}\s+SCC\s+\d+/;
import { extract } from "@eliziff/legal-citations";

const PROVINCES: Record<string, string> = { on: "Ontario", bc: "British Columbia", ab: "Alberta", qc: "Quebec", ns: "Nova Scotia" };
const LANGUAGES = ["en", "fr"];
const ISO_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
const VERSION = /^v(\d+)\.(\d+)\.(\d+)$/;
const PDF_NAME = /^(\d{4})([a-z]{2,10})(\d{1,5})(?: ?\(\d+\))?\.pdf$/iu;
const DOC_URL = /^https?:\/\/[^/]+\/en\/[a-z]+\/doc\/(\d{4})\//;
const EXACT = /2019 SCC 65/;
const WHITESPACE = /\s+/g;
const HEADERS = /^\s*(from|sent|to|cc|bcc|cci|subject)\s*:\s*(.*)$/iu;
const MONTHS = /\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*\s+\d{1,2},\s+\d{4}/;

export function extractCitations(text: string) {
  return extract(text, { resolve: true });
}

export function cacheKey(name: string) {
  return name.toLowerCase().replace(/[^a-z0-9]/g, "");
}

export function Help() {
  const copy = "Paste a judgment, e.g. R v Jordan, 2016 SCC 27 at para 5; we find ibid and supra references for you.";
  return <p title={copy}>Don't worry about citations: the engine handles them.</p>;
}

export const values = { PROVINCES, LANGUAGES, ISO_DATE, VERSION, PDF_NAME, DOC_URL, EXACT, WHITESPACE, HEADERS, MONTHS };
