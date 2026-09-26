#!/usr/bin/env python3
"""Differential harness: pinned eyecite vs legal-citations over a text corpus.

Runs eyecite's ``get_citations`` and legal-citations' ``extract`` over the same
plain text, pairs citations by span overlap, and reports per-citation
disagreements:

* ``missed``   eyecite found a citation legal-citations did not;
* ``extra``    legal-citations found one eyecite did not (expected for
               Canadian/Commonwealth material; counted, not a failure);
* ``mismatch`` both found it but the mapped fields disagree (form, authority,
               spans, volume/reporter/page, year, court, parties, pinpoint).

eyecite output is mapped onto the legal-citations model exactly as the ported
conformance cases are (conformance/tools/port-eyecite.py) and compared with the
conformance matcher (conformance/run.py), so a disagreement here reads the same
as a failing conformance case.

Corpus inputs (combine freely):

    --corpus DIR            *.txt as plain text; *.html/*.htm/*.xml cleaned first
    --jsonl FILE            one {"id"?, "text"} object per line
    --conformance           the inputs of conformance/cases/eyecite-*.json

Engines: the installed ``legal_citations`` Python package (default) or
``--cli path/to/legal-citations`` (JSON Lines ``batch`` mode). eyecite must be
importable (``pip install eyecite==<pin>``).

    python tools/eyecite-diff.py --conformance --report diff.json
    python tools/eyecite-diff.py --corpus opinions/ --cli target/release/legal-citations
    python tools/eyecite-diff.py --conformance --baseline tools/eyecite-diff-baseline.json

With ``--baseline`` the run fails (exit 1) when ``missed`` or ``mismatch``
grew, or when citations eyecite finds changed in number: that is upstream
drift (a new eyecite/reporters-db release) or an engine regression.
``--write-baseline`` records the current summary.
"""

from __future__ import annotations

import argparse
import logging
import importlib.util
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PORT = load("port_eyecite", ROOT / "conformance" / "tools" / "port-eyecite.py")
RUNNER = load("conformance_run", ROOT / "conformance" / "run.py")

MARKUP_SUFFIXES = {".html", ".htm", ".xml"}
MARKUP_STEPS = ["html", "all_whitespace"]


# ---------------------------------------------------------------- corpus


def corpus(arguments) -> list[tuple[str, str, bool]]:
    """``(id, text, is_markup)`` documents."""
    documents = []
    for directory in arguments.corpus or []:
        for path in sorted(Path(directory).rglob("*")):
            if path.is_file() and (path.suffix == ".txt" or path.suffix in MARKUP_SUFFIXES):
                documents.append(
                    (str(path), path.read_text(encoding="utf-8", errors="replace"), path.suffix in MARKUP_SUFFIXES)
                )
    for jsonl in arguments.jsonl or []:
        for number, line in enumerate(Path(jsonl).read_text(encoding="utf-8").splitlines(), 1):
            if line.strip():
                record = json.loads(line)
                documents.append((str(record.get("id", f"{jsonl}:{number}")), record["text"], False))
    if arguments.conformance:
        for path in sorted((ROOT / "conformance" / "cases").glob("eyecite-*.json")):
            for case in json.loads(path.read_text(encoding="utf-8"))["cases"]:
                if "input" in case:
                    documents.append((f"{path.stem}::{case['name']}", case["input"], False))
    return documents


# ---------------------------------------------------------------- engines


class Binding:
    def __init__(self):
        import legal_citations

        self.module = legal_citations

    def version(self) -> str:
        return self.module.version()["version"]

    def extract_all(self, texts: list[str]) -> list[list[dict]]:
        return [self.module.extract(text, offset_unit="char") for text in texts]

    def clean(self, text: str, steps: list[str]) -> str:
        return self.module.clean(text, steps)


class Cli:
    def __init__(self, binary: str):
        self.binary = binary

    def batch(self, calls: list[tuple[str, dict]]) -> list:
        lines = "".join(
            json.dumps({"id": index, "method": method, "request": request}) + "\n"
            for index, (method, request) in enumerate(calls)
        )
        output = subprocess.run([self.binary, "batch"], input=lines, capture_output=True, text=True, check=True)
        results = [None] * len(calls)
        for line in output.stdout.splitlines():
            response = json.loads(line)
            if "error" in response:
                raise RuntimeError(f"legal-citations: {response['error']}")
            results[response["id"]] = response["result"]
        return results

    def version(self) -> str:
        return self.batch([("version", {})])[0]["version"]

    def extract_all(self, texts: list[str]) -> list[list[dict]]:
        requests = [("extract", {"text": text, "offsetUnit": "char"}) for text in texts]
        return [result["citations"] for result in self.batch(requests)]

    def clean(self, text: str, steps: list[str]) -> str:
        return self.batch([("clean", {"text": text, "steps": steps})])[0]["text"]


# ---------------------------------------------------------------- comparison


def extent(expectation: dict, ours: bool, record: dict):
    """The span used for pairing: eyecite's span() maps to our span for
    full/short/unknown forms and to our fullSpan for ibid/supra/reference."""
    if ours:
        key = "fullSpan" if record["form"] in ("ibid", "supra", "reference") else "span"
        return record[key]["start"], record[key]["end"]
    for key in ("span", "fullSpan"):
        span = expectation.get(key)
        if span and "end" in span:
            return span["start"], span["end"]
    start = expectation["span"]["start"]
    return start, start + 1


def overlap(left, right) -> int:
    return max(0, min(left[1], right[1]) - max(left[0], right[0]))


def compare(document_id: str, text: str, theirs: list, ours: list) -> tuple[list, Counter]:
    counts: Counter = Counter()
    disagreements = []
    expectations = [PORT.expectation(cite, text) for cite in theirs]
    their_extents = [extent(expectation, False, None) for expectation in expectations]
    our_extents = [extent({}, True, record) for record in ours]
    used = set()
    for index, (cite, expectation, their_extent) in enumerate(zip(theirs, expectations, their_extents)):
        candidates = [
            (overlap(their_extent, our_extents[position]), position)
            for position in range(len(ours))
            if position not in used and overlap(their_extent, our_extents[position]) > 0
        ]
        counts[f"eyecite:{type(cite).__name__}"] += 1
        if not candidates:
            counts["missed"] += 1
            disagreements.append(
                {"document": document_id, "kind": "missed", "eyecite": PORT.upstream_record(cite), "expected": expectation}
            )
            continue
        _, position = max(candidates)
        used.add(position)
        problem = RUNNER.mismatch(expectation, ours[position])
        if problem:
            counts["mismatch"] += 1
            counts[f"mismatch:{type(cite).__name__}"] += 1
            disagreements.append(
                {
                    "document": document_id,
                    "kind": "mismatch",
                    "problem": problem,
                    "eyecite": PORT.upstream_record(cite),
                    "expected": expectation,
                    "ours": ours[position],
                }
            )
        else:
            counts["matched"] += 1
    for position, record in enumerate(ours):
        if position not in used:
            counts["extra"] += 1
            counts[f"extra:{record['authority']}"] += 1
            disagreements.append(
                {
                    "document": document_id,
                    "kind": "extra",
                    "ours": {key: record.get(key) for key in ("form", "authority", "span", "jurisdiction")},
                }
            )
    return disagreements, counts


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--corpus", action="append", help="directory of .txt/.html files")
    parser.add_argument("--jsonl", action="append", help='JSON Lines file of {"id", "text"}')
    parser.add_argument("--conformance", action="store_true", help="use the eyecite-derived conformance inputs")
    parser.add_argument("--cli", help="legal-citations binary (default: the Python binding)")
    parser.add_argument("--report", type=Path, help="write the full JSON report here")
    parser.add_argument("--max-disagreements", type=int, default=2000)
    parser.add_argument("--baseline", type=Path, help="fail if missed/mismatch grew versus this summary")
    parser.add_argument("--write-baseline", type=Path, help="write the summary as a new baseline")
    arguments = parser.parse_args()

    import eyecite

    # eyecite logs every overlap it resolves; the reports carry what matters.
    logging.getLogger("eyecite").setLevel(logging.ERROR)
    from importlib.metadata import version

    engine = Cli(arguments.cli) if arguments.cli else Binding()
    documents = corpus(arguments)
    if not documents:
        parser.error("no documents: pass --corpus, --jsonl or --conformance")

    texts = []
    for _, text, markup in documents:
        texts.append(engine.clean(text, MARKUP_STEPS) if markup else text)
    ours_all = engine.extract_all(texts)

    totals: Counter = Counter()
    disagreements = []
    for (document_id, _, _), text, ours in zip(documents, texts, ours_all):
        theirs = eyecite.get_citations(text)
        found, counts = compare(document_id, text, theirs, ours)
        totals.update(counts)
        totals["documents"] += 1
        totals["eyecite"] += len(theirs)
        totals["ours"] += len(ours)
        disagreements.extend(found)

    summary = dict(sorted(totals.items()))
    report = {
        "eyecite": version("eyecite"),
        "reporters-db": version("reporters-db"),
        "courts-db": version("courts-db"),
        "legal-citations": engine.version(),
        "summary": summary,
        "disagreements": disagreements[: arguments.max_disagreements],
        "truncated": len(disagreements) > arguments.max_disagreements,
    }
    if arguments.report:
        arguments.report.write_text(json.dumps(report, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")

    print(
        f"eyecite {report['eyecite']} vs legal-citations {report['legal-citations']}: "
        f"{summary.get('documents', 0)} documents, eyecite {summary.get('eyecite', 0)} / ours {summary.get('ours', 0)} citations; "
        f"matched {summary.get('matched', 0)}, mismatch {summary.get('mismatch', 0)}, "
        f"missed {summary.get('missed', 0)}, extra {summary.get('extra', 0)}",
        file=sys.stderr,
    )
    for key, value in summary.items():
        if ":" in key:
            print(f"  {key:<32} {value}", file=sys.stderr)

    baseline_record = {
        "eyecite": report["eyecite"],
        "reporters-db": report["reporters-db"],
        "courts-db": report["courts-db"],
        "summary": {key: summary.get(key, 0) for key in ("documents", "eyecite", "matched", "mismatch", "missed")},
    }
    if arguments.write_baseline:
        arguments.write_baseline.write_text(json.dumps(baseline_record, indent=2) + "\n", encoding="utf-8")
    if arguments.baseline:
        baseline = json.loads(arguments.baseline.read_text(encoding="utf-8"))["summary"]
        current = baseline_record["summary"]
        drift = [
            f"{key}: {baseline.get(key, 0)} -> {current[key]}"
            for key in ("missed", "mismatch")
            if current[key] > baseline.get(key, 0)
        ]
        if current["eyecite"] != baseline.get("eyecite") and current["documents"] == baseline.get("documents"):
            drift.append(f"eyecite citations found: {baseline.get('eyecite')} -> {current['eyecite']}")
        if drift:
            print("drift versus baseline:\n  " + "\n  ".join(drift), file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
