#!/usr/bin/env python3
"""Stdlib experiment ledger: freeze, measured baseline, guarded attempts, final lock.

Never edits engine files, downloads a toolchain, or opens the sealed fixture.
"""
import argparse
from datetime import datetime, timezone
from fractions import Fraction
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent


class LedgerError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise LedgerError(message)


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def file_hash(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write_new_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="utf-8") as out:
        json.dump(value, out, ensure_ascii=False, sort_keys=True, indent=2)
        out.write("\n")


def utcnow():
    return datetime.now(timezone.utc).isoformat()


def git(repo, *args, accept=(0,)):
    result = subprocess.run(["git", *args], cwd=repo, capture_output=True, check=False)
    require(result.returncode in accept, f"git {args[0]} failed: {result.stderr.decode(errors='replace')}")
    return result.stdout


def source_inventory(repo):
    names = git(repo, "ls-files", "--cached", "--others", "--exclude-standard", "-z")
    inventory = {}
    for raw in sorted(set(names.split(b"\0")) - {b""}):
        name = raw.decode("utf-8")
        path = Path(repo) / name
        require(path.resolve().is_relative_to(Path(repo).resolve()), "source path resolves outside repository")
        require(path.is_file(), f"tracked source missing or unsupported: {name}")
        inventory[name] = file_hash(path)
    return inventory


def current_patch(repo):
    patch = git(repo, "diff", "--binary", "--no-ext-diff", "HEAD", "--", ".")
    untracked = git(repo, "ls-files", "--others", "--exclude-standard", "-z")
    for raw in sorted(set(untracked.split(b"\0")) - {b""}):
        name = raw.decode("utf-8")
        patch += git(repo, "diff", "--no-index", "--binary", "--no-ext-diff", "--", "/dev/null", name,
                     accept=(0, 1))
    return patch


def ratio(metrics, kind):
    tp, fp, fn = (metrics[k] for k in ("tp", "fp", "fn"))
    require(all(type(n) is int and n >= 0 for n in (tp, fp, fn)), "invalid metric counts")
    if kind == "precision":
        return Fraction(tp, tp + fp) if tp + fp else Fraction(not (tp + fn))
    if kind == "recall":
        return Fraction(tp, tp + fn) if tp + fn else Fraction(not (tp + fp))
    return Fraction(2 * tp, 2 * tp + fp + fn) if 2 * tp + fp + fn else Fraction(1)


def scores(receipt):
    metrics = receipt["metrics"]
    whole = metrics["whole_partition_exact"]
    require(type(whole["correct"]) is int and type(whole["total"]) is int
            and 0 <= whole["correct"] <= whole["total"] and whole["total"] > 0,
            "invalid whole-partition counts")
    cases = receipt["cases"]
    require(isinstance(cases, list) and len(cases) == whole["total"], "case count mismatch")
    require(all(isinstance(row.get("id"), str) and row["id"] for row in cases), "invalid case IDs")
    ids = {row["id"] for row in cases}
    require(len(ids) == len(cases), "duplicate case IDs")
    require(all(type(row.get("whole_partition_exact")) is bool
                and type(row.get("all_character_preservation")) is bool for row in cases),
            "case outcomes must be booleans")
    successes = {row["id"] for row in cases if row["whole_partition_exact"]}
    require(len(successes) == whole["correct"], "success identities/count mismatch")
    require(not any(row["whole_partition_exact"] and not row["all_character_preservation"] for row in cases),
            "lossy case cannot be exact")
    preservation = all(row["all_character_preservation"] for row in cases)
    require(receipt["all_character_preservation"] is preservation, "preservation aggregate mismatch")
    return {"whole": whole["correct"], "ids": ids, "successes": successes,
            "preservation": preservation, "span_f1": ratio(metrics["exact_span"], "f1"),
            "boundary_precision": ratio(metrics["boundary"], "precision"),
            "boundary_recall": ratio(metrics["boundary"], "recall")}


def retention(baseline, incumbent, candidate):
    base, old, new = map(scores, (baseline, incumbent, candidate))
    gates = {
        "same_case_ids": base["ids"] == old["ids"] == new["ids"],
        "all_character_preservation": new["preservation"],
        "strict_whole_partition_gain": new["whole"] > old["whole"],
        "no_original_baseline_success_losses": base["successes"] <= new["successes"],
        "no_incumbent_success_losses": old["successes"] <= new["successes"],
        "nondecreasing_exact_span_f1": new["span_f1"] >= old["span_f1"],
        "nondecreasing_boundary_precision": new["boundary_precision"] >= old["boundary_precision"],
        "nondecreasing_boundary_recall": new["boundary_recall"] >= old["boundary_recall"],
    }
    return gates


class Ledger:
    def __init__(self, root=ROOT):
        self.root = Path(root).resolve()
        self.path = self.root / "attempts.jsonl"
        self.events = []
        if self.path.exists():
            for line in self.path.read_text(encoding="utf-8").splitlines():
                event = json.loads(line)
                claimed = event.pop("event_sha256")
                require(claimed == digest(event), "ledger event hash mismatch")
                require(event["sequence"] == len(self.events) + 1, "ledger sequence mismatch")
                require(event["previous_event_sha256"] == (self.events[-1]["event_sha256"] if self.events else None),
                        "ledger chain mismatch")
                event["event_sha256"] = claimed
                self.events.append(event)

    def append(self, kind, **data):
        event = {"schema_version": 1, "sequence": len(self.events) + 1, "timestamp_utc": utcnow(),
                 "kind": kind, "previous_event_sha256": self.events[-1]["event_sha256"] if self.events else None,
                 **data}
        event["event_sha256"] = digest(event)
        self.root.mkdir(parents=True, exist_ok=True)
        with self.path.open("a", encoding="utf-8") as out:
            out.write(canonical(event).decode("utf-8") + "\n")
            out.flush()
            os.fsync(out.fileno())
        self.events.append(event)
        return event

    def latest(self, kind):
        return next((e for e in reversed(self.events) if e["kind"] == kind), None)

    def contract(self):
        event = self.latest("contract_frozen")
        require(event is not None, "freeze the reviewed contract first")
        for label, item in event["files"].items():
            require(file_hash(item["path"]) == item["sha256"], f"frozen {label} changed")
        require(git(event["repo"], "rev-parse", "HEAD").decode().strip() == event["repository_head"],
                "repository HEAD changed during experiment")
        return event

    def baseline(self):
        event = self.latest("baseline_measured")
        require(event is not None, "no executable baseline: parser mutations/attempts are forbidden")
        return event

    def incumbent(self):
        event = self.latest("candidate_accepted") or self.baseline()
        require(file_hash(event["receipt"]) == event["receipt_sha256"], "incumbent receipt changed")
        return event

    def active(self):
        begin = self.latest("attempt_opened")
        require(begin is not None, "open an attempt first")
        closed = [e for e in self.events if e["kind"] in ("candidate_accepted", "candidate_rejected", "attempt_abandoned")]
        require(not closed or closed[-1]["sequence"] < begin["sequence"], "no active attempt")
        return begin

    def mutable(self):
        require(self.latest("final_locked") is None, "final candidate is locked; development is closed")

    def verify_source(self, expected):
        contract = self.contract()
        inventory = source_inventory(contract["repo"])
        require(digest(inventory) == expected, "checkout differs from required source inventory")
        return inventory

    def initial_source(self):
        contract = self.contract()
        original = read_json(contract["files"]["initial_source"]["path"])
        return self.verify_source(digest(original))

    def run(self, label, argv, cwd):
        require(re.fullmatch(r"[A-Za-z0-9_.-]+", label) is not None, "invalid run label")
        folder = self.root / "runs" / f"{len(self.events) + 1:04d}-{label}"
        folder.mkdir(parents=True, exist_ok=False)
        log = folder / "output.txt"
        started = utcnow()
        with log.open("x", encoding="utf-8") as out:
            try:
                result = subprocess.run(argv, cwd=cwd, stdout=out, stderr=subprocess.STDOUT, check=False)
                code = result.returncode
            except OSError as exc:
                out.write(f"{type(exc).__name__}: {exc}\n")
                code = None
        return {"command": argv, "cwd": str(Path(cwd).resolve()), "started_at": started,
                "finished_at": utcnow(), "returncode": code, "log": str(log), "log_sha256": file_hash(log)}

    def freeze(self, repo, files, required_check_labels=("source-tests", "conformance")):
        require(self.latest("contract_frozen") is None, "contract already frozen")
        repo = str(Path(repo).resolve())
        require(source_inventory(repo) == read_json(files["initial_source"]), "production changed before baseline")
        resolved = {label: {"path": str(Path(path).resolve()), "sha256": file_hash(path)}
                    for label, path in files.items()}
        return self.append("contract_frozen", repo=repo, files=resolved,
                           repository_head=git(repo, "rev-parse", "HEAD").decode().strip(),
                           initial_source_inventory_sha256=digest(read_json(files["initial_source"])),
                           required_check_labels=list(required_check_labels))

    def open_attempt(self, attempt_id, reason):
        self.mutable()
        baseline = self.baseline()
        require(not any(e.get("attempt_id") == attempt_id for e in self.events), "attempt ID already used")
        require(re.fullmatch(r"[A-Za-z0-9_.-]+", attempt_id) is not None, "invalid attempt ID")
        require(bool(reason.strip()), "candidate hypothesis required")
        begin = self.latest("attempt_opened")
        if begin:
            require(any(e["sequence"] > begin["sequence"] and e["kind"] in
                        ("candidate_accepted", "candidate_rejected", "attempt_abandoned") for e in self.events),
                    "finish the active attempt first")
        incumbent = self.incumbent()
        self.verify_source(incumbent["source_inventory_sha256"])
        return self.append("attempt_opened", attempt_id=attempt_id, hypothesis=reason,
                           baseline_receipt_sha256=baseline["receipt_sha256"],
                           incumbent_receipt_sha256=incumbent["receipt_sha256"])

    def check(self, label, argv):
        self.mutable()
        attempt = self.active()
        contract = self.contract()
        before = digest(source_inventory(contract["repo"]))
        record = self.run(label, argv, contract["repo"])
        after = digest(source_inventory(contract["repo"]))
        return self.append("candidate_check", attempt_id=attempt["attempt_id"], label=label,
                           source_inventory_sha256=before, source_unchanged=before == after,
                           passed=record["returncode"] == 0 and before == after, **record)

    def verify_checks(self, attempt_id, inventory_hash):
        checks = {}
        for event in self.events:
            if event["kind"] == "candidate_check" and event["attempt_id"] == attempt_id:
                checks[event["label"]] = event
        required = set(self.contract()["required_check_labels"])
        require(required and required <= set(checks), "required cheap correctness checks have not run")
        require(all(event["passed"] and event["source_inventory_sha256"] == inventory_hash
                    for event in checks.values()),
                "passing cheap correctness checks on the current source are required before full dev")
        return checks

    def build(self, cli, argv):
        self.mutable()
        contract = self.contract()
        baseline = self.latest("baseline_measured")
        if baseline is None:
            inventory = self.initial_source()
            attempt_id = None
        else:
            inventory = source_inventory(contract["repo"])
            try:
                attempt_id = self.active()["attempt_id"]
            except LedgerError:
                self.verify_source(self.incumbent()["source_inventory_sha256"])
                attempt_id = None
        before = digest(inventory)
        cli = Path(cli).resolve()
        record = self.run("cli-build", argv, contract["repo"])
        after = digest(source_inventory(contract["repo"]))
        cli_hash = file_hash(cli) if cli.is_file() else None
        passed = record["returncode"] == 0 and before == after and cli_hash is not None and os.access(cli, os.X_OK)
        return self.append("cli_build", attempt_id=attempt_id, source_inventory_sha256=before,
                           source_unchanged=before == after, cli=str(cli), cli_sha256=cli_hash,
                           passed=passed, **record)

    def verify_build(self, cli, cli_hash, inventory_hash):
        # A command receipt binds the current candidate inventory to the binary it
        # produced. Merely observing independent source/binary hashes is not enough.
        candidates = [event for event in self.events if event["kind"] == "cli_build"
                      and event["cli"] == str(Path(cli).resolve())]
        require(candidates, "no recorded build binds this binary to source; run ledger build first")
        build = candidates[-1]
        require(build["passed"] and build["source_inventory_sha256"] == inventory_hash
                and build["cli_sha256"] == cli_hash,
                "latest build does not bind the current source and binary")
        require(file_hash(build["log"]) == build["log_sha256"], "build log changed")
        return build["event_sha256"]

    def validate_receipt(self, receipt, cli_hash, inventory_hash):
        contract = self.contract()
        require(receipt.get("status") == "completed" and receipt.get("split") == "dev"
                and receipt.get("parser_executed") is True, "run did not produce an executable dev baseline/score")
        require(receipt.get("cli_sha256", receipt.get("candidate_sha256")) == cli_hash, "report binary hash mismatch")
        require(receipt.get("fixture_sha256") == contract["files"]["dev_fixture"]["sha256"], "report dev hash mismatch")
        require(receipt.get("evaluator_sha256") == contract["files"]["evaluator"]["sha256"], "report evaluator hash mismatch")
        require(receipt.get("fixture_manifest_sha256") == contract["files"]["fixture_manifest"]["sha256"],
                "report fixture-manifest hash mismatch")
        require(receipt.get("source_inventory_sha256") == inventory_hash, "report source hash mismatch")
        require(receipt.get("initial_source_manifest_sha256") == contract["files"]["initial_source"]["sha256"],
                "report initial source manifest mismatch")
        require(receipt.get("harness_manifest_sha256") == contract["files"]["harness_manifest"]["sha256"],
                "report harness manifest mismatch")
        expected_ids = {row["id"] for row in read_json(contract["files"]["dev_fixture"]["path"])["cases"]}
        require(scores(receipt)["ids"] == expected_ids, "report does not cover the exact frozen dev case IDs")

    def measure(self, cli, baseline=False):
        self.mutable()
        contract = self.contract()
        if baseline:
            require(self.latest("baseline_measured") is None, "baseline already measured")
            inventory = self.initial_source()
            label, attempt_id = "baseline", None
        else:
            self.baseline()
            attempt = self.active()
            attempt_id = attempt["attempt_id"]
            inventory = source_inventory(contract["repo"])
            inventory_hash = digest(inventory)
            self.verify_checks(attempt_id, inventory_hash)
            label = attempt_id
        inventory_hash = digest(inventory)
        cli = Path(cli).resolve()
        cli_hash = file_hash(cli) if cli.is_file() else None
        folder = self.root / "measurements" / f"{len(self.events) + 1:04d}-{label}"
        folder.mkdir(parents=True, exist_ok=False)
        inventory_file = folder / "source-inventory.json"
        write_new_json(inventory_file, inventory)
        patch = folder / "candidate.patch"
        patch.write_bytes(current_patch(contract["repo"]))
        receipt_path = folder / "dev-receipt.json"
        argv = [sys.executable, contract["files"]["evaluator"]["path"], "--cli", str(cli),
                "--split", "dev", "--output", str(receipt_path)]
        evidence = {"attempt_id": attempt_id, "source_inventory_sha256": inventory_hash,
                    "source_inventory": str(inventory_file), "source_inventory_file_sha256": file_hash(inventory_file),
                    "cli": str(cli), "cli_sha256": cli_hash, "patch": str(patch), "patch_sha256": file_hash(patch),
                    "receipt": str(receipt_path)}
        if cli_hash is not None:
            try:
                evidence["build_event_sha256"] = self.verify_build(cli, cli_hash, inventory_hash)
            except LedgerError as exc:
                return self.append("baseline_blocked" if baseline else "candidate_run_failed", reason=str(exc),
                                   measured=False, command=argv, command_executed=False, **evidence)
        run = self.run(label, argv, self.root)
        evidence.update(receipt_sha256=file_hash(receipt_path) if receipt_path.exists() else None,
                        command_executed=True, **run)
        try:
            require(run["returncode"] in (0, 1), "evaluator execution blocked/failed; no score recorded")
            require(digest(source_inventory(contract["repo"])) == inventory_hash, "source changed during evaluation")
            require(cli_hash is not None and file_hash(cli) == cli_hash, "binary missing/changed during evaluation")
            receipt = read_json(receipt_path)
            self.validate_receipt(receipt, cli_hash, inventory_hash)
        except (LedgerError, OSError, ValueError, KeyError, TypeError) as exc:
            return self.append("baseline_blocked" if baseline else "candidate_run_failed", reason=str(exc),
                               measured=False, **evidence)
        if baseline:
            return self.append("baseline_measured", measured=True, metrics=receipt["metrics"],
                               baseline_success_ids=sorted(scores(receipt)["successes"]), **evidence)
        incumbent = self.incumbent()
        base = self.baseline()
        require(file_hash(base["receipt"]) == base["receipt_sha256"], "baseline receipt changed")
        gates = retention(read_json(base["receipt"]), read_json(incumbent["receipt"]), receipt)
        accepted = all(gates.values())
        return self.append("candidate_accepted" if accepted else "candidate_rejected", measured=True,
                           metrics=receipt["metrics"], gates=gates,
                           reason="all frozen acceptance gates passed" if accepted else
                                  "; ".join(key for key, passed in gates.items() if not passed), **evidence)

    def lock(self):
        self.mutable()
        contract = self.contract()
        incumbent = self.incumbent()
        begin = self.latest("attempt_opened")
        require(begin is None or any(e["sequence"] > begin["sequence"] and e["kind"] in
                    ("candidate_accepted", "candidate_rejected", "attempt_abandoned") for e in self.events),
                "close active attempt before final lock")
        self.verify_source(incumbent["source_inventory_sha256"])
        require(file_hash(incumbent["cli"]) == incumbent["cli_sha256"], "incumbent binary changed")
        require(scores(read_json(incumbent["receipt"]))["preservation"], "incumbent fails preservation")
        output = self.root / "final-incumbent.json"
        run = self.run("final-lock", [sys.executable, contract["files"]["evaluator"]["path"],
                       "--freeze-incumbent", incumbent["receipt"], "--output", str(output)], self.root)
        if run["returncode"] != 0:
            return self.append("final_lock_failed", **run)
        require(output.is_file(), "evaluator did not create final lock")
        return self.append("final_locked", final_incumbent=str(output), final_incumbent_sha256=file_hash(output),
                           source_inventory_sha256=incumbent["source_inventory_sha256"],
                           cli_sha256=incumbent["cli_sha256"], development_receipt_sha256=incumbent["receipt_sha256"], **run)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="experiment directory (tests may use a temporary root)")
    sub = parser.add_subparsers(dest="action", required=True)
    probe = sub.add_parser("probe", help="execute and record an environment/self-test command without claiming a baseline")
    probe.add_argument("--label", required=True)
    probe.add_argument("--cwd", type=Path, default=ROOT)
    probe.add_argument("command", nargs=argparse.REMAINDER)
    freeze = sub.add_parser("freeze", help="freeze reviewed artifacts; never opens holdout")
    freeze.add_argument("--repo", type=Path, default=ROOT.parent / "common-law-cite")
    freeze.add_argument("--required-check-label", action="append", help="required targeted check labels, default source-tests and conformance")
    for name, default in (("evaluator", "evaluate.py"), ("fixture-manifest", "fixtures/manifest.json"),
                          ("dev-fixture", "fixtures/dev.json"), ("protocol", "PROTOCOL.txt"),
                          ("initial-source", "initial-source-sha256.json"),
                          ("harness-manifest", "evaluator-manifest.json")):
        freeze.add_argument("--" + name, type=Path, default=ROOT / default)
    freeze.add_argument("--baseline-checks", type=Path, default=ROOT / "baseline-checks.json")
    freeze.add_argument("--conformance-baseline", type=Path, default=ROOT / "conformance-baseline.json")
    baseline = sub.add_parser("baseline", help="execute actual dev baseline; unavailable binary records blocked run")
    baseline.add_argument("--cli", type=Path, default=ROOT.parent / "common-law-cite/target/release/legal-citations")
    begin = sub.add_parser("begin", help="authorize a candidate attempt only after real baseline")
    begin.add_argument("--id", required=True)
    begin.add_argument("--reason", required=True)
    check = sub.add_parser("check", help="execute a cheap candidate correctness check before full dev")
    check.add_argument("--label", required=True)
    check.add_argument("command", nargs=argparse.REMAINDER)
    build = sub.add_parser("build", help="execute build and bind produced CLI hash to the current source")
    build.add_argument("--cli", type=Path, required=True)
    build.add_argument("command", nargs=argparse.REMAINDER)
    measure = sub.add_parser("measure", help="execute full frozen dev and record keep/reject gates")
    measure.add_argument("--cli", type=Path, required=True)
    abandon = sub.add_parser("abandon", help="close an unmeasured failed attempt; requires restoration first")
    abandon.add_argument("--reason", required=True)
    sub.add_parser("lock", help="freeze the accepted incumbent without opening holdout")
    sub.add_parser("status", help="verify event chain and show measurement status")
    args = parser.parse_args(argv)
    try:
        ledger = Ledger(args.root)
        if args.action == "probe":
            command = args.command[1:] if args.command[:1] == ["--"] else args.command
            require(command, "probe command required")
            event = ledger.append("command_probe", label=args.label, measured=False,
                                  **ledger.run(args.label, command, args.cwd))
        elif args.action == "freeze":
            files = {name: getattr(args, name) for name in
                     ("evaluator", "fixture_manifest", "dev_fixture", "protocol", "initial_source", "harness_manifest")}
            files.update(ledger=Path(__file__).resolve(), ledger_tests=ROOT / "test_ledger.py",
                         baseline_checks=args.baseline_checks, conformance_baseline=args.conformance_baseline,
                         conformance_gate=ROOT / "check_conformance.py",
                         conformance_gate_tests=ROOT / "test_check_conformance.py")
            event = ledger.freeze(args.repo, files, args.required_check_label or ("source-tests", "conformance"))
        elif args.action == "baseline":
            event = ledger.measure(args.cli, baseline=True)
        elif args.action == "begin":
            event = ledger.open_attempt(args.id, args.reason)
        elif args.action == "check":
            command = args.command[1:] if args.command[:1] == ["--"] else args.command
            require(command, "check command required")
            event = ledger.check(args.label, command)
        elif args.action == "build":
            command = args.command[1:] if args.command[:1] == ["--"] else args.command
            require(command, "build command required")
            event = ledger.build(args.cli, command)
        elif args.action == "measure":
            event = ledger.measure(args.cli)
        elif args.action == "abandon":
            ledger.mutable()
            attempt = ledger.active()
            ledger.verify_source(ledger.incumbent()["source_inventory_sha256"])
            event = ledger.append("attempt_abandoned", attempt_id=attempt["attempt_id"], reason=args.reason, measured=False)
        elif args.action == "lock":
            event = ledger.lock()
        else:
            event = {"events": len(ledger.events), "contract_frozen": ledger.latest("contract_frozen") is not None,
                     "executable_baseline": ledger.latest("baseline_measured") is not None,
                     "accepted_candidates": sum(e["kind"] == "candidate_accepted" for e in ledger.events),
                     "final_locked": ledger.latest("final_locked") is not None,
                     "last_event_sha256": ledger.events[-1]["event_sha256"] if ledger.events else None}
        print(json.dumps(event, ensure_ascii=False, indent=2, sort_keys=True))
        failed = event.get("kind") in ("baseline_blocked", "candidate_run_failed", "final_lock_failed")
        failed |= event.get("passed") is False
        failed |= event.get("kind") == "command_probe" and event.get("returncode") != 0
        return 2 if failed else 0
    except (LedgerError, OSError, ValueError, KeyError, TypeError) as exc:
        print(f"ledger refused: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
