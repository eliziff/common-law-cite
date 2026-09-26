// expect: regex/neutral, regex/reporter, regex/court, code/citation-function, code/canlii-url
const NEUTRAL = /\b((?:18|19|20)\d{2})\s+([A-Z][A-Z0-9-]{1,15})\s+(\d+)\b/g;
const CANLII = /\b(?:19|20)\d{2}\s+CanLII\s+\d+\b/gi;
const series = String.raw`[A-Z][A-Za-z]*(?:\s+[A-Z][A-Za-z]*){0,4}`;
const bracketed = new RegExp(String.raw`\[(?:18|19|20)\d{2}\]\s+(?:\d+\s+)?${series}\s+\d+`);
const french = /\b(?:S\.?C\.?C\.?|C\.?S\.?C\.?)\b(?=\s*\d)/;

function neutralCitations(text) {
  return [...text.matchAll(NEUTRAL)].map((match) => match[0]);
}

function canliiLink(year, court, number) {
  return `https://www.canlii.org/en/on/${court}/doc/${year}/${year}${court}${number}/${year}${court}${number}.html`;
}
module.exports = { neutralCitations, canliiLink, CANLII, bracketed, french };
