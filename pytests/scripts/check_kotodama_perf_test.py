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


if __name__ == "__main__":
    unittest.main()
