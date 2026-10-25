#!/usr/bin/env python3
"""Write the legislation series names the grammar corpus reads from registry/laws.json.

The statute grammar (cite.ca.statute.first) reads each form of citation by the series names its defs list. Those
names live once, in the registry's law rows: a row's `forms` say which forms it is cited in (a revised or annual
statute series is cited by year and chapter when it says nothing), and its `french` which of its names are French.
This writes each form's names into its def, a dot allowed after each letter of an abbreviation and whitespace where a
name has a space, and updates the corpus manifest. --check exits 1 when the corpus is stale.
"""
import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LAWS = ROOT / "crates/legal-citations/registry/laws.json"
CORPUS = ROOT / "crates/legal-grammar/data/grammar-corpus.json"
MANIFEST = ROOT / "crates/legal-grammar/data/manifest.json"
# Each def and the names it lists: a form, and whether its French names or its others (both when None).
DEFS = {
    "ca_statute_series": ("statute", False),
    "fr_statute_series": ("statute", True),
    "ca_compiled_series": ("compiled", None),
    "ca_instrument_series": ("instrument", False),
    "fr_instrument_series": ("instrument", True),
    "ca_consolidated_series": ("consolidated", None),
    "ca_revised_regulation_series": ("revised_regulation", None),
}
SPECIAL = set("\\^$.|?*+()[]{}/")


def forms(row):
    if "forms" in row:
        return row["forms"]
    return ["statute"] if row["kind"] in ("revised_statutes", "annual_statutes") else []


def pattern(name):
    """A name as the grammar reads it: each word of capitals and the lower-case letters after one may be followed by a
    period ("R.S.C.", "S.Nu."), and a space is any whitespace or none."""
    output = []
    for token in re.findall(r"[A-Z][a-z]*|[a-z]+|\d+|\s+|.", name):
        if token.isspace():
            output.append(r"\s*")
        elif token[0].isalnum():
            output.append(token + r"\.?")
        else:
            output.append("\\" + token if token in SPECIAL else token)
    return "".join(output)


def defs(rows):
    names = {name: set() for name in DEFS}
    for row in rows:
        french = set(row.get("french", []))
        for surface in [row["abbreviation"], *row.get("variations", [])]:
            for name, (form, wanted) in DEFS.items():
                if form in forms(row) and (wanted is None or (surface in french) == wanted):
                    names[name].add(surface)
    return {name: "|".join(pattern(surface) for surface in sorted(surfaces, key=lambda value: (-len(value), value)))
            for name, surfaces in names.items()}


def written(corpus, values):
    """The corpus with each def's line holding its value, a def it lacks added after the series defs it joins."""
    for name, value in values.items():
        line = f'"{name}": {json.dumps(value, ensure_ascii=False)}'
        found = re.search(rf'^(\s*)"{name}": ".*?(?<!\\)",?$', corpus, re.M)
        if found:
            ending = "," if found.group(0).endswith(",") else ""
            corpus = corpus[:found.start()] + found.group(1) + line + ending + corpus[found.end():]
        else:
            anchor = re.search(r'^(\s*)"fr_statute_series": ".*?(?<!\\)",$', corpus, re.M)
            corpus = corpus[:anchor.end()] + "\n" + anchor.group(1) + line + "," + corpus[anchor.end():]
    return corpus


def manifest(corpus):
    data = json.loads(corpus)
    value = json.loads(MANIFEST.read_text(encoding="utf-8"))
    value["sha256"] = hashlib.sha256(corpus.replace("\r\n", "\n").encode("utf-8")).hexdigest()
    value["entries"] = sum(len(table.get("entries", [])) for table in data["tables"].values())
    value["vectors"] = sum(len(entry.get("vectors", [])) for table in data["tables"].values()
                           for entry in table.get("entries", []))
    return json.dumps(value, indent=2) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--check", action="store_true", help="exit 1 if the corpus is stale")
    args = parser.parse_args()
    rows = json.loads(LAWS.read_text(encoding="utf-8"))
    current = CORPUS.read_bytes().decode("utf-8")
    corpus = written(current, defs(rows))
    files = [(CORPUS, current, corpus), (MANIFEST, MANIFEST.read_text(encoding="utf-8"), manifest(corpus))]
    stale = [path for path, before, after in files if before.replace("\r\n", "\n") != after.replace("\r\n", "\n")]
    if args.check:
        if stale:
            sys.exit(f"stale: {', '.join(str(path.relative_to(ROOT)) for path in stale)}; run tools/registry-grammar.py")
        return
    for path, before, after in files:
        if path in stale:
            path.write_bytes(after.encode("utf-8"))
    print(f"{len(stale)} file(s) written")


if __name__ == "__main__":
    main()
