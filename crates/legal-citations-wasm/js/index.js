// Browser, bundler, Deno and MV3-extension entry. Call `await init()` once
// (optionally with a URL, Response, bytes or WebAssembly.Module), then use the
// synchronous functions. No top-level await, so it also loads in service
// workers.
import wasmInit, { initSync as wasmInitSync, call as rawCall } from "./pkg/legal_citations_wasm.js";
import { makeApi, LegalCitationsError } from "./api.js";

let ready = false;

export async function init(input) {
  if (!ready) {
    await wasmInit(input === undefined ? undefined : { module_or_path: input });
    ready = true;
  }
}

export function initSync(module) {
  if (!ready) {
    wasmInitSync({ module });
    ready = true;
  }
}

const api = makeApi(() => {
  if (!ready) throw new Error("legal-citations: call `await init()` before using the API");
  return rawCall;
});

export { LegalCitationsError };
export const {
  call, extract, resolve, key, keyForText, format, formatPinpoint, url, annotate, clean, registry, classifyExcerpt, hasCitation, version,
} = api;
