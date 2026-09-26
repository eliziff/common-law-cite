import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { applyAllowlist, globToRegExp, loadConfig, ruleMatches, staleEntries, writeBaseline } from '../lib/config.mjs';
import { describe } from '../lib/rules.mjs';

const temp = () => mkdtempSync(join(tmpdir(), 'citation-boundary-config-'));
const write = (dir, value) => { const path = join(dir, '.citation-boundary.json'); writeFileSync(path, JSON.stringify(value)); return path; };
const finding = (file, rule, line = 1) => ({ file, rule, line, endLine: line, message: 'm', evidence: [], snippet: '' });

test('globs: **, *, {a,b} and basename patterns', () => {
  assert.ok(globToRegExp('src/**/*.ts').test('src/a/b/c.ts'));
  assert.ok(globToRegExp('src/**/*.ts').test('src/c.ts'));
  assert.ok(!globToRegExp('src/*.ts').test('src/a/c.ts'));
  assert.ok(globToRegExp('**/*.{py,rs}').test('a/b.rs'));
  assert.ok(globToRegExp('canliiUrls.ts').test('backend/src/lib/canliiUrls.ts'));
  assert.ok(!globToRegExp('backend/src/lib/canliiUrls.ts').test('x/backend/src/lib/canliiUrls.ts'));
  assert.ok(globToRegExp('benchmarks/').test('benchmarks/x/y.json'));
});

test('rule patterns: exact, family wildcard, everything', () => {
  assert.ok(ruleMatches('regex/provision', 'regex/provision'));
  assert.ok(ruleMatches('regex/*', 'regex/neutral'));
  assert.ok(!ruleMatches('regex/*', 'data/court-table'));
  assert.ok(ruleMatches('*', 'code/lookup-key'));
});

test('config validation rejects entries without a reason, unknown rules and bad engine pins', () => {
  const dir = temp();
  const { errors } = loadConfig(write(dir, {
    engine: 'legal-citations',
    allow: [{ path: 'a.ts', rule: 'regex/nope', reason: 'because' }, { path: 'b.ts', rule: 'regex/neutral' }, { path: 'c.ts', rule: 'regex/*', reason: 'document numbering parser', until: 'soon' }],
  }));
  assert.equal(errors.length, 5, errors.join('\n'));
  assert.ok(errors.some((error) => error.includes('"engine"')));
  assert.ok(errors.some((error) => error.includes('unknown rule')));
  assert.ok(errors.some((error) => error.includes('"reason" is required')));
  assert.ok(errors.some((error) => error.includes('"until"')));
});

test('a missing config means defaults, not an error', () => {
  const { exists, errors, config } = loadConfig(join(temp(), '.citation-boundary.json'));
  assert.equal(exists, false);
  assert.deepEqual(errors, []);
  assert.deepEqual(config.allow, []);
});

test('allow entries, inline suppressions, stale and expired entries', () => {
  const dir = temp();
  const { config, errors } = loadConfig(write(dir, {
    engine: 'eliziff/legal-citations@v0.1.0',
    allow: [
      { path: 'src/structure/**', rule: 'regex/provision', reason: 'document-structure numbering, not citations' },
      { path: 'old/**', rule: 'regex/*', reason: 'nothing matches this any more' },
      { path: 'legacy.py', rule: '*', reason: 'deleted next sprint', until: '2000-01-01' },
    ],
  }));
  assert.deepEqual(errors, []);
  const findings = [
    finding('src/structure/numbering.ts', 'regex/provision'),
    finding('src/structure/numbering.ts', 'regex/neutral'),
    finding('src/inline.ts', 'regex/court', 5),
    finding('legacy.py', 'regex/neutral'),
  ];
  const suppressions = [
    { file: 'src/inline.ts', line: 4, rules: ['regex/court'], reason: 'jurisdiction hint', used: false },
    { file: 'src/other.ts', line: 1, rules: ['regex/court'], reason: 'unused', used: false },
  ];
  const { violations, allowed, expired } = applyAllowlist(findings, suppressions, config, new Date('2026-09-26'));
  assert.deepEqual(violations.map((violation) => `${violation.file} ${violation.rule}`), ['src/structure/numbering.ts regex/neutral', 'legacy.py regex/neutral']);
  assert.equal(allowed.length, 2);
  assert.deepEqual(expired.map((entry) => entry.path), ['legacy.py']);
  const stale = staleEntries(config, suppressions);
  assert.deepEqual(stale.map((entry) => entry.kind === 'allow' ? entry.path : `${entry.file}:${entry.line}`), ['old/**', 'src/other.ts:1']);
});

test('baseline keeps used entries, drops stale ones and adds one entry per file and rule', () => {
  const dir = temp();
  const path = write(dir, {
    engine: 'eliziff/legal-citations@v0.1.0',
    allow: [
      { path: 'keep.ts', rule: 'regex/neutral', reason: 'still needed for now' },
      { path: 'gone.ts', rule: 'regex/neutral', reason: 'file was migrated' },
    ],
  });
  const { config } = loadConfig(path);
  applyAllowlist([finding('keep.ts', 'regex/neutral')], [], config);
  const summary = writeBaseline(path, [finding('new.ts', 'regex/court', 1), finding('new.ts', 'regex/court', 9), finding('new.ts', 'code/lookup-key')], config, describe);
  assert.deepEqual(summary, { added: 2, kept: 1, removed: 1 });
  const written = JSON.parse(readFileSync(path, 'utf8'));
  assert.equal(written.engine, 'eliziff/legal-citations@v0.1.0');
  assert.deepEqual(written.allow.map((entry) => `${entry.path} ${entry.rule}`), ['keep.ts regex/neutral', 'new.ts regex/court', 'new.ts code/lookup-key']);
  assert.ok(written.allow[1].reason.startsWith('baseline '));
});
