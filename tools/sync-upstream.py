#!/usr/bin/env python3
"""Regenerate crates/legal-citations/registry/upstream/*.json from Free Law
Project's reporters-db and courts-db at the versions pinned in upstream.lock.

    python3 tools/sync-upstream.py            # fetch the pinned files, regenerate
    python3 tools/sync-upstream.py --check    # regenerate in memory, exit 1 on drift
    python3 tools/sync-upstream.py --update   # move the pins to upstream HEAD, then regenerate
    python3 tools/sync-upstream.py --source reporters-db=/path/to/clone --source courts-db=/path

Only the Python standard library is used, so a scheduled CI job can run it
with a stock interpreter. Files are fetched from raw.githubusercontent.com at
the pinned commit and verified against the sha256 recorded in the lock; a
`--source` checkout is read as-is (its commit is not verified). Output is
sorted and byte-for-byte deterministic.

Mapping (upstream -> registry schema, see crates/legal-citations/src/registry.rs):

reporters-db reporters.json -> upstream/reporters.json (Reporter)
    id        slug of the reporter key without periods (`U.S.` -> `us`, `F. Supp.` -> `f-supp`);
              `-us`/`-uk` is appended when it collides with an authored id
              (`or` is Ontario Reports, so Oregon Reports is `or-us`), and
              `-2`, `-3`... for several reporters under one key.
    kind      cite_type: state, neutral, scotus_early, federal `U.S.` -> official;
              federal, state_regional -> general; specialty -> specialty;
              specialty_west, specialty_lexis -> database.
    jurisdiction  `us-xx` when every mlz_jurisdiction names state xx, `uk-ew`
              for English entries, else `us`.
    editions  name + start/end years; variations kept; regexes, examples,
              notes, href, publisher and cite_format dropped.
reporters-db laws.json -> upstream/series.json (Series)
    leg_statute -> code; leg_session -> annual_statutes;
    admin_compilation, admin_register -> regulations. admin_docket,
    admin_filing, municipal and leg_act entries are not statute or
    regulation series and are skipped.
reporters-db journals.json -> upstream/journals.json (Journal)
courts-db courts.json -> upstream/courts.json (Court)
    id        courts-db id, unchanged (CourtListener uses the same ids).
    jurisdiction  `us` for federal courts; `us-xx` from `location` for state,
              territorial and tribal courts; English courts -> `uk-ew`; other
              international entries -> `int`.
    level     scotus and colr -> apex; iac -> appellate; gjc -> superior_trial;
              ljc -> inferior_trial; otherwise by type: appellate -> appellate,
              trial -> superior_trial, bankruptcy -> inferior_trial, anything
              else (special, ag, unset) -> tribunal.
    aliases   citation_string and name_abbreviation (e.g. `2d Cir.`).
    start/end earliest start and latest end year across `dates`; both are
              dropped when they contradict each other.
    parent    kept (courts-db parents always name another courts-db court).
"""

import argparse
import hashlib
import json
import re
import sys
import tomllib
import unicodedata
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOCK = ROOT / "upstream.lock"
REGISTRY = ROOT / "crates" / "legal-citations" / "registry"
OUT = REGISTRY / "upstream"

FILES = {
    "reporters-db": ["reporters_db/data/reporters.json", "reporters_db/data/laws.json",
                     "reporters_db/data/journals.json"],
    "courts-db": ["courts_db/data/courts.json", "courts_db/data/states.json"],
}
VERSION_FILE = "pyproject.toml"


# ---------------------------------------------------------------- fetching

def http_get(url):
    request = urllib.request.Request(url, headers={"User-Agent": "legal-citations-sync-upstream"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def read_lock():
    return tomllib.loads(LOCK.read_text(encoding="utf-8"))


def write_lock(lock):
    lines = [
        "# Pinned upstream data for crates/legal-citations/registry/upstream/.",
        "# Regenerate with `python3 tools/sync-upstream.py`; move the pins with `--update`.",
        "# Both projects are BSD-2-Clause (see NOTICE).",
        "",
    ]
    for name in sorted(lock):
        entry = lock[name]
        lines += [f"[{name}]",
                  f'repository = "{entry["repository"]}"',
                  f'version = "{entry["version"]}"',
                  f'commit = "{entry["commit"]}"',
                  f'license = "{entry["license"]}"',
                  f"[{name}.sha256]"]
        for path in sorted(entry["sha256"]):
            lines.append(f'"{path}" = "{entry["sha256"][path]}"')
        lines.append("")
    LOCK.write_text("\n".join(lines), encoding="utf-8")


def fetch(lock, sources):
    """Return {project: {path: bytes}} for every pinned file."""
    data = {}
    for name, paths in FILES.items():
        entry = lock[name]
        data[name] = {}
        for path in paths:
            if name in sources:
                blob = (Path(sources[name]) / path).read_bytes()
            else:
                url = f"https://raw.githubusercontent.com/{entry['repository']}/{entry['commit']}/{path}"
                blob = http_get(url)
                expected = entry.get("sha256", {}).get(path)
                actual = hashlib.sha256(blob).hexdigest()
                if expected and expected != actual:
                    sys.exit(f"{name}/{path}: sha256 {actual} does not match upstream.lock {expected}")
            data[name][path] = blob
    return data


def update_lock(lock):
    for name in FILES:
        entry = lock[name]
        head = json.loads(http_get(f"https://api.github.com/repos/{entry['repository']}/commits/HEAD"))
        commit = head["sha"]
        pyproject = http_get(
            f"https://raw.githubusercontent.com/{entry['repository']}/{commit}/{VERSION_FILE}").decode()
        version = re.search(r'^version\s*=\s*"([^"]+)"', pyproject, re.M).group(1)
        entry["commit"], entry["version"] = commit, version
        entry["sha256"] = {}
        for path in FILES[name]:
            blob = http_get(f"https://raw.githubusercontent.com/{entry['repository']}/{commit}/{path}")
            entry["sha256"][path] = hashlib.sha256(blob).hexdigest()
    write_lock(lock)


# ---------------------------------------------------------------- conversion helpers

def slug(value):
    value = unicodedata.normalize("NFKD", value)
    value = "".join(c for c in value if not unicodedata.combining(c))
    value = re.sub(r"[.'’]", "", value)  # `U.S.` -> `us`, `F. Supp.` -> `f-supp`
    return re.sub(r"[^A-Za-z0-9]+", "-", value).strip("-").lower() or "x"


def year(value):
    return int(value[:4]) if value else None


def authored_ids(table):
    path = REGISTRY / table
    return {row["id"] for row in json.loads(path.read_text(encoding="utf-8"))} if path.exists() else set()


def unique_id(base, taken, suffix):
    """`base`, else `base-suffix`, else the first free `-2`, `-3`... form."""
    candidates = [base] + ([f"{base}-{suffix}"] if suffix else [])
    for candidate in candidates:
        if candidate not in taken:
            taken.add(candidate)
            return candidate
    n = 2
    while f"{candidates[-1]}-{n}" in taken:
        n += 1
    taken.add(f"{candidates[-1]}-{n}")
    return f"{candidates[-1]}-{n}"


def state_codes(states_blob):
    by_name = {}
    for code, name in json.loads(states_blob).items():
        if len(code) == 2:
            by_name[name.lower()] = code.lower()
    by_name.update({"washington d.c.": "dc", "district of columbia": "dc"})
    return by_name


def mlz_jurisdiction(values, codes):
    states = set()
    for value in values:
        head = value.split(";")[0]
        parts = head.split(":")
        if parts[0] in ("gb", "en"):
            states.add("uk-ew")
        elif parts[0] == "us" and len(parts) > 1 and parts[1] in codes:
            states.add(f"us-{parts[1]}")
        else:
            states.add("us")
    return states.pop() if len(states) == 1 else "us"


REPORTER_KIND = {
    "state": "official", "neutral": "official", "scotus_early": "official",
    "federal": "general", "state_regional": "general", "specialty": "specialty",
    "specialty_west": "database", "specialty_lexis": "database",
}
SERIES_KIND = {
    "leg_statute": "code", "leg_session": "annual_statutes",
    "admin_compilation": "regulations", "admin_register": "regulations",
}


def convert_reporters(blob, codes):
    taken = authored_ids("reporters.json")
    upstream_taken = set(taken)
    rows = []
    for key, entries in sorted(json.loads(blob).items()):
        for entry in entries:
            jurisdiction = mlz_jurisdiction(entry.get("mlz_jurisdiction", []), codes)
            base = slug(key)
            if base in taken:  # an authored reporter owns the plain id
                base = f"{base}-{jurisdiction.split('-')[0]}"
            rid = unique_id(base, upstream_taken, "")
            kind = "official" if key == "U.S." else REPORTER_KIND.get(entry.get("cite_type"), "specialty")
            editions = []
            for abbreviation, span in sorted(entry["editions"].items(),
                                             key=lambda item: (item[1].get("start") or "", item[0])):
                edition = {"abbreviation": abbreviation}
                if year(span.get("start")):
                    edition["start"] = year(span["start"])
                if year(span.get("end")):
                    edition["end"] = year(span["end"])
                editions.append(edition)
            row = {"id": rid, "name": {"en": entry["name"]}, "kind": kind, "jurisdiction": jurisdiction,
                   "editions": editions}
            variations = entry.get("variations") or {}
            if variations:
                row["variations"] = dict(sorted(variations.items()))
            row["year_volume"] = False
            row["source"] = "reporters-db"
            rows.append(row)
    return rows


def convert_laws(blob, codes):
    taken = authored_ids("series.json")
    rows = []
    for key, entries in sorted(json.loads(blob).items()):
        for entry in entries:
            kind = SERIES_KIND.get(entry.get("cite_type"))
            if not kind:
                continue
            place = (entry.get("jurisdiction") or "").lower()
            jurisdiction = "us" if place in ("", "united states") else (
                f"us-{codes[place]}" if place in codes else "us")
            row = {"id": unique_id(slug(key), taken, "us"), "name": {"en": entry["name"]}, "kind": kind,
                   "jurisdiction": jurisdiction, "abbreviation": key}
            variations = sorted(set(entry.get("variations") or []))
            if variations:
                row["variations"] = variations
            rows.append(row)
    return rows


def convert_journals(blob):
    taken = authored_ids("journals.json")
    rows = []
    for key, entries in sorted(json.loads(blob).items()):
        for entry in entries:
            row = {"id": unique_id(slug(key), taken, "us"), "name": entry["name"], "abbreviation": key}
            variations = sorted(set(entry.get("variations") or []))
            if variations:
                row["variations"] = variations
            row["source"] = "reporters-db"
            rows.append(row)
    return rows


def court_level(court):
    level, kind = court.get("level") or "", court.get("type") or ""
    if court["id"] == "scotus" or level == "colr":
        return "apex"
    if level == "iac":
        return "appellate"
    if level.startswith("gjc"):
        return "superior_trial"
    if level == "ljc":
        return "inferior_trial"
    if kind == "appellate":
        return "appellate"
    if kind.startswith("trial"):
        return "superior_trial"
    if kind == "bankruptcy":
        return "inferior_trial"
    return "tribunal"


def court_jurisdiction(court, codes):
    system, location = court.get("system"), (court.get("location") or "").lower()
    if system == "federal":
        return "us"
    if system == "international":
        return "uk-ew" if location == "england" else "int"
    return f"us-{codes[location]}" if location in codes else "us"


def convert_courts(blob, codes):
    taken = authored_ids("courts.json")
    rows = []
    for court in sorted(json.loads(blob), key=lambda c: c["id"]):
        cid = court["id"]
        if cid in taken:
            sys.exit(f"courts-db id {cid!r} collides with an authored court id; rename the authored court")
        row = {"id": cid, "name": {"en": court["name"]}, "jurisdiction": court_jurisdiction(court, codes),
               "level": court_level(court)}
        aliases = []
        for alias in (court.get("citation_string"), court.get("name_abbreviation")):
            if alias and alias not in aliases and alias != court["name"]:
                aliases.append(alias)
        if aliases:
            row["aliases"] = aliases
        starts = [year(d.get("start")) for d in court.get("dates") or []]
        ends = [year(d.get("end")) for d in court.get("dates") or []]
        start = min(starts) if starts and all(starts) else None
        end = max(ends) if ends and all(ends) else None
        if start and end and start > end:  # inconsistent upstream dates (flaindcommn)
            start = end = None
        if start:
            row["start"] = start
        if end:
            row["end"] = end
        if court.get("parent"):
            row["parent"] = court["parent"]
        rows.append(row)
    return rows


def render(rows):
    body = ",\n".join(json.dumps(row, ensure_ascii=False, separators=(",", ":")) for row in rows)
    return f"[\n{body}\n]\n" if rows else "[]\n"


def generate(data):
    codes = state_codes(data["courts-db"]["courts_db/data/states.json"])
    rdb = data["reporters-db"]
    return {
        "reporters.json": render(convert_reporters(rdb["reporters_db/data/reporters.json"], codes)),
        "series.json": render(convert_laws(rdb["reporters_db/data/laws.json"], codes)),
        "journals.json": render(convert_journals(rdb["reporters_db/data/journals.json"])),
        "courts.json": render(convert_courts(data["courts-db"]["courts_db/data/courts.json"], codes)),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--check", action="store_true", help="exit 1 if the generated files are stale")
    parser.add_argument("--update", action="store_true", help="move the pins to upstream HEAD first")
    parser.add_argument("--source", action="append", default=[], metavar="PROJECT=DIR",
                        help="read a project from a local checkout instead of GitHub")
    args = parser.parse_args()
    sources = dict(item.split("=", 1) for item in args.source)
    lock = read_lock()
    if args.update:
        update_lock(lock)
    outputs = generate(fetch(lock, sources))
    stale = []
    for name, text in outputs.items():
        path = OUT / name
        if not path.exists() or path.read_text(encoding="utf-8") != text:
            stale.append(name)
            if not args.check:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text, encoding="utf-8")
    if args.check and stale:
        sys.exit(f"registry/upstream is stale: {', '.join(stale)}; run tools/sync-upstream.py")
    for name, text in outputs.items():
        rows = len(json.loads(text))
        print(f"upstream/{name}: {rows} rows, {len(text.encode())} bytes")


if __name__ == "__main__":
    main()
