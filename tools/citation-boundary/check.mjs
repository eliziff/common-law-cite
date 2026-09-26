#!/usr/bin/env node
// citation-boundary: fail when citation-engine logic lives outside legal-citations.
//
//   node check.mjs [--config .citation-boundary.json] [--format text|json|github]
//                  [--root DIR] [--baseline] [--allow-stale] [--verbose] [paths...]
//
// Zero dependencies; Node >= 18. See docs/boundary.md in legal-citations.

import { resolve } from 'node:path';
import { CONFIG_NAME, loadConfig, writeBaseline } from './lib/config.mjs';
import { formatGithub, formatJson, formatText } from './lib/report.mjs';
import { RULES, describe } from './lib/rules.mjs';
import { DEFAULT_ENGINE_ROOT, gitRoot, scanRepository } from './lib/scan.mjs';

const USAGE = `usage: node check.mjs [options] [paths...]

  --config FILE      allowlist/config (default: <root>/${CONFIG_NAME})
  --format FORMAT    text (default), json, github
  --root DIR         repository root (default: git toplevel of the cwd, else cwd)
  --engine-root DIR  legal-citations checkout for registry/corpus data
                     (default: the checkout this script lives in)
  --baseline         write every current violation into the config allow list
                     (staged migration only; the goal is an empty list)
  --allow-stale      do not fail on stale allow entries
  --verbose          show source snippets in text output
  --list-rules       print rule ids and exit

Exit status: 0 clean, 1 violations/stale/expired entries, 2 usage or config error.`;

function parseArgs(argv) {
  const args = { format: 'text', paths: [] };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const value = () => {
      if (arg.includes('=')) return arg.slice(arg.indexOf('=') + 1);
      if (i + 1 >= argv.length) throw new Error(`${arg} needs a value`);
      return argv[++i];
    };
    const name = arg.split('=')[0];
    if (name === '--config') args.config = value();
    else if (name === '--format') args.format = value();
    else if (name === '--root') args.root = value();
    else if (name === '--engine-root') args.engineRoot = value();
    else if (arg === '--baseline') args.baseline = true;
    else if (arg === '--allow-stale') args.allowStale = true;
    else if (arg === '--verbose' || arg === '-v') args.verbose = true;
    else if (arg === '--list-rules') args.listRules = true;
    else if (arg === '--help' || arg === '-h') args.help = true;
    else if (arg.startsWith('--')) throw new Error(`unknown option ${arg}`);
    else args.paths.push(arg);
  }
  if (!['text', 'json', 'github'].includes(args.format)) throw new Error(`unknown format ${args.format}`);
  return args;
}

function main() {
  let args;
  try { args = parseArgs(process.argv.slice(2)); } catch (error) {
    console.error(`citation-boundary: ${error.message}\n\n${USAGE}`);
    return 2;
  }
  if (args.help) { console.log(USAGE); return 0; }
  if (args.listRules) {
    for (const [id, rule] of Object.entries(RULES)) console.log(`${id.padEnd(26)} ${rule.explain}\n${' '.repeat(27)}use: ${rule.use}`);
    return 0;
  }
  const root = resolve(args.root ?? gitRoot(process.cwd()) ?? process.cwd());
  const configPath = resolve(args.config ?? resolve(root, CONFIG_NAME));
  const { config, errors, exists } = loadConfig(configPath);
  if (errors.length) {
    console.error(`citation-boundary: invalid ${configPath}\n  ${errors.join('\n  ')}`);
    return 2;
  }
  if (!exists && !args.baseline) console.error(`citation-boundary: no ${CONFIG_NAME} at ${root}; scanning with defaults (no allow entries).`);
  if (exists && !config.engine) console.error(`citation-boundary: ${CONFIG_NAME} has no "engine" pin (eliziff/legal-citations@<tag>).`);

  const engineRoot = resolve(args.engineRoot ?? DEFAULT_ENGINE_ROOT);
  const result = scanRepository(root, { config, paths: args.paths, engineRoot });

  if (args.baseline) {
    const summary = writeBaseline(configPath, result.violations, config, describe);
    const bar = '!'.repeat(78);
    console.error([bar,
      `citation-boundary --baseline wrote ${configPath}:`,
      `  ${summary.added} new allow entr(y/ies), ${summary.kept} kept, ${summary.removed} stale removed.`,
      'A baseline hides re-implemented citation logic; it is for staged migration ONLY.',
      'The intended end state is an EMPTY allow list: migrate each entry to the',
      'legal-citations engine and delete it. Stale entries fail CI, so the list can only shrink.',
      bar].join('\n'));
    return 0;
  }

  const options = { engine: config.engine, verbose: args.verbose, configPath: args.config ?? CONFIG_NAME };
  const output = args.format === 'json' ? formatJson(result, options)
    : args.format === 'github' ? formatGithub(result, options)
      : formatText(result, options);
  console.log(output);
  const failing = result.violations.length + result.expired.length + (args.allowStale ? 0 : result.stale.length);
  return failing ? 1 : 0;
}

process.exitCode = main();
