#!/usr/bin/env python3
"""Run unchanged repository conformance; gate new regressions against frozen debt.

A successful wrapper does NOT mean the raw conformance command passed. The raw
exit code, unchanged maintenance failures, source/binary hashes and logs survive.
"""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import subprocess
import sys

from ledger import digest, file_hash, source_inventory

ROOT = Path(__file__).resolve().parent


class GateError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise GateError(message)


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write_new(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="utf-8") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")


def case_metadata(repo):
    cases = {}
    for path in sorted((Path(repo) / "conformance/cases").glob("*.json")):
        document = read_json(path)
        require(document.get("format") == "legal-citations-conformance:v1", "unknown conformance format")
        for row in document["cases"]:
            identity = f"{path.stem}::{row['name']}"
            require(identity not in cases, "duplicate conformance case ID")
            require(row.get("status") in ("pass", "pending"), f"invalid status: {identity}")
            cases[identity] = {"status": row["status"],
                               "case_sha256_without_status": digest({k: v for k, v in row.items() if k != "status"})}
    require(cases, "no conformance cases")
    return cases


def check_metadata(baseline, current):
    old = baseline["cases"]
    require(set(old) <= set(current), "baseline conformance cases were removed")
    for identity, original in old.items():
        require(current[identity]["case_sha256_without_status"] == original["case_sha256_without_status"],
                f"baseline conformance input/expectation changed: {identity}")
        if original["status"] == "pass":
            require(current[identity]["status"] == "pass", f"baseline passing case was demoted: {identity}")
    for identity in set(current) - set(old):
        require(current[identity]["status"] == "pass", f"new candidate conformance case must be pass: {identity}")


def assess(baseline, current, report, raw_returncode):
    check_metadata(baseline, current)
    failures = report.get("failures")
    require(isinstance(failures, list), "raw report lacks failure list")
    prefix, suffix = "NOW PASSING ", ': set "status": "pass" (--promote)'
    now_passing = []
    for failure in failures:
        require(isinstance(failure, str) and failure.startswith(prefix) and failure.endswith(suffix),
                f"new raw conformance failure: {failure}")
        now_passing.append(failure[len(prefix):-len(suffix)])
    require(len(now_passing) == len(set(now_passing)), "duplicate raw status-maintenance failure")
    require(set(now_passing) == set(baseline["preexisting_now_passing"]),
            "NOW PASSING set differs from the frozen preexisting maintenance failures")
    for identity in now_passing:
        require(identity in current and current[identity]["status"] == "pending", "invalid NOW PASSING identity/status")
    totals = {status: sum(row["status"] == status for row in current.values()) for status in ("pass", "pending")}
    require(report.get("total") == totals, "raw conformance report does not cover all current cases")
    require(raw_returncode == (1 if now_passing else 0), "raw return code differs from documented baseline failures")
    satisfied = {identity for identity, row in current.items() if row["status"] == "pass"} | set(now_passing)
    expected_successes = {identity for identity, row in baseline["cases"].items() if row["status"] == "pass"}
    expected_successes |= set(baseline["preexisting_now_passing"])
    require(expected_successes <= satisfied, "baseline successful conformance case lost")
    return {"gate": "no_new_regression_relative_to_frozen_baseline", "gate_passed": True,
            "raw_command_succeeded": raw_returncode == 0, "raw_returncode": raw_returncode,
            "preexisting_maintenance_failure_count": len(now_passing),
            "retained_baseline_success_count": len(expected_successes),
            "current_declared_pass_count": totals["pass"], "current_declared_pending_count": totals["pending"],
            "additional_satisfied_cases": sorted(satisfied - expected_successes)}


def snapshot(repo, baseline_checks, output):
    """Metadata only: reuse actual baseline command receipt, never rerun/sculpt gold."""
    checks = read_json(baseline_checks)
    entry = next(row for row in checks["checks"] if row["label"] == "cli-conformance-python")
    log_path = Path(baseline_checks).resolve().parent / entry["log"]
    require(file_hash(log_path) == entry["log_sha256"], "baseline raw log changed")
    metadata = case_metadata(repo)
    require(sum(row["status"] == "pass" for row in metadata.values()) == entry["declared_pass_successes"],
            "baseline passing count differs from actual log metadata")
    require(sum(row["status"] == "pending" for row in metadata.values()) == entry["declared_pending"],
            "baseline pending count differs from actual log metadata")
    value = {"schema_version": 1, "scope": "repository conformance identities and content hashes only; no citation benchmark gold",
             "baseline_checks": str(Path(baseline_checks).resolve()), "baseline_checks_sha256": file_hash(baseline_checks),
             "source_inventory_sha256": digest(source_inventory(repo)),
             "runner_sha256": file_hash(Path(repo) / "conformance/run.py"),
             "preexisting_now_passing": sorted(entry["preexisting_now_passing"]), "cases": metadata}
    write_new(output, value)
    return {"snapshot": str(Path(output).resolve()), "sha256": file_hash(output), "cases": len(metadata),
            "preexisting_now_passing": len(value["preexisting_now_passing"])}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=ROOT.parent / "common-law-cite")
    parser.add_argument("--baseline", type=Path, default=ROOT / "conformance-baseline.json")
    parser.add_argument("--baseline-checks", type=Path, default=ROOT / "baseline-checks.json")
    parser.add_argument("--snapshot", action="store_true", help="write baseline metadata snapshot from actual baseline evidence")
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    receipt = {"schema_version": 1, "started_at": datetime.now(timezone.utc).isoformat(), "gate_passed": False,
               "raw_command_succeeded": False, "raw_returncode": None}
    try:
        require(not args.output.exists(), "output exists; evidence is immutable")
        if args.snapshot:
            print(json.dumps(snapshot(args.repo, args.baseline_checks, args.output), indent=2))
            return 0
        require(args.cli is not None, "--cli is required")
        args.cli = args.cli.resolve()
        baseline = read_json(args.baseline)
        require(file_hash(baseline["baseline_checks"]) == baseline["baseline_checks_sha256"], "baseline-checks changed")
        runner = args.repo / "conformance/run.py"
        require(file_hash(runner) == baseline["runner_sha256"], "repository conformance runner changed")
        current = case_metadata(args.repo)
        check_metadata(baseline, current)
        source_hash, cli_hash = digest(source_inventory(args.repo)), file_hash(args.cli)
        output = args.output.resolve()
        raw_report = output.with_suffix(".raw-report.json")
        raw_log = output.with_suffix(".raw-command.log")
        require(not raw_report.exists() and not raw_log.exists(), "raw evidence output exists")
        output.parent.mkdir(parents=True, exist_ok=True)
        command = [sys.executable, str(runner.resolve()), "--cli", str(args.cli), "--report", str(raw_report)]
        with raw_log.open("x", encoding="utf-8") as log:
            result = subprocess.run(command, cwd=args.repo, stdout=log, stderr=subprocess.STDOUT, check=False)
        receipt.update(command=command, cwd=str(args.repo.resolve()), raw_returncode=result.returncode,
                       raw_log=str(raw_log), raw_log_sha256=file_hash(raw_log),
                       raw_report=str(raw_report), raw_report_sha256=file_hash(raw_report) if raw_report.exists() else None,
                       source_inventory_sha256=source_hash, cli_sha256=cli_hash,
                       baseline_sha256=file_hash(args.baseline))
        require(digest(source_inventory(args.repo)) == source_hash and file_hash(args.cli) == cli_hash,
                "source or CLI changed during conformance check")
        receipt.update(assess(baseline, current, read_json(raw_report), result.returncode))
    except (GateError, OSError, ValueError, KeyError, TypeError, StopIteration) as exc:
        receipt["error"] = str(exc)
    receipt["finished_at"] = datetime.now(timezone.utc).isoformat()
    if not args.output.exists():
        write_new(args.output, receipt)
    print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0 if receipt["gate_passed"] else 2


if __name__ == "__main__":
    sys.exit(main())
