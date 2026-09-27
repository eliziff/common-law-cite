#!/usr/bin/env python3
"""Freeze the legacy citation data tables of the owner's repos into
crates/legal-citations/tests/fixtures/sources/*.json.

    python3 tools/extract-source-tables.py                 # repos beside this checkout (or $SOURCE_ROOT)
    python3 tools/extract-source-tables.py --root /path    # repos under /path
    python3 tools/extract-source-tables.py --check         # exit 1 if a fixture would change

The registry must stay a strict superset of every table extracted here
(tests/registry_coverage.rs). Each fixture holds one source table: a header
naming the repo, path, table and the commit the table was read at, and the
table's keys (court codes, reporter/journal abbreviations, series surfaces)
with whatever the source said about each (CanLII route, court level, the key
it is an alias of). The fixtures outlive the tables: consumer repos may delete
theirs once they read the registry instead.

A repo that is missing is skipped with a warning and its fixtures are left as
they are, so a partial checkout never deletes a frozen table. Only the Python
standard library is used; output is deterministic (no timestamps).
"""

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "crates" / "legal-citations" / "tests" / "fixtures" / "sources"
TOOL = "tools/extract-source-tables.py"


# ---------------------------------------------------------------- helpers

def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True,
                          text=True).stdout.strip()


def header(repo, path, table, note=None):
    repo_dir = repo["dir"]
    dirty = bool(git(repo_dir, "status", "--porcelain", "--", path))
    head = {"repo": repo["name"], "path": path, "table": table, "commit": git(repo_dir, "rev-parse", "HEAD")}
    if dirty:
        head["worktree_modified"] = True
    head["extracted_by"] = TOOL
    if note:
        head["note"] = note
    return head


def read(repo, path):
    return (repo["dir"] / path).read_text(encoding="utf-8")


def between(text, start, end, after=0):
    """The text between the first `start` at or after `after` and the next `end`."""
    begin = text.index(start, after) + len(start)
    return text[begin:text.index(end, begin)]


def words(text):
    return re.findall(r"[^\s,\"'\[\]]+", text)


def quoted(text):
    return re.findall(r"[\"']([^\"']+)[\"']", text)


def split_top(pattern):
    """Top-level `|` alternatives of a regex (escapes and groups respected)."""
    out, cur, depth, i = [], "", 0, 0
    while i < len(pattern):
        c = pattern[i]
        if c == "\\":
            cur += pattern[i:i + 2]
            i += 2
            continue
        if c in "([":
            depth += 1
        elif c in ")]":
            depth -= 1
        if c == "|" and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += c
        i += 1
    out.append(cur)
    return out


def unwrap(pattern):
    """`(?:a|b)` -> `a|b` when the group spans the whole pattern."""
    if pattern.startswith("(?:") and pattern.endswith(")"):
        depth = 0
        for i, c in enumerate(pattern):
            if c == "(" and (i == 0 or pattern[i - 1] != "\\"):
                depth += 1
            elif c == ")" and pattern[i - 1] != "\\":
                depth -= 1
                if depth == 0 and i != len(pattern) - 1:
                    return pattern
        return pattern[3:-1]
    return pattern


def literal(alternative):
    """A regex alternative made of escaped literals and `\\s*` -> its text,
    or None when it contains real regex structure."""
    text = re.sub(r"\\s[*+?]?", " ", alternative)
    text = text.replace("\\.?", ".")
    if re.search(r"(?<!\\)[()\[\]?*+{}^$|]", text):
        return None
    text = re.sub(r"\\(.)", r"\1", text)
    return re.sub(r"\s+", " ", text).strip()


def expand(pattern):
    """Every string of a small regex made of literals, `\\.?`, `\\s+`,
    `(?:a|b)` groups and `(?:x)?` optionals (dots and spaces are kept as
    written; the registry folds them away)."""
    return list(dict.fromkeys(re.sub(r"\s+", " ", r).strip() for r in _expand(pattern)))


def _expand(pattern):
    pattern = unwrap(pattern)
    alternatives = split_top(pattern)
    if len(alternatives) > 1:
        return [s for alternative in alternatives for s in _expand(alternative)]
    results = [""]
    i = 0
    while i < len(pattern):
        c = pattern[i]
        if c == "(":
            depth, j = 0, i
            while True:
                if pattern[j] == "\\":
                    j += 2
                    continue
                if pattern[j] == "(":
                    depth += 1
                elif pattern[j] == ")":
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            inner = pattern[i:j + 1]
            options = _expand(inner)
            j += 1
            if j < len(pattern) and pattern[j] == "?":
                options = [""] + options
                j += 1
            results = [r + o for r in results for o in options]
            i = j
            continue
        if c == "\\":
            token = pattern[i:i + 2]
            i += 2
            char = " " if token[1] == "s" else token[1]
            if i < len(pattern) and pattern[i] in "?*+":
                i += 1
                char = " " if token[1] == "s" else char  # optional punctuation kept as written
            results = [r + char for r in results]
            continue
        if c in "?*+":
            i += 1
            continue
        if c == "[":
            j = pattern.index("]", i)
            chars = pattern[i + 1:j]
            results = [r + ch for r in results for ch in chars]
            i = j + 1
            continue
        results = [r + c for r in results]
        i += 1
    return results


def dedupe(entries):
    seen, out = set(), []
    for entry in entries:
        marker = json.dumps(entry, sort_keys=True)
        if marker not in seen:
            seen.add(marker)
            out.append(entry)
    return out


def fixture(name, head, kind, entries, **extra):
    body = {"source": head, "kind": kind}
    body.update(extra)
    body["entries"] = dedupe(entries)
    return name, body


# ---------------------------------------------------------------- Beaver

def beaver(repo):
    out = []
    path = "backend/src/lib/canliiUrls.ts"
    text = read(repo, path)
    block = between(text, "export const A2AJ_CANLII_COURT_ROUTES", "\n};")
    routes = {}
    for jurisdiction, codes in re.findall(r"\b([a-z]{2}): `([^`]*)`", block):
        for code in codes.split():
            routes[code] = f"{jurisdiction}/{code.lower()}"
    for code, route in re.findall(r"^\s+([A-Z0-9-]+): \"([^\"]+)\",", block, re.M):
        routes[code] = route
    out.append(fixture("beaver--canlii-court-routes", header(repo, path, "A2AJ_CANLII_COURT_ROUTES"), "court",
                       [{"key": k, "canlii": v} for k, v in routes.items()]))

    path = "backend/src/lib/courtLevels.ts"
    block = between(read(repo, path), "const LEVELS", "\n};")
    out.append(fixture("beaver--court-levels", header(repo, path, "LEVELS"), "court",
                       [{"key": k, "level": v} for k, v in re.findall(r"^\s+([A-Z0-9-]+): \{ level: \d, kind: \"(\w+)\"",
                                                                    block, re.M)]))

    path = "shared/court-registry.json"
    data = json.loads(read(repo, path))
    courts = []
    for court in data["courts"]:
        if court["jurisdictionId"] == "general":
            continue  # the "No preset" placeholder row is not a court
        for code in court["abbreviation"].split("/"):  # `FC/FCA` offers either court
            courts.append({"key": code, "name": court["label"], "source_id": court["id"]})
    out.append(fixture("beaver--court-registry-courts", header(repo, path, "courts"), "court", courts))
    out.append(fixture("beaver--court-registry-jurisdictions", header(repo, path, "jurisdictions"), "jurisdiction",
                       [{"key": j["id"], "name": j["label"]} for j in data["jurisdictions"] if j["id"] != "general"]))

    path = "backend/src/lib/canliiLawUrls.ts"
    codes = re.search(r"\(LEGISLATION\|REGULATIONS\)-\(([A-Z|]+)\)", read(repo, path)).group(1).split("|")
    out.append(fixture("beaver--canlii-law-jurisdictions", header(repo, path, "buildCanliiLawUrl dataset jurisdictions"),
                       "jurisdiction", [{"key": c, "canlii": "ca" if c == "FED" else c.lower()} for c in codes]))

    path = "backend/src/lib/legalSources/a2aj.ts"
    text = read(repo, path)
    block = between(text, "const JURISDICTIONS = {", "}")
    out.append(fixture("beaver--a2aj-jurisdictions", header(repo, path, "JURISDICTIONS"), "jurisdiction",
                       [{"key": k, "name": v} for k, v in re.findall(r"\b([A-Z]{2,3}): \"([^\"]+)\"", block)]))
    block = between(text, "const TRIBUNALS = new Set([", "]);")
    out.append(fixture("beaver--a2aj-tribunals", header(repo, path, "TRIBUNALS"), "court",
                       [{"key": k, "level": "tribunal"} for k in quoted(block)]))

    path = "backend/experiments/deterministic-library-analysis/legalTextAnchors.ts"
    text = read(repo, path)
    out.append(fixture("beaver--anchors-ca-statute-series", header(repo, path, "CA_STATUTE_SERIES"), "series",
                       [{"key": k} for k in quoted(between(text, "const CA_STATUTE_SERIES = [", "];"))]))
    out.append(fixture("beaver--anchors-fr-statute-series", header(repo, path, "FR_STATUTE_SERIES"), "series",
                       [{"key": k, "same_as": v.upper()} for k, v in
                        re.findall(r"(\w+): \"(\w+)\"", between(text, "const FR_STATUTE_SERIES", "};"))]))
    out.append(fixture("beaver--anchors-instrument-registers", header(repo, path, "INSTRUMENT_REGISTERS"), "series",
                       [{"key": k.upper(), "same_as": v.upper()} for k, v in
                        re.findall(r"(\w+): \"(\w+)\"", between(text, "const INSTRUMENT_REGISTERS", "};"))]))
    neutral = re.search(r"const CITE_NEUTRAL_RE =\s*/.*?\\s\+\(([A-Z|]+)\)", text, re.S).group(1)
    out.append(fixture("beaver--anchors-neutral-courts", header(repo, path, "CITE_NEUTRAL_RE courts"), "court",
                       [{"key": k} for k in neutral.split("|")]))
    out.append(fixture("beaver--anchors-fr-courts", header(repo, path, "FR_COURTS"), "court",
                       [{"key": k.upper(), "same_as": v.upper()} for k, v in
                        re.findall(r"(\w+): \"(\w+)\"", between(text, "const FR_COURTS", "};"))]))
    us = re.search(r"const CITE_US_REPORTER_RE =\s*/\\b\(\\d\{1,4\}\)\\s\+\((.*)\)\\s\+\(\\d\{1,5\}\)\\b/gu;", text).group(1)
    out.append(fixture("beaver--anchors-us-reporters", header(repo, path, "CITE_US_REPORTER_RE reporters"), "reporter",
                       [{"key": k} for k in expand(us)]))

    path = "backend/scripts/build_citator_graph.py"
    text = read(repo, path)
    statute = between(text, "STATUTE_RE = re.compile(", "\n)")
    groups = re.findall(r"\(\?:([A-Za-z |/]+)\)", statute.replace('"\n    r"', "").replace("\"\n    r\"", ""))
    joined = "".join(re.findall(r"r\"([^\"]*)\"", statute))
    keys = [k for group in re.findall(r"\(\?:([A-Za-z |/]+)\)", joined) for k in group.split("|")]
    out.append(fixture("beaver--citator-graph-statute-series", header(repo, path, "STATUTE_RE series"), "series",
                       [{"key": k} for k in keys]))

    path = "packages/alr-quote-splitter/deterministic_splitter.py"
    joined = "".join(re.findall(r"r\"([^\"]*)\"", between(read(repo, path), "_STATUTE_RE = re.compile(", "\n)")))
    keys = [k for group in re.findall(r"\(\?:([A-Za-z |/]+)\)", joined) for k in group.split("|")]
    out.append(fixture("beaver--alr-quote-splitter-statute-series", header(repo, path, "_STATUTE_RE series"), "series",
                       [{"key": k} for k in keys]))

    path = "experiments/a2aj_decision_roster_qwen/scrape_judge_registry.py"
    text = read(repo, path)
    entries = []
    for cid, name, aliases, datasets in re.findall(
            r"^\s+\"(\w+)\": \(\"([^\"]+)\", \[([^\]]*)\], \[([^\]]*)\]\),", between(text, "COURTS = {", "\n}"), re.M):
        primary = quoted(datasets)[0]
        for surface in quoted(aliases) + quoted(datasets):
            entries.append({"key": surface, "same_as": primary, "name": name, "source_id": cid})
    for cid, code in re.findall(r"^\s+\"(\w+)\": \(\"([^\"]+)\", lambda",
                                between(text, "FEDERAL_PROFILES = {", "\n}"), re.M):
        target = next((e["same_as"] for e in entries if e["source_id"] == cid), None)
        entries.append({"key": code, **({"same_as": target} if target else {}), "source_id": cid,
                        "note": "FEDERAL_PROFILES organisation code"})
    out.append(fixture("beaver--judge-registry-courts", header(repo, path, "COURTS, FEDERAL_PROFILES"), "court",
                       entries))

    for name, path, start, end, table in [
        ("beaver--case-treatment-datasets", "backend/experiments/a2aj-case-treatment/cli.ts",
         "const COURT_DATASETS = [", "]", "COURT_DATASETS"),
        ("beaver--semantic-mvp-datasets", "experiments/a2aj_decision_roster_qwen/scratch/semantic_mvp_candidates.ts",
         "const datasets = [", "]", "datasets"),
        ("beaver--probe-class1-surfaces-courts", "benchmarks/structure_stress/probes/cite_class1_surfaces.py",
         "COURTS = [", "]", "COURTS"),
        ("beaver--probe-class2-statutes-courts", "benchmarks/structure_stress/probes/cite_class2_statutes.py",
         "COURTS = [", "]", "COURTS"),
        ("beaver--probe-sample-texts-courts", "benchmarks/structure_stress/probes/cite_sample_texts.py",
         "COURTS = [", "]", "COURTS"),
    ]:
        out.append(fixture(name, header(repo, path, table), "court",
                           [{"key": k} for k in quoted(between(read(repo, path), start, end))]))
    path = "benchmarks/structure_stress/probes/cite_class1_propose.py"
    out.append(fixture("beaver--probe-class1-propose-courts", header(repo, path, "ANCHORS_COURTS"), "court",
                       [{"key": k} for k in " ".join(quoted(between(read(repo, path), "ANCHORS_COURTS = (", ")"))).split()]))
    path = "benchmarks/structure_stress/probes/cite_class2_statutes.py"
    text = read(repo, path)
    out.append(fixture("beaver--probe-class2-statutes-series", header(repo, path, "CA_STATUTE_SERIES, FR_STATUTE_SERIES"),
                       "series", [{"key": k} for k in quoted(between(text, "CA_STATUTE_SERIES = [", "]"))
                                  + quoted(between(text, "FR_STATUTE_SERIES = [", "]"))]))
    return out


# ---------------------------------------------------------------- legal-pinpointer

def pinpointer(repo):
    out = []
    path = "canlii-courts.js"
    text = read(repo, path)
    routes = re.findall(r"^\s+'?([A-Z0-9-]+)'?: '([^']+)',?$", between(text, "const routes = Object.freeze({", "});"), re.M)
    french = re.findall(r"^\s+'?([A-Z0-9-]+)'?: '([^']+)',?$", between(text, "const frenchRoutes = Object.freeze({", "});"), re.M)
    out.append(fixture("pinpointer--canlii-court-routes", header(repo, path, "routes"), "court",
                       [{"key": k, "canlii": v} for k, v in routes]))
    out.append(fixture("pinpointer--canlii-court-french-routes", header(repo, path, "frenchRoutes"), "court",
                       [{"key": k, "canlii_fr": v} for k, v in french]))

    path = "canlii-legislation.js"
    text = read(repo, path)
    jurisdictions = dict(re.findall(r"\['(\w+)', '(\w+)'\]", between(text, "const SERIES_JURISDICTIONS = new Map([", "]);")))
    series = quoted(between(text, "const SERIES = new Set([", "]);"))
    out.append(fixture("pinpointer--legislation-series", header(repo, path, "SERIES, SERIES_JURISDICTIONS"), "series",
                       [{"key": k, **({"canlii_jurisdiction": jurisdictions[k]} if k in jurisdictions else {})}
                        for k in series]))
    prefixes = re.findall(r"\['(\w+)', '(\w+)'\]", between(text, "const REGULATION_PREFIXES = new Map([", "]);"))
    out.append(fixture("pinpointer--regulation-prefixes", header(repo, path, "REGULATION_PREFIXES",
                                                                "Read as `<key> Reg`, the form the source matches."),
                       "series", [{"key": f"{k} Reg", "same_as": f"{v} Reg"} for k, v in prefixes]))

    path = "core.js"
    text = read(repo, path)
    start = re.search(r"const citationStart = /(.*)/;", text).group(1)
    tokens = ["CCSM", "CPLM", "CQLR", "RLRQ", "SOR", "SI", "DORS", "TR", "CRC"]
    assert all(t in start.replace("\\.?", "") for t in tokens), "core.js citationStart changed"
    out.append(fixture("pinpointer--core-legislation-heading-series", header(repo, path, "splitLegislationHeading citationStart"),
                       "series", [{"key": t} for t in tokens],
                       note="Closed series tokens of the heading pattern; the open `[A-Z.]{2,8}` and `X Reg` arms are patterns."))
    not_reporter = re.search(r"const notReporter = /(.*)/i;", text).group(1)
    services = re.search(r"\\b\(\?:([A-Za-z|]+)\)\\b", not_reporter).group(1).split("|")
    out.append(fixture("pinpointer--core-not-reporter", header(repo, path, "reporterCandidates notReporter"),
                       "reporter_exclusion", [{"key": k} for k in ["Carswell"] + services]))
    score = re.search(r"/\\b\(\?:([A-Z|]+)\)\\b/\.test\(cite\)\) score \+= 30", text).group(1).split("|")
    out.append(fixture("pinpointer--core-reporter-score", header(repo, path, "reporterScore"), "reporter",
                       [{"key": k} for k in score]))
    for table, label in [("const equivalents = {", "chooseCaseCitation equivalents"),
                         ("const englishToFrench = {", "canliiCourtRoute englishToFrench")]:
        pairs = re.findall(r"(\w+): '(\w+)'", between(text, table, "}"))
        out.append(fixture(f"pinpointer--core-{'equivalents' if 'equivalents' in table else 'english-to-french'}",
                           header(repo, path, label), "court", [{"key": k, "same_as": v} for k, v in pairs]))

    path = "a2aj-index/build/datasets.json"
    data = json.loads(read(repo, path))
    codes = [c for c in data["include"] if "-" not in c] + [c.strip() for c in re.sub(r"\([^)]*\)", "", data["notIncluded"]).split(",")]
    out.append(fixture("pinpointer--a2aj-index-datasets", header(repo, path, "include, notIncluded"), "court",
                       [{"key": c} for c in codes]))
    path = "a2aj-index/src/page.mjs"
    block = between(read(repo, path), "const COURTS = {", "};")
    out.append(fixture("pinpointer--a2aj-index-courts", header(repo, path, "COURTS"), "court",
                       [{"key": k, "name": v} for k, v in re.findall(r"(\w+): '([^']+)'", block)]))
    return out


# ---------------------------------------------------------------- legal-citations grammar corpus

def grammar(repo):
    out = []
    path = "crates/legal-grammar/data/grammar-corpus.json"
    citations = json.loads(read(repo, path))["tables"]["citations"]
    defs = citations["defs"]
    rules = {entry["id"]: entry for entry in citations["entries"]}
    out.append(fixture("grammar--reporter-words", header(repo, path, "defs.reporter_words"), "pattern", [],
                       pattern=defs["reporter_words"],
                       note="An open character-class pattern: it names no reporter, so there is nothing to enumerate."))
    fr_map = rules["cite.statute.judgment.fr"]["canonical"]["map"]["series"]
    for name, key in [("grammar--ca-statute-series", "ca_statute_series"), ("grammar--fr-statute-series", "fr_statute_series")]:
        entries = []
        for alternative in split_top(defs[key]):
            surface = alternative.replace("\\.?", "")
            entry = {"key": surface}
            if surface.lower() in fr_map:
                entry["same_as"] = fr_map[surface.lower()].upper()
            entries.append(entry)
        out.append(fixture(name, header(repo, path, f"defs.{key}"), "series", entries))
    for name, key, kind in [("grammar--us-reporters", "us_reporters", "reporter"),
                            ("grammar--us-journals", "us_journals", "journal")]:
        keys = [literal(a) for a in split_top(unwrap(defs[key]))]
        assert all(keys), key
        out.append(fixture(name, header(repo, path, f"defs.{key}"), kind, [{"key": k} for k in keys]))
    laws = []
    structural = {"§§?", "No.", "Number", "§", "Sec", "sec", "Section", "section", "U.S.C.", "USC"}
    for alternative in split_top(unwrap(defs["us_laws"])):
        for found in re.finditer(r"\(\?:((?:[^()]|\\\(|\\\))+)\)", alternative):
            before = alternative[:found.start()]
            if before.endswith(("(?:", "]", ")")):
                continue  # a fragment of a larger token (`[Ss](?:(?:ec)(?:tion)?)?`)
            group = found.group(1)
            names = [literal(a) for a in split_top(group)]
            if not all(names) or not any(re.search(r"[A-Za-z]{2}", n or "") for n in names):
                continue
            if set(names) <= {"No.", "Number", "§", "Sec", "sec", "Section", "section"}:
                continue
            laws.extend(n for n in names if n not in ("§§?",))
    out.append(fixture("grammar--us-laws", header(repo, path, "defs.us_laws"), "series", [{"key": k} for k in laws],
                       note="The literal code/register names inside each us_laws alternative."))

    neutral_map = rules["cite.neutral"]["canonical"]["map"]["court"]
    out.append(fixture("grammar--cite-neutral-court-map", header(repo, path, "cite.neutral canonical.map.court"), "court",
                       [{"key": k.upper(), "same_as": v.upper()} for k, v in neutral_map.items()]))
    tribunal = rules["cite.neutral.tribunal"]
    alternation = re.search(r"\(\?<court>(.*?)\)\\s\+\(\?<num>", tribunal["pattern"]).group(1)
    tmap = tribunal["canonical"]["map"]["court"]
    entries = [{"key": s} for s in expand(alternation)]
    entries += [{"key": k, "same_as": v} for k, v in tmap.items()]
    out.append(fixture("grammar--cite-neutral-tribunal-courts", header(repo, path, "cite.neutral.tribunal court"),
                       "court", entries))
    bracketed = rules["cite.neutral.bracketed"]["pattern"]
    alternation = re.search(r"\(\?<court>(.*?)\)\\s\+\(\?<num>", bracketed).group(1)
    out.append(fixture("grammar--cite-neutral-bracketed-courts", header(repo, path, "cite.neutral.bracketed court"),
                       "court", [{"key": s} for s in expand(alternation)]))
    judgment = [{"key": a.replace("\\.?", "")} for a in split_top(defs["ca_statute_series"])]
    out.append(fixture("grammar--cite-statute-judgment-series", header(repo, path, "cite.statute.judgment series"),
                       "series", judgment))
    out.append(fixture("grammar--cite-statute-judgment-fr-map", header(repo, path, "cite.statute.judgment.fr canonical.map.series"),
                       "series", [{"key": k.upper(), "same_as": v.upper()} for k, v in fr_map.items()]))
    for rule in ("cite.statute.splitter", "cite.statute.toa"):
        keys = [k for group in re.findall(r"\(\?:([A-Za-z |/]+)\)", rules[rule]["pattern"]) for k in group.split("|")]
        out.append(fixture(f"grammar--{rule.replace('.', '-')}-series", header(repo, path, f"{rule} series"),
                           "series", [{"key": k} for k in keys]))
    return out


# ---------------------------------------------------------------- legal-pdf-parser

def pdf_parser(repo):
    out = []
    path = "data/mcgill_reporters.json"
    out.append(fixture("pdfparser--mcgill-reporters", header(repo, path, "McGill reporter inventory"),
                       "reporter_or_journal", [{"key": k} for k in json.loads(read(repo, path))]))
    path = "legal-pdf-support/src/pairing_support.rs"
    text = read(repo, path)
    const = lambda name: re.search(rf"const {name}: &str =\s*r?\"(.*?)\";", text, re.S).group(1)
    out.append(fixture("pdfparser--pairing-court-codes", header(repo, path, "COURT_CODE_PATTERN"), "court",
                       [{"key": k} for k in const("COURT_CODE_PATTERN").split("|")]))
    reporters, folded = [], set()
    for alternative in split_top(const("REPORTER_TOKEN_PATTERN")):
        for variant in expand(alternative):  # `Ch(?:\s+D)?` is both `Ch` and `Ch D`
            fold = re.sub(r"[^0-9a-z]", "", variant.lower())
            if fold not in folded:
                folded.add(fold)
                reporters.append({"key": variant})
    out.append(fixture("pdfparser--pairing-reporter-tokens", header(repo, path, "REPORTER_TOKEN_PATTERN"), "reporter",
                       reporters))
    out.append(fixture("pdfparser--pairing-statute-sources", header(repo, path, "STATUTE_PATTERN"), "series",
                       [{"key": k} for k in const("STATUTE_PATTERN").split("|")]))
    return out


# ---------------------------------------------------------------- AuthoritiesHelper

def authorities(repo):
    path = "web/court-profiles.json"
    profiles = json.loads(read(repo, path))["profiles"]
    return [fixture("authorities--court-profiles", header(repo, path, "profiles"), "court",
                    [{"key": p["id"], "name": p["label"]} for p in profiles if p["id"] != "general"])]


REPOS = [("Beaver", beaver), ("legal-pinpointer", pinpointer), ("legal-citations", grammar),
         ("legal-pdf-parser", pdf_parser), ("AuthoritiesHelper", authorities)]


def render(body):
    return json.dumps(body, ensure_ascii=False, indent=1) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--root", default=os.environ.get("SOURCE_ROOT", str(ROOT.parent)),
                        help="directory holding the source repos (default: $SOURCE_ROOT, else this checkout's parent)")
    parser.add_argument("--check", action="store_true", help="exit 1 if any fixture would change")
    args = parser.parse_args()
    stale = []
    OUT.mkdir(parents=True, exist_ok=True)
    for name, extract in REPOS:
        directory = ROOT if name == "legal-citations" else Path(args.root) / name
        if not (directory / ".git").exists():
            print(f"warning: {directory} is not a checkout; keeping its fixtures", file=sys.stderr)
            continue
        try:
            tables = extract({"name": name, "dir": directory})
        except (OSError, ValueError, AttributeError, KeyError, IndexError, StopIteration) as error:
            # A table the repo no longer has (or reshaped): its frozen fixtures stay.
            print(f"warning: {name}: {error!r}; keeping its fixtures", file=sys.stderr)
            continue
        for fixture_name, body in tables:
            path = OUT / f"{fixture_name}.json"
            text = render(body)
            if not path.exists() or path.read_text(encoding="utf-8") != text:
                stale.append(fixture_name)
                if not args.check:
                    path.write_text(text, encoding="utf-8")
            print(f"{fixture_name}: {len(body['entries'])} keys")
    if args.check and stale:
        sys.exit(f"stale fixtures: {', '.join(stale)}")


if __name__ == "__main__":
    main()
