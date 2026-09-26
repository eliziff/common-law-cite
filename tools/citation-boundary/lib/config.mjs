// .citation-boundary.json: loading, validation, allowlist application, baseline.

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { RULES } from './rules.mjs';

export const CONFIG_NAME = '.citation-boundary.json';
const ENGINE_REF = /^[\w.-]+\/[\w.-]+@[\w./-]+$/;

/** Glob -> RegExp. Supports **, *, ?, {a,b}; a pattern without `/` matches a basename anywhere. */
export function globToRegExp(glob) {
  let pattern = glob.replace(/^\.\//, '');
  const anchored = pattern.includes('/');
  let out = '';
  for (let i = 0; i < pattern.length; i++) {
    const c = pattern[i];
    if (c === '*') {
      if (pattern[i + 1] === '*') {
        const slash = pattern[i + 2] === '/';
        out += slash ? '(?:.*/)?' : '.*';
        i += slash ? 2 : 1;
      } else out += '[^/]*';
    } else if (c === '?') out += '[^/]';
    else if (c === '{') {
      const close = pattern.indexOf('}', i);
      if (close < 0) { out += '\\{'; continue; }
      out += '(?:' + pattern.slice(i + 1, close).split(',').map((part) => part.replace(/[.+^$()|[\]\\]/g, '\\$&').replace(/\*/g, '[^/]*')).join('|') + ')';
      i = close;
    } else out += c.replace(/[.+^$()|[\]\\]/g, '\\$&');
  }
  if (pattern.endsWith('/')) out += '.*';
  return new RegExp(anchored ? `^${out}$` : `(?:^|/)${out}$`);
}

export function ruleMatches(pattern, rule) {
  if (pattern === rule || pattern === '*') return true;
  if (pattern.endsWith('/*')) return rule.startsWith(pattern.slice(0, -1));
  return false;
}

export function defaultConfig() {
  return { engine: null, ignore: [], allow: [], tableThreshold: 5, engineModules: null };
}

/** Load and validate. Returns { config, errors, path, exists }. */
export function loadConfig(path) {
  const config = defaultConfig();
  const errors = [];
  if (!existsSync(path)) return { config, errors, path, exists: false };
  let raw;
  try { raw = JSON.parse(readFileSync(path, 'utf8')); } catch (error) {
    return { config, errors: [`${path}: invalid JSON (${error.message})`], path, exists: true };
  }
  if (raw.engine !== undefined) {
    if (typeof raw.engine !== 'string' || !ENGINE_REF.test(raw.engine)) errors.push('"engine" must look like "eliziff/legal-citations@<tag or rev>"');
    config.engine = raw.engine;
  }
  for (const key of ['ignore', 'include']) {
    if (raw[key] !== undefined) {
      if (!Array.isArray(raw[key]) || raw[key].some((item) => typeof item !== 'string')) errors.push(`"${key}" must be an array of globs`);
      else config[key] = raw[key];
    }
  }
  if (raw.tableThreshold !== undefined) {
    if (!Number.isInteger(raw.tableThreshold) || raw.tableThreshold < 3) errors.push('"tableThreshold" must be an integer >= 3');
    else config.tableThreshold = raw.tableThreshold;
  }
  if (raw.engineModules !== undefined) {
    if (!Array.isArray(raw.engineModules)) errors.push('"engineModules" must be an array of strings');
    else config.engineModules = raw.engineModules;
  }
  if (raw.allow !== undefined && !Array.isArray(raw.allow)) errors.push('"allow" must be an array');
  (Array.isArray(raw.allow) ? raw.allow : []).forEach((entry, index) => {
    const where = `allow[${index}]`;
    if (!entry || typeof entry !== 'object') { errors.push(`${where}: must be an object`); return; }
    if (typeof entry.path !== 'string' || !entry.path) errors.push(`${where}: "path" (glob) is required`);
    if (typeof entry.rule !== 'string' || !entry.rule) errors.push(`${where}: "rule" is required`);
    else if (entry.rule !== '*' && !RULES[entry.rule] && !(entry.rule.endsWith('/*') && Object.keys(RULES).some((rule) => rule.startsWith(entry.rule.slice(0, -1))))) {
      errors.push(`${where}: unknown rule "${entry.rule}"`);
    }
    if (typeof entry.reason !== 'string' || entry.reason.trim().length < 8) errors.push(`${where}: "reason" is required (say why this code may keep citation logic)`);
    if (entry.until !== undefined && (typeof entry.until !== 'string' || Number.isNaN(Date.parse(entry.until)))) errors.push(`${where}: "until" must be an ISO date`);
    config.allow.push({ ...entry, index, matcher: typeof entry.path === 'string' ? globToRegExp(entry.path) : /$^/, used: 0 });
  });
  return { config, errors, path, exists: true };
}

/**
 * Split findings into violations and allowed ones; mark entries used; report
 * expired entries. Inline suppressions are applied first.
 */
export function applyAllowlist(findings, suppressions, config, today = new Date()) {
  const violations = [];
  const allowed = [];
  const expired = [];
  const day = today.toISOString().slice(0, 10);
  for (const entry of config.allow) {
    entry.expired = Boolean(entry.until && entry.until < day);
    if (entry.expired) expired.push(entry);
  }
  for (const finding of findings) {
    if (finding.rule === 'suppression/invalid') { violations.push(finding); continue; }
    const inline = suppressions.find((suppression) => !suppression.invalid && suppression.file === finding.file
      && suppression.rules.some((rule) => ruleMatches(rule, finding.rule))
      && (suppression.fileWide || (suppression.line >= finding.line - 1 && suppression.line <= finding.endLine)));
    if (inline) { inline.used = true; allowed.push({ ...finding, allowedBy: `inline:${inline.line}` }); continue; }
    const entry = config.allow.find((candidate) => !candidate.expired && candidate.matcher.test(finding.file) && ruleMatches(candidate.rule, finding.rule));
    if (entry) { entry.used++; allowed.push({ ...finding, allowedBy: `allow[${entry.index}]` }); continue; }
    violations.push(finding);
  }
  return { violations, allowed, expired };
}

export function staleEntries(config, suppressions) {
  const stale = config.allow.filter((entry) => !entry.expired && entry.used === 0)
    .map((entry) => ({ kind: 'allow', index: entry.index, path: entry.path, rule: entry.rule, reason: entry.reason }));
  for (const suppression of suppressions) {
    if (!suppression.invalid && !suppression.used) stale.push({ kind: 'inline', file: suppression.file, line: suppression.line, rule: suppression.rules.join(','), reason: suppression.reason });
  }
  return stale;
}

/** Rewrite the config so current violations are allowed (staged migration only). */
export function writeBaseline(path, violations, config, describe) {
  let raw = {};
  if (existsSync(path)) { try { raw = JSON.parse(readFileSync(path, 'utf8')); } catch { raw = {}; } }
  const keep = (raw.allow ?? []).filter((_, index) => {
    const entry = config.allow.find((candidate) => candidate.index === index);
    return !entry || entry.used > 0;
  });
  const date = new Date().toISOString().slice(0, 10);
  const seen = new Set(keep.map((entry) => `${entry.path}\u0000${entry.rule}`));
  const added = [];
  for (const violation of violations) {
    if (violation.rule === 'suppression/invalid') continue;
    const key = `${violation.file}\u0000${violation.rule}`;
    if (seen.has(key)) continue;
    seen.add(key);
    added.push({ path: violation.file, rule: violation.rule, reason: `baseline ${date}: migrate to legal-citations (${describe(violation.rule).use.split(';')[0]})` });
  }
  const next = { engine: raw.engine ?? 'eliziff/legal-citations@<tag>', ...raw, allow: [...keep, ...added] };
  writeFileSync(path, JSON.stringify(next, null, 2) + '\n');
  return { added: added.length, kept: keep.length, removed: (raw.allow ?? []).length - keep.length };
}
