// Output formats: text (file:line), json, github (workflow annotations).

import { describe } from './rules.mjs';

const escapeData = (value) => String(value).replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A');
const escapeProperty = (value) => escapeData(value).replace(/:/g, '%3A').replace(/,/g, '%2C');

function range(finding) {
  return finding.endLine && finding.endLine !== finding.line ? `${finding.line}-${finding.endLine}` : `${finding.line}`;
}

export function formatText(result, { engine, verbose = false, configPath = '.citation-boundary.json' } = {}) {
  const lines = [];
  for (const finding of result.violations) {
    const rule = describe(finding.rule);
    const evidence = finding.evidence?.length ? ` [${finding.evidence.join(', ')}]` : '';
    lines.push(`${finding.file}:${range(finding)}  ${finding.rule}  ${finding.message}${evidence}`);
    if (verbose && finding.snippet) lines.push(`    | ${finding.snippet}`);
    lines.push(`    use: ${rule.use}`);
  }
  for (const entry of result.expired) {
    lines.push(`${configPath} allow[${entry.index}]  allow/expired  "${entry.path}" ${entry.rule} expired ${entry.until}; migrate it or renew with a reason`);
  }
  for (const entry of result.stale) {
    lines.push(entry.kind === 'allow'
      ? `${configPath} allow[${entry.index}]  allow/stale  "${entry.path}" ${entry.rule} matches nothing; delete it (allowlists only shrink)`
      : `${entry.file}:${entry.line}  allow/stale  inline citation-boundary-allow for ${entry.rule} matches nothing; delete it`);
  }
  const byRule = new Map();
  for (const finding of result.violations) byRule.set(finding.rule, (byRule.get(finding.rule) ?? 0) + 1);
  lines.push('');
  lines.push(`citation-boundary: ${result.violations.length} violation(s), ${result.stale.length} stale and ${result.expired.length} expired allow entr(y/ies), ${result.allowed.length} allowed finding(s), ${result.files} file(s) scanned${engine ? `, engine ${engine}` : ''}.`);
  if (byRule.size) lines.push('  ' + [...byRule].sort((a, b) => b[1] - a[1]).map(([rule, count]) => `${rule}=${count}`).join('  '));
  if (result.violations.length) {
    lines.push('Citation grammar, court/reporter/series data, citation keys, resolution and citation formatting live only in legal-citations.');
    lines.push('Call the engine; if it lacks a capability, add it there (docs/boundary.md in legal-citations).');
  }
  return lines.join('\n');
}

export function formatJson(result, { engine } = {}) {
  return JSON.stringify({
    engine: engine ?? null,
    files: result.files,
    violations: result.violations.map((finding) => ({ ...finding, use: describe(finding.rule).use })),
    stale: result.stale,
    expired: result.expired.map(({ index, path, rule, until, reason }) => ({ index, path, rule, until, reason })),
    allowed: result.allowed.map(({ file, line, rule, allowedBy }) => ({ file, line, rule, allowedBy })),
    skipped: result.skipped,
  }, null, 2);
}

export function formatGithub(result, { configPath = '.citation-boundary.json' } = {}) {
  const lines = [];
  for (const finding of result.violations) {
    const rule = describe(finding.rule);
    const props = [`file=${escapeProperty(finding.file)}`, `line=${finding.line}`];
    if (finding.endLine && finding.endLine !== finding.line) props.push(`endLine=${finding.endLine}`);
    props.push(`title=${escapeProperty(`citation-boundary ${finding.rule}`)}`);
    const evidence = finding.evidence?.length ? ` [${finding.evidence.join(', ')}]` : '';
    lines.push(`::error ${props.join(',')}::${escapeData(`${finding.message}${evidence}. ${rule.explain} Use legal-citations: ${rule.use}`)}`);
  }
  for (const entry of result.expired) {
    lines.push(`::error file=${escapeProperty(configPath)},title=${escapeProperty('citation-boundary allow/expired')}::${escapeData(`allow[${entry.index}] ${entry.path} ${entry.rule} expired ${entry.until}`)}`);
  }
  for (const entry of result.stale) {
    lines.push(entry.kind === 'allow'
      ? `::error file=${escapeProperty(configPath)},title=${escapeProperty('citation-boundary allow/stale')}::${escapeData(`allow[${entry.index}] ${entry.path} ${entry.rule} matches nothing; delete it`)}`
      : `::error file=${escapeProperty(entry.file)},line=${entry.line},title=${escapeProperty('citation-boundary allow/stale')}::${escapeData(`inline allow for ${entry.rule} matches nothing; delete it`)}`);
  }
  lines.push(`citation-boundary: ${result.violations.length} violation(s), ${result.stale.length} stale, ${result.expired.length} expired, ${result.files} file(s) scanned.`);
  return lines.join('\n');
}
