// A sentence splitter's abbreviation list and a text-fragment builder are not citation grammar.
const ABBREVIATION = /\b(?:Mr|Mrs|Ms|Dr|Prof|No|Nos|para|paras|ss?|art|arts|al)\.\s*$/iu;
const FOOTNOTE_MARK = /^\s*(\d{1,4}|[*†‡§¶#])(?:\s|[.)\],:;-])/;
const x = 10, y = 2;
const ratio = x / y / 2;
export function fragment(text) {
  return `#:~:text=${encodeURIComponent(text.slice(0, 40))}`;
}
export { ABBREVIATION, FOOTNOTE_MARK, ratio };
