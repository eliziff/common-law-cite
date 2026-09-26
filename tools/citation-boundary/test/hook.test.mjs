import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { contentAfter, evaluate, introduced } from '../claude-hook.mjs';
import { DEFAULT_ENGINE_ROOT } from '../lib/scan.mjs';

const HOOK = join(dirname(fileURLToPath(import.meta.url)), '..', 'claude-hook.mjs');
const OFFENDER = 'export const NEUTRAL = /\\b(?:19|20)\\d{2}\\s+[A-Z]{2,8}\\s+\\d+\\b/g;\n';

function project(files, config = { engine: 'eliziff/legal-citations@v0.1.0', allow: [] }) {
  const dir = mkdtempSync(join(tmpdir(), 'citation-boundary-hook-'));
  if (config) writeFileSync(join(dir, '.citation-boundary.json'), JSON.stringify(config));
  for (const [path, content] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true });
    writeFileSync(join(dir, path), content);
  }
  return dir;
}

const input = (dir, tool_name, tool_input) => ({ hook_event_name: 'PreToolUse', tool_name, cwd: dir, tool_input });

test('contentAfter applies Write, Edit (once / all) and MultiEdit', () => {
  assert.equal(contentAfter('Write', { content: 'x' }, 'old'), 'x');
  assert.equal(contentAfter('Edit', { old_string: 'a', new_string: 'b' }, 'a a'), 'b a');
  assert.equal(contentAfter('Edit', { old_string: 'a', new_string: 'b', replace_all: true }, 'a a'), 'b b');
  assert.equal(contentAfter('Edit', { old_string: 'zz', new_string: 'b' }, 'a'), null);
  assert.equal(contentAfter('MultiEdit', { edits: [{ old_string: 'a', new_string: 'b' }, { old_string: 'b b', new_string: 'c' }] }, 'a b'), 'c');
  assert.equal(contentAfter('Edit', { old_string: 'a', new_string: '$&$&' }, 'a'), '$&$&');
});

test('introduced() subtracts findings that already existed', () => {
  const f = (rule, snippet) => ({ rule, snippet, evidence: [] });
  assert.deepEqual(introduced([f('r', 'x')], [f('r', 'x'), f('r', 'x'), f('q', 'y')]).map((x) => x.rule), ['r', 'q']);
});

test('Write of a citation regex is blocked with the engine API in the message', () => {
  const dir = project({});
  const result = evaluate(input(dir, 'Write', { file_path: join(dir, 'src', 'cite.ts'), content: OFFENDER }));
  assert.equal(result.block, true);
  assert.match(result.message, /regex\/neutral/);
  assert.match(result.message, /use instead: extract \{ text/);
});

test('Edit that adds a court table is blocked; an unrelated edit next to an existing violation is not', () => {
  const dir = project({ 'src/a.ts': OFFENDER + 'export const x = 1;\n' });
  const file = join(dir, 'src', 'a.ts');
  assert.equal(evaluate(input(dir, 'Edit', { file_path: file, old_string: 'export const x = 1;', new_string: 'export const x = 2;' })).block, false);
  const table = 'const LEVELS = { SCC: 5, ONCA: 4, BCCA: 4, ABCA: 4, ONSC: 3, BCSC: 3 };';
  const result = evaluate(input(dir, 'MultiEdit', { file_path: file, edits: [{ old_string: 'export const x = 1;', new_string: table }] }));
  assert.equal(result.block, true);
  assert.match(result.message, /data\/court-table/);
});

test('allowlisted paths, ignored paths, non-source files and the engine itself pass', () => {
  const dir = project({}, {
    engine: 'eliziff/legal-citations@v0.1.0',
    ignore: ['vendor/**'],
    allow: [{ path: 'src/structure/**', rule: 'regex/*', reason: 'document-structure numbering parser' }],
  });
  assert.equal(evaluate(input(dir, 'Write', { file_path: join(dir, 'src', 'structure', 'n.ts'), content: OFFENDER })).block, false);
  assert.equal(evaluate(input(dir, 'Write', { file_path: join(dir, 'vendor', 'n.ts'), content: OFFENDER })).block, false);
  assert.equal(evaluate(input(dir, 'Write', { file_path: join(dir, 'README.md'), content: OFFENDER })).block, false);
  assert.equal(evaluate(input(dir, 'Read', { file_path: join(dir, 'a.ts') })).block, false);
  assert.equal(evaluate(input(DEFAULT_ENGINE_ROOT, 'Write', { file_path: join(DEFAULT_ENGINE_ROOT, 'crates', 'x.rs'), content: 'let r = Regex::new(r"\\bsupra\\s+note\\s+\\d+");' })).block, false);
});

test('an inline suppression with a reason lets the edit through', () => {
  const dir = project({});
  const content = '// citation-boundary-allow: regex/neutral -- parser fixture agreed with the owner\n' + OFFENDER;
  assert.equal(evaluate(input(dir, 'Write', { file_path: join(dir, 'a.ts'), content })).block, false);
});

test('the script speaks the hook protocol: exit 2 + stderr to block, exit 0 to allow', () => {
  const dir = project({});
  const blocked = spawnSync(process.execPath, [HOOK], { input: JSON.stringify(input(dir, 'Write', { file_path: join(dir, 'a.py'), content: 'import re\nIBID = re.compile(r"^\\s*ibid\\b", re.I)\n' })), encoding: 'utf8' });
  assert.equal(blocked.status, 2);
  assert.match(blocked.stderr, /regex\/short-form/);
  const allowed = spawnSync(process.execPath, [HOOK], { input: JSON.stringify(input(dir, 'Write', { file_path: join(dir, 'a.py'), content: 'x = 1\n' })), encoding: 'utf8' });
  assert.equal(allowed.status, 0);
  assert.equal(allowed.stderr, '');
  const garbage = spawnSync(process.execPath, [HOOK], { input: 'not json', encoding: 'utf8' });
  assert.equal(garbage.status, 1);
});

test('Windows-style backslash paths are normalized before matching', { skip: process.platform === 'win32' }, () => {
  const dir = project({});
  const file = join(dir, 'src', 'a.ts').replace(/\//g, '\\');
  assert.equal(evaluate(input(dir, 'Write', { file_path: file, content: OFFENDER })).block, true);
});
