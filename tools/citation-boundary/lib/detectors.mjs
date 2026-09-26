// Detectors: turn one file's text into boundary findings.

import { analyze, languageOf } from './lexer.mjs';
import {
  CANLII_ROUTE, CORPUS_MARKERS, DISTINCT_PINPOINT_LABEL, PINPOINT_LABEL_LITERAL,
  classifyRegex, codeKind, isCitationFunctionName, looksLikeRegex, normalizeSourceLine,
} from './signatures.mjs';

export const DEFAULT_ENGINE_MODULES = ['legal_citations', 'legal-citations', 'legalCitations', 'LegalCitations', '@eliziff/legal-citations'];

const REGEX_CONSTRUCTORS = {
  js: /\b(?:new\s+)?RegExp\s*\(/g,
  py: /\b(?:re|regex)\s*\.\s*(?:compile|search|match|fullmatch|sub|subn|findall|finditer|split)\s*\(/g,
  rs: /\b(?:Regex|RegexBuilder|RegexSet|RegexSetBuilder|FancyRegex)\s*::\s*new\s*\(|\b(?:regex|lazy_regex|regex_is_match|regex_find|regex_captures|bytes_regex)!\s*\(/g,
};

const FUNCTION_DEFS = {
  js: [
    /\bfunction\s*\*?\s+([A-Za-z_$][\w$]*)\s*[(<]/g,
    /\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=\n]{1,80})?=\s*(?:async\s+)?(?:function\b|\([^)]*\)\s*(?::[^=\n]{1,80})?=>|[A-Za-z_$][\w$]*\s*=>)/g,
    /^[ \t]*(?:(?:public|private|protected|static|async|export|override|readonly)\s+)*([A-Za-z_$][\w$]*)\s*\([^)\n]*\)\s*(?::[^{\n]{1,80})?\{/gm,
    /([A-Za-z_$][\w$]*)\s*:\s*(?:async\s+)?(?:function\b|\([^)]*\)\s*=>|[A-Za-z_$][\w$]*\s*=>)/g,
  ],
  py: [/^[ \t]*(?:async\s+)?def\s+([A-Za-z_]\w*)\s*\(/gm],
  rs: [/\bfn\s+([A-Za-z_]\w*)\s*[(<]/g],
};
const NOT_FUNCTIONS = new Set(['if', 'for', 'while', 'switch', 'catch', 'function', 'return', 'with', 'else']);

const FOLD_CASE = /toLowerCase\(\)|toUpperCase\(\)|toLocaleLowerCase\(\)|casefold\(\)|\.lower\(\)|\.upper\(\)|to_lowercase\(\)|to_ascii_lowercase\(\)|to_uppercase\(\)/;
const STRIP_NON_ALNUM = /\[\^(?:a-z0-9|A-Za-z0-9|a-zA-Z0-9|A-Z0-9|0-9a-z|a-z\\d|A-Za-z|a-zA-Z|\\w|\\p\{L\}\\p\{N\}|\\p\{L\}\\p\{Nd\}|\\p\{Alphabetic\}\\p\{N\}|\\p\{L\}\\d)\]|\\W|is_alphanumeric|isalnum\(\)/;
const CITATION_WORD = /citation|reporter|court(?![_ ]?(?:file|house|room))|neutral|\bcite|canlii|authorit/i;
/** Names of functions that produce citation lookup keys (`key`, `citationKey`, `reporter_key`, ...). */
const KEY_FUNCTION = /^(?:key|(?:citation|cite|reporter|court|series|neutral|authority|lookup|case|report)\w*key)$/i;
export const TEST_PATH = /(?:^|\/)(?:__tests__|tests?|spec|fixtures?)\/|[._-](?:test|spec)\.[a-z]+$|(?:^|\/)test_[^/]*\.py$|_test\.(?:py|rs)$/;

const SF_LIT = String.raw`(["'\`])(?:ibid|supra|idem|id\.)\1`;
const SHORT_FORM_LOGIC = [
  new RegExp(String.raw`(?:[!=]==?|\bcase\b|\?)\s*` + SF_LIT, 'i'),
  new RegExp(SF_LIT + String.raw`\s*(?:[!=]==?)`, 'i'),
  /\bsupra\s+note\s+(?:\$\{|\{[A-Za-z_(])/i,
  /\$\{[^}\n]+\},?\s*supra\b/i,
  /["'`]\s*,\s*supra(?:\s+note)?\s*["'`]\s*\+/i,
];

const TABLE_GAP = 8;

/**
 * Scan one file. Returns { findings, suppressions } where suppressions are the
 * inline `citation-boundary-allow` comments (with their parse state).
 */
export function scanFile(path, text, data, options = {}) {
  const lower = path.toLowerCase();
  if (lower.endsWith('.json')) return { findings: scanJson(path, text, data, options), suppressions: [] };
  const language = languageOf(path);
  if (!language) return { findings: [], suppressions: [] };
  const context = { path, text, data, options, language, findings: [] };
  const analysis = analyze(language, text);
  Object.assign(context, analysis);
  context.lines = text.split('\n');
  context.codeLines = analysis.code.split('\n');
  context.stringLines = analysis.codeWithStrings.split('\n');

  const suppressions = parseSuppressions(context);
  const consumed = new Set();
  const regexCandidates = collectRegexCandidates(context, consumed);
  detectRegexes(context, regexCandidates);
  const definitions = detectFunctions(context);
  detectLookupKeys(context, definitions);
  detectShortFormLogic(context);
  detectPinpointFormatting(context);
  detectTables(context, consumed);
  detectCanlii(context, regexCandidates);
  detectVendoredCode(context, regexCandidates);
  return { findings: context.findings, suppressions };
}

function add(context, rule, start, end, message, evidence = []) {
  context.findings.push({
    file: context.path, rule, line: start, endLine: Math.max(start, end ?? start),
    message, evidence: [...new Set(evidence)].slice(0, 8),
    snippet: (context.lines[start - 1] ?? '').trim().slice(0, 160),
  });
}

// ---------------------------------------------------------------- suppressions

const SUPPRESSION = /citation-boundary-allow(-file)?\s*:\s*(.*)$/;

function parseSuppressions(context) {
  const suppressions = [];
  for (const token of context.tokens) {
    if (token.type !== 'comment') continue;
    const lines = token.value.split('\n');
    lines.forEach((value, index) => {
      const match = value.match(SUPPRESSION);
      if (!match) return;
      const line = context.line(token.start) + index;
      const [rulesPart, ...reasonParts] = match[2].replace(/\*\/\s*$/, '').split(/\s+(?:--|—)\s+/);
      const rules = (rulesPart ?? '').split(/[\s,]+/).filter(Boolean);
      const reason = reasonParts.join(' -- ').trim();
      const suppression = { file: context.path, line, fileWide: Boolean(match[1]), rules, reason, used: false };
      if (!rules.length || !reason) {
        add(context, 'suppression/invalid', line, line, 'citation-boundary-allow needs `<rule> -- <reason>`.');
        suppression.invalid = true;
      }
      suppressions.push(suppression);
    });
  }
  return suppressions;
}

// ---------------------------------------------------------------- regex sources

function closeArgument(code, open) {
  let depth = 0;
  for (let i = open; i < code.length; i++) {
    const c = code[i];
    if (c === '(' || c === '[' || c === '{') depth++;
    else if (c === ')' || c === ']' || c === '}') { depth--; if (depth === 0) return { end: i, callEnd: i }; }
    else if (c === ',' && depth === 1) {
      // End of the first argument; find the call end for flag inspection.
      let d = depth, j = i;
      for (; j < code.length; j++) {
        if ('([{'.includes(code[j])) d++;
        else if (')]}'.includes(code[j])) { d--; if (d === 0) break; }
      }
      return { end: i, callEnd: j };
    }
  }
  return { end: code.length, callEnd: code.length };
}

function collectRegexCandidates(context, consumed) {
  const { tokens, code, language, text } = context;
  const candidates = [];
  const strings = tokens.filter((token) => token.type === 'string' || token.type === 'template');

  if (language === 'js') {
    for (const token of tokens) {
      if (token.type === 'regex') {
        candidates.push({ start: token.start, end: token.end, source: token.value, caseInsensitive: token.flags.includes('i'), kind: 'literal' });
      }
    }
  }

  const constructor = REGEX_CONSTRUCTORS[language];
  constructor.lastIndex = 0;
  let match;
  while ((match = constructor.exec(code))) {
    const open = match.index + match[0].length - 1;
    const { end, callEnd } = closeArgument(code, open);
    const pieces = strings.filter((token) => token.start > open && token.start < end && !consumed.has(token));
    if (!pieces.length) continue;
    pieces.forEach((token) => consumed.add(token));
    const rest = text.slice(end, callEnd + 1) + code.slice(end, callEnd + 1);
    const caseInsensitive = /\bre\.(?:I|IGNORECASE)\b|\bregex\.I\b|["'`][a-z]*i[a-z]*["'`]|case_insensitive\(\s*true/.test(rest);
    candidates.push({
      start: pieces[0].start, end: pieces[pieces.length - 1].end,
      source: pieces.map((token) => token.value).join(''), caseInsensitive, kind: 'constructor',
    });
  }

  // Remaining strings that read as regex source (fragments, raw strings, String.raw).
  // Test files quote citations and Word field codes in strings; only their
  // regex literals and constructor arguments are regex source.
  const rest = TEST_PATH.test(context.path) ? [] : strings.filter((token) => !consumed.has(token));
  let group = [];
  const flush = () => {
    if (!group.length) return;
    const source = group.map((token) => token.value).join('');
    // Multi-line templates and docstrings are code or prose, not regex fragments.
    const lines = source.split('\n').length;
    if (looksLikeRegex(source) && (lines <= 3 || group.every((token) => token.raw && !token.triple && token.type !== 'template'))) {
      group.forEach((token) => consumed.add(token));
      candidates.push({ start: group[0].start, end: group[group.length - 1].end, source, caseInsensitive: false, kind: 'string' });
    }
    group = [];
  };
  for (const token of rest) {
    const previous = group[group.length - 1];
    if (previous) {
      const between = code.slice(previous.end, token.start);
      const joins = language === 'py' ? /^\s*$/.test(between) : /^\s*\+?\s*$/.test(between) && (language !== 'js' || between.includes('+'));
      if (!joins || previous.nested !== token.nested) flush();
    }
    group.push(token);
  }
  flush();
  return candidates;
}

function detectRegexes(context, candidates) {
  const { data } = context;
  for (const candidate of candidates) {
    const categories = classifyRegex(data, candidate.source, { caseInsensitive: candidate.caseInsensitive });
    if (!categories.size) continue;
    const corpusId = corpusMatch(data, candidate.source);
    const start = context.line(candidate.start);
    const end = context.line(Math.max(candidate.start, candidate.end - 1));
    for (const [category, evidence] of categories) {
      const suffix = corpusId ? ` (verbatim copy of corpus entry ${corpusId})` : '';
      add(context, `regex/${category}`, start, end, `regex encodes ${category} citation grammar${suffix}`, [...evidence]);
    }
  }
}

function corpusMatch(data, source) {
  if (!data.corpusPatterns?.size || source.length < 30) return null;
  const exact = data.corpusPatterns.get(source);
  if (exact) return exact;
  for (const [pattern, id] of data.corpusPatterns) if (pattern.length >= 60 && source.includes(pattern)) return id;
  return null;
}

// ---------------------------------------------------------------- functions

function detectFunctions(context) {
  const { code, language, options } = context;
  const engineModules = options.engineModules ?? DEFAULT_ENGINE_MODULES;
  const definitions = [];
  for (const pattern of FUNCTION_DEFS[language]) {
    pattern.lastIndex = 0;
    let match;
    while ((match = pattern.exec(code))) {
      const name = match[1];
      if (NOT_FUNCTIONS.has(name)) continue;
      const offset = match.index + match[0].indexOf(name);
      definitions.push({ name, line: context.line(offset) });
    }
  }
  definitions.sort((a, b) => a.line - b.line);
  const engineNames = engineImports(context.codeWithStrings, engineModules);
  const reported = [];
  for (const definition of definitions) {
    if (!isCitationFunctionName(definition.name)) continue;
    if (reported.some((other) => other.line === definition.line)) continue;
    const body = context.stringLines.slice(definition.line - 1, definition.line + 24).join('\n');
    // Thin wrappers over the engine are fine.
    if (engineModules.some((module) => body.includes(module))) continue;
    if (engineNames.some((name) => new RegExp(`(?<![\\w$.])${name.replace(/\$/g, '\\$')}\\s*[.(]`).test(body.split('\n').slice(1).join('\n') || body))) continue;
    reported.push(definition);
    add(context, 'code/citation-function', definition.line, definition.line, `defines ${definition.name}() outside the engine`, [definition.name]);
  }
  context.citationFunctionLines = reported.map((definition) => definition.line);
  return definitions;
}

/** Local names bound to the engine by import/require/from-import statements. */
function engineImports(code, engineModules) {
  const names = [];
  const quoted = engineModules.map((module) => module.replace(/[.*+?^${}()|[\]\\/]/g, '\\$&')).join('|');
  const patterns = [
    new RegExp(`import\\s+([\\s\\S]{1,400}?)\\s+from\\s+["'](?:${quoted})(?:/[^"']*)?["']`, 'g'),
    new RegExp(`(?:const|let|var)\\s+([^=]{1,400}?)=\\s*(?:await\\s+import|require)\\(\\s*["'](?:${quoted})(?:/[^"']*)?["']`, 'g'),
    new RegExp(`from\\s+(?:${quoted})(?:\\.\\w+)*\\s+import\\s+\\(?([\\w\\s,]+)`, 'g'),
    new RegExp(`import\\s+(?:${quoted})(?:\\s+as\\s+(\\w+))?`, 'g'),
    new RegExp(`use\\s+(?:${quoted.replace(/-/g, '_')})::\\{?([\\w\\s,:]+)`, 'g'),
  ];
  for (const pattern of patterns) {
    for (const match of code.matchAll(pattern)) {
      const binding = match[1] ?? '';
      for (const part of binding.replace(/[{}*]/g, ' ').split(',')) {
        const name = part.trim().split(/\s+as\s+|\s*:\s*/).pop()?.trim();
        if (name && /^[A-Za-z_$][\w$]*$/.test(name)) names.push(name);
      }
    }
  }
  return names;
}

function detectLookupKeys(context, definitions) {
  const lines = context.stringLines;
  for (let index = 0; index < lines.length; index++) {
    const window = lines.slice(index, index + 3).join('\n');
    if (!STRIP_NON_ALNUM.test(lines[index]) || !FOLD_CASE.test(window)) continue;
    const line = index + 1;
    const enclosing = [...definitions].reverse().find((definition) => definition.line <= line && line - definition.line <= 12);
    const keyFunction = enclosing && KEY_FUNCTION.test(enclosing.name.replace(/_/g, '')) && CITATION_WORD.test(lines.slice(Math.max(0, line - 40), line + 40).join('\n'));
    const citationSubject = CITATION_WORD.test(lines[index]);
    if (!keyFunction && !citationSubject) continue;
    if (enclosing && context.citationFunctionLines.includes(enclosing.line)) continue;
    add(context, 'code/lookup-key', line, line,
      `folds ${keyFunction ? `in ${enclosing.name}()` : 'a citation string'} to a lookup key`, [lines[index].trim().slice(0, 80)]);
  }
}

const PY_MEMBERSHIP = new RegExp(String.raw`\bin\s*[(\[{][^)\]}\n]*` + SF_LIT, 'i');

function detectShortFormLogic(context) {
  const patterns = context.language === 'py' ? [...SHORT_FORM_LOGIC, PY_MEMBERSHIP] : SHORT_FORM_LOGIC;
  context.stringLines.forEach((value, index) => {
    const hit = patterns.find((pattern) => pattern.test(value));
    if (hit) add(context, 'code/short-form-logic', index + 1, index + 1, 'branches on or renders ibid/supra', [value.match(hit)[0]]);
  });
}

function detectPinpointFormatting(context) {
  const byLine = new Map();
  for (const token of context.tokens) {
    if (token.type !== 'string' && token.type !== 'template') continue;
    const line = context.line(token.start);
    if (!byLine.has(line)) byLine.set(line, []);
    byLine.get(line).push(token);
    const templated = token.type === 'template' || token.prefix?.includes('f');
    const pattern = token.type === 'template'
      ? /(?:^|[\s(])(?:at (?:paras?|pp?)|paras?|pp)\.? \$\{/
      : /(?:^|[\s(])(?:at (?:paras?|pp?)|paras?|pp)\.? \{[A-Za-z_]/;
    if (templated && pattern.test(token.value)) {
      add(context, 'code/pinpoint-format', line, line, 'renders a pinpoint label', [token.value.slice(0, 60)]);
    }
  }
  for (const [line, tokens] of byLine) {
    const labels = new Set(tokens.map((token) => token.value).filter((value) => PINPOINT_LABEL_LITERAL.test(value)).map((value) => value.trim()));
    const distinctive = [...labels].some((value) => DISTINCT_PINPOINT_LABEL.test(value));
    if (labels.size >= 2 && distinctive) add(context, 'code/pinpoint-format', line, line, 'chooses pinpoint labels', [...labels]);
  }
}

// ---------------------------------------------------------------- tables

function detectTables(context, consumed) {
  const { tokens, data, code } = context;
  const hits = []; // { line, kind, code }
  const routes = [];
  for (const token of tokens) {
    if ((token.type !== 'string' && token.type !== 'template') || consumed.has(token)) continue;
    const value = token.value.trim();
    if (CANLII_ROUTE.test(value)) routes.push({ line: context.line(token.start), code: value });
    if (!value || value.length > 4000) continue;
    const parts = value.split(/[\s,|;]+/).filter(Boolean);
    if (parts.length === 1) {
      const hit = parts[0].length <= 16 && codeKind(data, parts[0], { caseInsensitive: parts[0].length >= 3 });
      if (hit) hits.push({ line: context.line(token.start), ...hit });
      continue;
    }
    if (parts.length < 3 || parts.some((part) => part.length > 20)) continue;
    const rawText = context.text.slice(token.start, token.end);
    let known = 0;
    const local = [];
    for (const part of parts) {
      const hit = codeKind(data, part);
      if (hit && hit.strength === 'strong') {
        known++;
        const at = rawText.indexOf(part);
        local.push({ line: context.line(token.start + Math.max(0, at)), ...hit });
      }
    }
    if (known >= Math.max(3, parts.length * 0.3)) hits.push(...local);
  }
  if (context.language === 'js') {
    const key = /(?:^|[{,\s])([A-Z][A-Za-z0-9]{1,15})\s*:(?!:)/gm;
    let match;
    while ((match = key.exec(code))) {
      const hit = codeKind(data, match[1]);
      if (hit) hits.push({ line: context.line(match.index + match[0].indexOf(match[1])), ...hit });
    }
  }
  const threshold = context.options.tableThreshold ?? 5;
  for (const cluster of clusters(hits, TABLE_GAP)) {
    // Weak codes (FC, CF, OR) only count next to at least three strong ones.
    const strong = new Set(cluster.filter((hit) => hit.strength === 'strong').map((hit) => hit.code));
    const distinct = new Map(cluster.filter((hit) => strong.size >= 3 || hit.strength === 'strong').map((hit) => [hit.code, hit.kind]));
    if (strong.size < 3 || distinct.size < threshold) continue;
    const counts = { court: 0, reporter: 0, series: 0 };
    for (const kind of distinct.values()) counts[kind]++;
    const kind = Object.entries(counts).sort((a, b) => b[1] - a[1])[0][0];
    const rule = kind === 'court' ? 'data/court-table' : kind === 'reporter' ? 'data/reporter-table' : 'data/series-table';
    add(context, rule, cluster[0].line, cluster[cluster.length - 1].line,
      `literal table of ${distinct.size} ${kind} codes`, [...distinct.keys()].slice(0, 8));
  }
  for (const cluster of clusters(routes, TABLE_GAP)) {
    if (cluster.length < 2) continue;
    add(context, 'data/canlii-route', cluster[0].line, cluster[cluster.length - 1].line,
      `${cluster.length} literal CanLII routes`, cluster.map((route) => route.code));
  }
}

function clusters(hits, gap) {
  const sorted = [...hits].sort((a, b) => a.line - b.line);
  const out = [];
  let current = [];
  for (const hit of sorted) {
    if (current.length && hit.line - current[current.length - 1].line > gap) { out.push(current); current = []; }
    current.push(hit);
  }
  if (current.length) out.push(current);
  return out;
}

// ---------------------------------------------------------------- CanLII URLs

function detectCanlii(context, candidates) {
  const { tokens, code } = context;
  for (const token of tokens) {
    if ((token.type !== 'string' && token.type !== 'template') || !/canlii\.(?:org|ca)/i.test(token.value)) continue;
    const before = code.slice(Math.max(0, token.start - 3), token.start);
    const after = code.slice(token.end, token.end + 12);
    const interpolated = (token.type === 'template' && token.value.includes('${'))
      || (token.prefix?.includes('f') && /\{[^}]+\}/.test(token.value))
      || /\{\}|%s|\{\d*\}/.test(token.value);
    const concatenated = /\+\s*$/.test(before) || /^\s*\+/.test(after) || /^\s*\.format\(|^\s*%/.test(after);
    const pathy = /canlii\.(?:org|ca)\/(?:en|fr|t|\$\{|\{)/i.test(token.value) || /\/doc\/|\/laws\//.test(token.value);
    if ((interpolated || concatenated) && pathy) {
      add(context, 'code/canlii-url', context.line(token.start), context.line(token.start), 'builds a CanLII URL', [token.value.slice(0, 80)]);
    }
  }
  for (const candidate of candidates) {
    if (/canlii\\?\.(?:org|ca)[^|)]{0,60}?(?:doc|laws|\\\/\(|\/\(|\\\/(?:en|fr)|\/(?:en|fr))/i.test(candidate.source)) {
      add(context, 'code/canlii-url', context.line(candidate.start), context.line(candidate.start), 'parses CanLII URLs', [candidate.source.slice(0, 80)]);
    }
  }
}

// ---------------------------------------------------------------- vendored

function detectVendoredCode(context, candidates) {
  const { tokens, data } = context;
  const marker = tokens.find((token) => (token.type === 'string' || token.type === 'template') && CORPUS_MARKERS.test(token.value));
  if (marker) add(context, 'vendored/corpus', context.line(marker.start), context.line(marker.start), 'loads the grammar corpus directly', [marker.value.slice(0, 60)]);

  const corpusIds = new Set();
  for (const candidate of candidates) { const id = corpusMatch(data, candidate.source); if (id) corpusIds.add(id); }
  if (corpusIds.size >= 3) {
    add(context, 'vendored/corpus', 1, 1, `embeds ${corpusIds.size} grammar-corpus patterns verbatim`, [...corpusIds]);
  }

  if (data.engineLines?.size) {
    let eligible = 0, copied = 0, first = 0;
    context.lines.forEach((line, index) => {
      const normalized = normalizeSourceLine(line);
      if (!normalized) return;
      eligible++;
      if (data.engineLines.has(normalized)) { copied++; if (!first) first = index + 1; }
    });
    if (copied >= 20 && copied >= eligible * 0.25) {
      add(context, 'vendored/engine', first, first, `${copied}/${eligible} source lines are copied from legal-citations crates`);
    }
  }
}

// ---------------------------------------------------------------- JSON data

const REGISTRY_DISTINCT = new Set(['neutral', 'canlii', 'editions', 'variations', 'cite_type', 'citation_string', 'mlz_jurisdiction', 'court_url', 'year_volume']);
const REGISTRY_FIELDS = new Set([...REGISTRY_DISTINCT, 'abbreviation', 'aliases', 'level', 'jurisdiction', 'name', 'examples', 'regex', 'start', 'end', 'kind']);
const ABBREVIATION = /^(?:(?:[A-Z][A-Za-z'’.-]*|&|of|de|du|des|la|le|et|and|on|in|the|\d+\w*|\([^)]*\)|[:-])\s?){1,10}$/;
const ABBREVIATION_TAIL = /\b(?:LJ|LR|J|Rev|Rep|R|Q|Dec|L|Rts|Stud|Ct|Cas|Reports?|Law)\b/;

export function scanJson(path, text, data, options = {}) {
  const findings = [];
  const push = (rule, needle, message, evidence = []) => {
    const at = needle ? text.indexOf(needle) : 0;
    const line = at > 0 ? text.slice(0, at).split('\n').length : 1;
    findings.push({ file: path, rule, line, endLine: line, message, evidence, snippet: '' });
  };
  const base = path.split('/').pop().toLowerCase();
  const head = text.slice(0, 4096);
  if (base === 'grammar-corpus.json' || /"format"\s*:\s*"legal-grammar-(?:corpus|manifest):v\d/.test(head)) {
    push('vendored/corpus', null, `copy of the legal-citations grammar ${/manifest/.test(head) ? 'manifest' : 'corpus'}`);
    return findings;
  }
  if (text.length > 5_000_000) return findings;
  let value;
  try { value = JSON.parse(text); } catch { return findings; }

  let registryObjects = 0;
  const threshold = Math.max(8, (options.tableThreshold ?? 5) + 3);
  const visit = (node, depth) => {
    if (depth > 8 || node === null || typeof node !== 'object') return;
    if (Array.isArray(node)) {
      const strings = node.filter((item) => typeof item === 'string');
      if (strings.length >= threshold) {
        const known = new Map();
        for (const item of strings) { const hit = codeKind(data, item); if (hit?.strength === 'strong') known.set(hit.code, hit.kind); }
        if (strings.length >= 50) {
          const abbreviations = strings.filter((item) => item.length <= 80 && ABBREVIATION.test(item) && ABBREVIATION_TAIL.test(item));
          if (abbreviations.length >= strings.length * 0.4) {
            push('data/reporter-table', JSON.stringify(strings[0]), `JSON list of ${abbreviations.length} reporter/journal abbreviations`, abbreviations.slice(0, 5));
          }
        }
      }
      for (const item of node.slice(0, 20000)) visit(item, depth + 1);
      return;
    }
    const keys = Object.keys(node);
    if (keys.some((key) => REGISTRY_DISTINCT.has(key)) && keys.filter((key) => REGISTRY_FIELDS.has(key)).length >= 2) registryObjects++;
    const known = new Map();
    for (const key of keys) { const hit = codeKind(data, key); if (hit?.strength === 'strong') known.set(hit.code, hit.kind); }
    const lookupValues = keys.filter((key) => codeKind(data, key)).every((key) => typeof node[key] === 'string' || (node[key] && typeof node[key] === 'object'));
    if (known.size >= threshold && known.size >= keys.length * 0.5 && lookupValues) {
      push(tableRule(known), JSON.stringify(keys.find((key) => codeKind(data, key))), `JSON object keyed by ${known.size} codes`, [...known.keys()].slice(0, 8));
    }
    for (const key of keys) visit(node[key], depth + 1);
  };
  visit(value, 0);
  if (registryObjects >= 5) push('vendored/registry', null, `${registryObjects} registry-shaped records (courts/reporters/series)`);
  return findings;
}

function tableRule(known) {
  const counts = { court: 0, reporter: 0, series: 0 };
  for (const kind of known.values()) counts[kind]++;
  const kind = Object.entries(counts).sort((a, b) => b[1] - a[1])[0][0];
  return kind === 'court' ? 'data/court-table' : kind === 'reporter' ? 'data/reporter-table' : 'data/series-table';
}
