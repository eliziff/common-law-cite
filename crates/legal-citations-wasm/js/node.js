// Node.js (and Deno via npm:) entry: the module is loaded synchronously on
// import, so no init() call is needed; init()/initSync() exist as no-ops for
// code shared with the browser entry.
import { readFileSync } from "node:fs";
import { initSync as wasmInitSync, call as rawCall } from "./pkg/legal_citations_wasm.js";
import { makeApi, LegalCitationsError } from "./api.js";

wasmInitSync({ module: readFileSync(new URL("./pkg/legal_citations_wasm_bg.wasm", import.meta.url)) });

export async function init() {}
export function initSync() {}

const api = makeApi(() => rawCall);

export { LegalCitationsError };
export const {
  call, extract, resolve, key, keyForText, format, formatPinpoint, url, annotate, clean, registry, classifyExcerpt, hasCitation, version,
} = api;
