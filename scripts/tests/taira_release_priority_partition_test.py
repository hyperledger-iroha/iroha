#!/usr/bin/env python3
"""Pure exact-census controls for the early Taira native regression partition."""

import importlib.util
from pathlib import Path
import unittest


SOURCE = Path(__file__).resolve().parents[1] / "taira_release_check.py"
SPEC = importlib.util.spec_from_file_location("taira_release_check", SOURCE)
GATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GATE)


def names(stages):
    return tuple(name for _, tests in stages for name in tests)


class PriorityPartitionTests(unittest.TestCase):
    def test_both_scopes_preserve_the_complete_native_census(self):
        for scope in ("basic", "full"):
            with self.subTest(scope=scope):
                scoped = GATE.qualification_stages(scope)
                cli_first, cli_later = GATE.partition_priority_stages(
                    scoped["cli"], test_names=GATE.PRIORITY_CLI_TESTS)
                torii_first, torii_later = GATE.partition_priority_stages(
                    scoped["torii-unit"], stage_labels=GATE.PRIORITY_TORII_STAGE_LABELS)
                self.assertEqual(names(cli_first), GATE.PRIORITY_CLI_TESTS)
                self.assertEqual(tuple(label for label, _ in torii_first),
                                 GATE.PRIORITY_TORII_STAGE_LABELS)
                for original, first, later in (
                    (scoped["cli"], cli_first, cli_later),
                    (scoped["torii-unit"], torii_first, torii_later),
                ):
                    full = names(original)
                    self.assertEqual(len(full), len(set(full)))
                    self.assertEqual(set(names(first)) & set(names(later)), set())
                    self.assertCountEqual(names(first) + names(later), full)
                    self.assertTrue(all(tests for _, tests in first + later))
                original_total = sum(len(names(stages)) for stages in scoped.values())
                replacement_total = (original_total - len(names(scoped["cli"]))
                                     - len(names(scoped["torii-unit"]))
                                     + len(names(cli_first)) + len(names(cli_later))
                                     + len(names(torii_first)) + len(names(torii_later)))
                self.assertEqual(replacement_total, GATE.selected_regression_count(scope))

    def test_missing_or_duplicate_selectors_fail_closed(self):
        stages = (("one", ("test.one", "test.two")), ("two", ("test.three",)))
        with self.assertRaises(GATE.CheckError):
            GATE.partition_priority_stages(stages, test_names=("test.missing",))
        with self.assertRaises(GATE.CheckError):
            GATE.partition_priority_stages(stages, stage_labels=("missing",))
        with self.assertRaises(GATE.CheckError):
            GATE.partition_priority_stages(stages, test_names=("test.one", "test.one"))
        with self.assertRaises(GATE.CheckError):
            GATE.partition_priority_stages(stages, stage_labels=("one", "one"))
        with self.assertRaises(GATE.CheckError):
            GATE.partition_priority_stages((("one", ("test.one",)),
                                            ("two", ("test.one",))), test_names=("test.one",))
        with self.assertRaises(GATE.CheckError):
            GATE.partition_priority_stages((("one", ("test.one",)),
                                            ("one", ("test.two",))), stage_labels=("one",))

    def test_reduced_census_runs_every_selected_case_early_when_selector_is_absent(self):
        stages = (("synthetic", ("test.one", "test.two")),)
        for selectors in ({"test_names": ("test.missing",)},
                          {"stage_labels": ("missing",)}):
            with self.subTest(selectors=selectors):
                early, deferred = GATE.partition_priority_stages(
                    stages, whole_census_if_absent=True, **selectors)
                self.assertEqual(early, stages)
                self.assertEqual(deferred, ())


if __name__ == "__main__":
    unittest.main()
