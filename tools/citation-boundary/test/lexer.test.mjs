import assert from 'node:assert/strict';
import { test } from 'node:test';
import { analyze, languageOf, lexJs, lexPy, lexRs } from '../lib/lexer.mjs';

const values = (tokens, type) => tokens.filter((token) => token.type === type).map((token) => token.value);

test('languageOf maps extensions and skips declaration files', () => {
  assert.equal(languageOf('a/b.tsx'), 'js');
  assert.equal(languageOf('a/b.mjs'), 'js');
  assert.equal(languageOf('a/b.py'), 'py');
  assert.equal(languageOf('a/b.rs'), 'rs');
  assert.equal(languageOf('a/b.d.ts'), null);
  assert.equal(languageOf('a/b.md'), null);
});

test('JS: regex literals versus division', () => {
  const tokens = lexJs('const a = x / y / 2;\nconst r = /\\d{4}\\s+SCC/g;\nif (ok) return /a[/]b/.test(s);');
  assert.deepEqual(values(tokens, 'regex'), ['\\d{4}\\s+SCC', 'a[/]b']);
  assert.equal(tokens.find((token) => token.type === 'regex').flags, 'g');
});

test('JS: strings, escapes, comments and nested templates', () => {
  const tokens = lexJs("// c1 'not a string'\nconst s = 'it\\'s'; /* c2 */ const t = `a ${b ? 'x' : `y${z}`} c`;\nconst raw = String.raw`\\d+`;");
  assert.deepEqual(values(tokens, 'comment').map((value) => value.trim()), ["c1 'not a string'", 'c2']);
  assert.ok(values(tokens, 'string').includes("it's"));
  assert.ok(values(tokens, 'string').includes('x'));
  const raw = tokens.find((token) => token.type === 'template' && token.raw);
  assert.equal(raw.value, '\\d+');
});

test('JS: an apostrophe in JSX text ends at the newline instead of swallowing the file', () => {
  const tokens = lexJs("const a = <p>Don't</p>;\nconst r = /\\d+/;");
  assert.deepEqual(values(tokens, 'regex'), ['\\d+']);
});

test('Python: prefixes, raw strings, f-strings and triple quotes', () => {
  const tokens = lexPy('x = r"\\d+" rf"{name}\\s{{2}}"  # comment "q"\ny = """doc\n"string" """\nz = b\'\\n\'');
  const strings = tokens.filter((token) => token.type === 'string');
  assert.equal(strings[0].value, '\\d+');
  assert.equal(strings[0].raw, true);
  assert.equal(strings[1].value, '{name}\\s{2}');
  assert.equal(strings[2].triple, true);
  assert.equal(strings[3].value, '\n');
  assert.deepEqual(values(tokens, 'comment'), [' comment "q"']);
});

test('Rust: raw strings, lifetimes, chars and nested block comments', () => {
  const tokens = lexRs("fn f<'a>(s: &'a str) -> char { let c = '\"'; let r = r#\"he said \"hi\"\"#; /* a /* b */ c */ '\\'' }\nlet x = \"\\\\d+\";");
  const strings = values(tokens, 'string');
  assert.deepEqual(strings, ['he said "hi"', '\\d+']);
  assert.equal(values(tokens, 'comment').length, 1);
});

test('analyze keeps offsets and masks comments and string contents', () => {
  const src = 'const a = "SCC"; // SCC\nconst b = 1;';
  const { code, codeWithStrings, line } = analyze('js', src);
  assert.equal(code.length, src.length);
  assert.ok(!code.includes('SCC'));
  assert.ok(codeWithStrings.includes('"SCC"'));
  assert.ok(!codeWithStrings.includes('// SCC'));
  assert.equal(line(src.indexOf('const b')), 2);
});
