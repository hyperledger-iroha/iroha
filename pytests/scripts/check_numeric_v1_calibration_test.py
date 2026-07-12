"""Tests for the Numeric V1 Criterion calibration verifier."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_numeric_v1_calibration",
    ROOT / "scripts" / "check_numeric_v1_calibration.py",
)
assert SPEC is not None and SPEC.loader is not None
CALIBRATION = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CALIBRATION
SPEC.loader.exec_module(CALIBRATION)


class NumericV1CalibrationTests(unittest.TestCase):
    """Exercise evidence discovery, normalization, and fail-closed limits."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def estimate(self, benchmark: str, median: float) -> None:
        """Write one minimal Criterion estimate fixture."""

        path = self.root / benchmark / "new" / "estimates.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps({"median": {"point_estimate": median}}),
            encoding="utf-8",
        )

    def complete_fixture(self, numeric_median: float = 40.0) -> None:
        """Write the harness and minimum required numeric samples."""

        self.estimate("ivm-gas-cal/EMPTY_HARNESS", 100.0)
        self.estimate("ivm-gas-cal/ADD", 500_100.0)
        required = sorted(CALIBRATION.REQUIRED_NUMERIC_BENCHMARKS)
        for label in required:
            denominator = (
                "gas"
                if "pipeline" in label
                else "work"
            )
            median = 32.0 if denominator == "gas" else numeric_median
            if label == "entry_control_pipeline":
                median += 100.0
            self.estimate(
                f"ivm-numeric-limb-cal/{label}/case;{denominator}=4",
                median,
            )
        for index in range(CALIBRATION.MIN_NUMERIC_SAMPLES - len(required)):
            self.estimate(
                f"ivm-numeric-limb-cal/op-{index}/limbs=1;work=4",
                numeric_median,
            )

    def test_accepts_safety_adjusted_ratios_at_or_below_four(self) -> None:
        self.complete_fixture()
        add_ns, samples = CALIBRATION.evaluate_calibration(self.root)
        self.assertEqual(add_ns, 10.0)
        self.assertEqual(len(samples), CALIBRATION.MIN_NUMERIC_SAMPLES)
        self.assertEqual(samples[0].safety_adjusted_ratio, 1.0)
        self.assertEqual(samples[0].allowed_ratio, 1.0)
        self.assertTrue(
            any(sample.safety_adjusted_ratio == 1.25 for sample in samples)
        )
        self.assertEqual(CALIBRATION.main([str(self.root)]), 0)

    def test_rejects_underpriced_or_incomplete_evidence(self) -> None:
        self.complete_fixture(numeric_median=129.0)
        with self.assertRaisesRegex(CALIBRATION.CalibrationError, "insufficient"):
            CALIBRATION.evaluate_calibration(self.root)

        missing = self.root / "ivm-gas-cal" / "EMPTY_HARNESS" / "new" / "estimates.json"
        missing.unlink()
        with self.assertRaisesRegex(CALIBRATION.CalibrationError, "EMPTY_HARNESS"):
            CALIBRATION.evaluate_calibration(self.root)

    def test_rejects_malformed_estimates_and_undeclared_denominators(self) -> None:
        self.complete_fixture()
        bad = (
            self.root
            / "ivm-numeric-limb-cal"
            / "op-0"
            / "limbs=1;work=4"
            / "new"
            / "estimates.json"
        )
        bad.write_text("{}", encoding="utf-8")
        with self.assertRaisesRegex(CALIBRATION.CalibrationError, "invalid Criterion"):
            CALIBRATION.evaluate_calibration(self.root)

        bad.unlink()
        self.estimate("ivm-numeric-limb-cal/no-denominator", 1.0)
        with self.assertRaisesRegex(CALIBRATION.CalibrationError, "does not declare"):
            CALIBRATION.evaluate_calibration(self.root)


if __name__ == "__main__":
    unittest.main()
