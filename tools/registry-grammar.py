#!/usr/bin/env python3
"""Write the court and series names the grammar corpus reads from the registry.

The statute grammar (cite.ca.statute.first) reads each form of citation by the series names its defs list. Those
names live once, in the registry's law rows: a row's `forms` say which forms it is cited in (a revised or annual
statute series is cited by year and chapter when it says nothing), and its `french` which of its names are French.
This writes each form's names into its def, a dot allowed after each letter of an abbreviation and whitespace where a
name has a space. A court row's `bracketed` names are the neutral citations written after a bracketed year
("[2020] UKSC 5") and its `multiword` the forms of a neutral name of several words ("2005 Comp. Trib. 12"); a treaty
series' row names its forms (`treaty` by volume and page, "1155 UNTS 331"; `treaty_year` by year and number, "Can TS
1990 No 3"; `treaty_number` by number alone, "ETS No 5"), its names also written with a period after each letter. The curated cue defs must list registry names. This updates the corpus manifest; --check exits 1
when the corpus is stale.
"""
import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REGISTRY = ROOT / "crates/legal-citations/registry"
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
# Each def a court field lists the names of, as they are written.
COURT_DEFS = {"neutral_bracketed_courts": "bracketed", "neutral_multiword_courts": "multiword"}
# Each def a treaty series' form lists the names of: by volume and page, by year and number, by number alone.
TREATY_DEFS = {"treaty_series": "treaty", "treaty_year_series": "treaty_year", "treaty_number_series": "treaty_number"}
# The curated defs, whose every name a court or law row must answer to.
CURATED = ("cue_court_codes", "cue_statute_sources")
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


def as_written(name):
    """A name as it is written, its punctuation required and a space any whitespace."""
    return "".join(r"\s+" if token.isspace() else "\\" + token if token in SPECIAL else token
                   for token in re.findall(r"\s+|.", name))


def fold(surface):
    return "".join(character for character in surface if character.isalnum()).lower()


def defs(laws, courts, journals):
    names = {name: set() for name in [*DEFS, *COURT_DEFS, *TREATY_DEFS]}
    for row in laws:
        french = set(row.get("french", []))
        for surface in [row["abbreviation"], *row.get("variations", [])]:
            for name, (form, wanted) in DEFS.items():
                if form in forms(row) and (wanted is None or (surface in french) == wanted):
                    names[name].add(surface)
    for row in courts:
        for name, field in COURT_DEFS.items():
            names[name].update(row.get(field, []))
    for row in [*laws, *journals]:
        for name, form in TREATY_DEFS.items():
            if form not in row.get("forms", []):
                continue
            for surface in [row["abbreviation"], *row.get("variations", [])]:
                names[name].add(surface)
                if surface.isalpha():
                    names[name].add("".join(letter + "." for letter in surface))
    return {name: "|".join((pattern if name in DEFS else as_written)(surface)
                           for surface in sorted(surfaces, key=lambda value: (-len(value), value)))
            for name, surfaces in names.items()}


def unheld(corpus, laws, courts):
    """Each name a curated def lists that no court or law row answers to."""
    held = {fold(surface) for row in courts for surface in [row["id"], *row.get("neutral", []), *row.get("aliases", [])]}
    held |= {fold(surface) for row in laws for surface in [row["abbreviation"], *row.get("variations", [])]}
    return [f"{name}: {surface}" for table in json.loads(corpus)["tables"].values()
            for name, value in table.get("defs", {}).items() if name in CURATED
            for surface in value.split("|") if fold(surface) not in held]


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
    laws, courts, journals = (json.loads((REGISTRY / f"{name}.json").read_text(encoding="utf-8"))
                              for name in ("laws", "courts", "journals"))
    current = CORPUS.read_bytes().decode("utf-8").replace("\r\n", "\n")
    corpus = written(current, defs(laws, courts, journals))
    missing = unheld(corpus, laws, courts)
    if missing:
        sys.exit("curated defs list names no registry row answers to; add each to registry/*.json:\n" + "\n".join(missing))
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
