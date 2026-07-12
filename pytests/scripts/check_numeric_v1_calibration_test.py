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
        self.metadata = self.root / "reference-host.json"

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

    def write_metadata(self, **overrides: object) -> None:
        """Write a valid reference-host record with optional adversarial fields."""

        payload: dict[str, object] = {
            "format": CALIBRATION.REFERENCE_HOST_FORMAT,
            "hardware_model": CALIBRATION.EXPECTED_HARDWARE_MODEL,
            "chip": CALIBRATION.EXPECTED_CHIP,
            "architecture": CALIBRATION.EXPECTED_ARCHITECTURE,
            "runner_os": CALIBRATION.EXPECTED_RUNNER_OS,
            "runner_arch": CALIBRATION.EXPECTED_RUNNER_ARCH,
            "runner_name": "numeric-v1-m1-ultra-01",
            "rustc_release": CALIBRATION.EXPECTED_RUSTC_RELEASE,
            "rustc_host": CALIBRATION.EXPECTED_RUSTC_HOST,
            "rustc_commit_hash": CALIBRATION.EXPECTED_RUSTC_COMMIT_HASH,
            "rustc_commit_date": CALIBRATION.EXPECTED_RUSTC_COMMIT_DATE,
            "source_commit": "a" * 40,
            "release_tag": "v1.0.0-rc.1",
            "repository": "hyperledger/iroha",
            "workflow_ref": (
                "hyperledger/iroha/.github/workflows/"
                "numeric_v1_calibration.yml@refs/heads/release"
            ),
            "workflow_repository": "hyperledger/iroha",
            "workflow_sha": "a" * 40,
            "workflow_run_id": "1234",
            "workflow_run_attempt": "1",
        }
        payload.update(overrides)
        self.metadata.write_text(json.dumps(payload), encoding="utf-8")

    def main_arguments(self, *, output: Path | None = None) -> list[str]:
        """Return the complete fail-closed verifier command line."""

        arguments = [
            str(self.root),
            "--host-metadata",
            str(self.metadata),
            "--expected-commit",
            "a" * 40,
            "--expected-release-tag",
            "v1.0.0-rc.1",
            "--expected-repository",
            "hyperledger/iroha",
        ]
        if output is not None:
            arguments.extend(("--json-output", str(output)))
        return arguments

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
        self.write_metadata()
        add_ns, samples = CALIBRATION.evaluate_calibration(self.root)
        self.assertEqual(add_ns, 10.0)
        self.assertEqual(len(samples), CALIBRATION.MIN_NUMERIC_SAMPLES)
        self.assertEqual(samples[0].safety_adjusted_ratio, 1.0)
        self.assertEqual(samples[0].allowed_ratio, 1.0)
        self.assertTrue(
            any(sample.safety_adjusted_ratio == 1.25 for sample in samples)
        )
        output = self.root / "report.json"
        self.assertEqual(CALIBRATION.main(self.main_arguments(output=output)), 0)
        report = json.loads(output.read_text(encoding="utf-8"))
        self.assertTrue(report["accepted"])
        self.assertEqual(report["format"], CALIBRATION.REPORT_FORMAT)
        self.assertEqual(report["reference_host"]["hardware_model"], "Mac13,2")
        self.assertRegex(report["criterion_estimates_sha256"], r"^[0-9a-f]{64}$")

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

    def test_reference_host_metadata_is_mandatory_and_exact(self) -> None:
        self.complete_fixture()
        with self.assertRaisesRegex(CALIBRATION.CalibrationError, "invalid reference-host"):
            CALIBRATION.load_reference_host_metadata(
                self.metadata,
                expected_commit="a" * 40,
                expected_release_tag="v1.0.0-rc.1",
                expected_repository="hyperledger/iroha",
            )

        for field, invalid in (
            ("hardware_model", "Mac14,13"),
            ("chip", "Apple M2 Ultra"),
            ("architecture", "x86_64"),
            ("runner_os", "Linux"),
            ("runner_arch", "X64"),
            ("rustc_release", "1.93.0"),
            ("rustc_host", "x86_64-apple-darwin"),
            ("rustc_commit_hash", "0" * 40),
            ("rustc_commit_date", "2026-02-10"),
            ("source_commit", "b" * 40),
            ("release_tag", "v1.0.1"),
            ("repository", "fork/iroha"),
            ("workflow_repository", "fork/iroha"),
            ("workflow_sha", "b" * 40),
        ):
            with self.subTest(field=field):
                self.write_metadata(**{field: invalid})
                with self.assertRaisesRegex(
                    CALIBRATION.CalibrationError,
                    rf"{field} mismatch",
                ):
                    CALIBRATION.load_reference_host_metadata(
                        self.metadata,
                        expected_commit="a" * 40,
                        expected_release_tag="v1.0.0-rc.1",
                        expected_repository="hyperledger/iroha",
                    )

        self.write_metadata(workflow_ref="fork/other/.github/workflows/build.yml@main")
        with self.assertRaisesRegex(CALIBRATION.CalibrationError, "workflow_ref mismatch"):
            CALIBRATION.load_reference_host_metadata(
                self.metadata,
                expected_commit="a" * 40,
                expected_release_tag="v1.0.0-rc.1",
                expected_repository="hyperledger/iroha",
            )

    def test_reference_host_schema_and_run_identity_fail_closed(self) -> None:
        self.write_metadata(unexpected="value")
        with self.assertRaisesRegex(CALIBRATION.CalibrationError, "invalid schema"):
            CALIBRATION.load_reference_host_metadata(
                self.metadata,
                expected_commit="a" * 40,
                expected_release_tag="v1.0.0-rc.1",
                expected_repository="hyperledger/iroha",
            )

        self.write_metadata(runner_name="")
        with self.assertRaisesRegex(CALIBRATION.CalibrationError, "non-empty string"):
            CALIBRATION.load_reference_host_metadata(
                self.metadata,
                expected_commit="a" * 40,
                expected_release_tag="v1.0.0-rc.1",
                expected_repository="hyperledger/iroha",
            )

        self.write_metadata(workflow_run_attempt="0")
        with self.assertRaisesRegex(CALIBRATION.CalibrationError, "positive decimal"):
            CALIBRATION.load_reference_host_metadata(
                self.metadata,
                expected_commit="a" * 40,
                expected_release_tag="v1.0.0-rc.1",
                expected_repository="hyperledger/iroha",
            )

    def test_host_only_mode_does_not_require_criterion_evidence(self) -> None:
        self.write_metadata()
        empty_criterion = self.root / "does-not-exist"
        arguments = self.main_arguments()
        arguments[0] = str(empty_criterion)
        arguments.append("--validate-host-only")
        self.assertEqual(CALIBRATION.main(arguments), 0)

    def test_criterion_digest_binds_paths_and_raw_estimates(self) -> None:
        self.complete_fixture()
        before = CALIBRATION.criterion_estimates_sha256(self.root)
        self.estimate("ivm-numeric-limb-cal/op-0/limbs=1;work=4", 41.0)
        after = CALIBRATION.criterion_estimates_sha256(self.root)
        self.assertNotEqual(before, after)

    def test_rejection_report_records_the_fail_closed_reason(self) -> None:
        self.complete_fixture()
        self.write_metadata(rustc_release="1.92.0")
        output = self.root / "rejected.json"
        self.assertEqual(CALIBRATION.main(self.main_arguments(output=output)), 1)
        report = json.loads(output.read_text(encoding="utf-8"))
        self.assertFalse(report["accepted"])
        self.assertIn("rustc_release mismatch", report["error"])

    def test_release_workflow_keeps_the_reference_and_archive_gates(self) -> None:
        workflow = (ROOT / ".github/workflows/numeric_v1_calibration.yml").read_text(
            encoding="utf-8"
        )
        for required in (
            "workflow_dispatch:",
            "workflow_call:",
            "numeric-v1-release-calibration",
            "apple-m1-ultra",
            "mac13-2",
            "toolchain: 1.93.1",
            "GITHUB_REF_PROTECTED",
            "actions/attest-build-provenance@v2",
            'gh release upload "$EVIDENCE_RELEASE_TAG"',
            'asset.get("digest")',
        ):
            with self.subTest(required=required):
                self.assertIn(required, workflow)
        for forbidden in ("macos-14", "retention-days: 90", "push:\n    tags:"):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, workflow)

    def test_envelope_benchmarks_measure_snapshot_and_publication_work(self) -> None:
        """Calibration must cover the transport work named by its denominators."""

        benchmark = (ROOT / "crates/ivm/benches/gas_calibration.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("let snapshot = envelope.to_vec();", benchmark)
        self.assertIn(".alloc_host_tlv(&envelope)", benchmark)


if __name__ == "__main__":
    unittest.main()
