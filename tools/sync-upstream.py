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

All six source files are retained byte-for-byte under registry/upstream/<project>/.
Cargo compiles matching indexes from those files and authored extensions; no
renamed or stripped upstream tables are checked in. The registry API exposes
original source field names, including records the extraction pipeline does not use.
"""

import argparse
import hashlib
import json
import re
import sys
import tomllib
import urllib.request
from pathlib import Path
from string import Template

ROOT = Path(__file__).resolve().parent.parent
LOCK = ROOT / "upstream.lock"
REGISTRY = ROOT / "crates" / "legal-citations" / "registry"
OUT = REGISTRY / "upstream"
GRAMMAR = ROOT / "crates" / "legal-grammar" / "data"

FILES = {
    "reporters-db": ["reporters_db/data/reporters.json", "reporters_db/data/laws.json",
                     "reporters_db/data/journals.json", "reporters_db/data/regexes.json"],
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
                cached = OUT / name / Path(path).name
                expected = entry.get("sha256", {}).get(path)
                candidate = cached.read_bytes() if cached.exists() else b""
                blob = candidate if expected and hashlib.sha256(candidate).hexdigest() == expected else http_get(url)
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


def citation_extractors(data):
    """Port eyecite tokenizers._populate_reporter_extractors at eyecite.lock.

    Keep captures, exact/variant edition candidates and prefilter strings.
    Edition dates remain the original source values, not inferred year bounds.
    """
    raw = json.loads(data["reporters_db/data/regexes.json"])
    corpus = json.loads((GRAMMAR / "grammar-corpus.json").read_text(encoding="utf-8"))
    raw["full_cite"][""] = "$volume $reporter,? $page"
    raw["page"][""] = "(?P<page>" + corpus["tables"]["citations"]["defs"]["us_page"] + ")"

    # reporters_db.utils.process_variables / recursive_substitute.
    def flatten(values, prefix=""):
        result = {}
        for key, value in values.items():
            if key.endswith("#"):
                continue
            name = "_".join(part for part in (prefix, key) if part)
            result.update(flatten(value, name) if isinstance(value, dict) else {name: value})
        return result

    variables = flatten(raw)
    variables.update({key + "_optional": f"(?:{value} ?)?" for key, value in list(variables.items())})

    def expand(value):
        for _ in range(100):
            expanded = Template(value).safe_substitute(variables)
            if expanded == value:
                return value
            value = expanded
        raise ValueError("recursive reporter template exceeds source maximum depth")

    variables = {key: expand(value) for key, value in variables.items()}

    def relaxed(name):
        return re.sub(r"(?:\\?\ )+", r"\\s*", re.escape(name).replace(r"\.", r"\.\s*"))

    editions, extractors = [], {}

    def add(template, names, edition, kind, standard):
        pattern = Template(expand(template)).safe_substitute(edition="|".join(map(relaxed, names)))
        strings = names if relaxed(names[0]) in pattern else []
        short = pattern.replace(r"(?P<page>", r"at\s?(p(\.|age)?)? (?P<page>")
        for expression, is_short in [(pattern, False)] + ([(short, True)] if short != pattern else []):
            cluster = extractors.setdefault(expression, {
                "exact": [], "variations": [], "strings": [], "short": is_short,
                "standard": standard,
            })
            cluster[kind].append(edition)
            cluster["strings"].extend(strings)

    for source_name in ("reporters", "laws", "journals"):
        sources = json.loads(data[f"reporters_db/data/{source_name}.json"])
        for source_key, cluster in sources.items():
            for source in cluster:
                reporter = {"short_name": source_key, "name": source["name"],
                            "cite_type": source["cite_type"], "source": source_name}
                rows = source["editions"] if source_name == "reporters" else {source_key: source}
                for name, row in rows.items():
                    edition = len(editions)
                    editions.append({"short_name": name, "reporter": reporter,
                                     "start": row["start"], "end": row["end"]})
                    variations = ([key for key, value in source["variations"].items() if value == name]
                                  if source_name == "reporters" else source.get("variations", []))
                    for template in row.get("regexes") or ["$full_cite"]:
                        if source_name == "laws":
                            template = template.replace("§ ", "§§? ?")
                        standard = template == "$full_cite" and source_name != "laws"
                        add(template, [name], edition, "exact", standard)
                        if variations:
                            add(template, variations, edition, "variations", standard)

    rows = []
    for pattern, cluster in extractors.items():
        if cluster["short"]:
            pattern = pattern.replace(r"at\s?(p(\.|age)?)? (?P<page>",
                                      r"(?P<__short_at>at)\s?(p(\.|age)?)? (?P<page>")
        # JS-style names are the corpus representation; the expression is unchanged.
        pattern = pattern.replace("(?P<", "(?<")
        pattern = rf"(?:^|[^a-zA-Z0-9])(?<__citation>{pattern})(?:[^a-zA-Z0-9]|$)"
        rows.append({"pattern": pattern, **cluster, "strings": sorted(set(cluster["strings"]))})
    # Retain Beaver's always-on USC form (including unmarked sections) in the
    # same capture-bearing pipeline, with the original source edition records.
    common = next(entry["pattern"] for entry in corpus["tables"]["citations"]["entries"]
                  if entry["id"] == "cite.us.law.common")
    common = common.removeprefix("(?<![A-Za-z0-9])").removesuffix("(?![A-Za-z0-9])")
    rows.append({"pattern": rf"(?<![A-Za-z0-9])(?<__citation>{common})(?![A-Za-z0-9])",
                 "exact": [i for i, edition in enumerate(editions)
                           if edition["short_name"] == "U.S.C." and edition["reporter"]["source"] == "laws"],
                 "variations": [], "strings": [], "short": False, "standard": True, "ecmascript": True})
    return json.dumps({"editions": editions, "extractors": rows}, ensure_ascii=False,
                      separators=(",", ":")) + "\n"


def generate(data):
    return {f"{project}/{Path(path).name}": blob
            for project, files in data.items() for path, blob in files.items()}


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
    data = fetch(lock, sources)
    outputs = generate(data)
    pins = {name: {key: lock[name][key] for key in ("repository", "version", "commit")} for name in sorted(lock)}
    outputs["pins.json"] = (json.dumps(pins, indent=2, sort_keys=True) + "\n").encode()
    stale = []
    files = [(OUT / name, text) for name, text in outputs.items()]
    files.append((GRAMMAR / "us-extractors.json", citation_extractors(data["reporters-db"]).encode()))
    for path, text in files:
        if not path.exists() or path.read_bytes() != text:
            stale.append(str(path.relative_to(ROOT)))
            if not args.check:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(text)
    if args.check and stale:
        sys.exit(f"registry/upstream is stale: {', '.join(stale)}; run tools/sync-upstream.py")
    for name, text in outputs.items():
        if name == "pins.json":
            continue
        rows = len(json.loads(text))
        print(f"upstream/{name}: {rows} rows, {len(text)} bytes")


if __name__ == "__main__":
    main()
