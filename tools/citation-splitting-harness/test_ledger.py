"""Ledger unit tests use synthetic score records only, never a production CLI score."""
import copy
import contextlib
import io
import sys
from fractions import Fraction
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import ledger


def receipt(successes, *, lossless=True, span=(4, 1, 1), boundary=(2, 1, 1), count=4):
    """Synthetic metric-only input for arithmetic/gate tests; not executable evidence."""
    def metric(counts):
        return dict(zip(("tp", "fp", "fn"), counts))
    return {
        "unit_test_only": True,
        "metrics": {"whole_partition_exact": {"correct": len(successes), "total": count},
                    "exact_span": metric(span), "boundary": metric(boundary)},
        "cases": [{"id": str(i), "whole_partition_exact": i in successes,
                   "all_character_preservation": lossless or i in successes} for i in range(count)],
        "all_character_preservation": lossless or len(successes) == count,
    }


class ScoreGates(unittest.TestCase):
    def test_strict_primary_gain(self):
        baseline = receipt({0})
        self.assertTrue(all(ledger.retention(baseline, baseline, receipt({0, 1})).values()))
        gates = ledger.retention(baseline, baseline, receipt({0}))
        self.assertFalse(gates["strict_whole_partition_gain"])

    def test_original_baseline_success_cannot_be_traded_away(self):
        gates = ledger.retention(receipt({0}), receipt({0}), receipt({1, 2}))
        self.assertTrue(gates["strict_whole_partition_gain"])
        self.assertFalse(gates["no_original_baseline_success_losses"])

    def test_new_incumbent_successes_cannot_be_traded_away(self):
        gates = ledger.retention(receipt({0}), receipt({0, 1}), receipt({0, 2, 3}))
        self.assertTrue(gates["strict_whole_partition_gain"])
        self.assertTrue(gates["no_original_baseline_success_losses"])
        self.assertFalse(gates["no_incumbent_success_losses"])

    def test_whole_gain_does_not_override_span_f1_regression(self):
        gates = ledger.retention(receipt({0}), receipt({0}), receipt({0, 1}, span=(3, 2, 2)))
        self.assertFalse(gates["nondecreasing_exact_span_f1"])

    def test_boundary_precision_and_recall_independently_gated(self):
        old = receipt({0}, boundary=(2, 1, 2))
        precision_loss = ledger.retention(old, old, receipt({0, 1}, boundary=(3, 2, 1)))
        self.assertFalse(precision_loss["nondecreasing_boundary_precision"])
        self.assertTrue(precision_loss["nondecreasing_boundary_recall"])
        recall_loss = ledger.retention(old, old, receipt({0, 1}, boundary=(1, 0, 3)))
        self.assertTrue(recall_loss["nondecreasing_boundary_precision"])
        self.assertFalse(recall_loss["nondecreasing_boundary_recall"])

    def test_baseline_failure_is_scoreable_but_candidate_must_preserve(self):
        old = receipt({0}, lossless=False)
        self.assertEqual(ledger.scores(old)["whole"], 1)
        self.assertFalse(ledger.scores(old)["preservation"])
        self.assertFalse(ledger.retention(old, old, receipt({0, 1}, lossless=False))["all_character_preservation"])
        self.assertTrue(all(ledger.retention(old, old, receipt({0, 1})).values()))

    def test_exact_rational_comparison(self):
        counts = {"tp": 10**18, "fp": 1, "fn": 0}
        self.assertLess(ledger.ratio(counts, "precision"), Fraction(1))
        self.assertEqual(ledger.ratio({"tp": 2, "fp": 1, "fn": 3}, "f1"), Fraction(1, 2))

    def test_empty_denominator_convention(self):
        for kind in ("precision", "recall", "f1"):
            self.assertEqual(ledger.ratio({"tp": 0, "fp": 0, "fn": 0}, kind), 1)
            self.assertEqual(ledger.ratio({"tp": 0, "fp": 0, "fn": 2}, kind), 0)
            self.assertEqual(ledger.ratio({"tp": 0, "fp": 2, "fn": 0}, kind), 0)

    def test_case_or_aggregate_tampering_rejected(self):
        value = receipt({0})
        value["metrics"]["whole_partition_exact"]["correct"] = 2
        with self.assertRaises(ledger.LedgerError):
            ledger.scores(value)
        value = receipt({0})
        value["cases"][1]["id"] = "0"
        with self.assertRaises(ledger.LedgerError):
            ledger.scores(value)
        value = receipt({0})
        value["all_character_preservation"] = False
        with self.assertRaises(ledger.LedgerError):
            ledger.scores(value)

    def test_lossy_exact_case_rejected(self):
        value = receipt({0})
        value["cases"][0]["all_character_preservation"] = False
        value["all_character_preservation"] = False
        with self.assertRaises(ledger.LedgerError):
            ledger.scores(value)

    def test_scope_change_rejected_even_with_more_successes(self):
        gates = ledger.retention(receipt({0}), receipt({0}), receipt({0, 1}, count=5))
        self.assertFalse(gates["same_case_ids"])


class LedgerControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="split-ledger-unit-")
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        (self.repo / "source.rs").write_text("original\n")
        subprocess.run(["git", "add", "source.rs"], cwd=self.repo, check=True)
        subprocess.run(["git", "-c", "user.name=Unit Test", "-c", "user.email=unit-test@noreply.invalid",
                        "commit", "-qm", "synthetic test repository"], cwd=self.repo, check=True)
        self.artifacts = self.root / "experiment"
        self.artifacts.mkdir()
        self.log = ledger.Ledger(self.artifacts)

    def tearDown(self):
        self.temp.cleanup()

    def freeze(self):
        initial = self.artifacts / "initial.json"
        initial.write_text(json.dumps(ledger.source_inventory(self.repo)))
        contract = self.artifacts / "contract.txt"
        contract.write_text("unit-test contract\n")
        return self.log.freeze(self.repo, {"initial_source": initial, "protocol": contract}, required_check_labels=("source-tests",))

    def test_chain_survives_reload_and_detects_edits(self):
        self.log.append("unit_test", marker=1)
        self.log.append("unit_test", marker=2)
        self.assertEqual(len(ledger.Ledger(self.artifacts).events), 2)
        text = self.log.path.read_text().replace('"marker":1', '"marker":9')
        self.log.path.write_text(text)
        with self.assertRaises(ledger.LedgerError):
            ledger.Ledger(self.artifacts)

    def test_no_candidate_before_executable_baseline(self):
        self.freeze()
        with self.assertRaisesRegex(ledger.LedgerError, "no executable baseline"):
            self.log.open_attempt("candidate-1", "synthetic hypothesis")

    def test_freeze_rejects_prebaseline_source_changes(self):
        initial = self.artifacts / "initial.json"
        initial.write_text(json.dumps(ledger.source_inventory(self.repo)))
        (self.repo / "source.rs").write_text("modified\n")
        with self.assertRaisesRegex(ledger.LedgerError, "production changed"):
            self.log.freeze(self.repo, {"initial_source": initial})

    def test_baseline_guard_rechecks_original_source(self):
        self.freeze()
        (self.repo / "source.rs").write_text("modified after freeze\n")
        with self.assertRaisesRegex(ledger.LedgerError, "checkout differs"):
            self.log.initial_source()

    def test_frozen_contract_changes_rejected(self):
        self.freeze()
        (self.artifacts / "contract.txt").write_text("changed\n")
        with self.assertRaisesRegex(ledger.LedgerError, "frozen protocol changed"):
            self.log.contract()

    def test_new_source_is_hashed_and_in_exact_patch(self):
        original = ledger.digest(ledger.source_inventory(self.repo))
        (self.repo / "new.rs").write_text("new behavior\n")
        self.assertNotEqual(ledger.digest(ledger.source_inventory(self.repo)), original)
        patch = ledger.current_patch(self.repo)
        self.assertIn(b"diff --git a/new.rs b/new.rs", patch)
        self.assertIn(b"+new behavior", patch)

    def test_probe_failure_records_actual_command_and_log(self):
        result = self.log.run("missing-binary", [str(self.root / "unavailable-executable")], self.root)
        self.assertIsNone(result["returncode"])
        self.assertIn("FileNotFoundError", Path(result["log"]).read_text())
        self.assertEqual(result["log_sha256"], ledger.file_hash(result["log"]))

    def test_completed_exit_one_baseline_is_measured_in_temporary_unit_fixture(self):
        # No parser is simulated or scored; this synthetic temporary receipt tests
        # only the ledger control path accepting completed exit-1 evidence.
        self.freeze()
        cli = self.root / "unit-test-artifact"
        cli.write_text("not an executable parser; unit-test input only")
        contract = self.log.latest("contract_frozen")
        contract["files"]["evaluator"] = {"path": str(self.artifacts / "not-executed.py"), "sha256": "unused"}
        value = receipt({0}, lossless=False)
        value.update(status="completed", split="dev", parser_executed=True)
        def controlled_run(label, argv, cwd):
            output = Path(argv[argv.index("--output") + 1])
            output.write_text(json.dumps(value))
            return {"returncode": 1, "command": argv, "cwd": str(cwd)}
        with patch.object(self.log, "contract", return_value=contract), \
             patch.object(self.log, "run", side_effect=controlled_run), \
             patch.object(self.log, "validate_receipt", side_effect=lambda value, *args: ledger.scores(value)), \
             patch.object(self.log, "verify_build", return_value="unit-test-build-not-executed"):
            measured = self.log.measure(cli, baseline=True)
        self.assertEqual(measured["kind"], "baseline_measured")
        self.assertEqual(measured["returncode"], 1)
        self.assertEqual(measured["baseline_success_ids"], ["0"])
        self.assertEqual(measured["metrics"]["whole_partition_exact"]["correct"], 1)

    def test_changed_source_after_passing_check_is_rejected(self):
        self.freeze()
        before = ledger.digest(ledger.source_inventory(self.repo))
        self.log.append("candidate_check", attempt_id="unit", label="source-tests",
                        source_inventory_sha256=before, passed=True)
        self.log.verify_checks("unit", before)
        (self.repo / "source.rs").write_text("changed after correctness check")
        after = ledger.digest(ledger.source_inventory(self.repo))
        with self.assertRaisesRegex(ledger.LedgerError, "current source"):
            self.log.verify_checks("unit", after)

    def test_latest_check_can_supersede_an_environment_failure(self):
        self.freeze()
        inventory_hash = ledger.digest(ledger.source_inventory(self.repo))
        for passed in (False, True):
            self.log.append("candidate_check", attempt_id="unit", label="source-tests",
                            source_inventory_sha256=inventory_hash, passed=passed)
        self.assertTrue(self.log.verify_checks("unit", inventory_hash)["source-tests"]["passed"])

    def test_required_targeted_check_cannot_be_omitted(self):
        self.freeze()
        inventory_hash = ledger.digest(ledger.source_inventory(self.repo))
        self.log.append("candidate_check", attempt_id="unit", label="unrelated-check",
                        source_inventory_sha256=inventory_hash, passed=True)
        with self.assertRaisesRegex(ledger.LedgerError, "have not run"):
            self.log.verify_checks("unit", inventory_hash)

    def test_unbound_or_stale_binary_build_is_rejected(self):
        cli = self.root / "unit-binary"
        cli.write_text("unit test artifact only")
        cli_hash = ledger.file_hash(cli)
        inventory_hash = ledger.digest(ledger.source_inventory(self.repo))
        with self.assertRaisesRegex(ledger.LedgerError, "no recorded build"):
            self.log.verify_build(cli, cli_hash, inventory_hash)
        log = self.artifacts / "unit-build-log.txt"
        log.write_text("unit test evidence, no engine build")
        self.log.append("cli_build", unit_test_only=True, passed=True, cli=str(cli.resolve()),
                        cli_sha256=cli_hash, source_inventory_sha256=inventory_hash,
                        log=str(log), log_sha256=ledger.file_hash(log))
        self.log.verify_build(cli, cli_hash, inventory_hash)
        with self.assertRaisesRegex(ledger.LedgerError, "current source and binary"):
            self.log.verify_build(cli, "changed-binary", inventory_hash)
        with self.assertRaisesRegex(ledger.LedgerError, "current source and binary"):
            self.log.verify_build(cli, cli_hash, "changed-source")
        log.write_text("changed log")
        with self.assertRaisesRegex(ledger.LedgerError, "build log changed"):
            self.log.verify_build(cli, cli_hash, inventory_hash)

    def test_probe_failure_is_not_reported_as_success_by_cli(self):
        with contextlib.redirect_stdout(io.StringIO()):
            code = ledger.main(["--root", str(self.artifacts), "probe", "--label", "failed-command",
                                "--cwd", str(self.root), "--", sys.executable, "-c", "raise SystemExit(7)"])
        self.assertEqual(code, 2)
        self.assertEqual(ledger.Ledger(self.artifacts).events[-1]["returncode"], 7)

    def test_final_lock_closes_development(self):
        self.log.append("final_locked", unit_test_only=True)
        with self.assertRaisesRegex(ledger.LedgerError, "development is closed"):
            self.log.mutable()


if __name__ == "__main__":
    unittest.main()
