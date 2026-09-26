// Minimal, dependency-free lexers for JavaScript/TypeScript, Python and Rust.
//
// They are not parsers: they only separate comments, string literals and (for
// JavaScript) regex literals from code, with offsets, so detectors can look at
// regex sources, string tables and code shape without being fooled by prose in
// comments or by quotes inside strings. Every lexer recovers from malformed
// input (an unterminated single-line string ends at the newline).

export const LANGUAGES = {
  js: ['.js', '.mjs', '.cjs', '.jsx', '.ts', '.tsx', '.mts', '.cts'],
  py: ['.py', '.pyi'],
  rs: ['.rs'],
};

export function languageOf(path) {
  const lower = path.toLowerCase();
  if (lower.endsWith('.d.ts')) return null;
  for (const [language, extensions] of Object.entries(LANGUAGES)) {
    if (extensions.some((extension) => lower.endsWith(extension))) return language;
  }
  return null;
}

const IDENT = /[A-Za-z0-9_$]/;
const JS_REGEX_KEYWORDS = new Set([
  'return', 'typeof', 'instanceof', 'in', 'of', 'new', 'delete', 'void', 'throw',
  'case', 'do', 'else', 'yield', 'await', 'export', 'default',
]);

// ---------------------------------------------------------------- JavaScript

function unescapeJs(body) {
  return body.replace(/\\(u\{[0-9a-fA-F]+\}|u[0-9a-fA-F]{4}|x[0-9a-fA-F]{2}|\r?\n|[\s\S])/g, (_, escape) => {
    if (escape[0] === 'u' && escape.length > 1) {
      const hex = escape[1] === '{' ? escape.slice(2, -1) : escape.slice(1);
      try { return String.fromCodePoint(parseInt(hex, 16)); } catch { return escape; }
    }
    if (escape[0] === 'x' && escape.length === 3) return String.fromCharCode(parseInt(escape.slice(1), 16));
    if (escape === '\n' || escape === '\r\n') return '';
    return { n: '\n', t: '\t', r: '\r', b: '\b', f: '\f', v: '\v', 0: '\0' }[escape] ?? escape;
  });
}

function lexJsRange(src, start, tokens, stopAtBrace) {
  let i = start;
  let prev = null; // { kind, text } of the previous significant token
  let depth = 0;
  const n = src.length;
  while (i < n) {
    const c = src[i];
    if (c === '\n' || c === ' ' || c === '\t' || c === '\r') { i++; continue; }
    if (c === '/' && src[i + 1] === '/') {
      let j = src.indexOf('\n', i);
      if (j < 0) j = n;
      tokens.push({ type: 'comment', start: i, end: j, value: src.slice(i + 2, j) });
      i = j;
      continue;
    }
    if (c === '/' && src[i + 1] === '*') {
      let j = src.indexOf('*/', i + 2);
      j = j < 0 ? n : j + 2;
      tokens.push({ type: 'comment', start: i, end: j, value: src.slice(i + 2, j - 2) });
      i = j;
      continue;
    }
    if (c === '#' && i === 0 && src[1] === '!') { const j = src.indexOf('\n'); i = j < 0 ? n : j; continue; }
    if (c === '"' || c === "'") {
      let j = i + 1;
      while (j < n && src[j] !== c && src[j] !== '\n') j += src[j] === '\\' ? 2 : 1;
      const end = Math.min(j + (src[j] === c ? 1 : 0), n);
      tokens.push({ type: 'string', start: i, end, value: unescapeJs(src.slice(i + 1, src[j] === c ? j : end)), quote: c });
      i = end;
      prev = { kind: 'string' };
      continue;
    }
    if (c === '`') {
      const isRaw = prev?.kind === 'ident' && /String\.raw$/.test(src.slice(Math.max(0, i - 12), i).replace(/\s+/g, ''));
      i = lexTemplate(src, i, tokens, isRaw);
      prev = { kind: 'string' };
      continue;
    }
    if (c === '/') {
      const allowed = !prev || (prev.kind === 'punct' && !')]'.includes(prev.text))
        || (prev.kind === 'ident' && JS_REGEX_KEYWORDS.has(prev.text));
      if (allowed) {
        let j = i + 1, inClass = false, ok = false;
        while (j < n) {
          const d = src[j];
          if (d === '\n') break;
          if (d === '\\') { j += 2; continue; }
          if (inClass) { if (d === ']') inClass = false; } else if (d === '[') inClass = true;
          else if (d === '/') { ok = true; break; }
          j++;
        }
        if (ok && j > i + 1) {
          let k = j + 1;
          while (k < n && /[a-z]/.test(src[k])) k++;
          tokens.push({ type: 'regex', start: i, end: k, value: src.slice(i + 1, j), flags: src.slice(j + 1, k) });
          i = k;
          prev = { kind: 'regex' };
          continue;
        }
      }
      i++;
      prev = { kind: 'punct', text: '/' };
      continue;
    }
    if (IDENT.test(c)) {
      let j = i + 1;
      while (j < n && IDENT.test(src[j])) j++;
      // Keep `String.raw` recognisable as one identifier run.
      prev = { kind: /[0-9]/.test(c) ? 'num' : 'ident', text: src.slice(i, j) };
      i = j;
      continue;
    }
    if (stopAtBrace) {
      if (c === '{') depth++;
      else if (c === '}') { if (depth === 0) return i + 1; depth--; }
    }
    prev = { kind: 'punct', text: c };
    i++;
  }
  return i;
}

function lexTemplate(src, start, tokens, isRaw) {
  const n = src.length;
  let i = start + 1;
  let text = '';
  const nested = [];
  while (i < n && src[i] !== '`') {
    if (src[i] === '\\') { text += src.slice(i, i + 2); i += 2; continue; }
    if (src[i] === '$' && src[i + 1] === '{') {
      const end = lexJsRange(src, i + 2, nested, true);
      text += src.slice(i, end);
      i = end;
      continue;
    }
    text += src[i];
    i++;
  }
  const end = Math.min(i + 1, n);
  tokens.push({ type: 'template', start, end, value: isRaw ? text : unescapeJs(text), raw: isRaw });
  // Strings and templates nested in `${...}` stay separate tokens too.
  for (const token of nested) tokens.push({ ...token, nested: true });
  return end;
}

export function lexJs(src) {
  const tokens = [];
  lexJsRange(src, 0, tokens, false);
  return tokens;
}

// ---------------------------------------------------------------- Python

function unescapePy(body) {
  return body.replace(/\\(N\{[^}]*\}|u[0-9a-fA-F]{4}|U[0-9a-fA-F]{8}|x[0-9a-fA-F]{2}|\r?\n|[\\'"abfnrtv0])/g, (_, escape) => {
    if (escape === '\n' || escape === '\r\n') return '';
    if (/^[uUx]/.test(escape)) return String.fromCodePoint(parseInt(escape.slice(1), 16));
    if (escape[0] === 'N') return '?';
    return { '\\': '\\', "'": "'", '"': '"', a: '\x07', b: '\b', f: '\f', n: '\n', r: '\r', t: '\t', v: '\v', 0: '\0' }[escape];
  });
}

export function lexPy(src) {
  const tokens = [];
  const n = src.length;
  let i = 0;
  while (i < n) {
    const c = src[i];
    if (c === '#') {
      let j = src.indexOf('\n', i);
      if (j < 0) j = n;
      tokens.push({ type: 'comment', start: i, end: j, value: src.slice(i + 1, j) });
      i = j;
      continue;
    }
    if (c === '"' || c === "'") {
      let p = i;
      while (p > 0 && /[rRbBuUfFtT]/.test(src[p - 1]) && i - p < 3) p--;
      if (p > 0 && /[A-Za-z0-9_]/.test(src[p - 1])) p = i;
      const prefix = src.slice(p, i).toLowerCase();
      const raw = prefix.includes('r');
      const triple = src.startsWith(c.repeat(3), i);
      const quote = triple ? c.repeat(3) : c;
      let j = i + quote.length;
      let closed = false;
      while (j < n) {
        if (src[j] === '\\') { j += 2; continue; }
        if (!triple && src[j] === '\n') break;
        if (src.startsWith(quote, j)) { closed = true; break; }
        j++;
      }
      const bodyEnd = Math.min(j, n);
      const end = closed ? j + quote.length : bodyEnd;
      let value = src.slice(i + quote.length, bodyEnd);
      if (!raw) value = unescapePy(value);
      if (prefix.includes('f')) value = value.replace(/\{\{/g, '{').replace(/\}\}/g, '}');
      tokens.push({ type: 'string', start: p, end, value, raw, prefix, triple, quote: c });
      i = end;
      continue;
    }
    i++;
  }
  return tokens;
}

// ---------------------------------------------------------------- Rust

function unescapeRust(body) {
  return body.replace(/\\(u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|\r?\n\s*|[\s\S])/g, (_, escape) => {
    if (escape[0] === 'u' && escape.length > 1) return String.fromCodePoint(parseInt(escape.slice(2, -1), 16));
    if (escape[0] === 'x' && escape.length === 3) return String.fromCharCode(parseInt(escape.slice(1), 16));
    if (escape[0] === '\n' || escape[0] === '\r') return '';
    return { n: '\n', t: '\t', r: '\r', 0: '\0' }[escape] ?? escape;
  });
}

export function lexRs(src) {
  const tokens = [];
  const n = src.length;
  let i = 0;
  while (i < n) {
    const c = src[i];
    if (c === '/' && src[i + 1] === '/') {
      let j = src.indexOf('\n', i);
      if (j < 0) j = n;
      tokens.push({ type: 'comment', start: i, end: j, value: src.slice(i + 2, j) });
      i = j;
      continue;
    }
    if (c === '/' && src[i + 1] === '*') {
      let depth = 1, j = i + 2;
      while (j < n && depth) {
        if (src[j] === '/' && src[j + 1] === '*') { depth++; j += 2; } else if (src[j] === '*' && src[j + 1] === '/') { depth--; j += 2; } else j++;
      }
      tokens.push({ type: 'comment', start: i, end: j, value: src.slice(i + 2, j - 2) });
      i = j;
      continue;
    }
    const identStart = i === 0 || !/[A-Za-z0-9_]/.test(src[i - 1]);
    // Raw strings: r"..", r#".."#, br"..", br#".."#.
    if (identStart && (c === 'r' || (c === 'b' && src[i + 1] === 'r'))) {
      let j = i + (c === 'b' ? 2 : 1), hashes = 0;
      while (src[j] === '#') { hashes++; j++; }
      if (src[j] === '"') {
        const close = '"' + '#'.repeat(hashes);
        let k = src.indexOf(close, j + 1);
        const end = k < 0 ? n : k + close.length;
        tokens.push({ type: 'string', start: i, end, value: src.slice(j + 1, k < 0 ? n : k), raw: true, quote: '"' });
        i = end;
        continue;
      }
    }
    if (c === '"' || (identStart && c === 'b' && src[i + 1] === '"')) {
      const q = c === '"' ? i : i + 1;
      let j = q + 1;
      while (j < n && src[j] !== '"') j += src[j] === '\\' ? 2 : 1;
      const end = Math.min(j + 1, n);
      tokens.push({ type: 'string', start: i, end, value: unescapeRust(src.slice(q + 1, Math.min(j, n))), quote: '"' });
      i = end;
      continue;
    }
    if (c === "'") {
      // Char literal or lifetime.
      if (src[i + 1] === '\\') {
        const k = src.indexOf("'", i + 2);
        if (k > 0 && k - i < 12) { i = k + 1; continue; }
      } else {
        const width = src.codePointAt(i + 1) > 0xffff ? 2 : 1;
        if (src[i + 1 + width] === "'") { i += 2 + width; continue; }
      }
      i++;
      continue;
    }
    i++;
  }
  return tokens;
}

// ---------------------------------------------------------------- shared

export function lex(language, src) {
  if (language === 'js') return lexJs(src);
  if (language === 'py') return lexPy(src);
  if (language === 'rs') return lexRs(src);
  return [];
}

/** Line starts, for offset -> 1-based line conversion. */
export function lineIndex(src) {
  const starts = [0];
  for (let i = 0; i < src.length; i++) if (src.charCodeAt(i) === 10) starts.push(i + 1);
  return (offset) => {
    let lo = 0, hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= offset) lo = mid; else hi = mid - 1;
    }
    return lo + 1;
  };
}

function blank(chars, start, end, keepEdges) {
  for (let i = start + (keepEdges ? 1 : 0); i < end - (keepEdges ? 1 : 0); i++) {
    if (chars[i] !== '\n') chars[i] = ' ';
  }
}

/**
 * Lex `src` and return tokens plus two masked views with identical offsets:
 * `code` (comments and string/regex contents blanked) and `codeWithStrings`
 * (only comments blanked).
 */
export function analyze(language, src) {
  const tokens = lex(language, src).sort((a, b) => a.start - b.start);
  const code = src.split('');
  const codeWithStrings = src.split('');
  for (const token of tokens) {
    if (token.type === 'comment') {
      blank(code, token.start, token.end, false);
      blank(codeWithStrings, token.start, token.end, false);
    } else if (!token.nested) {
      blank(code, token.start, token.end, true);
    }
  }
  return { tokens, code: code.join(''), codeWithStrings: codeWithStrings.join(''), line: lineIndex(src) };
}
