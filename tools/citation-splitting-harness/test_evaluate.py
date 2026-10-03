"""Scorer-only unit tests using invented raw API outputs; NOT parser benchmarks."""
import copy
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import evaluate as e


def raw(text, spans, status="deterministic_complete"):
    return {"status": status,
            "parts": [{"start": a, "end": b, "text": text[a:b]} for a, b in spans],
            "delimiters": [[left[1], right[0], text[left[1]:right[0]]]
                           for left, right in zip(spans, spans[1:])]}


def gold(text, spans):
    return {"id": "test-invented", "text": text, "expected_spans": [{"start": a, "end": b} for a, b in spans],
            "expected_parts": [text[a:b] for a, b in spans]}


class ScorerTests(unittest.TestCase):
    def test_whole_source_with_unicode_whitespace_delimiter_and_suffix(self):
        text = "  🦫A; \nB. \t"
        self.assertEqual(e.normalize_prediction(text, raw(text, [(2, 4), (7, 9)])), [(0, 7), (7, 11)])

    def test_reordered_parts_rejected_even_with_same_characters(self):
        text = "AB; CD"
        with self.assertRaisesRegex(e.EvaluationError, "reorder"):
            e.normalize_prediction(text, raw(text, [(4, 6), (0, 2)]))

    def test_source_text_mutation_rejected(self):
        result = raw("AB; CD", [(0, 2), (4, 6)])
        result["parts"][0]["text"] = "BA"
        with self.assertRaisesRegex(e.EvaluationError, "source slice"):
            e.normalize_prediction("AB; CD", result)

    def test_wrong_delimiter_and_missing_delimiter_rejected(self):
        original = raw("AB; CD", [(0, 2), (4, 6)])
        for change in ([], [[2, 3, ";"]], [[2, 4, " ;"]]):
            result = copy.deepcopy(original)
            result["delimiters"] = change
            with self.assertRaises(e.EvaluationError):
                e.normalize_prediction("AB; CD", result)

    def test_nonwhitespace_outer_loss_and_terminal_semicolon_rejected(self):
        for text, spans in (("!AB", [(1, 3)]), ("AB;", [(0, 2)]), ("AB!", [(0, 2)])):
            with self.assertRaisesRegex(e.EvaluationError, "prefix/suffix"):
                e.normalize_prediction(text, raw(text, spans))

    def test_empty_and_whitespace_abstentions_have_explicit_normalization(self):
        abstain = {"status": "abstain", "parts": [], "delimiters": []}
        self.assertEqual(e.normalize_prediction("", abstain), [])
        self.assertEqual(e.normalize_prediction(" \t\n", abstain), [(0, 3)])
        detail = e.score_case(gold(" \t\n", [(0, 3)]), abstain)
        self.assertTrue(detail["raw_abstention"])
        self.assertEqual(detail["normalization_reason"], "whitespace_only_no_authority")
        with self.assertRaisesRegex(e.EvaluationError, "substantive"):
            e.normalize_prediction("Words", abstain)

    def test_exact_spans_penalize_wrong_duplicate_occurrence(self):
        self.assertEqual(e.metric([(0, 2)], [(4, 6)])["true_positives"], 0)
        self.assertEqual(e.metric([(0, 2)], [(0, 2), (0, 2)])["precision"], 0.5)

    def test_wrong_cut_and_correct_count_are_not_exact(self):
        text = "AB CD EF"
        row = gold(text, [(0, 3), (3, 8)])
        detail = e.score_case(row, raw(text, [(0, 5), (6, 8)]))
        self.assertTrue(detail["lossless"])
        self.assertFalse(detail["exact_partition"])
        self.assertEqual(detail["spans"]["true_positives"], 0)
        self.assertEqual(detail["boundaries"]["recall"], 0)

    def test_gold_requires_literal_complete_partition(self):
        document = {"schema_version": 1, "annotation_policy_version": e.NORMALIZATION,
                    "partition": "development", "cases": [gold("AB CD", [(0, 2), (3, 5)])]}
        with self.assertRaisesRegex(e.EvaluationError, "tile"):
            e.validate_gold(document, "dev")

    def test_extraction_is_separate_and_urls_are_ignored(self):
        text = "AB; CD"
        row = gold(text, [(0, 4), (4, 6)])
        row["extraction_cores"] = [{"start": 4, "end": 6, "text": "CD"}]
        extracted = {"citations": [{"span": {"start": 4, "end": 6, "text": "CD"}, "url": None}]}
        detail = e.score_case(row, raw(text, [(0, 6)]), extracted)
        self.assertFalse(detail["exact_partition"])
        self.assertEqual(detail["extraction"]["f1"], 1)
        extracted["citations"][0]["url"] = "https://invented.invalid/path"
        self.assertEqual(e.score_case(row, raw(text, [(0, 6)]), extracted)["extraction"], detail["extraction"])

    def test_zero_denominator_conventions(self):
        self.assertEqual(e.metric([], [])["f1"], 1)
        self.assertEqual(e.metric([], [(0, 1)])["f1"], 0)
        self.assertEqual(e.metric([(0, 1)], [])["f1"], 0)

    def test_invalid_prediction_never_passes_exact_or_losslessness(self):
        detail = e.score_case(gold("AB", [(0, 2)]), {"status": "abstain", "parts": [], "delimiters": []})
        summary = e.summarize([detail])
        self.assertEqual(summary["exact_partition_count"], 0)
        self.assertFalse(summary["mandatory_losslessness_pass"])

    def test_missing_cli_is_truthful(self):
        with self.assertRaisesRegex(e.EvaluationError, "no parser was run"):
            e.resolve_cli("/invented-no-such-executable/legal-citations")

    def test_holdout_gate_does_not_read_holdout(self):
        with patch.object(e, "read_json", side_effect=AssertionError("no file should be read")):
            with self.assertRaisesRegex(e.EvaluationError, "requires --final-incumbent"):
                e.verify_incumbent(None, "a", "b")

    def test_offsets_do_not_accept_bool_or_out_of_bounds(self):
        for start, end in ((False, 1), (0, 8), (-1, 1)):
            with self.assertRaises(e.EvaluationError):
                e.span_pair({"start": start, "end": end}, "A", "invalid")

    def test_batch_protocol_rejects_malformed_envelopes(self):
        requests = [{"id": "x", "method": "splitSources", "request": {"text": "A"}}]
        for output in ('[]\n', 'null\n', '{"id":"x","result":{},"error":{}}\n',
                       '{"id":"x"}\n', '{"id":"y","result":{}}\n'):
            completed = subprocess.CompletedProcess(["invented-double", "batch"], 0, output, "")
            with patch.object(e.subprocess, "run", return_value=completed):
                with self.assertRaises(e.EvaluationError):
                    e.run_batch(Path("invented-double"), requests, 1)

    def test_batch_timeout_exit_and_duplicate_ids_fail(self):
        requests = [{"id": "x", "method": "splitSources", "request": {"text": "A"}}]
        with patch.object(e.subprocess, "run", side_effect=subprocess.TimeoutExpired("invented-double", 1)):
            with self.assertRaisesRegex(e.EvaluationError, "execution failed"):
                e.run_batch(Path("invented-double"), requests, 1)
        for code, output in ((3, ""), (0, '{"id":"x","result":{}}\n' * 2)):
            with patch.object(e.subprocess, "run", return_value=subprocess.CompletedProcess([], code, output, "")):
                with self.assertRaises(e.EvaluationError):
                    e.run_batch(Path("invented-double"), requests, 1)

    def test_same_coordinates_across_cases_never_cross_match(self):
        first = e.score_case(gold("AB CD", [(0, 3), (3, 5)]), raw("AB CD", [(0, 5)]))
        second = e.score_case(gold("XY ZZ", [(0, 5)]), raw("XY ZZ", [(0, 2), (3, 5)]))
        self.assertEqual(e.summarize([first, second])["spans"]["true_positives"], 0)

    def test_main_holdout_gate_and_one_shot_precede_fixture_access(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(e, "ROOT", root), patch.object(e, "verify_harness", return_value="h"), \
                 patch.object(e, "sha256", return_value="a" * 64), \
                 patch.object(e, "source_inventory_hash", return_value="s"), \
                 patch.object(e, "resolve_cli", return_value=root / "invented-not-executed"), \
                 patch.object(e, "checked_fixture", side_effect=AssertionError("sealed fixture must not be opened")) as fixture:
                self.assertEqual(e.main(["--split", "holdout", "--output", str(root / "no-lock.json")]), 2)
                with patch.object(e, "verify_incumbent", side_effect=e.EvaluationError("invalid lock")):
                    self.assertEqual(e.main(["--split", "holdout", "--final-incumbent", str(root / "lock.json"),
                                             "--output", str(root / "bad-lock.json")]), 2)
                (root / "sealed-evaluation-started.json").write_text("{}")
                with patch.object(e, "verify_incumbent", return_value="f"):
                    self.assertEqual(e.main(["--split", "holdout", "--final-incumbent", str(root / "lock.json"),
                                             "--output", str(root / "already-used.json")]), 2)
                fixture.assert_not_called()


if __name__ == "__main__":
    unittest.main()
