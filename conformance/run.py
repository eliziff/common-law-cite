#!/usr/bin/env python3
"""Run the conformance cases through the Python binding or the CLI.

    python conformance/run.py                      # the installed `legal_citations` package
    python conformance/run.py --cli target/release/legal-citations
    python conformance/run.py --cli ... --promote  # flip newly passing pending cases to pass

A ``pass`` case must match and a ``pending`` case must not; either surprise is
a failure (exit 1) unless ``--promote`` rewrites the pending ones. Matcher
semantics are documented in conformance/README.md and shared with the Rust
(crates/legal-citations-cli/tests/conformance.rs) and JS (conformance/run.mjs)
runners. Standard library only.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

CASES = Path(__file__).resolve().parent / "cases"
FORMAT = "legal-citations-conformance:v1"


def build_request(case: dict) -> tuple[str, dict]:
    method = case.get("method", "extract")
    if "request" in case:
        return method, case["request"]
    options = dict(case.get("options") or {})
    unit = options.pop("offsetUnit", "char")
    return method, {"text": case["input"], "options": options, "offsetUnit": unit}


def mismatch(expected, actual, path="$"):
    """None when ``actual`` satisfies ``expected``, else the first mismatch."""
    if expected is None:
        return None if actual is None else f"{path}: expected absent, got {json.dumps(actual)}"
    if isinstance(expected, dict) and len(expected) == 1 and "$contains" in expected:
        if not isinstance(actual, list):
            return f"{path}: expected an array, got {json.dumps(actual)}"
        cursor = 0
        for position, item in enumerate(expected["$contains"]):
            for offset, candidate in enumerate(actual[cursor:]):
                if mismatch(item, candidate, path) is None:
                    cursor += offset + 1
                    break
            else:
                return f"{path}: no element (in order) matches $contains[{position}] = {json.dumps(item)}"
        return None
    if isinstance(expected, dict) and len(expected) == 1 and "$in" in expected:
        if any(mismatch(choice, actual, path) is None for choice in expected["$in"]):
            return None
        return f"{path}: {json.dumps(actual)} is not one of {json.dumps(expected['$in'])}"
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            return f"{path}: expected an object, got {json.dumps(actual)}"
        for key, value in expected.items():
            problem = mismatch(value, actual.get(key), f"{path}.{key}")
            if problem:
                return problem
        return None
    if isinstance(expected, list):
        if not isinstance(actual, list):
            return f"{path}: expected an array, got {json.dumps(actual)}"
        if len(actual) != len(expected):
            return f"{path}: expected {len(expected)} elements, got {len(actual)}"
        for index, (item, value) in enumerate(zip(expected, actual)):
            problem = mismatch(item, value, f"{path}[{index}]")
            if problem:
                return problem
        return None
    if isinstance(expected, bool) or isinstance(actual, bool):
        same = expected is actual
    else:
        same = expected == actual
    return None if same else f"{path}: expected {json.dumps(expected)}, got {json.dumps(actual)}"


class BindingBackend:
    name = "python binding"

    def __init__(self):
        import legal_citations

        self.module = legal_citations

    def run(self, calls):
        results = []
        for method, request in calls:
            try:
                results.append(self.module.call(method, request))
            except self.module.LegalCitationsError as error:
                results.append({"error": {"code": error.code, "message": str(error)}})
        return results


class CliBackend:
    name = "cli"

    def __init__(self, binary: str):
        self.binary = binary

    def run(self, calls):
        lines = "".join(
            json.dumps({"id": index, "method": method, "request": request}) + "\n"
            for index, (method, request) in enumerate(calls)
        )
        completed = subprocess.run(
            [self.binary, "batch"], input=lines, capture_output=True, text=True, check=True
        )
        results = [None] * len(calls)
        for line in completed.stdout.splitlines():
            response = json.loads(line)
            results[response["id"]] = (
                response["result"] if "result" in response else {"error": response["error"]}
            )
        return results


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--cli", help="run through this `legal-citations` binary instead of the Python binding")
    parser.add_argument("--filter", help="only cases whose file::name contains this")
    parser.add_argument("--promote", action="store_true", help="rewrite newly passing pending cases as pass")
    parser.add_argument("--verbose", action="store_true", help="print why each pending case fails")
    parser.add_argument("--report", type=Path, help="write a JSON report here")
    arguments = parser.parse_args()
    backend = CliBackend(arguments.cli) if arguments.cli else BindingBackend()

    failures, promoted, report = [], 0, {}
    for path in sorted(CASES.glob("*.json")):
        document = json.loads(path.read_text(encoding="utf-8"))
        if document.get("format") != FORMAT:
            failures.append(f"{path.name}: unknown format {document.get('format')!r}")
            continue
        selected = [
            case
            for case in document["cases"]
            if not arguments.filter or arguments.filter in f"{path.stem}::{case['name']}"
        ]
        results = backend.run([build_request(case) for case in selected])
        tally = {"pass": 0, "pending": 0}
        changed = False
        for case, actual in zip(selected, results):
            label = f"{path.stem}::{case['name']}"
            problem = mismatch(case["expect"], actual)
            status = case.get("status")
            if status not in tally:
                failures.append(f"{label}: status must be pass or pending, got {status!r}")
                continue
            if status == "pass" and problem:
                failures.append(f"REGRESSION {label}: {problem}")
            elif status == "pending" and not problem:
                if arguments.promote:
                    case["status"] = "pass"
                    status = "pass"
                    changed = True
                    promoted += 1
                else:
                    failures.append(f"NOW PASSING {label}: set \"status\": \"pass\" (--promote)")
            elif status == "pending" and arguments.verbose:
                print(f"pending {label}: {problem}", file=sys.stderr)
            tally[status] += 1
        report[path.stem] = tally
        if changed:
            path.write_text(json.dumps(document, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")

    print(f"conformance ({backend.name}):")
    total = {"pass": 0, "pending": 0}
    for name, tally in report.items():
        print(f"  {name:<24} pass {tally['pass']:>4}  pending {tally['pending']:>4}")
        for key in total:
            total[key] += tally[key]
    print(f"  {'total':<24} pass {total['pass']:>4}  pending {total['pending']:>4}")
    if promoted:
        print(f"promoted {promoted} case(s) to pass")
    if arguments.report:
        arguments.report.write_text(json.dumps({"files": report, "total": total, "failures": failures}, indent=2))
    for failure in failures:
        print(failure, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
