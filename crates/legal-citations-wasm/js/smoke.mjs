// Plumbing smoke test for the npm package (behavior lives in conformance/).
import assert from "node:assert/strict";
import * as lc from "./node.js";

const text = "Voilà — 😀 R c Jordan, 2016 CSC 27";
const citations = lc.extract(text);
assert.ok(citations.length > 0, "expected a citation");
const { start, end, text: core } = citations[0].span;
assert.equal(text.slice(start, end), core, "utf16 offsets index JS strings");
assert.equal(lc.version().version, JSON.parse((await import("node:fs")).readFileSync(new URL("./package.json", import.meta.url))).version);
assert.throws(() => lc.call("nope"), (error) => error instanceof lc.LegalCitationsError && error.code === "unknown_method");
assert.throws(() => lc.extract("x", { resolv: true }), TypeError);
assert.equal(lc.hasCitation("nothing here"), false);
console.log("npm smoke: ok");
