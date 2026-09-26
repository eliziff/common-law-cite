import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const CHECK = join(dirname(fileURLToPath(import.meta.url)), '..', 'check.mjs');

function repo(files, { git = true } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'citation-boundary-cli-'));
  for (const [path, content] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true });
    writeFileSync(join(dir, path), typeof content === 'string' ? content : JSON.stringify(content, null, 2));
  }
  if (git) {
    execFileSync('git', ['init', '-q'], { cwd: dir });
  }
  return dir;
}

const run = (dir, ...args) => spawnSync(process.execPath, [CHECK, '--root', dir, ...args], { encoding: 'utf8' });

const OFFENDER = 'export const NEUTRAL = /\\b(?:19|20)\\d{2}\\s+[A-Z]{2,8}\\s+\\d+\\b/g;\n';
const CLEAN = 'export const add = (a, b) => a + b;\n';
const ENGINE = 'eliziff/legal-citations@v0.1.0';

test('clean repository exits 0', () => {
  const dir = repo({ '.citation-boundary.json': { engine: ENGINE, allow: [] }, 'src/a.mjs': CLEAN });
  const result = run(dir);
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /0 violation/);
});

test('violations exit 1 with file:line text output naming the engine API', () => {
  const dir = repo({ '.citation-boundary.json': { engine: ENGINE, allow: [] }, 'src/a.mjs': CLEAN + OFFENDER });
  const result = run(dir);
  assert.equal(result.status, 1);
  assert.match(result.stdout, /^src\/a\.mjs:2 {2}regex\/neutral/m);
  assert.match(result.stdout, /use: extract \{ text/);
});

test('json and github formats', () => {
  const dir = repo({ '.citation-boundary.json': { engine: ENGINE, allow: [] }, 'src/a.mjs': OFFENDER });
  const json = JSON.parse(run(dir, '--format', 'json').stdout);
  assert.equal(json.engine, ENGINE);
  assert.equal(json.violations[0].file, 'src/a.mjs');
  assert.equal(json.violations[0].rule, 'regex/neutral');
  assert.ok(json.violations[0].use);
  const github = run(dir, '--format=github').stdout;
  assert.match(github, /^::error file=src\/a\.mjs,line=1,title=citation-boundary regex\/neutral::/m);
});

test('.gitignore is respected in git repositories; untracked files are scanned', () => {
  const dir = repo({ '.gitignore': 'generated/\n', 'generated/a.mjs': OFFENDER, 'src/new.mjs': CLEAN });
  assert.equal(run(dir).status, 0);
  writeFileSync(join(dir, 'src', 'new.mjs'), OFFENDER);
  assert.equal(run(dir).status, 1);
});

test('non-git directories are walked', () => {
  const dir = repo({ 'lib/a.py': 'import re\nSUPRA = re.compile(r"\\bsupra\\s+note\\s+(\\d+)")\n' }, { git: false });
  const result = run(dir, '--format', 'json');
  assert.equal(result.status, 1);
  assert.deepEqual(JSON.parse(result.stdout).violations.map((violation) => violation.rule), ['regex/short-form']);
});

test('allow entries suppress, stale entries fail unless --allow-stale, path arguments narrow the scan', () => {
  const dir = repo({
    '.citation-boundary.json': { engine: ENGINE, allow: [
      { path: 'src/legacy/**', rule: 'regex/*', reason: 'migrating to legal-citations in #12' },
      { path: 'src/removed.mjs', rule: 'regex/neutral', reason: 'file no longer exists' },
    ] },
    'src/legacy/a.mjs': OFFENDER,
    'src/b.mjs': CLEAN,
  });
  const result = run(dir);
  assert.equal(result.status, 1);
  assert.match(result.stdout, /allow\/stale {2}"src\/removed\.mjs"/);
  assert.equal(run(dir, '--allow-stale').status, 0);
  // A partial scan cannot judge staleness.
  assert.equal(run(dir, join(dir, 'src', 'b.mjs')).status, 0);
});

test('inline suppression needs a reason', () => {
  const dir = repo({
    'a.mjs': '// citation-boundary-allow: regex/neutral -- fixture for the parser tests\n' + OFFENDER,
    'b.mjs': '// citation-boundary-allow: regex/neutral\n' + OFFENDER,
  });
  const json = JSON.parse(run(dir, '--format', 'json').stdout);
  assert.deepEqual(json.violations.map((violation) => `${violation.file} ${violation.rule}`).sort(), ['b.mjs regex/neutral', 'b.mjs suppression/invalid']);
});

test('--baseline writes the allow list with a loud warning, after which the scan passes', () => {
  const dir = repo({ '.citation-boundary.json': { engine: ENGINE, allow: [] }, 'src/a.mjs': OFFENDER });
  const baseline = run(dir, '--baseline');
  assert.equal(baseline.status, 0);
  assert.match(baseline.stderr, /staged migration ONLY/);
  assert.match(baseline.stderr, /EMPTY allow list/);
  const config = JSON.parse(readFileSync(join(dir, '.citation-boundary.json'), 'utf8'));
  assert.deepEqual(config.allow.map((entry) => `${entry.path} ${entry.rule}`), ['src/a.mjs regex/neutral']);
  assert.equal(run(dir).status, 0);
});

test('invalid config exits 2', () => {
  const dir = repo({ '.citation-boundary.json': { engine: ENGINE, allow: [{ path: 'a', rule: 'regex/neutral' }] } });
  const result = run(dir);
  assert.equal(result.status, 2);
  assert.match(result.stderr, /reason/);
});

test('unknown options exit 2; --list-rules prints every rule', () => {
  assert.equal(spawnSync(process.execPath, [CHECK, '--nope'], { encoding: 'utf8' }).status, 2);
  const rules = spawnSync(process.execPath, [CHECK, '--list-rules'], { encoding: 'utf8' });
  assert.equal(rules.status, 0);
  assert.match(rules.stdout, /regex\/provision/);
  assert.match(rules.stdout, /vendored\/corpus/);
});
