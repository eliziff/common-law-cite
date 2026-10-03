#!/usr/bin/env python3
"""Frozen source-boundary evaluator; invokes the real legal-citations CLI only.

No parser, citation patterns, URL scoring, network access, or model calls live here.
All offsets are Unicode code points. See evaluator-contract.txt for the contract.
"""
import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parent
NORMALIZATION = "full-authority-left-v1"
SPLIT_REQUEST = {"recallFirst": True, "extendedUs": False, "offsetUnit": "char"}
EXTRACT_REQUEST = {"options": {"extendedUs": False, "resolve": False, "parallel": False,
                               "removeAmbiguous": False}, "offsetUnit": "char"}


class EvaluationError(ValueError):
    pass


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def require(condition, message):
    if not condition:
        raise EvaluationError(message)


def integer(value):
    return type(value) is int


def span_pair(span, text, label, allow_empty=False):
    require(isinstance(span, dict), f"{label}: expected object")
    start, end = span.get("start"), span.get("end")
    require(integer(start) and integer(end), f"{label}: offsets must be integers")
    require(0 <= start <= end <= len(text), f"{label}: offsets out of bounds")
    require(allow_empty or start < end, f"{label}: empty span")
    if "text" in span:
        require(span["text"] == text[start:end], f"{label}: text differs from source slice")
    return start, end


def validate_gold(document, split):
    require(document.get("schema_version") == 1, "unsupported fixture schema")
    require(document.get("annotation_policy_version") == NORMALIZATION, "gold normalization mismatch")
    require(document.get("partition") == {"dev": "development", "holdout": "holdout"}[split],
            "fixture partition mismatch")
    cases = document.get("cases")
    require(isinstance(cases, list) and cases, "fixture cases must be nonempty")
    ids = set()
    for row in cases:
        require(isinstance(row, dict), "fixture case must be an object")
        identity, text = row.get("id"), row.get("text")
        require(isinstance(identity, str) and identity and identity not in ids, "case ID missing/duplicate")
        ids.add(identity)
        require(isinstance(text, str), f"{identity}: text must be a string")
        spans, parts = row.get("expected_spans"), row.get("expected_parts")
        require(isinstance(spans, list) and isinstance(parts, list) and len(spans) == len(parts),
                f"{identity}: expected spans/parts missing or unequal")
        cursor = 0
        for index, (span, part) in enumerate(zip(spans, parts)):
            start, end = span_pair(span, text, f"{identity}.gold[{index}]")
            require(start == cursor and part == text[start:end], f"{identity}: gold does not tile source")
            cursor = end
        require(cursor == len(text) and "".join(parts) == text, f"{identity}: gold loses source text")
        if "extraction_cores" in row:
            cores = row["extraction_cores"]
            require(isinstance(cores, list), f"{identity}: extraction_cores must be an array")
            pairs = [span_pair(core, text, f"{identity}.core") for core in cores]
            require(pairs == sorted(pairs) and len(set(pairs)) == len(pairs), f"{identity}: unordered/duplicate gold cores")
    return cases


def normalize_prediction(text, result):
    """Validate raw API accounting before deterministic whole-text normalization."""
    require(isinstance(result, dict), "split response must be an object")
    parts, delimiters = result.get("parts"), result.get("delimiters")
    require(isinstance(parts, list) and isinstance(delimiters, list), "missing parts/delimiters")
    status = result.get("status")
    require(status in ("deterministic_complete", "abstain"), "unknown split status")
    if not parts:
        require(not delimiters, "empty parts with delimiters")
        require(not text.strip(), "nonempty substantive source has no raw parts")
        return [(0, len(text))] if text else []
    require(status == "deterministic_complete", "abstention returned parts")
    raw = []
    for index, part in enumerate(parts):
        require(isinstance(part, dict) and isinstance(part.get("text"), str), "raw part text missing")
        raw.append(span_pair(part, text, f"raw[{index}]"))
    require(all(left[1] <= right[0] for left, right in zip(raw, raw[1:])), "raw parts overlap or reorder")
    require(len(delimiters) == len(raw) - 1, "delimiter count differs from raw gaps")
    prefix, suffix = text[:raw[0][0]], text[raw[-1][1]:]
    require(not prefix.strip() and not suffix.strip(), "substantive prefix/suffix dropped")
    reconstruction = prefix + parts[0]["text"]
    for index, (left, right) in enumerate(zip(raw, raw[1:])):
        delimiter = delimiters[index]
        require(isinstance(delimiter, list) and len(delimiter) == 3, "invalid delimiter shape")
        start, end, surface = delimiter
        require(integer(start) and integer(end), "delimiter offsets must be integers")
        require((start, end) == (left[1], right[0]), "delimiter does not cover exact raw gap")
        require(surface == text[start:end], "delimiter text differs from source")
        reconstruction += surface + parts[index + 1]["text"]
    reconstruction += suffix
    require(reconstruction == text, "raw accounting is not lossless")
    boundaries = [0] + [start for start, _ in raw[1:]] + [len(text)]
    full = list(zip(boundaries, boundaries[1:]))
    require("".join(text[start:end] for start, end in full) == text, "normalized partition is not lossless")
    return full


def metric(gold, actual):
    expected, predicted = Counter(gold), Counter(actual)
    tp = sum((expected & predicted).values())
    wanted, produced = sum(expected.values()), sum(predicted.values())
    precision = tp / produced if produced else (1.0 if not wanted else 0.0)
    recall = tp / wanted if wanted else (1.0 if not produced else 0.0)
    return {"true_positives": tp, "gold": wanted, "predicted": produced,
            "precision": precision, "recall": recall,
            "f1": 2 * precision * recall / (precision + recall) if precision + recall else 0.0}


def summarize_counts(metrics):
    tp = sum(item["true_positives"] for item in metrics)
    gold = sum(item["gold"] for item in metrics)
    predicted = sum(item["predicted"] for item in metrics)
    precision = tp / predicted if predicted else (1.0 if not gold else 0.0)
    recall = tp / gold if gold else (1.0 if not predicted else 0.0)
    return {"true_positives": tp, "gold": gold, "predicted": predicted,
            "precision": precision, "recall": recall,
            "f1": 2 * precision * recall / (precision + recall) if precision + recall else 0.0}


def score_case(row, split_result, extraction_result=None):
    text = row["text"]
    gold = [(span["start"], span["end"]) for span in row["expected_spans"]]
    error = None
    try:
        actual = normalize_prediction(text, split_result)
    except EvaluationError as exc:
        actual, error = [], str(exc)
    valid = error is None
    detail = {"id": row["id"], "family_id": row.get("family_id"),
              "lossless": valid, "validation_error": error,
              "raw_split_response": split_result, "boundary_metrics_valid": valid,
              "raw_abstention": isinstance(split_result, dict) and split_result.get("status") == "abstain",
              "normalization_reason": "whitespace_only_no_authority" if valid and text and not text.strip() else None,
              "exact_partition": valid and actual == gold,
              "gold_spans": gold, "actual_spans": actual,
              "spans": metric(gold, actual),
              "boundaries": metric([a for a, _ in gold[1:]], [a for a, _ in actual[1:]])}
    if "extraction_cores" in row:
        expected = [(span["start"], span["end"]) for span in row["extraction_cores"]]
        try:
            require(isinstance(extraction_result, dict) and isinstance(extraction_result.get("citations"), list),
                    "extract response missing citations")
            predicted = [span_pair(citation.get("span"), text, "extracted core")
                         for citation in extraction_result["citations"]]
            detail["extraction"] = {"valid": True, "predicted_core_spans": predicted, **metric(expected, predicted)}
        except (EvaluationError, AttributeError) as exc:
            detail["extraction"] = {"valid": False, "validation_error": str(exc), **metric(expected, [])}
    return detail


def summarize(details):
    extraction = [row["extraction"] for row in details if "extraction" in row]
    return {"cases": len(details), "exact_partition_count": sum(row["exact_partition"] for row in details),
            "exact_partition_accuracy": sum(row["exact_partition"] for row in details) / len(details),
            "lossless_count": sum(row["lossless"] for row in details),
            "invalid_split_cases": sum(not row["lossless"] for row in details),
            "mandatory_losslessness_pass": all(row["lossless"] for row in details),
            "exact_success_ids": sorted(row["id"] for row in details if row["exact_partition"]),
            "spans": summarize_counts([row["spans"] for row in details]),
            "boundaries": summarize_counts([row["boundaries"] for row in details]),
            "extraction": {"annotated_cases": len(extraction),
                           "valid_cases": sum(row["valid"] for row in extraction),
                           "metrics": summarize_counts(extraction) if extraction else None}}


def verify_harness():
    path = ROOT / "evaluator-manifest.json"
    manifest = read_json(path)
    require(manifest.get("normalization") == NORMALIZATION and manifest.get("frozen") is True,
            "harness manifest is not frozen")
    files = manifest.get("files", {})
    require("evaluate.py" in files and "test_evaluate.py" in files, "manifest omits evaluator/tests")
    for name, digest in files.items():
        target = (ROOT / name).resolve()
        require(target.is_relative_to(ROOT) and target.is_file(), "unsafe/missing harness file")
        require(sha256(target) == digest, f"frozen harness changed: {name}")
    return sha256(path)


def resolve_cli(value):
    selected = value or str(ROOT.parent / "common-law-cite/target/release/legal-citations")
    found = shutil.which(selected)
    if not found:
        raise EvaluationError(f"real legal-citations CLI unavailable: {selected}; no parser was run")
    path = Path(found).resolve()
    require(path.is_file() and os.access(path, os.X_OK), "CLI is not an executable file")
    return path


def fixture_path(split):
    return ROOT / ({"dev": "fixtures/dev.json", "holdout": "sealed/holdout.json"}[split])


def checked_fixture(split):
    # This function must never be called for holdout before verify_incumbent.
    path = fixture_path(split)
    manifest = read_json(ROOT / "fixtures/manifest.json")
    expected = manifest.get("files", {}).get(path.relative_to(ROOT).as_posix())
    require(isinstance(expected, str) and len(expected) == 64, "fixture manifest lacks expected hash")
    require(sha256(path) == expected, "frozen fixture bytes changed")
    return validate_gold(read_json(path), split), expected


def source_inventory_hash():
    repo = ROOT.parent / "common-law-cite"
    listed = subprocess.run(["git", "-C", str(repo), "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
                            check=True, capture_output=True).stdout
    inventory = {}
    for name in sorted(set(listed.decode("utf-8").split("\0")) - {""}):
        target = repo / name
        require(target.resolve().is_relative_to(repo.resolve()), "source file resolves outside repository")
        inventory[name] = sha256(target)
    content = json.dumps(inventory, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    return hashlib.sha256(content).hexdigest()


def public_metric(value):
    return {"tp": value["true_positives"], "fp": value["predicted"] - value["true_positives"],
            "fn": value["gold"] - value["true_positives"],
            **{name: value[name] for name in ("precision", "recall", "f1")}}


def add_protocol_fields(receipt):
    summary, details = receipt["summary"], receipt["details"]
    receipt["all_character_preservation"] = summary["mandatory_losslessness_pass"]
    receipt["metrics"] = {
        "whole_partition_exact": {"correct": summary["exact_partition_count"], "total": summary["cases"]},
        "exact_span": public_metric(summary["spans"]), "boundary": public_metric(summary["boundaries"]),
        "extraction": summary["extraction"]}
    receipt["cases"] = [{"id": row["id"], "whole_partition_exact": row["exact_partition"],
                          "all_character_preservation": row["lossless"]} for row in details]


def verify_incumbent(path, cli_hash, harness_hash):
    require(path is not None, "holdout requires --final-incumbent frozen hash receipt")
    lock = read_json(path)
    require(lock.get("status") == "incumbent_frozen" and lock.get("selection_closed") is True,
            "final-incumbent receipt does not close candidate selection")
    require(lock.get("candidate_sha256") == cli_hash, "final incumbent binary changed")
    require(lock.get("harness_manifest_sha256") == harness_hash, "final incumbent harness changed")
    require(lock.get("development_fixture_sha256") == sha256(fixture_path("dev")), "development gold changed")
    source_path = Path(lock.get("development_receipt", ""))
    require(source_path.is_file() and sha256(source_path) == lock.get("development_receipt_sha256"),
            "final incumbent development receipt missing/changed")
    source = read_json(source_path)
    require(source.get("status") == "completed" and source.get("split") == "dev", "incumbent has no completed dev run")
    require(source.get("summary", {}).get("mandatory_losslessness_pass") is True, "incumbent failed losslessness")
    require(source.get("candidate_sha256") == cli_hash and source.get("harness_manifest_sha256") == harness_hash,
            "incumbent receipt hash mismatch")
    require(source.get("source_inventory_sha256") == source_inventory_hash(), "source changed after development run")
    require(source.get("fixture_manifest_sha256") == sha256(ROOT / "fixtures/manifest.json"), "fixture manifest changed")
    return sha256(path)


def freeze_incumbent(source_path, output, harness_hash):
    source = read_json(source_path)
    require(source.get("status") == "completed" and source.get("split") == "dev", "freeze requires completed dev receipt")
    require(source.get("summary", {}).get("mandatory_losslessness_pass") is True, "cannot freeze losslessness failure")
    require(source.get("harness_manifest_sha256") == harness_hash, "development used a different evaluator")
    require(source.get("fixture_sha256") == sha256(fixture_path("dev")), "development fixture changed")
    require(source.get("candidate_sha256") == sha256(source["cli"]), "candidate binary changed after development")
    require(source.get("source_inventory_sha256") == source_inventory_hash(), "source changed after development run")
    require(source.get("fixture_manifest_sha256") == sha256(ROOT / "fixtures/manifest.json"), "fixture manifest changed")
    result = {"schema_version": 1, "status": "incumbent_frozen", "selection_closed": True,
              "candidate_sha256": source["candidate_sha256"], "harness_manifest_sha256": harness_hash,
              "development_fixture_sha256": source["fixture_sha256"],
              "development_receipt": str(Path(source_path).resolve()),
              "development_receipt_sha256": sha256(source_path)}
    write_json(output, result)
    return result


def run_batch(cli, requests, timeout):
    content = "".join(json.dumps(request, ensure_ascii=False) + "\n" for request in requests)
    started = time.monotonic()
    try:
        completed = subprocess.run([str(cli), "batch"], input=content, text=True, encoding="utf-8",
                                   capture_output=True, timeout=timeout, check=False)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise EvaluationError(f"CLI batch execution failed: {exc}") from exc
    require(completed.returncode == 0, f"CLI batch exited {completed.returncode}: {completed.stderr[:1000]}")
    responses = {}
    for line in completed.stdout.splitlines():
        response = json.loads(line)
        require(isinstance(response, dict), "CLI response must be a JSON object")
        identity = response.get("id")
        require(isinstance(identity, str), "CLI response ID must be a string")
        require(("result" in response) != ("error" in response), "CLI response requires exactly one result/error")
        require(identity not in responses, "duplicate CLI response ID")
        responses[identity] = response
    require(set(responses) == {request["id"] for request in requests}, "CLI response IDs missing/unexpected")
    return responses, time.monotonic() - started


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", help="real legal-citations executable; never downloaded or built by this evaluator")
    parser.add_argument("--split", choices=("dev", "holdout"), default="dev")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--final-incumbent", type=Path)
    parser.add_argument("--freeze-incumbent", type=Path, metavar="DEV_RECEIPT")
    parser.add_argument("--timeout", type=float, default=120)
    args = parser.parse_args(argv)
    receipt = {"schema_version": 1, "run_id": str(uuid.uuid4()), "status": "blocked", "split": args.split,
               "normalization": NORMALIZATION, "split_request": SPLIT_REQUEST,
               "extraction_request": EXTRACT_REQUEST,
               "started_at": datetime.now(timezone.utc).isoformat(),
               "parser_executed": False, "holdout_read": False}
    try:
        require(args.timeout > 0, "timeout must be positive")
        require(not args.output.exists(), "output already exists; receipts are immutable")
        receipt["harness_manifest_sha256"] = verify_harness()
        receipt["evaluator_sha256"] = sha256(__file__)
        receipt["fixture_manifest_sha256"] = sha256(ROOT / "fixtures/manifest.json")
        receipt["initial_source_manifest_sha256"] = sha256(ROOT / "initial-source-sha256.json")
        receipt["source_inventory_sha256"] = source_inventory_hash()
        if args.freeze_incumbent:
            require(args.split == "dev" and not args.final_incumbent, "freeze operation cannot evaluate holdout")
            freeze_incumbent(args.freeze_incumbent, args.output, receipt["harness_manifest_sha256"])
            print(f"incumbent_frozen: {args.output}")
            return 0
        if args.split == "holdout":
            require(args.final_incumbent is not None, "holdout requires --final-incumbent frozen hash receipt")
        cli = resolve_cli(args.cli)
        receipt.update(cli=str(cli), candidate_sha256=sha256(cli), cli_sha256=sha256(cli))
        if args.split == "holdout":
            receipt["final_incumbent_sha256"] = verify_incumbent(
                args.final_incumbent, receipt["candidate_sha256"], receipt["harness_manifest_sha256"])
            # Exclusive creation makes the sealed set a one-shot check even after a failed run.
            marker = ROOT / "sealed-evaluation-started.json"
            with marker.open("x", encoding="utf-8") as handle:
                json.dump({"final_incumbent_sha256": receipt["final_incumbent_sha256"],
                           "output": str(args.output.resolve())}, handle)
            receipt["holdout_read"] = True
        rows, digest = checked_fixture(args.split)
        receipt["fixture_sha256"] = digest
        requests = []
        for row in rows:
            requests.append({"id": row["id"] + "/split", "method": "splitSources",
                             "request": {"text": row["text"], **SPLIT_REQUEST}})
            if "extraction_cores" in row:
                requests.append({"id": row["id"] + "/extract", "method": "extract",
                                 "request": {"text": row["text"], **EXTRACT_REQUEST}})
        receipt["parser_invocation_attempted"] = True
        responses, duration = run_batch(cli, requests, args.timeout)
        receipt["parser_executed"] = True
        require(sha256(cli) == receipt["candidate_sha256"], "CLI bytes changed during evaluation")
        require(source_inventory_hash() == receipt["source_inventory_sha256"], "source bytes changed during evaluation")
        details = [score_case(row, responses[row["id"] + "/split"].get("result"),
                              responses.get(row["id"] + "/extract", {}).get("result")) for row in rows]
        for row, detail in zip(rows, details):
            if "error" in responses[row["id"] + "/split"]:
                detail["split_api_error"] = responses[row["id"] + "/split"]["error"]
        receipt.update(status="completed", duration_seconds=duration, details=details, summary=summarize(details))
        add_protocol_fields(receipt)
        code = 0 if receipt["summary"]["mandatory_losslessness_pass"] else 1
    except (EvaluationError, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        receipt["error"] = str(exc)
        code = 2
    if not args.output.exists():
        write_json(args.output, receipt)
    print(json.dumps({key: receipt[key] for key in ("status", "split", "parser_executed", "error", "summary") if key in receipt}, ensure_ascii=False))
    return code


if __name__ == "__main__":
    sys.exit(main())
