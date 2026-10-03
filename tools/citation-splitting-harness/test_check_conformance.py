"""Regression-gate tests use metadata/report stubs, never benchmark scores."""
import copy
import unittest
import check_conformance as c


def baseline():
    return {"cases": {"a": {"status": "pass", "case_sha256_without_status": "a-hash"},
                      "b": {"status": "pending", "case_sha256_without_status": "b-hash"},
                      "c": {"status": "pending", "case_sha256_without_status": "c-hash"}},
            "preexisting_now_passing": ["b"]}


def report(*, passed=1, pending=2, ids=("b",)):
    return {"total": {"pass": passed, "pending": pending},
            "failures": [f'NOW PASSING {identity}: set "status": "pass" (--promote)' for identity in ids]}


class ConformanceGateTests(unittest.TestCase):
    def test_unchanged_maintenance_failure_is_explicit_not_raw_pass(self):
        value = baseline()
        result = c.assess(value, value["cases"], report(), 1)
        self.assertTrue(result["gate_passed"])
        self.assertFalse(result["raw_command_succeeded"])
        self.assertEqual(result["raw_returncode"], 1)
        self.assertEqual(result["retained_baseline_success_count"], 2)

    def test_lost_now_passing_case_rejected(self):
        value = baseline()
        with self.assertRaisesRegex(c.GateError, "NOW PASSING set"):
            c.assess(value, value["cases"], report(ids=()), 0)

    def test_new_regression_rejected(self):
        value = baseline()
        raw = report()
        raw["failures"].append("REGRESSION a: wrong result")
        with self.assertRaisesRegex(c.GateError, "new raw conformance failure"):
            c.assess(value, value["cases"], raw, 1)

    def test_case_removal_demotion_or_expectation_weakening_rejected(self):
        value = baseline()
        for change in ("remove", "demote", "weaken"):
            current = copy.deepcopy(value["cases"])
            if change == "remove":
                del current["a"]
            elif change == "demote":
                current["a"]["status"] = "pending"
            else:
                current["a"]["case_sha256_without_status"] = "weakened"
            with self.assertRaises(c.GateError):
                c.assess(value, current, report(), 1)

    def test_extra_passing_cases_allowed(self):
        value = baseline()
        current = copy.deepcopy(value["cases"])
        current["new"] = {"status": "pass", "case_sha256_without_status": "new-hash"}
        result = c.assess(value, current, report(passed=2), 1)
        self.assertEqual(result["additional_satisfied_cases"], ["new"])

    def test_unrelated_status_promotions_and_unpromoted_new_wins_rejected(self):
        value = baseline()
        promoted = copy.deepcopy(value["cases"])
        promoted["b"]["status"] = "pass"
        with self.assertRaises(c.GateError):
            c.assess(value, promoted, report(passed=2, pending=1, ids=()), 0)
        with self.assertRaises(c.GateError):
            c.assess(value, value["cases"], report(ids=("b", "c")), 1)

    def test_raw_coverage_or_code_mismatch_rejected(self):
        value = baseline()
        with self.assertRaisesRegex(c.GateError, "cover all"):
            c.assess(value, value["cases"], report(passed=0), 1)
        with self.assertRaisesRegex(c.GateError, "return code"):
            c.assess(value, value["cases"], report(), 0)


if __name__ == "__main__":
    unittest.main()
