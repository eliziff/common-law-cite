// Repository scan: list files, run detectors, apply the allowlist.

import { execFileSync } from 'node:child_process';
import { closeSync, openSync, readdirSync, readFileSync, readSync, realpathSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { applyAllowlist, globToRegExp, staleEntries } from './config.mjs';
import { scanFile } from './detectors.mjs';
import { languageOf } from './lexer.mjs';
import { loadEngineData } from './signatures.mjs';

export const TOOL_DIR = resolve(dirname(fileURLToPath(import.meta.url)), '..');
export const DEFAULT_ENGINE_ROOT = resolve(TOOL_DIR, '..', '..');

const ALWAYS_IGNORED = [
  '**/node_modules/**', '**/target/**', '**/.git/**', '**/*.min.js', '**/*.bundle.js',
  '**/citation-boundary/test/fixtures/**',
];
/** Inside legal-citations itself these paths are the engine. */
const ENGINE_SELF = ['crates/**', 'bindings/**', 'python/**', 'wasm/**', 'cli/**', 'conformance/**', 'tests/**', 'tools/**', 'docs/**'];
const MAX_BYTES = 1_500_000;

let cachedData = null;
export function engineData(engineRoot = DEFAULT_ENGINE_ROOT) {
  if (!cachedData || cachedData.root !== engineRoot) cachedData = { root: engineRoot, data: loadEngineData(engineRoot) };
  return cachedData.data;
}

function safeRealpath(path) {
  try { return realpathSync(path); } catch { return resolve(path); }
}

export function isEngineRoot(root, engineRoot = DEFAULT_ENGINE_ROOT) {
  return safeRealpath(root) === safeRealpath(engineRoot);
}

export function gitRoot(dir) {
  try {
    return execFileSync('git', ['-C', dir, 'rev-parse', '--show-toplevel'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch { return null; }
}

function listGit(root) {
  const out = execFileSync('git', ['-C', root, 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], {
    encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, stdio: ['ignore', 'pipe', 'ignore'],
  });
  return [...new Set(out.split('\0').filter(Boolean))];
}

function listWalk(root) {
  const out = [];
  const walk = (dir) => {
    let entries;
    try { entries = readdirSync(dir, { withFileTypes: true }); } catch { return; }
    for (const entry of entries) {
      if (entry.name === '.git' || entry.name === 'node_modules' || entry.name === 'target') continue;
      const full = join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.isFile()) out.push(relative(root, full).split(sep).join('/'));
    }
  };
  walk(root);
  return out;
}

function readHead(path, bytes = 4096) {
  const buffer = Buffer.alloc(bytes);
  const fd = openSync(path, 'r');
  try { return buffer.subarray(0, readSync(fd, buffer, 0, bytes, 0)).toString('utf8'); } finally { closeSync(fd); }
}

export function scannable(path) {
  return Boolean(languageOf(path)) || path.toLowerCase().endsWith('.json');
}

/**
 * Scan `root`. Options: { config, paths, engineRoot, onSkip }.
 * Returns { violations, allowed, expired, stale, suppressions, files, skipped }.
 */
export function scanRepository(root, { config, paths = [], engineRoot = DEFAULT_ENGINE_ROOT } = {}) {
  const data = engineData(engineRoot);
  const inGit = Boolean(gitRoot(root)) && safeRealpath(gitRoot(root)) === safeRealpath(root);
  let files = inGit ? listGit(root) : listWalk(root);
  const ignore = [...ALWAYS_IGNORED, ...(config.ignore ?? []), ...(isEngineRoot(root, engineRoot) ? ENGINE_SELF : [])].map(globToRegExp);
  const include = (config.include ?? []).map(globToRegExp);
  const wanted = paths.map((path) => relative(root, resolve(path)).split(sep).join('/')).map((path) => (path === '' ? '.' : path));
  files = files.filter((file) => scannable(file)
    && !ignore.some((pattern) => pattern.test(file))
    && (!include.length || include.some((pattern) => pattern.test(file)))
    && (!wanted.length || wanted.some((path) => path === '.' || file === path || file.startsWith(path.replace(/\/$/, '') + '/'))));

  const findings = [];
  const suppressions = [];
  const skipped = [];
  let scanned = 0;
  for (const file of files) {
    const full = join(root, file);
    let stat;
    try { stat = statSync(full); } catch { continue; }
    if (!stat.isFile()) continue;
    const isJson = file.toLowerCase().endsWith('.json');
    if (stat.size > (isJson ? 5_000_000 : MAX_BYTES)) {
      // Oversized JSON is only checked for a corpus/manifest copy, from its head.
      if (isJson) findings.push(...scanFile(file, readHead(full), data, { tableThreshold: config.tableThreshold }).findings);
      else skipped.push({ file, reason: 'larger than 1.5 MB' });
      continue;
    }
    const text = readFileSync(full, 'utf8');
    if (!isJson && text.length > 20000 && text.length / (text.split('\n').length) > 500) { skipped.push({ file, reason: 'minified' }); continue; }
    scanned++;
    const result = scanFile(file, text, data, { tableThreshold: config.tableThreshold, engineModules: config.engineModules ?? undefined });
    findings.push(...result.findings);
    suppressions.push(...result.suppressions);
  }
  const { violations, allowed, expired } = applyAllowlist(findings, suppressions, config);
  const stale = wanted.length ? [] : staleEntries(config, suppressions);
  return { violations: merge(violations), allowed, expired, stale, suppressions, files: scanned, skipped, data };
}

/** Merge nearby findings of the same rule in the same file into one report line. */
export function merge(findings) {
  const sorted = [...findings].sort((a, b) => a.file.localeCompare(b.file) || a.rule.localeCompare(b.rule) || a.line - b.line);
  const out = [];
  for (const finding of sorted) {
    const last = out[out.length - 1];
    const gap = finding.rule.startsWith('regex/') ? 3 : 12;
    if (last && last.file === finding.file && last.rule === finding.rule && finding.line - last.endLine <= gap) {
      last.endLine = Math.max(last.endLine, finding.endLine);
      last.evidence = [...new Set([...last.evidence, ...finding.evidence])].slice(0, 8);
      last.count = (last.count ?? 1) + 1;
      continue;
    }
    out.push({ ...finding });
  }
  return out.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line || a.rule.localeCompare(b.rule));
}
