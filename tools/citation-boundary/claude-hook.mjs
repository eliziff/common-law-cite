#!/usr/bin/env node
// Claude Code PreToolUse hook: block Edit/Write/MultiEdit calls that would add
// citation-engine logic outside legal-citations.
//
// Contract (https://code.claude.com/docs/en/hooks): the hook reads one JSON
// object on stdin ({ hook_event_name, tool_name, tool_input, cwd, ... });
// tool_input is { file_path, content } for Write, { file_path, old_string,
// new_string, replace_all } for Edit and { file_path, edits: [...] } for
// MultiEdit. Exit 0 lets the call proceed; exit 2 blocks it and stderr is
// shown to Claude as the reason; any other exit is a non-blocking hook error.
//
// Only violations the edit introduces are reported: the file is scanned before
// and after the edit and pre-existing findings (tracked by the repository's
// .citation-boundary.json and CI) do not block unrelated edits.
//
// .claude/settings.json in a consumer repository:
//   { "hooks": { "PreToolUse": [ { "matcher": "Edit|Write|MultiEdit", "hooks": [
//       { "type": "command", "command": "node", "args": ["<legal-citations>/tools/citation-boundary/claude-hook.mjs"] } ] } ] } }

import { existsSync, readFileSync } from 'node:fs';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { CONFIG_NAME, applyAllowlist, globToRegExp, loadConfig } from './lib/config.mjs';
import { scanFile } from './lib/detectors.mjs';
import { describe } from './lib/rules.mjs';
import { DEFAULT_ENGINE_ROOT, engineData, gitRoot, isEngineRoot, merge, scannable } from './lib/scan.mjs';

const TOOLS = new Set(['Write', 'Edit', 'MultiEdit']);
const ALWAYS_IGNORED = ['**/node_modules/**', '**/target/**', '**/*.min.js', '**/citation-boundary/test/fixtures/**'].map(globToRegExp);

function readStdin() {
  try { return readFileSync(0, 'utf8'); } catch { return ''; }
}

function findRoot(file, input) {
  for (let dir = dirname(file); ; dir = dirname(dir)) {
    if (existsSync(join(dir, CONFIG_NAME))) return dir;
    if (dirname(dir) === dir) break;
  }
  return gitRoot(dirname(file)) ?? process.env.CLAUDE_PROJECT_DIR ?? input.cwd ?? dirname(file);
}

function replaceOnce(text, oldString, newString, all) {
  if (typeof oldString !== 'string' || typeof newString !== 'string') return null;
  if (oldString === '') return text === '' ? newString : null;
  if (!text.includes(oldString)) return null;
  return all ? text.split(oldString).join(newString) : text.replace(oldString, () => newString);
}

/** The file content after the tool call, or null when it cannot be computed. */
export function contentAfter(toolName, toolInput, before) {
  if (toolName === 'Write') return typeof toolInput.content === 'string' ? toolInput.content : null;
  if (toolName === 'Edit') return replaceOnce(before, toolInput.old_string, toolInput.new_string, toolInput.replace_all);
  if (toolName === 'MultiEdit') {
    let text = before;
    for (const edit of toolInput.edits ?? []) {
      text = replaceOnce(text, edit.old_string, edit.new_string, edit.replace_all);
      if (text === null) return null;
    }
    return text;
  }
  return null;
}

/** Findings present after but not before (multiset difference on rule + source line). */
export function introduced(beforeFindings, afterFindings) {
  const key = (finding) => `${finding.rule}\u0000${finding.snippet}\u0000${finding.evidence.join(',')}`;
  const remaining = new Map();
  for (const finding of beforeFindings) remaining.set(key(finding), (remaining.get(key(finding)) ?? 0) + 1);
  return afterFindings.filter((finding) => {
    const count = remaining.get(key(finding)) ?? 0;
    if (count > 0) { remaining.set(key(finding), count - 1); return false; }
    return true;
  });
}

/** Evaluate one hook input. Returns { block: boolean, message }. */
export function evaluate(input, { engineRoot = DEFAULT_ENGINE_ROOT } = {}) {
  if (!input || !TOOLS.has(input.tool_name) || !input.tool_input) return { block: false };
  const rawPath = String(input.tool_input.file_path ?? '').replace(/\\/g, '/');
  if (!rawPath) return { block: false };
  const file = isAbsolute(rawPath) ? rawPath : resolve(input.cwd ?? process.cwd(), rawPath);
  if (!scannable(file)) return { block: false };
  const root = findRoot(file, input);
  // Edits inside legal-citations itself are engine work.
  const inEngine = relative(engineRoot, file);
  if (isEngineRoot(root, engineRoot) || (inEngine && !inEngine.startsWith('..') && !isAbsolute(inEngine))) return { block: false };
  const rel = relative(root, file).split(sep).join('/');
  if (rel.startsWith('..')) return { block: false };
  const { config, errors } = loadConfig(join(root, CONFIG_NAME));
  if (errors.length) return { block: false, warning: `citation-boundary hook: invalid ${CONFIG_NAME}: ${errors.join('; ')}` };
  const ignore = [...ALWAYS_IGNORED, ...(config.ignore ?? []).map(globToRegExp)];
  if (ignore.some((pattern) => pattern.test(rel))) return { block: false };
  const include = (config.include ?? []).map(globToRegExp);
  if (include.length && !include.some((pattern) => pattern.test(rel))) return { block: false };

  let before = '';
  try { before = readFileSync(file, 'utf8'); } catch { before = ''; }
  const after = contentAfter(input.tool_name, input.tool_input, before);
  if (after === null) return { block: false };

  const data = engineData(engineRoot);
  const options = { tableThreshold: config.tableThreshold, engineModules: config.engineModules ?? undefined };
  const scan = (text) => {
    const { findings, suppressions } = scanFile(rel, text, data, options);
    return applyAllowlist(findings, suppressions, config).violations;
  };
  const added = merge(introduced(scan(before), scan(after)));
  if (!added.length) return { block: false };

  const lines = [
    `citation-boundary: this ${input.tool_name} would add citation-engine logic to ${rel}.`,
    'Citation grammar, court/reporter/series data, citation keys, resolution and citation formatting',
    'live only in legal-citations (eliziff/legal-citations); application code calls the engine.',
    '',
  ];
  for (const finding of added) {
    const rule = describe(finding.rule);
    const evidence = finding.evidence.length ? ` [${finding.evidence.join(', ')}]` : '';
    lines.push(`- line ${finding.line}: ${finding.rule}: ${finding.message}${evidence}`);
    lines.push(`  use instead: ${rule.use}`);
  }
  lines.push('',
    'Rewrite the change to call the engine. If the engine lacks the capability, stop and propose adding it to',
    'legal-citations (grammar in the corpus with vectors, data in the registry, a conformance case) instead of',
    'working around it here. Only if this is genuinely not citation logic (for example document-structure',
    'numbering), and the user agrees, mark it with `citation-boundary-allow: <rule> -- <reason>` on or above the line.');
  return { block: true, message: lines.join('\n') };
}

function main() {
  const raw = readStdin();
  let input;
  try { input = JSON.parse(raw); } catch {
    process.stderr.write('citation-boundary hook: stdin is not hook JSON; skipping.\n');
    return 1;
  }
  let result;
  try { result = evaluate(input); } catch (error) {
    process.stderr.write(`citation-boundary hook failed: ${error.message}\n`);
    return 1;
  }
  if (result.warning) { process.stderr.write(result.warning + '\n'); return 1; }
  if (!result.block) return 0;
  process.stderr.write(result.message + '\n');
  return 2;
}

if (process.argv[1]?.endsWith('claude-hook.mjs')) {
  process.exitCode = main();
}
