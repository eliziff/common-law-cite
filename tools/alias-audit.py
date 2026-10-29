"""Audits the alias table (registry/aliases.json) for identities that hold two decisions, for review into
registry/alias-apart.tsv. Each audit prints one suspect identity a line: its head key, then why.

    python tools/alias-audit.py names  ALIASES.json --cli legal-citations --a2aj a2aj.sqlite --canlii canlii.db [--cache KEYS.tsv]
    python tools/alias-audit.py dates  ALIASES.json
    python tools/alias-audit.py places ALIASES.json

names:  the members a name is known for (a CanLII identifier's title in canlii.db; a citation A2AJ records, keyed by
        the engine's CLI, cached in --cache) share no party word.
dates:  its neutral citations, CanLII identifiers and database numbers, each the decision's own year, lie more than a
        year apart.
places: it holds two provinces' citations (outside the pairs whose reports are shared), or a Supreme Court citation
        beside a province's Quicklaw number.
A decision whose own citations a provider prints is checked by alias-inventory --judge, which lists each wrong pair.
"""
import argparse, collections, json, re, sqlite3, subprocess, sys
from pathlib import Path

REGISTRY = Path(__file__).resolve().parent.parent / "crates" / "legal-citations" / "registry"
DATABASE_PLACES = {
    "carswellont": "ca-on", "carswellbc": "ca-bc", "carswellalta": "ca-ab", "carswellsask": "ca-sk", "carswellman": "ca-mb",
    "carswellns": "ca-ns", "carswellnb": "ca-nb", "carswellnfld": "ca-nl", "carswellpei": "ca-pe", "carswellque": "ca-qc",
    "carswellyukon": "ca-yt", "carswellnwt": "ca-nt", "carswellnun": "ca-nu", "oj": "ca-on", "bcj": "ca-bc", "aj": "ca-ab",
    "sj": "ca-sk", "mj": "ca-mb", "nsj": "ca-ns", "nbj": "ca-nb", "nj": "ca-nl", "peij": "ca-pe", "qj": "ca-qc",
    "yj": "ca-yt", "nwtj": "ca-nt", "nuj": "ca-nu"}
SHARED = [{"ca-bc", "ca-yt"}, {"ca-ab", "ca-nt", "ca-nu"}, {"ca-nl", "ca-pe"}]
STOP = set("""r v the and of in re canada attorney general her his majesty queen king regina rex ltd inc co corp
corporation limited et al minister province ontario quebec british columbia alberta manitoba saskatchewan nova scotia
new brunswick newfoundland labrador prince edward island yukon northwest territories nunavut city town municipality
county district board commission council society association company bank insurance du de la le les des et c v
estate trustee trustees ex parte matter application appeal reference union local workers""".split())


def identities(path):
    targets = json.load(open(path, encoding="utf-8-sig"))["targets"]
    members = collections.defaultdict(set)
    for key, target in targets.items():
        members[target["key"]].update((key, target["key"]))
    return targets, members


def year(key):
    parts = key.split(":")
    at = {"neutral": 2, "canlii": 2, "database": 3}.get(parts[1])
    value = parts[at] if at is not None and at < len(parts) else ""
    return int(value) if value.isdigit() and 1700 <= int(value) <= 2100 else None


def dates(args):
    _, members = identities(args.aliases)
    for head, keys in members.items():
        dated = {key: year(key) for key in keys if year(key)}
        if dated and max(dated.values()) - min(dated.values()) > 1:
            print(head + "\t" + " ".join(sorted(dated, key=dated.get)))


def places(args):
    _, members = identities(args.aliases)
    courts = {court["id"]: court.get("region") or court.get("jurisdiction") for court in json.load(open(REGISTRY / "courts.json", encoding="utf-8"))}
    reports = {row.get("citation_id"): row.get("jurisdiction") for row in json.load(open(REGISTRY / "reporters.json", encoding="utf-8"))}

    def place(key):
        parts = key.split(":")
        found = {"neutral": lambda: courts.get(parts[3]), "database": lambda: DATABASE_PLACES.get(parts[2]),
                 "reporter": lambda: reports.get(parts[2])}.get(parts[1], lambda: None)()
        return found if found and found.startswith("ca-") else None

    for head, keys in members.items():
        found = {place(key) for key in keys} - {None}
        apart = len(found) > 1 and not any(found <= group for group in SHARED)
        supreme = any(k.startswith(("4:neutral:2", "4:neutral:1")) and k.split(":")[3] == "scc"
                      or k.startswith(("4:database:scj:", "4:reporter:scr:")) for k in keys)
        quicklaw = [k for k in keys if k.startswith("4:database:") and k.split(":")[2] in DATABASE_PLACES and not k.split(":")[2].startswith("carswell")]
        if apart or supreme and quicklaw:
            print(head + "\t" + ("provinces " + ",".join(sorted(found)) if apart else "supreme+quicklaw " + " ".join(quicklaw)))


def names(args):
    targets, members = identities(args.aliases)
    known = collections.defaultdict(set)
    canlii = sqlite3.connect(f"file:{args.canlii}?mode=ro", uri=True)
    for keys in members.values():
        for key in keys:
            parts = key.split(":")
            if parts[1] == "canlii":
                for (title,) in canlii.execute("select title from cases where caseId = ?", (f"{parts[2]}canlii{parts[3]}",)):
                    known[key].add(title)
    cache = Path(args.cache)
    if not cache.exists():
        store = sqlite3.connect(f"file:{args.a2aj}?mode=ro", uri=True)
        texts = [(c, row[4]) for row in store.execute("select citation_en, citation_fr, citation2_en, citation2_fr, "
                 "coalesce(name_en, name_fr) from document where doc_type = 'cases'") for c in row[:4] if c and c.strip() and row[4]]
        lines = "".join(json.dumps({"text": c}) + "\n" for c, _ in texts).encode()
        keys = [json.loads(line).get("key") for line in subprocess.run([args.cli, "key", "--jsonl"], input=lines,
                capture_output=True).stdout.decode().splitlines()]
        cache.write_text("".join(f"{key}\t{' '.join(name.split())}\n" for (_, name), key in zip(texts, keys) if key), encoding="utf-8")
    for line in cache.open(encoding="utf-8"):
        key, name = line.rstrip("\n").split("\t", 1)
        known[key].add(name)
    words = lambda name: {w for w in re.findall(r"[a-zà-ÿ]{3,}", name.lower()) if w not in STOP}
    for head, keys in members.items():
        groups = []
        for key in sorted(keys):
            for name in known.get(key, ()):
                bag = words(name)
                group = next((group for group in groups if group[0] & bag), None) if bag else None
                if group:
                    group[0].update(bag)
                    group[1].append(f"{key} = {name}")
                elif bag:
                    groups.append([set(bag), [f"{key} = {name}"]])
        if len(groups) > 1:
            print(head + "\t" + " || ".join("; ".join(group[1]) for group in groups))


parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
parser.add_argument("audit", choices=["names", "dates", "places"])
parser.add_argument("aliases")
parser.add_argument("--cli", default="legal-citations")
parser.add_argument("--a2aj")
parser.add_argument("--canlii")
parser.add_argument("--cache", default="a2aj-keys.tsv")
args = parser.parse_args()
{"names": names, "dates": dates, "places": places}[args.audit](args)
