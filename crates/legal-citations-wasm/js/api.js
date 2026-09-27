// Shared JS surface over the wasm `call(method, requestJson)` export.
// Offsets default to UTF-16 code units, i.e. JavaScript string indices.

export class LegalCitationsError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "LegalCitationsError";
    this.code = code;
  }
}

const OPTION_NAMES = new Set(["resolve", "parallel", "extendedUs", "notes", "removeAmbiguous", "jurisdictionPriority"]);

export function makeApi(getRawCall) {
  function call(method, request = {}) {
    let response;
    try {
      response = getRawCall()(method, JSON.stringify(request));
    } catch (error) {
      let parsed;
      try {
        parsed = JSON.parse(error.message);
      } catch {
        throw error;
      }
      throw new LegalCitationsError(parsed.code, parsed.message);
    }
    return JSON.parse(response);
  }

  function splitOptions(options = {}) {
    const engine = {};
    const rest = {};
    for (const [name, value] of Object.entries(options)) {
      if (OPTION_NAMES.has(name)) engine[name] = value;
      else rest[name] = value;
    }
    return [engine, rest];
  }

  function rejectUnknown(rest, allowed) {
    for (const name of Object.keys(rest)) {
      if (!allowed.includes(name)) {
        throw new TypeError(`unknown option ${JSON.stringify(name)}`);
      }
    }
  }

  function citationOrText(input, options) {
    const [engine, rest] = splitOptions(options);
    const request = { options: engine };
    if (typeof input === "string") request.text = input;
    else request.citation = input;
    return [request, rest];
  }

  return {
    call,
    resolve(citations, options = {}) {
      rejectUnknown(options, ["notes", "aliasGroups"]);
      return call("resolve", { citations, ...options });
    },
    extract(text, options = {}) {
      const [engine, rest] = splitOptions(options);
      rejectUnknown(rest, ["offsetUnit", "markupText"]);
      return call("extract", { text, options: engine, offsetUnit: "utf16", ...rest }).citations;
    },
    key(citation) {
      return call("key", { citation }).key;
    },
    keyForText(text, options = {}) {
      const [engine, rest] = splitOptions(options);
      rejectUnknown(rest, []);
      return call("keyForText", { text, options: engine });
    },
    format(input, options = {}) {
      const [request, rest] = citationOrText(input, options);
      rejectUnknown(rest, ["style", "language", "rangeDash"]);
      return call("format", { ...request, ...rest }).citations;
    },
    formatPinpoint(kind, locators, options = {}) {
      rejectUnknown(options, ["style", "language", "rangeDash"]);
      return call("format", { pinpoint: { kind, locators }, ...options }).pinpoint;
    },
    url(input, options = {}) {
      const [request, rest] = citationOrText(input, options);
      rejectUnknown(rest, ["language", "anchor"]);
      return call("url", { ...request, ...rest }).urls;
    },
    annotate(text, options = {}) {
      const [engine, rest] = splitOptions(options);
      rejectUnknown(rest, ["before", "after", "annotations", "span", "source", "cleanSteps", "unbalancedTags", "offsetUnit"]);
      return call("annotate", { text, options: engine, offsetUnit: "utf16", ...rest }).text;
    },
    clean(text, steps) {
      return call("clean", { text, steps }).text;
    },
    registry(table, surface) {
      const request = {};
      if (table !== undefined) request.table = table;
      if (surface !== undefined) request.surface = surface;
      return call("registry", request);
    },
    classifyExcerpt(excerpt) {
      return call("classifyExcerpt", { excerpt });
    },
    hasCitation(text) {
      return call("hasCitation", { text }).hasCitation;
    },
    version() {
      return call("version");
    },
  };
}
