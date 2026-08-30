"""Tests for deterministic AtomicPrivateSettlementV1 TLC evidence."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts" / "formal" / "private_settlement_tlc_report.py"
RUNNER = ROOT / "scripts" / "formal" / "run_atomic_private_settlement_tlc.sh"
SPEC = importlib.util.spec_from_file_location("private_settlement_tlc_report", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def run_header(*, seed: int = 20260829, fingerprint: int = 0, workers: int = 4) -> str:
    """Return deterministic TLC version and invocation headers."""

    return (
        "TLC2 Version 2.19 of 08 August 2024 (rev: test)\n"
        "Running breadth-first search Model-Checking "
        f"with fp {fingerprint} and seed {seed} with {workers} workers on test host.\n"
    )


def positive_log(
    *, generated: str = "1,234", distinct: str = "567", queued: str = "0", depth: int = 42
) -> str:
    """Return a minimal complete passing TLC transcript."""

    return run_header() + (
        f"{generated} states generated, {distinct} distinct states found, "
        f"{queued} states left on queue.\n"
        f"The depth of the complete state graph search is {depth}.\n"
        f"{MODULE.SUCCESS_MARKER}\n"
        "Finished in 1s at (2026-08-30 12:34:56)\n"
    )


def negative_log(*, generated: int = 9, distinct: int = 7, depth: int = 3) -> str:
    """Return a minimal complete expected-mutant TLC transcript."""

    return run_header() + (
        f"{MODULE.SAFETY_VIOLATION_MARKER}\n"
        "Error: The behavior up to this point is:\n"
        f"{generated} states generated, {distinct} distinct states found, 0 states left on queue.\n"
        f"The depth of the state graph search is {depth}.\n"
        "Finished in 2s at (2026-08-30 12:35:00)\n"
    )


class PrivateSettlementTlcReportTests(unittest.TestCase):
    """Exercise report parsing, hashing, and exact matrix generation."""

    def test_parse_positive_and_negative_results(self) -> None:
        passing = MODULE.parse_run(
            name="positive.cfg",
            model="Positive.tla",
            expected_outcome="pass",
            stdout=positive_log(),
            stderr="",
            status=0,
            seed=20260829,
            fingerprint_index=0,
            workers="4",
            tlc_version="2.19",
        )
        self.assertEqual(passing.generated_states, 1234)
        self.assertEqual(passing.distinct_states, 567)
        self.assertEqual(passing.depth, 42)
        self.assertEqual(passing.observed_outcome, "pass")

        negative = MODULE.parse_run(
            name="negative.cfg",
            model="Negative.tla",
            expected_outcome="safety_violation",
            stdout=negative_log(),
            stderr="",
            status=12,
            seed=20260829,
            fingerprint_index=0,
            workers="4",
            tlc_version="2.19",
        )
        self.assertEqual(negative.observed_outcome, "safety_violation")
        self.assertEqual(negative.generated_states, 9)

    def test_parse_rejects_stderr_wrong_status_and_trailing_output(self) -> None:
        with self.assertRaisesRegex(MODULE.ReportError, "separate stderr"):
            MODULE.parse_run(
                name="stderr.cfg",
                model="Positive.tla",
                expected_outcome="pass",
                stdout=positive_log(),
                stderr="warning\n",
                status=0,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )
        with self.assertRaisesRegex(MODULE.ReportError, "clean passing result"):
            MODULE.parse_run(
                name="status.cfg",
                model="Positive.tla",
                expected_outcome="pass",
                stdout=positive_log(),
                stderr="",
                status=1,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )
        with self.assertRaisesRegex(MODULE.ReportError, "continues after"):
            MODULE.parse_run(
                name="trailing.cfg",
                model="Positive.tla",
                expected_outcome="pass",
                stdout=positive_log() + "untrusted trailing text\n",
                stderr="",
                status=0,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )
        with self.assertRaisesRegex(MODULE.ReportError, "run controls differ"):
            MODULE.parse_run(
                name="seed-substitution.cfg",
                model="Positive.tla",
                expected_outcome="pass",
                stdout=positive_log(),
                stderr="",
                status=0,
                seed=20260830,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )
        with self.assertRaisesRegex(MODULE.ReportError, "retained queued states"):
            MODULE.parse_run(
                name="queued.cfg",
                model="Positive.tla",
                expected_outcome="pass",
                stdout=positive_log(queued="1"),
                stderr="",
                status=0,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )
        with self.assertRaisesRegex(MODULE.ReportError, "distinct state count"):
            MODULE.parse_run(
                name="impossible-count.cfg",
                model="Positive.tla",
                expected_outcome="pass",
                stdout=positive_log(generated="1", distinct="2"),
                stderr="",
                status=0,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )
        duplicate_primary = negative_log().replace(
            "Error: The behavior up to this point is:\n",
            "Error: Invariant Other is violated.\n"
            "Error: The behavior up to this point is:\n",
        )
        with self.assertRaisesRegex(MODULE.ReportError, "unexpected diagnostics"):
            MODULE.parse_run(
                name="multiple-primary-diagnostics.cfg",
                model="Negative.tla",
                expected_outcome="safety_violation",
                stdout=duplicate_primary,
                stderr="",
                status=12,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )

    def test_formal_package_digest_binds_models_configs_and_paths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            formal_dir = Path(directory)
            self._write_formal_inputs(formal_dir)
            original = MODULE.formal_package_sha256(formal_dir)
            self.assertEqual(
                original,
                "189ade59d8c92aadc413bb7c4283ef36b91a201600b82e038bdd9d734cc2ccc1",
            )
            changed_path = formal_dir / MODULE.CONFIGURATIONS[0][0]
            changed_path.write_text("changed\n", encoding="utf-8")
            changed = MODULE.formal_package_sha256(formal_dir)
            self.assertNotEqual(original, changed)

    def test_complete_matrix_report_has_exact_release_schema_and_binding(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            formal_dir = root / "formal"
            logs_dir = root / "logs"
            sany_dir = root / "sany"
            self._write_formal_inputs(formal_dir)
            logs_dir.mkdir()
            sany_dir.mkdir()
            for model in (MODULE.COUNT_MODEL, MODULE.INDEXED_MODEL):
                prefix = sany_dir / model
                prefix.with_suffix(prefix.suffix + ".stdout.log").write_text(
                    f"{MODULE.SANY_VERSION_MARKER}\n"
                    f"Semantic processing of module {Path(model).stem}\n",
                    encoding="utf-8",
                )
                prefix.with_suffix(prefix.suffix + ".stderr.log").write_bytes(b"")
                prefix.with_suffix(prefix.suffix + ".status").write_text(
                    "0\n", encoding="ascii"
                )
            for index, (name, outcome, _) in enumerate(MODULE.CONFIGURATIONS, start=1):
                prefix = logs_dir / name
                stdout = (
                    positive_log(generated=str(index + 10), distinct=str(index), depth=index)
                    if outcome == "pass"
                    else negative_log(generated=index + 10, distinct=index, depth=index)
                )
                prefix.with_suffix(prefix.suffix + ".stdout.log").write_text(
                    stdout, encoding="utf-8"
                )
                prefix.with_suffix(prefix.suffix + ".stderr.log").write_bytes(b"")
                prefix.with_suffix(prefix.suffix + ".status").write_text(
                    "0\n" if outcome == "pass" else "12\n", encoding="ascii"
                )

            report, transcript = MODULE.build_report(
                formal_dir=formal_dir,
                logs_dir=logs_dir,
                sany_dir=sany_dir,
                commit="a" * 40,
                tool_version="TLC 2.19 / TLA+ tools 1.7.4",
                tool_sha256="b" * 64,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                transcript_artifact_path="evidence/logs/formal_model_report.log",
            )
            self.assertEqual(
                set(report),
                {
                    "version",
                    "protocol",
                    "commit",
                    "tool",
                    "tool_version",
                    "tool_sha256",
                    "model_sha256",
                    "configurations",
                    "passed",
                    "transcript",
                },
            )
            self.assertEqual(
                [row["name"] for row in report["configurations"]],
                [name for name, _, _ in MODULE.CONFIGURATIONS],
            )
            self.assertEqual(report["transcript"]["bytes"], len(transcript))
            self.assertEqual(
                report["transcript"]["sha256"], hashlib.sha256(transcript).hexdigest()
            )
            self.assertIn(b"seed=20260829\n", transcript)
            self.assertIn(b"fingerprint_index=0\n", transcript)
            self.assertIn(b"workers=4\n", transcript)
            self.assertIn(b"===== SANY AtomicPrivateSettlementV1.tla", transcript)

            failed_sany = sany_dir / MODULE.COUNT_MODEL
            failed_sany.with_suffix(failed_sany.suffix + ".status").write_text(
                "1\n", encoding="ascii"
            )
            with self.assertRaisesRegex(MODULE.ReportError, "clean semantic result"):
                MODULE.build_report(
                    formal_dir=formal_dir,
                    logs_dir=logs_dir,
                    sany_dir=sany_dir,
                    commit="a" * 40,
                    tool_version="TLC 2.19 / TLA+ tools 1.7.4",
                    tool_sha256="b" * 64,
                    seed=20260829,
                    fingerprint_index=0,
                    workers="4",
                    transcript_artifact_path="evidence/logs/formal_model_report.log",
                )
            failed_sany.with_suffix(failed_sany.suffix + ".status").write_text(
                "0\n", encoding="ascii"
            )

            report_path = root / "formal_model_report.json"
            transcript_path = root / "formal_model_report.log"
            MODULE.write_report(
                report=report,
                transcript=transcript,
                report_output=report_path,
                transcript_output=transcript_path,
            )
            self.assertEqual(json.loads(report_path.read_text()), report)
            self.assertEqual(transcript_path.read_bytes(), transcript)
            with self.assertRaisesRegex(MODULE.ReportError, "refusing to replace"):
                MODULE.write_report(
                    report=report,
                    transcript=transcript,
                    report_output=report_path,
                    transcript_output=transcript_path,
                )

            with self.assertRaisesRegex(MODULE.ReportError, "explicit positive worker"):
                MODULE.build_report(
                    formal_dir=formal_dir,
                    logs_dir=logs_dir,
                    sany_dir=sany_dir,
                    commit="a" * 40,
                    tool_version="TLC 2.19 / TLA+ tools 1.7.4",
                    tool_sha256="b" * 64,
                    seed=20260829,
                    fingerprint_index=0,
                    workers="auto",
                    transcript_artifact_path="evidence/logs/formal_model_report.log",
                )

    def test_runner_lists_the_exact_matrix_without_a_toolchain(self) -> None:
        environment = {"PATH": "/usr/bin:/bin"}
        result = subprocess.run(
            ["bash", str(RUNNER), "--list-configs"],
            cwd=ROOT,
            env=environment,
            check=True,
            text=True,
            capture_output=True,
        )
        observed = [tuple(line.split("\t")) for line in result.stdout.splitlines()]
        expected = [(name, outcome) for name, outcome, _ in MODULE.CONFIGURATIONS]
        self.assertEqual(observed, expected)
        self.assertEqual(result.stderr, "")

        complete = subprocess.run(
            ["bash", str(RUNNER)],
            cwd=ROOT,
            env=environment,
            check=False,
            text=True,
            capture_output=True,
        )
        self.assertEqual(complete.returncode, 2)
        self.assertIn("explicit --workers count", complete.stderr)

    def test_runner_pins_the_candidate_before_invoking_the_toolchain(self) -> None:
        source = RUNNER.read_text(encoding="utf-8")
        capture = 'candidate_commit="$(git -C "$REPO_ROOT" rev-parse HEAD)"'
        self.assertLess(source.index(capture), source.index("readonly TLC=("))
        self.assertIn('commit="$candidate_commit"', source)
        self.assertGreaterEqual(source.count("assert_candidate_unchanged"), 3)
        self.assertIn(
            'install -m 600 -- "$REPORT_BUILDER" "$frozen_report_builder"', source
        )
        self.assertIn('python3 "$frozen_report_builder"', source)

    @staticmethod
    def _write_formal_inputs(formal_dir: Path) -> None:
        formal_dir.mkdir(parents=True, exist_ok=True)
        for name in [MODULE.COUNT_MODEL, MODULE.INDEXED_MODEL]:
            (formal_dir / name).write_text(f"---- MODULE {name} ----\n", encoding="utf-8")
        for name, _, _ in MODULE.CONFIGURATIONS:
            (formal_dir / name).write_text(f"CONSTANT Config = {name}\n", encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
