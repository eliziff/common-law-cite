import assert from 'node:assert/strict';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { scanFile } from '../lib/detectors.mjs';
import { RULES } from '../lib/rules.mjs';
import { DEFAULT_ENGINE_ROOT, engineData } from '../lib/scan.mjs';
import { classifyRegex, isCitationFunctionName } from '../lib/signatures.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = join(here, 'fixtures');
const data = engineData();

const rulesOf = (path, text) => new Set(scanFile(path, text, data, {}).findings.map((finding) => finding.rule));
const JSON_EXPECT = { 'corpus-copy.json': ['vendored/corpus'], 'registry-copy.json': ['vendored/registry'] };

for (const file of readdirSync(join(fixtures, 'positive'))) {
  test(`positive fixture ${file} reports its expected rules`, () => {
    const text = readFileSync(join(fixtures, 'positive', file), 'utf8');
    const expected = JSON_EXPECT[file] ?? text.match(/expect: (.*)/)[1].split(/,\s*/);
    const got = rulesOf(file, text);
    for (const rule of expected) {
      assert.ok(RULES[rule], `unknown rule in fixture: ${rule}`);
      assert.ok(got.has(rule), `${file}: expected ${rule}, got ${[...got].join(', ')}`);
    }
  });
}

for (const file of readdirSync(join(fixtures, 'negative'))) {
  test(`negative fixture ${file} reports nothing`, () => {
    const text = readFileSync(join(fixtures, 'negative', file), 'utf8');
    const { findings } = scanFile(file, text, data, {});
    assert.deepEqual(findings.map((finding) => `${finding.line} ${finding.rule} ${finding.evidence.join(';')}`), []);
  });
}

test('regex classification: true positives by category', () => {
  const cases = [
    [String.raw`\b(?:17|18|19|20)\d{2}\s+[A-Z][A-Z0-9-]{1,15}\s+\d+\b`, 'neutral'],
    [String.raw`\b\d{4}\s+CanLII\s+\d+`, 'neutral'],
    [String.raw`\[(?:18|19|20)\d{2}\]\s+\d+\s+S\.?C\.?R\.?\s+\d+`, 'reporter'],
    [String.raw`\d+\s+F\.\s?\dd\s+\d+`, 'reporter'],
    [String.raw`\b(?:R\.?S\.?O\.?|S\.?O\.?)\s+\d{4},\s*c\s+\d+`, 'legislation'],
    [String.raw`\bO\. ?Reg\.?\s+\d+/\d+`, 'legislation'],
    [String.raw`\b\d+\s+U\.?S\.?C\.?\s+§+\s*\d+`, 'legislation'],
    [String.raw`\b(?:SOR|DORS)[/-]\d{2,4}-\d+`, 'legislation'],
    [String.raw`\bsupra(?:\s+note\s+(\d+))?`, 'short-form'],
    [String.raw`^Id\.(?:\s+at\s+\d+)?`, 'short-form'],
    [String.raw`\bat\s+paras?\.?\s*(\d+)`, 'pinpoint'],
    [String.raw`\baff['’]?d\b|\brev['’]?d\b`, 'history'],
    [String.raw`leave\s+to\s+appeal\s+(?:to\s+SCC\s+)?refused`, 'history'],
    [String.raw`(?<style>[A-Z][\w.' ]+)\s+v\.?\s+[A-Z]\w+`, 'case-name'],
    [String.raw`\b(?:SCC|ONCA|BCCA)\b`, 'court'],
    [String.raw`^\s*(?:see\s+also|but\s+see|cf\.?|contra)\s+`, 'signal'],
    [String.raw`(?<volume>\d+)\s+(?<journal>[A-Z][\w .&]+)\s+(?<page>\d+)`, 'secondary'],
  ];
  for (const [source, category] of cases) {
    const found = classifyRegex(data, source);
    assert.ok(found.has(category), `${source} should be ${category}; got ${[...found.keys()].join(', ')}`);
  }
});

test('regex classification: true negatives', () => {
  const negatives = [
    String.raw`^(\d{4})-(\d{2})-(\d{2})$`,
    String.raw`\s+`,
    String.raw`^https?:\/\/[^/]+\/en\/on\/onca\/doc\/(\d{4})`,
    String.raw`__\d+_(?P<journal>.+?)_page-(?P<page>\d+)\.png$`,
    '2019 SCC 65',
    String.raw`^(?:v|vs|s|ss|no|nos|para|paras|art|arts|ibid|supra|infra)$`,
    String.raw`\b(?:JAN|FEB|MAR|APR|MAY|JUN)\s+\d{1,2}`,
    String.raw`^\s*(from|to|cc|cci|bcc)\s*:`,
    String.raw`^[A-Z][a-z]+(?:\s+[A-Z][a-z]+)*$`,
  ];
  for (const source of negatives) {
    const found = classifyRegex(data, source);
    assert.equal(found.size, 0, `${source} should be clean; got ${[...found].map(([key, value]) => `${key}:${[...value]}`).join(' ')}`);
  }
});

test('provision labels get their own allowlistable rule', () => {
  const found = classifyRegex(data, String.raw`^(?:section|s\.|art(?:icle)?|rule)\s+(\d+(?:\.\d+)*)`);
  assert.deepEqual([...found.keys()], ['provision']);
});

test('function names: definitions are flagged, calls and wrappers are not', () => {
  assert.ok(isCitationFunctionName('citationLookupKey'));
  assert.ok(isCitationFunctionName('citation_key'));
  assert.ok(isCitationFunctionName('resolveIbid'));
  assert.ok(isCitationFunctionName('buildCanliiCaseUrl'));
  assert.ok(isCitationFunctionName('formatPinpoint'));
  assert.ok(!isCitationFunctionName('cacheKey'));
  assert.ok(!isCitationFunctionName('mcgill_tenth_inventory_preserves_journal_punctuation'));
  const calls = 'const key = native.citationLookupKey(value);\nexport const k = citationKey(x);\n';
  assert.deepEqual([...rulesOf('a.ts', calls)], []);
  const wrapper = 'import { key } from "legal-citations";\nexport function citationKey(value) {\n  return key(value);\n}\n';
  assert.deepEqual([...rulesOf('a.mjs', wrapper)], []);
});

test('lookup-key normalizers', () => {
  assert.ok(rulesOf('a.ts', 'const reporter = args.reporter.toLowerCase().replace(/[^a-z0-9]/gu, "");').has('code/lookup-key'));
  assert.ok(rulesOf('a.mjs', "export const key = text => String(text).normalize('NFKD').toLowerCase().replace(/[^a-z0-9]/g, '');\nconst citation = 1;").has('code/lookup-key'));
  assert.ok(!rulesOf('a.ts', 'const personKey = (name) => name.toLowerCase().replace(/[^a-z0-9]/g, "");\nconst citation = 1;').has('code/lookup-key'));
});

test('short-form logic and pinpoint formatting', () => {
  assert.ok(rulesOf('a.ts', 'if (match.kind === "ibid") return last;').has('code/short-form-logic'));
  assert.ok(rulesOf('a.py', 'if kind in ("ibid", "supra"):\n    pass\n').has('code/short-form-logic'));
  assert.ok(rulesOf('a.ts', 'const s = `${name}, supra note ${n}`;').has('code/short-form-logic'));
  assert.ok(!rulesOf('a.ts', 'type Form = "full" | "supra" | "ibid";').has('code/short-form-logic'));
  assert.ok(rulesOf('a.mjs', "const pin = (k, v) => ({ paragraph: 'at para ', section: 's ', page: 'at p ' }[k] || '') + v;").has('code/pinpoint-format'));
  assert.ok(rulesOf('a.py', 'label = f"at para {number}"\n').has('code/pinpoint-format'));
  assert.ok(!rulesOf('a.tsx', 'const label = `Para ${value}`;').has('code/pinpoint-format'));
});

test('CanLII: builders and route tables, not plain links', () => {
  assert.ok(rulesOf('a.ts', 'const url = `https://www.canlii.org/${lang}/${route}/doc/${year}/${slug}/${slug}.html`;').has('code/canlii-url'));
  assert.ok(rulesOf('a.py', 'url = "https://www.canlii.org/en/" + route + "/doc/"\n').has('code/canlii-url'));
  assert.ok(!rulesOf('a.ts', 'const HELP = "https://www.canlii.org/en/";').has('code/canlii-url'));
  assert.ok(rulesOf('a.js', "const r = { FC: 'ca/fct', HRTO: 'on/onhrt' };").has('data/canlii-route'));
});

test('tables: court codes in keys, lists and packed strings; weak codes need company', () => {
  assert.ok(rulesOf('a.py', 'COURTS = ("SCC", "ONCA", "BCCA", "BCSC", "FCA", "NSCA")\n').has('data/court-table'));
  assert.ok(rulesOf('a.ts', 'const ab = `ABCA ABKB ABPC ABQB ABCJ`;').has('data/court-table'));
  assert.ok(!rulesOf('a.ts', 'const c = ["SCC", "FC", "OR", "AC"];').has('data/court-table'));
  assert.ok(rulesOf('a.js', "const SERIES = new Set(['rsc', 'rso', 'rsbc', 'rsa', 'rsns', 'ccsm']);").has('data/series-table'));
});

test('JSON: counts per code are not lookup tables; code-keyed records are', () => {
  const counts = JSON.stringify({ dataset_counts: { SCC: 1, ONCA: 2, BCCA: 3, BCSC: 4, FCA: 5, NSCA: 6, NSSC: 7, ABCA: 8, ONSC: 9 } });
  assert.equal(scanFile('x.json', counts, data, {}).findings.length, 0);
  const levels = JSON.stringify(Object.fromEntries(['SCC', 'ONCA', 'BCCA', 'BCSC', 'FCA', 'NSCA', 'NSSC', 'ABCA', 'ONSC'].map((code) => [code, { level: 3 }])));
  assert.deepEqual(scanFile('x.json', levels, data, {}).findings.map((finding) => finding.rule), ['data/court-table']);
});

test('inline suppression comments are parsed and need a reason', () => {
  const { suppressions, findings } = scanFile('a.ts', '// citation-boundary-allow: regex/provision -- document numbering\nconst r = /^(?:section|art)\\s+\\d+/;\n// citation-boundary-allow: regex/court\n', data, {});
  assert.equal(suppressions.length, 2);
  assert.deepEqual(suppressions[0].rules, ['regex/provision']);
  assert.equal(suppressions[0].reason, 'document numbering');
  assert.ok(findings.some((finding) => finding.rule === 'suppression/invalid' && finding.line === 3));
});

test('engine data: corpus patterns and engine sources load from the checkout', { skip: !existsSync(join(DEFAULT_ENGINE_ROOT, 'crates')) }, () => {
  assert.ok(data.corpusPatterns.size > 20);
  assert.ok(data.engineLines.size > 100);
});

test('recall over the engine grammar corpus (citations, pinpoints, references)', { skip: !data.corpusLoaded }, () => {
  const corpus = JSON.parse(readFileSync(join(DEFAULT_ENGINE_ROOT, 'crates', 'legal-grammar', 'data', 'grammar-corpus.json'), 'utf8'));
  let total = 0, hit = 0;
  for (const name of ['citations', 'pinpoints', 'references']) {
    const table = corpus.tables[name];
    if (!table) continue;
    const defs = table.defs ?? {};
    const expand = (source) => { for (let i = 0; i < 10; i++) source = source.replace(/\{\{(\w+)\}\}/g, (_, def) => defs[def] ?? ''); return source; };
    for (const entry of table.entries ?? []) {
      if (typeof entry.pattern !== 'string') continue;
      total++;
      if (classifyRegex(data, expand(entry.pattern), { caseInsensitive: (entry.flags ?? '').includes('i') }).size) hit++;
    }
  }
  // Entries such as URL, sentence-boundary and conjunction fragments are
  // generic by design; everything citation-specific must be caught.
  assert.ok(hit / total >= 0.7, `corpus recall ${hit}/${total}`);
});
