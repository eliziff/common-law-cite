#!/usr/bin/env node
// Run the conformance cases through the npm package (WebAssembly build).
//
//   crates/legal-citations-wasm/build.sh && node conformance/run.mjs [--filter s] [--verbose]
//
// Same statuses and matcher semantics as conformance/run.py and the Rust
// runner (see conformance/README.md). --promote is only in run.py.
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const value = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const packagePath = value("--package") ?? join(here, "../crates/legal-citations-wasm/js/node.js");
const lc = await import(pathToFileURL(packagePath).href);
const filter = value("--filter");
const verbose = flag("--verbose");

function buildRequest(testCase) {
  const method = testCase.method ?? "extract";
  if (testCase.request) return [method, testCase.request];
  const options = { ...(testCase.options ?? {}) };
  const offsetUnit = options.offsetUnit ?? "char";
  delete options.offsetUnit;
  return [method, { text: testCase.input, options, offsetUnit }];
}

const isObject = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
const show = (value) => JSON.stringify(value);

function mismatch(expected, actual, path = "$") {
  if (actual === undefined) actual = null;
  if (expected === null) return actual === null ? null : `${path}: expected absent, got ${show(actual)}`;
  if (isObject(expected) && Object.keys(expected).length === 1 && "$contains" in expected) {
    if (!Array.isArray(actual)) return `${path}: expected an array, got ${show(actual)}`;
    let cursor = 0;
    for (const [position, item] of expected.$contains.entries()) {
      const offset = actual.slice(cursor).findIndex((candidate) => mismatch(item, candidate, path) === null);
      if (offset < 0) return `${path}: no element (in order) matches $contains[${position}] = ${show(item)}`;
      cursor += offset + 1;
    }
    return null;
  }
  if (isObject(expected) && Object.keys(expected).length === 1 && "$in" in expected) {
    return expected.$in.some((choice) => mismatch(choice, actual, path) === null)
      ? null
      : `${path}: ${show(actual)} is not one of ${show(expected.$in)}`;
  }
  if (isObject(expected)) {
    if (!isObject(actual)) return `${path}: expected an object, got ${show(actual)}`;
    for (const [key, item] of Object.entries(expected)) {
      const problem = mismatch(item, actual[key], `${path}.${key}`);
      if (problem) return problem;
    }
    return null;
  }
  if (Array.isArray(expected)) {
    if (!Array.isArray(actual)) return `${path}: expected an array, got ${show(actual)}`;
    if (actual.length !== expected.length) return `${path}: expected ${expected.length} elements, got ${actual.length}`;
    for (let index = 0; index < expected.length; index++) {
      const problem = mismatch(expected[index], actual[index], `${path}[${index}]`);
      if (problem) return problem;
    }
    return null;
  }
  return expected === actual ? null : `${path}: expected ${show(expected)}, got ${show(actual)}`;
}

const casesDir = join(here, "cases");
const failures = [];
const totals = { pass: 0, pending: 0 };
console.log("conformance (npm/wasm):");
for (const file of readdirSync(casesDir).filter((name) => name.endsWith(".json")).sort()) {
  const stem = file.replace(/\.json$/, "");
  const document = JSON.parse(readFileSync(join(casesDir, file), "utf8"));
  if (document.format !== "legal-citations-conformance:v1") {
    failures.push(`${file}: unknown format ${show(document.format)}`);
    continue;
  }
  const tally = { pass: 0, pending: 0 };
  for (const testCase of document.cases) {
    const label = `${stem}::${testCase.name}`;
    if (filter && !label.includes(filter)) continue;
    if (testCase.status === "pending" && !verbose) {
      tally.pending++;
      totals.pending++;
      continue;
    }
    const [method, request] = buildRequest(testCase);
    let actual;
    try {
      actual = lc.call(method, request);
    } catch (error) {
      if (!(error instanceof lc.LegalCitationsError)) throw error;
      actual = { error: { code: error.code, message: error.message } };
    }
    const problem = mismatch(testCase.expect, actual);
    if (testCase.status === "pass") {
      if (problem) failures.push(`REGRESSION ${label}: ${problem}`);
    } else if (testCase.status === "pending") {
      if (!problem) console.error(`now passing ${label}`);
      else if (verbose) console.error(`pending ${label}: ${problem}`);
    } else {
      failures.push(`${label}: status must be pass or pending, got ${show(testCase.status)}`);
      continue;
    }
    tally[testCase.status]++;
    totals[testCase.status]++;
  }
  console.log(`  ${stem.padEnd(24)} pass ${String(tally.pass).padStart(4)}  pending ${String(tally.pending).padStart(4)}`);
}
console.log(`  ${"total".padEnd(24)} pass ${String(totals.pass).padStart(4)}  pending ${String(totals.pending).padStart(4)}`);
for (const failure of failures) console.error(failure);
process.exit(failures.length ? 1 : 0);
