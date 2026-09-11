"""Pure arithmetic controls for explicit benchmark network-session groups.

Synthetic inputs below are not process, warmup, settlement or evidence validation.
"""
from pathlib import Path
import hashlib
import importlib.util
import math
import sys
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_benchmark_report", ROOT / "scripts/private_settlement_benchmark_report.py")
assert SPEC is not None and SPEC.loader is not None
REPORT = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = REPORT
SPEC.loader.exec_module(REPORT)


def identity(value):
    """Hash one synthetic identity without a runtime or network dependency."""
    return hashlib.sha256(str(value).encode()).hexdigest()


def groups(*clusters):
    """Give every synthetic observation a distinct registered-identity shape."""
    return {f"{index:064x}": {identity((index, ordinal)): value for ordinal, value in enumerate(values)}
            for index, values in enumerate(clusters)}


class SessionStatisticsTests(unittest.TestCase):
    def summarize(self, value, *, paired=False, **changes):
        options = {"binding": bytes.fromhex("ab" * 32), "bootstrap_iterations": 100, **changes}
        function = REPORT.summarize_paired_session_values if paired else REPORT.summarize_session_values
        return function(value, **options)

    def test_constant_values_preserve_cluster_and_observation_counts(self):
        result = self.summarize(groups([7, 7, 7], [7, 7]))
        self.assertEqual((result["session_count"], result["count"], result["mad"]), (2, 5, 0))
        for label in ("p50", "p95", "p99"):
            self.assertEqual(result[label], 7)
            self.assertEqual(result[label + "_ci95"], [7, 7])
            self.assertEqual(result[label + "_undefined_replicates"], 0)
        self.assertEqual(result["unconditional_quantiles"], "not_estimated")

    def test_group_and_observation_mapping_order_do_not_change_statistics(self):
        value = groups([1, 3, 10], [20, 30], [40])
        reverse = {key: dict(reversed(list(bucket.items()))) for key, bucket in reversed(list(value.items()))}
        self.assertEqual(self.summarize(value), self.summarize(reverse))

    def test_whole_unequal_clusters_are_repeated_without_equal_cluster_weighting(self):
        # All three zeros are retained when the first session is drawn. The
        # pooled original median is zero, rather than the cluster-median mean 5.
        schedule = [(0, 0)] * 50 + [(1, 1)] * 50
        with mock.patch.object(REPORT, "_bootstrap_session_draws", return_value=schedule):
            result = self.summarize(groups([0, 0, 0], [10]))
        self.assertEqual(result["p50"], 0)
        self.assertEqual(result["p50_ci95"], [0, 10])
        self.assertEqual(result["observations_per_session"], {f"{0:064x}": 3, f"{1:064x}": 1})

    def test_draw_schedule_uses_session_count_and_binding_deterministically(self):
        first = list(REPORT._bootstrap_session_draws(3, b"a" * 32, 100))
        self.assertEqual(first, list(REPORT._bootstrap_session_draws(3, b"a" * 32, 100)))
        self.assertNotEqual(first, list(REPORT._bootstrap_session_draws(3, b"b" * 32, 100)))
        self.assertEqual(len(first), 100)
        self.assertTrue(all(len(draw) == 3 and all(0 <= index < 3 for index in draw) for draw in first))
        self.assertTrue(any(len(set(draw)) < 3 for draw in first))

    def test_empty_session_is_retained_and_undefined_replicate_is_not_redrawn(self):
        with mock.patch.object(REPORT, "_bootstrap_session_draws", return_value=[(0, 0)] + [(1, 1)] * 99):
            result = self.summarize(groups([], [4, 5]))
        self.assertEqual(result["session_count"], 2)
        self.assertEqual(result["nonempty_session_count"], 1)
        self.assertEqual(result["p50"], 4.5)
        self.assertIsNone(result["p50_ci95"])
        self.assertEqual(result["p50_undefined_replicates"], 1)

    def test_each_draw_is_summarized_before_the_next_is_requested(self):
        for paired in (False, True):
            with self.subTest(paired=paired), mock.patch.object(REPORT, "percentile", wraps=REPORT.percentile) as percentile:
                def draws(*_):
                    for _ in range(100):
                        calls = percentile.call_count
                        yield (0, 1)
                        self.assertGreater(percentile.call_count, calls,
                                           "a resample must be summarized before requesting the next")
                with mock.patch.object(REPORT, "_bootstrap_session_draws", side_effect=draws):
                    value = groups([(2, 1)], [(4, 2)]) if paired else groups([1], [2])
                    self.summarize(value, paired=paired)

    def test_no_accepted_observations_produces_no_quantile_or_interval(self):
        result = self.summarize(groups([], []))
        self.assertEqual(result["count"], 0)
        self.assertIsNone(result["mad"])
        for label in ("p50", "p95", "p99"):
            self.assertIsNone(result[label])
            self.assertIsNone(result[label + "_ci95"])
            self.assertEqual(result[label + "_undefined_replicates"], 100)

    def test_pair_resampling_preserves_a_constant_within_pair_shift(self):
        result = self.summarize(groups([(6, 1), (7, 2)], [(105, 100), (205, 200)]), paired=True)
        self.assertEqual(result["paired_observation_count"], 4)
        for label in ("p50", "p95", "p99"):
            self.assertAlmostEqual(result["difference"][label], 5)
            for bound in result["difference"][label + "_ci95"]:
                self.assertAlmostEqual(bound, 5)

    def test_paired_ratio_is_a_ratio_of_marginal_quantiles(self):
        # A ratio of medians is 50.5 / 5.5, not median([100, 0.1]).
        value = groups([(100, 1)], [(1, 10)])
        result = self.summarize(value, paired=True)
        self.assertAlmostEqual(result["ratio"]["p50"], 50.5 / 5.5)
        self.assertNotAlmostEqual(result["ratio"]["p50"], 50.05)

    def test_constant_paired_scale_has_a_constant_ratio_interval(self):
        result = self.summarize(groups([(2, 1), (20, 10)], [(200, 100)]), paired=True)
        for label in ("p50", "p95", "p99"):
            self.assertAlmostEqual(result["ratio"][label], 2)
            for bound in result["ratio"][label + "_ci95"]:
                self.assertAlmostEqual(bound, 2)

    def test_pair_zero_denominators_preserve_undefined_ratios(self):
        value = groups([(4, 0)], [(8, 2)])
        with mock.patch.object(REPORT, "_bootstrap_session_draws", return_value=[(0, 0)] + [(1, 1)] * 99):
            result = self.summarize(value, paired=True)
        self.assertEqual(result["ratio"]["p50"], 6)
        self.assertIsNone(result["ratio"]["p50_ci95"])
        self.assertEqual(result["ratio"]["p50_undefined_replicates"], 1)
        self.assertIsNotNone(result["difference"]["p50_ci95"])

    def test_empty_pairs_and_overflow_never_produce_infinite_statistics(self):
        for value in (groups([], []), groups([(1e308, 1e-300)], [(1e308, 1e-300)])):
            result = self.summarize(value, paired=True)
            self.assertIsNone(result["ratio"]["p50"])
            self.assertIsNone(result["ratio"]["p50_ci95"])
            self.assertEqual(result["ratio"]["p50_undefined_replicates"], 100)

    def test_duplicate_observation_identity_in_different_sessions_is_rejected(self):
        value = groups([1], [2])
        value[f"{1:064x}"] = dict(value[f"{0:064x}"])
        with self.assertRaises(REPORT.EvidenceError):
            self.summarize(value)

    def test_nonfinite_boolean_negative_and_oversized_observations_are_rejected(self):
        for invalid in (math.inf, -math.inf, math.nan, True, -1, "1", 10 ** 1000):
            with self.subTest(value_type=type(invalid).__name__):
                with self.assertRaises(REPORT.EvidenceError):
                    self.summarize(groups([invalid], [2]))

    def test_input_identity_shapes_and_minimum_independence_are_required(self):
        for value in ({}, groups([1]), {0: {}, 1: {}}, {"a": {}, "b": {}},
                      {f"{0:064x}": [], f"{1:064x}": {}},
                      {f"{0:064x}": {"attempt": 1}, f"{1:064x}": {}}):
            with self.subTest(value=value):
                with self.assertRaises(REPORT.EvidenceError):
                    self.summarize(value)

    def test_binding_iterations_and_paired_shape_are_strict(self):
        for change in ({"binding": b""}, {"binding": "a" * 32}, {"binding": bytearray(32)},
                       {"bootstrap_iterations": True}, {"bootstrap_iterations": 99},
                       {"bootstrap_iterations": 100.0}):
            with self.subTest(change=change):
                with self.assertRaises(REPORT.EvidenceError):
                    self.summarize(groups([1], [2]), **change)
        for value in (1, (1,), (1, 2, 3), (True, 1), (1, math.inf)):
            with self.assertRaises(REPORT.EvidenceError):
                self.summarize(groups([value], [(1, 2)]), paired=True)


if __name__ == "__main__":
    unittest.main()
