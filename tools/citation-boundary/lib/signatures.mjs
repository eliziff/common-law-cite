// Citation-grammar signatures: the surface forms and regex shapes that mark a
// piece of code as re-implementing what legal-citations owns.
//
// Code lists come from the engine registry (crates/legal-citations/registry)
// when the checker runs from a legal-citations checkout, merged with the seed
// lists below so the checker still works when the registry is empty or absent.

import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

// ---------------------------------------------------------------- seed data

const SEED_COURTS = `
SCC CSC FCA CAF TCC CCI CMAC CACM ONCA ONSC ONCJ ONSCDC ONSCSM BCCA BCSC BCPC ABCA ABQB ABKB ABPC ABCJ
SKCA SKQB SKKB SKPC MBCA MBQB MBKB MBPC NBCA NBQB NBKB NBPC NSCA NSSC NSPC NSFC NSSM NLCA NLSC NLTD
NLPC PECA PESC PEICA PEISC PESCAD PESCTD QCCA QCCS QCCQ QCTAQ QCTDP YKCA YKSC YKTC YKSM NWTCA NWTSC
NTCA NTSC NUCA NUCJ UKSC UKHL UKPC UKUT UKFTT EWCA EWHC EWCOP HCA FCAFC NSWCA NSWSC VSCA NZSC NZCA
NZHC CHRT CIRB CITT OLRB HRTO ONLTB ONWSIAT OHSTC PSLREB FPSLREB SCTC`;

const SEED_REPORTERS = `
SCR RCS FCR RCF DLR WWR CCC CPR BLR ETR RFL CCEL OAC BCLR BCAC ACWS WCB AWLD OWLD BCWLD WDFL CLLC
CELR CCLT MPLR CBR CHRR RJQ REJB EYB CarswellOnt CarswellBC CarswellAlta CarswellNat CarswellQue
AllER WLR CLR ALR NZLR F2d F3d F4th FSupp FSupp2d FSupp3d SCt LEd LEd2d OJ BCJ AJ`;

const SEED_SERIES = `
RSC RSO RSA RSBC RSM RSNS RSNB RSNL RSPEI RSQ RSY RSNWT RSS RRO SOR DORS CRC CQLR RLRQ CCSM CPLM SBC
SNS SNB SNL SPEI SNWT LRC LRO USC CFR OReg`;

/** Surface forms that are citation grammar only in company (two-letter reporters, ambiguous series). */
const WEAK = {
  court: ['FC', 'CF'],
  reporter: ['OR', 'AC', 'QB', 'KB', 'Ch', 'NR', 'AR', 'CR', 'US', 'SR', 'QL', 'WL'],
  series: ['SC', 'SO', 'SA', 'SS', 'SM', 'SY', 'LC', 'LQ', 'SI', 'TR', 'Reg', 'Stat'],
};

/** Canadian province prefix + court/tribunal suffix: ONCA, NBKB, SKHRT, ... */
const CANADIAN_CODE = /^(?:AB|BC|MB|NB|NL|NS|NT|NU|ON|PE|PEI|QC|SK|YK|YT|NWT)(?:CA|SC|QB|KB|PC|CJ|CS|CQ|SM|TD|CAD|SCAD|SCTD|SCDC|SCSM|HRT|HRC|HRAP|LRB|LB|WCAT|WCB|SEC|IPC|LA|LS|LT|LTB|CAT|PSLRB|FC|YJC|TC|DC|CPS|CPSDC|RB|MB)$/;

/** Short words that must never count as codes even when a registry lists them. */
const STOP = new Set([
  'THE', 'AND', 'FOR', 'NOT', 'API', 'URL', 'PDF', 'CSS', 'SQL', 'USA', 'UTF', 'XML', 'JSON', 'HTML', 'HTTP', 'HTTPS', 'ID', 'OK',
  // Months and weekdays appear in date regexes in every codebase.
  'JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'SEPT', 'OCT', 'NOV', 'DEC',
  'MON', 'TUE', 'TUES', 'WED', 'THU', 'THUR', 'THURS', 'FRI', 'SAT', 'SUN',
]);

export const fold = (surface) => String(surface).replace(/[^A-Za-z0-9]/g, '');

function words(block) {
  return block.split(/\s+/).filter(Boolean);
}

// ---------------------------------------------------------------- engine data

function readJson(path) {
  try { return JSON.parse(readFileSync(path, 'utf8')); } catch { return null; }
}

function listFiles(dir, predicate, out = []) {
  let entries;
  try { entries = readdirSync(dir, { withFileTypes: true }); } catch { return out; }
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) { if (entry.name !== 'target' && entry.name !== 'node_modules') listFiles(full, predicate, out); } else if (predicate(entry.name)) out.push(full);
  }
  return out;
}

/**
 * Load codes, corpus fingerprints and engine-source fingerprints from a
 * legal-citations checkout. Missing pieces degrade to the seed lists.
 */
export function loadEngineData(engineRoot) {
  const codes = { court: new Map(), reporter: new Map(), series: new Map() };
  const add = (kind, surface, strength) => {
    const key = fold(surface);
    if (key.length < 2 || STOP.has(key.toUpperCase())) return;
    if (!/[A-Z]/.test(key)) return;
    const current = codes[kind].get(key);
    if (current !== 'strong') codes[kind].set(key, strength);
  };
  const strengthOf = (surface) => {
    const key = fold(surface);
    if (key.length < 3) return 'weak';
    // Titlecase short words (`Cal`, `Mass`) collide with prose; all-caps codes do not.
    if (/^[A-Z][a-z]{1,4}$/.test(key)) return 'weak';
    return 'strong';
  };
  for (const code of words(SEED_COURTS)) add('court', code, 'strong');
  for (const code of words(SEED_REPORTERS)) add('reporter', code, 'strong');
  for (const code of words(SEED_SERIES)) add('series', code, 'strong');
  for (const [kind, list] of Object.entries(WEAK)) for (const code of list) add(kind, code, 'weak');

  const registryDir = engineRoot && join(engineRoot, 'crates', 'legal-citations', 'registry');
  let registryEntries = 0;
  if (registryDir && existsSync(registryDir)) {
    const tables = [
      ['courts.json', 'court'], ['upstream/courts.json', 'court'],
      ['reporters.json', 'reporter'], ['upstream/reporters.json', 'reporter'],
      ['series.json', 'series'],
    ];
    for (const [file, kind] of tables) {
      const rows = readJson(join(registryDir, file));
      if (!Array.isArray(rows)) continue;
      for (const row of rows) {
        registryEntries++;
        const surfaces = [];
        if (kind === 'court') surfaces.push(...(row.neutral ?? []), ...(row.aliases ?? []));
        if (kind === 'reporter') {
          surfaces.push(...(row.editions ?? []).map((edition) => edition.abbreviation));
          surfaces.push(...Object.keys(row.variations ?? {}));
        }
        if (kind === 'series') surfaces.push(row.abbreviation, ...(row.variations ?? []));
        // Upstream (US reporters-db/courts-db) data is large and full of short
        // English-looking forms (`Day`, `At`, `C.I.T.`): it only ever counts as
        // weak evidence, and single titlecase words not at all.
        const upstream = file.startsWith('upstream/');
        for (const surface of surfaces.filter(Boolean)) {
          const key = fold(surface);
          if (upstream) {
            if (key.length >= 3 && (/\d/.test(key) || (key.match(/[A-Z]/g) ?? []).length >= 2)) add(kind, surface, 'weak');
          } else add(kind, surface, strengthOf(surface));
        }
      }
    }
  }

  // Corpus patterns (verbatim copies of engine grammar are reported by id).
  const corpusPath = engineRoot && join(engineRoot, 'crates', 'legal-grammar', 'data', 'grammar-corpus.json');
  const corpusPatterns = new Map();
  const corpus = corpusPath && readJson(corpusPath);
  if (corpus?.tables) {
    for (const [table, value] of Object.entries(corpus.tables)) {
      for (const [name, pattern] of Object.entries(value.defs ?? {})) {
        if (typeof pattern === 'string' && pattern.length >= 30) corpusPatterns.set(pattern, `${table}.defs.${name}`);
      }
      for (const entry of value.entries ?? []) {
        if (typeof entry.pattern === 'string' && entry.pattern.length >= 30) corpusPatterns.set(entry.pattern, entry.id);
      }
    }
  }

  // Engine-source line fingerprints (to find copied engine code).
  const engineLines = new Set();
  const cratesDir = engineRoot && join(engineRoot, 'crates');
  if (cratesDir && existsSync(cratesDir)) {
    for (const file of listFiles(cratesDir, (name) => name.endsWith('.rs'))) {
      let text;
      try { if (statSync(file).size > 2_000_000) continue; text = readFileSync(file, 'utf8'); } catch { continue; }
      for (const line of text.split('\n')) {
        const normalized = normalizeSourceLine(line);
        if (normalized) engineLines.add(normalized);
      }
    }
  }

  return { codes, corpusPatterns, engineLines, registryEntries, corpusLoaded: Boolean(corpus) };
}

/** A source line worth fingerprinting: long, not boilerplate. */
export function normalizeSourceLine(line) {
  const trimmed = line.trim().replace(/\s+/g, ' ');
  if (trimmed.length < 40) return null;
  if (/^(?:\/\/|#|\*|\/\*)/.test(trimmed)) return null;
  if (/^(?:use |pub mod |mod |#\[|\}|\{|\)|\]|let _|impl |derive)/.test(trimmed)) return null;
  return trimmed;
}

// ---------------------------------------------------------------- code lookup

export function codeKind(data, token, { caseInsensitive = false } = {}) {
  const key = fold(token);
  if (!key) return null;
  for (const kind of ['court', 'reporter', 'series']) {
    const strength = data.codes[kind].get(key);
    if (strength) return { kind, strength, code: key };
  }
  if (/^Carswell[A-Z][a-z]+$/.test(key)) return { kind: 'reporter', strength: 'strong', code: key };
  if (CANADIAN_CODE.test(key)) return { kind: 'court', strength: 'strong', code: key };
  if (caseInsensitive) {
    const upper = key.toUpperCase();
    if (upper !== key) {
      for (const kind of ['court', 'reporter', 'series']) {
        for (const candidate of [upper, key[0].toUpperCase() + key.slice(1).toLowerCase()]) {
          const strength = data.codes[kind].get(candidate);
          if (strength) return { kind, strength, code: candidate };
        }
      }
      if (CANADIAN_CODE.test(upper)) return { kind: 'court', strength: 'strong', code: upper };
    }
  }
  return null;
}

// ---------------------------------------------------------------- regex sources

/** True when a string reads as regex source rather than prose or data. */
export function looksLikeRegex(value) {
  return /\\[dsbwDSWB]|\(\?(?:[:=!i]|P?<)|\[(?:\^?[A-Za-z0-9]-[A-Za-z0-9]|\\[dsw])|\\\.|[\])]\{\d+(?:,\d*)?\}|\\[pP]\{/.test(value);
}

/** True when a regex source is a pattern, not a literal needle like /2019 SCC 65/. */
export function hasPatternSyntax(source) {
  // `?` and `(?:` alone still describe one literal string (`/Bhasin v\. Hrynew(?:, supra)?/`).
  return /\\[dswDSWpP]|(?<!\\)\[[^\]]+\]|(?<!\\)\||(?<!\\)[*+]|\{\d+(?:,\d*)?\}/.test(source);
}

/** `^(?:v|vs|s|ss|no|para|ibid|supra|...)$`: an abbreviation/stop-word list, not grammar. */
export function isWordList(source) {
  const body = source.replace(/^\(\?[a-z]+\)/, '').replace(/\\[bB]|\^|\$|\\\.\??|\\s[*+?]?/g, '')
    .replace(/^\(\?:/, '').replace(/\)\??$/, '');
  const words = body.split('|');
  return words.length >= 8 && words.every((word) => /^[A-Za-z\u00c0-\u024f]{1,12}(?:s\?|\?)?$/.test(word));
}

const YEAR = String.raw`(?:\\d\{4\}|\\d\\d\\d\\d|\[12\](?:\\d|\[0-9\])\{3\}|\(\?:(?:1[6-9]|20|\[12\]\[0-9\])(?:\|(?:1[6-9]|20|\[12\]\[0-9\]))*\)(?:\\d|\[0-9\])\{2\}|(?:1[6-9]|20)(?:\\d|\[0-9\])\{2\}|(?:19|20)\\d\\d)`;
const OPEN_GROUP = String.raw`(?:\((?:\?:|\?<\w+>|\?P<\w+>)?)?`;
const SPACE = String.raw`(?:\\s|\s|\\x20|\[\s\\s\]|\[\\s\\u00a0\])[+*?]?`;
const UPPER_CLASS = String.raw`(?:\[A-Z|\[A-Za-z|CanLII|[A-Z]{2,}|\$\{|\{[A-Za-z_]\w*\})`;

const SHAPES = {
  // 2016 SCC 27, [year] [COURT] [number]
  neutral: new RegExp(`${YEAR}\\)?\\??${SPACE}${OPEN_GROUP}${UPPER_CLASS}[\\s\\S]{0,120}?${SPACE}${OPEN_GROUP}(?:\\\\d|\\[0-9\\]|\\[1-9\\])`),
  // [2016] 1 SCR 631, (1998) 40 OR (3d) 1
  yearVolume: new RegExp(String.raw`(?:\\\[|\\\()\??${OPEN_GROUP}${YEAR}\)?(?:\\\]|\\\))`),
  // RSC 1985, c C-46 / SO 2006, c 21
  chapter: new RegExp(`${YEAR}\\)?[\\s\\S]{0,40}?(?:,|\\\\s)[\\s\\S]{0,20}?(?:\\(\\?:)?c(?:h)?(?:\\\\\\.|\\(\\?:h\\)|\\\\?\\.)?\\??(?:\\\\s|\\s|\\\\b)`),
  // F.2d / F.3d / F. Supp.
  usReporter: /(?<![A-Za-z])F\\?\.?\??(?:\\s\??|\s)?(?:\\d|\[23\]|\[234\]|\[2-4\]|[234])d(?![a-z])|F\\?\.\??(?:\\s[?*+]?|\s)?Supp/,  // (1998) 40 OR (3d) 1, (2004) 49 McGill LJ 1: year, volume, Title, page
  yearVolumeTitle: new RegExp(`${YEAR}\\)?\\??\\)?\\??${SPACE}${OPEN_GROUP}(?:\\\\d|\\[0-9\\])(?:\\{\\d(?:,\\d)?\\}|[+*])?[\\s\\S]{0,60}?(?:\\[A-Z|\\{[A-Za-z_]\\w*\\})[\\s\\S]{0,160}?${SPACE}${OPEN_GROUP}(?:\\\\d|\\[0-9\\])`),
};

/** Named capture groups whose names say what the regex parses. */
const GROUP_NAMES = [
  [/^(?:neutral|neutral_?citation|neutral_?cite)$/i, 'neutral'],
  [/^(?:court|tribunal|court_?code)$/i, 'court'],
  [/^(?:reporter|report|reporter_?series|law_?report)$/i, 'reporter'],
  [/^(?:chapter|statute|regulation|reg|sor|series|act_?title|revised)$/i, 'legislation'],
  [/^(?:short|short_?form|supra|ibid|antecedent|short_?name)$/i, 'short-form'],
  [/^(?:pin|pins|pinpoints?|pin_?cite|para|paras|paragraphs?)$/i, 'pinpoint'],
  [/^(?:journal|author|authors|edition|edition_?imprint|imprint|publisher|session|record|house|sitting|hansard|debates)$/i, 'secondary'],
  [/^(?:history|disposition|subsequent)$/i, 'history'],
  [/^(?:style|style_?of_?cause|parties|plaintiff|defendant|appellant|respondent|versus)$/i, 'case-name'],
];

const SIGNALS = [
  /(?<![A-Za-z])cf(?:\\?\.)?(?![a-z])/i, /(?<![A-Za-z])contra(?![a-z])/i, /(?<![A-Za-z])citing(?![a-z])/i,
  /(?<![A-Za-z])quoting(?![a-z])/i, /see(?:\\s[+*?]?|\s)+(?:also|generally)/i, /but(?:\\s[+*?]?|\s)+see/i,
  /(?<![A-Za-z])accord(?![a-z])/i, /(?<![A-Za-z])voir(?:\\s[+*?]?|\s)+(?:aussi|également)/i,
];

const LEGAL_TITLE = /(?<![A-Za-z])Acts?\??(?:\)\?)?\|[^\n]{0,80}(?:Code|Regulations?|Convention|Treaty|Charter)|(?:Code|Regulations?|Rules\??)\|[^\n]{0,40}(?<![A-Za-z])Acts?(?![a-z])/;

const HISTORY = [
  /(?<![a-z])(?:aff|rev)(?:['’]|\[['’]+\]|\\u2019)\??(?:d|g|\[[dg]+\])(?![a-z])/i,
  /leave(?:\\s[+*?]?|\s|\\?\s)+to(?:\\s[+*?]?|\s)+appeal/i,
  /(?:affirmed|reversed|varied)(?:\\s[+*?]?|\s)+(?:by|on appeal)/i,
  /cert(?:\\?\.?\??)(?:\\s[+*?]?|\s)*denied/i,
];

const SHORT_FORM = [
  /(?<![A-Za-z])(?:supra|ibid|infra|idem)(?![a-z])/i,
  /(?<![A-Za-z\\])Id\\\./,
  /(?<![A-Za-z])above(?:\\s[+*?]?|\s)+n(?:ote)?(?![a-z])/i,
];

const PINPOINT = [
  /(?<![A-Za-z])at(?:\\s[+*?]?|\s|\)\??)+(?:\(\?:)*(?:paras?|pp?|pages?)(?![a-oq-z])/i,
  /(?<![A-Za-z])paras?(?:\\\.|\\?\.)?\?(?![a-z])/i, // paras? para\.?
  /(?<![A-Za-z])para\??s?\??(?:\\\.\??)?(?:\\s|\s|\)|\()/i, // para?s?\.?\s+
  /(?<![A-Za-z])paras?\??(?:\(\?:(?:graph|graphs\?|graphs)\??\)\?|\|)/i,
  /\|paras?(?![A-Za-z_])/,
  /(?:¶|\\u00b6|\\xb6)\??(?:\\s[+*?]?|\s|\(\?:|\()*(?:\\d|\[0-9\]|\[1-9\])/i,
];

const PROVISION_LABELS = [
  /(?<![A-Za-z])(?:sub)?sections?\??(?![a-rt-z])/i,
  /(?<![A-Za-z\\])ss?\\?\.\??(?![a-z])|(?<![A-Za-z\\])ss\?(?![a-z])/,
  /(?<![A-Za-z])(?:arts?|articles?)(?:\\?\.|\?)?(?![a-rt-z])/i,
  /(?<![A-Za-z])(?:rules?|rr?\\?\.)(?![a-z])/i,
  /(?<![A-Za-z])(?:parts?|divisions?|schedules?|subsections?|paragraphs?|clauses?|chapters?)\??(?![a-rt-z])/i,
  /§|\\u00a7|\\xa7/,
];

const CASE_NAME = [
  /(?<![A-Za-z])(?:Regina|Reginam|Rex|(?:Her|His)(?:\\s[+*?]?|\s)+Majesty|Attorney(?:\\s[+*?]?|\s)+General|Procureur|Reference(?:\\s[+*?]?|\s)+re)(?![a-z])/,
  // style of cause: `\s+v\.?\s+`, `(?:v|c)\.?\s`, `R\.? v`
  /(?:\\s[+*]?|\s)(?:\(\?:|\()?v(?:\|vs?|\|c)?\)?(?:\\\.|\\?\.)?\??(?:\\s|\s)(?![^|]{0,4}\\d)/,
  /(?<![A-Za-z])R(?:\\\.)?\??(?:\\s[+*?]?|\s)v(?:\\\.)?/,
];

/** Remove regex noise between letters so `R\.?\s?C\.?\s?S` reads `RCS`. */
function compactTokens(source) {
  const compact = source
    .replace(/\\[bBdDwWsS]|\\[pP]\{[^}]*\}|\\u[0-9a-fA-F]{4}|\\x[0-9a-fA-F]{2}/g, (m) => (/^\\s/i.test(m) ? '\u0000' : ' '))
    .replace(/\\\./g, '\u0001')
    .replace(/\[['’`]+\]\??|['’]\?/g, '')
    .replace(/\u0001\??/g, '')
    .replace(/\u0000[+*?]?/g, '')
    .replace(/ \?(?=[A-Za-z])/g, '');
  return compact.split(/[^A-Za-z0-9]+/).filter(Boolean);
}

function spacedTokens(source) {
  return source
    .replace(/\\[bBdDwWsS]|\\[pP]\{[^}]*\}|\\u[0-9a-fA-F]{4}|\\x[0-9a-fA-F]{2}/g, ' ')
    .replace(/\\\./g, '')
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean);
}

/** File-name and URL regexes are not citation grammar (`..._page-(\d+)\.png$`, `https?://...`). */
export function isPathOrUrlRegex(source) {
  return /\\\.(?:png|jpe?g|gif|webp|pdf|json|txt|html?|docx?|xml|csv|tsv|md|zip|js|ts|py|rs)\b/i.test(source)
    || /^\^?(?:\(\?:)?https\??|:\\?\/\\?\/|^\^?\\?\//.test(source);
}

/**
 * Classify one regex source. Returns a map category -> evidence strings for
 * each citation-grammar category it encodes.
 */
export function classifyRegex(data, raw, { caseInsensitive = false } = {}) {
  const found = new Map();
  const note = (category, evidence) => {
    if (!found.has(category)) found.set(category, new Set());
    found.get(category).add(evidence);
  };
  if (!hasPatternSyntax(raw) || isPathOrUrlRegex(raw)) return found;
  const wordList = isWordList(raw);

  // `\b` would otherwise read as a letter before the word it bounds.
  const source = raw.replace(/(?<!\\)\\[bBAZ]/g, ' ')
    // `[Pp]aragraph` reads as `paragraph`.
    .replace(/\[([A-Za-z])([A-Za-z])\]/g, (m, a, b) => (a.toLowerCase() === b.toLowerCase() ? a.toLowerCase() : m));

  for (const [, name] of raw.matchAll(/\(\?P?<([A-Za-z_]\w*)>/g)) {
    const hit = GROUP_NAMES.find(([pattern]) => pattern.test(name));
    if (hit) note(hit[1], `(?<${name}>)`);
  }
  if (/\(\?P?<volume>/.test(raw) && /\(\?P?<page>/.test(raw)) note('reporter', '(?<volume>) + (?<page>)');
  if (SHAPES.yearVolumeTitle.test(source)) note('reporter', 'year volume title page shape');
  const signals = SIGNALS.filter((pattern) => pattern.test(source)).length;
  if (signals >= 2 && !wordList) note('signal', `${signals} introductory signals`);
  if (LEGAL_TITLE.test(source)) note('legislation', 'Act|Code|Regulations title alternation');

  if (SHAPES.neutral.test(source)) note('neutral', 'year + court code + number shape');
  if (/(?<![A-Za-z])CanLII(?:\\s[+*?]?|\s|\)|\(\?:|\(|\\?\.)*(?:\\d|\[0-9\]|\[1-9\])/i.test(source)) note('neutral', 'CanLII + number');
  if (SHAPES.yearVolume.test(source)) note('reporter', '[year]/(year) volume shape');
  if (SHAPES.usReporter.test(source)) note('reporter', 'F.2d/F.3d/F. Supp.');
  if (SHAPES.chapter.test(source)) note('legislation', 'year, c chapter shape');
  for (const pattern of HISTORY) { const m = source.match(pattern); if (m) note('history', m[0]); }
  if (!wordList) for (const pattern of SHORT_FORM) { const m = source.match(pattern); if (m) note('short-form', m[0]); }
  const numbered = /\\d|\[0-9\]|\[1-9\]|\$\{|\{[A-Za-z_]\w*\}/.test(source);
  if (numbered && !wordList) for (const pattern of PINPOINT) { const m = source.match(pattern); if (m) note('pinpoint', m[0]); }
  for (const pattern of CASE_NAME) { const m = source.match(pattern); if (m) note('case-name', m[0].trim()); }

  const labels = PROVISION_LABELS.filter((pattern) => pattern.test(source)).length;
  if (labels >= 2 && /\\d|\[0-9\]|\[1-9\]/.test(source)) note('provision', `${labels} provision labels + number`);

  const weak = { court: new Set(), reporter: new Set(), series: new Set() };
  const seen = new Set();
  for (const token of [...compactTokens(source), ...spacedTokens(source)]) {
    if (seen.has(token)) continue;
    seen.add(token);
    // Codes are matched as written: a case-insensitive `cci` is not the Tax Court.
    const hit = codeKind(data, token);
    if (!hit) continue;
    if (hit.strength === 'strong') note(hit.kind === 'series' ? 'legislation' : hit.kind, hit.code);
    else weak[hit.kind].add(hit.code);
  }
  const hasNumber = /\\d|\[0-9\]|\[1-9\]/.test(source);
  for (const [kind, set] of Object.entries(weak)) {
    const category = kind === 'series' ? 'legislation' : kind;
    if ((set.size >= 3 && hasNumber) || (set.size >= 1 && found.has(category))) {
      for (const code of set) note(category, code);
    }
  }
  return found;
}

// ---------------------------------------------------------------- names

/** Function names that implement citation keys, resolution, formatting or routing. */
export const CITATION_FUNCTION = new RegExp('^(?:' + [
  'citation(?:lookup)?keys?', 'lookupkeys?', '(?:reporter|court|series|neutral|cite|authority)(?:lookup)?key',
  'normali[sz]e(?:citation|reporter|court|neutral|series|pinpoint)s?',
  'canonical(?:i[sz]e)?(?:citation|reporter|court|series)s?',
  'resolve(?:ibid|supra|idem|shortforms?|shortcites?|citationreferences?|antecedents?)',
  '(?:format|render|build|make)?pinpoint(?:label|prefix|text|string)?s?',
  '(?:format|render)(?:citation|neutral|reporter|mcgill|shortform|supra|ibid)s?',
  '(?:build|make|get|resolve|to)?canlii(?:case|legislation|court|statute|decision)?(?:url|link|route|path|href)s?',
  'canliicourtroute', 'canliiroutes?',
  'parse(?:citation|neutral|reporter|pinpoint)s?', '(?:extract|find|split|detect)(?:case)?citations?',
  'neutralcitations?', 'reportercandidates?', 'reportercitations?', 'canliicitations?',
  'mcgill(?:party|parties|citation|reporter|style|format|casename|abbreviat)[a-z]*', 'courtlevels?', 'courtfromcitation', 'jurisdictionfromcitation', 'legislationidcandidates?',
  'citationaliases?', 'citationaliasgroups?', 'citationforms?', 'cleancasename', 'splitcaseheading',
  'splitlegislationheading', 'legislationcitationcore', 'databasecitation',
].join('|') + ')$');

export function isCitationFunctionName(name) {
  return CITATION_FUNCTION.test(name.replace(/[^A-Za-z0-9]/g, '').toLowerCase());
}

// ---------------------------------------------------------------- misc signatures

export const CANLII_ROUTE = /^(?:ca|ab|bc|mb|nb|nl|ns|nt|nu|on|pe|qc|sk|yk|yt)\/[A-Za-z][A-Za-z0-9-]{1,15}$/;

export const PINPOINT_LABEL_LITERAL = /^(?:at )?(?:paras?|pp?|ss?|rr?|arts?|nn?|¶|§|at)\.? ?$/;
export const DISTINCT_PINPOINT_LABEL = /^(?:at )?(?:paras?|pp)\.? ?$|^at (?:p|pp)\.? ?$/;

export const CORPUS_MARKERS = /legal-grammar-(?:corpus|manifest):v\d|grammar-corpus\.json/;
