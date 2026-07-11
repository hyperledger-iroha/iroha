"""Tests for the strict Kotodama Criterion regression gate."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_kotodama_perf", ROOT / "scripts" / "check_kotodama_perf.py"
)
assert SPEC is not None and SPEC.loader is not None
PERF = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = PERF
SPEC.loader.exec_module(PERF)


def write_estimate(path: Path, median: float) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps({"median": {"point_estimate": median}}), encoding="utf-8"
    )


def populate(root: Path, sample: str, multiplier: float = 1.0) -> None:
    for index, name in enumerate(PERF.REPRESENTATIVE_BENCHMARKS, start=1):
        write_estimate(
            root / name / sample / "estimates.json", index * 1000.0 * multiplier
        )


class KotodamaPerfGateTests(unittest.TestCase):
    """Exercise baseline capture, strict coverage, and the 5% ceiling."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_gate_accepts_five_percent_and_rejects_more(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new", 1.05)
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 0)

        populate(self.root, "new", 1.051)
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 1)

    def test_gate_fails_closed_on_missing_or_invalid_samples(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        missing = (
            self.root
            / PERF.REPRESENTATIVE_BENCHMARKS[0]
            / "new"
            / "estimates.json"
        )
        missing.unlink()
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 1)

        write_estimate(missing, float("nan"))
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 1)

    def test_checked_in_baseline_roundtrip_and_coverage(self) -> None:
        populate(self.root, "new")
        baseline = self.root / "baseline.json"
        self.assertEqual(
            PERF.main(
                [
                    "--criterion-dir",
                    str(self.root),
                    "--write-baseline",
                    str(baseline),
                ]
            ),
            0,
        )
        self.assertEqual(
            PERF.main(
                ["--criterion-dir", str(self.root), "--baseline", str(baseline)]
            ),
            0,
        )

        payload = json.loads(baseline.read_text(encoding="utf-8"))
        del payload["benchmarks"][PERF.REPRESENTATIVE_BENCHMARKS[0]]
        baseline.write_text(json.dumps(payload), encoding="utf-8")
        self.assertEqual(
            PERF.main(
                ["--criterion-dir", str(self.root), "--baseline", str(baseline)]
            ),
            1,
        )

        payload = {
            "schema": PERF.SCHEMA,
            "unit": "ns",
            "benchmarks": {
                name: 1_000.0 for name in PERF.REPRESENTATIVE_BENCHMARKS
            },
        }
        payload["benchmarks"]["unreviewed_benchmark"] = 1_000.0
        baseline.write_text(json.dumps(payload), encoding="utf-8")
        self.assertEqual(
            PERF.main(
                ["--criterion-dir", str(self.root), "--baseline", str(baseline)]
            ),
            1,
        )

    def test_threshold_cannot_be_loosened(self) -> None:
        comparisons = [PERF.Comparison("bench", 100.0, 100.0)]
        with self.assertRaisesRegex(PERF.GateError, "cannot be loosened"):
            PERF.enforce(comparisons, 0.051)

    def test_list_sugar_must_not_be_slower_than_the_manual_loop(self) -> None:
        samples = {
            PERF.LIST_SUGAR_BENCHMARK: 100.0,
            PERF.LIST_MANUAL_BENCHMARK: 100.0,
        }
        PERF.enforce_list_sugar(samples)

        samples[PERF.LIST_SUGAR_BENCHMARK] = 100.1
        with self.assertRaisesRegex(PERF.GateError, "manual-loop baseline"):
            PERF.enforce_list_sugar(samples)

        samples[PERF.LIST_SUGAR_BENCHMARK] = 99.9
        PERF.enforce_list_sugar(samples)

    def test_v1_list_amount_and_typed_query_samples_are_required(self) -> None:
        required = {
            PERF.LIST_SUGAR_BENCHMARK,
            PERF.LIST_MANUAL_BENCHMARK,
            "kotodama_list_get_64",
            "kotodama_amount_add",
            "kotodama_amount_div_exact",
            "kotodama_amount_div_round_nearest_even",
            "typed_core_query_accounts_page_64",
        }
        self.assertLessEqual(required, set(PERF.REPRESENTATIVE_BENCHMARKS))

        populate(self.root, "base")
        populate(self.root, "new")
        for name in required:
            sample = self.root / name / "new" / "estimates.json"
            sample.unlink()
            self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 1)
            write_estimate(sample, 1_000.0)

    def test_new_v1_benchmarks_require_current_evidence_but_not_fake_base_samples(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        candidate_only = set(PERF.REPRESENTATIVE_BENCHMARKS) - set(
            PERF.REGRESSION_BENCHMARKS
        )
        self.assertTrue(candidate_only)
        for name in candidate_only:
            (self.root / name / "base" / "estimates.json").unlink()
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 0)

        missing = next(iter(candidate_only))
        (self.root / missing / "new" / "estimates.json").unlink()
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 1)

    def test_baseline_capture_and_comparison_are_mutually_exclusive(self) -> None:
        with self.assertRaises(SystemExit):
            PERF.parse_args(
                [
                    "--baseline",
                    "baseline.json",
                    "--write-baseline",
                    "replacement.json",
                ]
            )

    def test_release_workflow_runs_the_complete_artifact_gate(self) -> None:
        workflow = (
            ROOT / ".github" / "workflows" / "kotodama_perf.yml"
        ).read_text(encoding="utf-8")
        marker = "      - name: Enforce complete artifact release gate\n"
        self.assertEqual(workflow.count(marker), 1)
        release_gate = workflow.split(marker, 1)[1]
        next_step = release_gate.find("\n      - name:")
        if next_step >= 0:
            release_gate = release_gate[:next_step]

        self.assertIn("scripts/regenerate_kotodama_goldens.py", release_gate)
        self.assertIn("--check", release_gate)
        self.assertIn(
            "--koto ../target-kotodama-perf/debug/koto", release_gate
        )
        self.assertIn(
            "--iroha ../target-kotodama-perf/debug/iroha", release_gate
        )
        self.assertNotIn("--skip-runtime-manifest-check", release_gate)
        self.assertNotIn("--skip-contract-tests", release_gate)

        build_marker = "      - name: Build canonical Kotodama release tools\n"
        self.assertEqual(workflow.count(build_marker), 1)
        build_step = workflow.split(build_marker, 1)[1].split(marker, 1)[0]
        self.assertIn(
            "cargo build -p ivm --bin koto -p iroha_cli --bin iroha",
            build_step,
        )
        self.assertIn("python3 scripts/check_kotodama_docs.py", build_step)
        self.assertIn(
            "--koto ../target-kotodama-perf/debug/koto", build_step
        )


if __name__ == "__main__":
    unittest.main()
