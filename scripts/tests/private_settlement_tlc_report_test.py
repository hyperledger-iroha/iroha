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
RESULT_CONTRACT = ROOT / "scripts" / "formal" / "sumeragi_v2_tlc_result_contract.sh"
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


def action_property_negative_log(
    *, generated: int = 12, distinct: int = 8, depth: int = 4
) -> str:
    """Return a minimal complete TLC action-property counterexample."""

    return run_header() + (
        f"{MODULE.ACTION_PROPERTY_VIOLATION_MARKER}\n"
        f"{MODULE.VIOLATION_BEHAVIOR_MARKER}\n"
        "State 1: <Initial predicate>\n"
        "State 2: <CrashCommitteeAt>\n"
        f"{generated} states generated, {distinct} distinct states found, "
        "0 states left on queue.\n"
        f"The depth of the state graph search is {depth}.\n"
        "Finished in 3s at (2026-08-30 12:35:01)\n"
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

        action_property = MODULE.parse_run(
            name="action-property-negative.cfg",
            model="Negative.tla",
            expected_outcome="action_property_violation",
            stdout=action_property_negative_log(),
            stderr="",
            status=13,
            seed=20260829,
            fingerprint_index=0,
            workers="4",
            tlc_version="2.19",
        )
        self.assertEqual(
            action_property.observed_outcome, "action_property_violation"
        )
        self.assertEqual(action_property.generated_states, 12)

    def test_action_property_result_contract_is_distinct_and_fail_closed(self) -> None:
        with self.assertRaisesRegex(
            MODULE.ReportError, "exact action-property violation"
        ):
            MODULE.parse_run(
                name="action-property-wrong-status.cfg",
                model="Negative.tla",
                expected_outcome="action_property_violation",
                stdout=action_property_negative_log(),
                stderr="",
                status=12,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )

        missing_trace_header = action_property_negative_log().replace(
            f"{MODULE.VIOLATION_BEHAVIOR_MARKER}\n", ""
        )
        with self.assertRaisesRegex(
            MODULE.ReportError, "exact action-property violation"
        ):
            MODULE.parse_run(
                name="action-property-missing-trace.cfg",
                model="Negative.tla",
                expected_outcome="action_property_violation",
                stdout=missing_trace_header,
                stderr="",
                status=13,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )

        with self.assertRaisesRegex(MODULE.ReportError, "exact Safety violation"):
            MODULE.parse_run(
                name="action-property-as-safety.cfg",
                model="Negative.tla",
                expected_outcome="safety_violation",
                stdout=action_property_negative_log(),
                stderr="",
                status=13,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )

        unrelated_failure = action_property_negative_log().replace(
            "State 1: <Initial predicate>\n",
            "Error: unrelated TLC failure\nState 1: <Initial predicate>\n",
        )
        with self.assertRaisesRegex(MODULE.ReportError, "unexpected diagnostics"):
            MODULE.parse_run(
                name="action-property-unrelated-error.cfg",
                model="Negative.tla",
                expected_outcome="action_property_violation",
                stdout=unrelated_failure,
                stderr="",
                status=13,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )

    def test_shell_action_property_result_contract_uses_status_13(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "action-property.log"
            log.write_text(action_property_negative_log(), encoding="utf-8")
            command = (
                'source "$1"; '
                'sumeragi_v2_tlc_assert_action_property_violation '
                '"$2" "$3" "$4" "$5"'
            )
            invocation = [
                "bash",
                "-c",
                command,
                "bash",
                str(RESULT_CONTRACT),
                "action-property",
                str(log),
            ]
            accepted = subprocess.run(
                [*invocation, "13", MODULE.ACTION_PROPERTY_VIOLATION_MARKER],
                check=False,
                text=True,
                capture_output=True,
            )
            self.assertEqual(accepted.returncode, 0, accepted.stderr)

            wrong_status = subprocess.run(
                [*invocation, "12", MODULE.ACTION_PROPERTY_VIOLATION_MARKER],
                check=False,
                text=True,
                capture_output=True,
            )
            self.assertEqual(wrong_status.returncode, 1)
            self.assertIn("expected action-property status 13", wrong_status.stderr)

            log.write_text(
                action_property_negative_log().replace(
                    "State 1: <Initial predicate>\n",
                    "Error: unrelated TLC failure\nState 1: <Initial predicate>\n",
                ),
                encoding="utf-8",
            )
            extra_diagnostic = subprocess.run(
                [*invocation, "13", MODULE.ACTION_PROPERTY_VIOLATION_MARKER],
                check=False,
                text=True,
                capture_output=True,
            )
            self.assertEqual(extra_diagnostic.returncode, 1)
            self.assertIn("exactly its primary", extra_diagnostic.stderr)

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

        unrelated_failure = negative_log().replace(
            MODULE.SAFETY_VIOLATION_MARKER,
            MODULE.SAFETY_VIOLATION_MARKER + "\nError: unrelated TLC failure",
        )
        with self.assertRaisesRegex(MODULE.ReportError, "unexpected diagnostics"):
            MODULE.parse_run(
                name="unrelated-negative-error.cfg",
                model="Negative.tla",
                expected_outcome="safety_violation",
                stdout=unrelated_failure,
                stderr="",
                status=12,
                seed=20260829,
                fingerprint_index=0,
                workers="4",
                tlc_version="2.19",
            )

        reordered = positive_log().replace(
            "The depth of the complete state graph search is 42.\n"
            f"{MODULE.SUCCESS_MARKER}\n",
            f"{MODULE.SUCCESS_MARKER}\n"
            "The depth of the complete state graph search is 42.\n",
        )
        with self.assertRaisesRegex(MODULE.ReportError, "out of order"):
            MODULE.parse_run(
                name="reordered.cfg",
                model="Positive.tla",
                expected_outcome="pass",
                stdout=reordered,
                stderr="",
                status=0,
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
                "3928371d12ea9523dec157a37ed466d729c17efe810131bc9648c2d7342f9ac8",
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
            java_version_path = root / "java-version.log"
            java_version_path.write_text(
                'openjdk version "21.0.8" 2025-07-15 LTS\n',
                encoding="utf-8",
            )
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
                if outcome == "pass":
                    stdout = positive_log(
                        generated=str(index + 10), distinct=str(index), depth=index
                    )
                elif outcome == "safety_violation":
                    stdout = negative_log(
                        generated=index + 10, distinct=index, depth=index
                    )
                else:
                    self.assertEqual(outcome, "action_property_violation")
                    stdout = action_property_negative_log(
                        generated=index + 10, distinct=index, depth=index
                    )
                prefix.with_suffix(prefix.suffix + ".stdout.log").write_text(
                    stdout, encoding="utf-8"
                )
                prefix.with_suffix(prefix.suffix + ".stderr.log").write_bytes(b"")
                prefix.with_suffix(prefix.suffix + ".status").write_text(
                    {
                        "pass": "0\n",
                        "safety_violation": "12\n",
                        "action_property_violation": "13\n",
                    }[outcome],
                    encoding="ascii",
                )

            report, transcript = MODULE.build_report(
                formal_dir=formal_dir,
                logs_dir=logs_dir,
                sany_dir=sany_dir,
                commit="a" * 40,
                tool_version="TLC 2.19 / TLA+ tools 1.7.4",
                tool_sha256="b" * 64,
                java_binary_sha256="c" * 64,
                java_binary_bytes=123456,
                java_version_output_path=java_version_path,
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
                    "evidence_code_sha256",
                    "java_runtime",
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
            self.assertEqual(report["java_runtime"]["binary_sha256"], "c" * 64)
            self.assertEqual(
                report["java_runtime"]["version_output"],
                'openjdk version "21.0.8" 2025-07-15 LTS\n',
            )

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
                    java_binary_sha256="c" * 64,
                    java_binary_bytes=123456,
                    java_version_output_path=java_version_path,
                    seed=20260829,
                    fingerprint_index=0,
                    workers="4",
                    transcript_artifact_path="evidence/logs/formal_model_report.log",
                )
            failed_sany.with_suffix(failed_sany.suffix + ".status").write_text(
                "0\n", encoding="ascii"
            )
            failed_sany.with_suffix(failed_sany.suffix + ".stdout.log").write_text(
                f"{MODULE.SANY_VERSION_MARKER}\n"
                f"Semantic processing of module {Path(MODULE.COUNT_MODEL).stem}\n"
                "Semantic error: injected diagnostic\n",
                encoding="utf-8",
            )

            java_version_path.write_text("fabricated runtime\n", encoding="utf-8")
            with self.assertRaisesRegex(MODULE.ReportError, "Java version output"):
                MODULE.build_report(
                    formal_dir=formal_dir,
                    logs_dir=logs_dir,
                    sany_dir=sany_dir,
                    commit="a" * 40,
                    tool_version="TLC 2.19 / TLA+ tools 1.7.4",
                    tool_sha256="b" * 64,
                    java_binary_sha256="c" * 64,
                    java_binary_bytes=123456,
                    java_version_output_path=java_version_path,
                    seed=20260829,
                    fingerprint_index=0,
                    workers="4",
                    transcript_artifact_path="evidence/logs/formal_model_report.log",
                )
            java_version_path.write_text(
                'openjdk version "21.0.8" 2025-07-15 LTS\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(MODULE.ReportError, "clean semantic result"):
                MODULE.build_report(
                    formal_dir=formal_dir,
                    logs_dir=logs_dir,
                    sany_dir=sany_dir,
                    commit="a" * 40,
                    tool_version="TLC 2.19 / TLA+ tools 1.7.4",
                    tool_sha256="b" * 64,
                    java_binary_sha256="c" * 64,
                    java_binary_bytes=123456,
                    java_version_output_path=java_version_path,
                    seed=20260829,
                    fingerprint_index=0,
                    workers="4",
                    transcript_artifact_path="evidence/logs/formal_model_report.log",
                )
            failed_sany.with_suffix(failed_sany.suffix + ".stdout.log").write_text(
                f"{MODULE.SANY_VERSION_MARKER}\n"
                f"Semantic processing of module {Path(MODULE.COUNT_MODEL).stem}\n",
                encoding="utf-8",
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
                    java_binary_sha256="c" * 64,
                    java_binary_bytes=123456,
                    java_version_output_path=java_version_path,
                    seed=20260829,
                    fingerprint_index=0,
                    workers="auto",
                    transcript_artifact_path="evidence/logs/formal_model_report.log",
                )
            with self.assertRaisesRegex(MODULE.ReportError, "explicit positive worker"):
                MODULE.build_report(
                    formal_dir=formal_dir,
                    logs_dir=logs_dir,
                    sany_dir=sany_dir,
                    commit="a" * 40,
                    tool_version="TLC 2.19 / TLA+ tools 1.7.4",
                    tool_sha256="b" * 64,
                    java_binary_sha256="c" * 64,
                    java_binary_bytes=123456,
                    java_version_output_path=java_version_path,
                    seed=20260829,
                    fingerprint_index=0,
                    workers="04",
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
        expected = list(MODULE.CONFIGURATIONS)
        self.assertEqual(observed, expected)
        self.assertIn(
            (
                "AtomicPrivateSettlementV1CommitteeFaults_commit_without_registration_bug.cfg",
                "safety_violation",
                MODULE.INDEXED_MODEL,
            ),
            expected,
        )
        self.assertIn(
            (
                "AtomicPrivateSettlementV1CommitteeFaults_drop_stage_bug.cfg",
                "action_property_violation",
                MODULE.INDEXED_MODEL,
            ),
            expected,
        )
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

        for option in ("--seed", "--fingerprint-index"):
            noncanonical = subprocess.run(
                ["bash", str(RUNNER), "--workers", "1", option, "00"],
                cwd=ROOT,
                env=environment,
                check=False,
                text=True,
                capture_output=True,
            )
            self.assertEqual(noncanonical.returncode, 2)
            self.assertIn("unsigned integer", noncanonical.stderr)

    def test_indexed_model_uses_liveness_safe_fault_canonicalization(self) -> None:
        model_path = ROOT / "formal" / "private_settlement" / MODULE.INDEXED_MODEL
        source = model_path.read_text(encoding="utf-8")

        self.assertIn('NetworkModes == {"Deliver", "Impaired"}', source)
        self.assertIn("CanonicalFaultValidator == 1", source)
        self.assertIn("InjectValidatorFault(leg, kind) ==", source)
        self.assertIn(
            '!.validatorStatus[leg][CanonicalFaultValidator] = kind,', source
        )
        self.assertIn("RestoreValidator(leg) ==", source)
        self.assertIn(
            'st.validatorStatus[leg][CanonicalFaultValidator] # "Honest"', source
        )
        self.assertIn("ImpairChannel(channel, leg) ==", source)
        self.assertIn('!.networkMode[channel][leg] = "Impaired",', source)
        self.assertNotIn("InjectValidatorFault(leg, validator, kind) ==", source)
        self.assertNotIn("RestoreValidator(leg, validator) ==", source)
        self.assertNotIn("ChannelFaultKinds", source)
        self.assertNotIn("ImpairChannel(channel, leg, mode) ==", source)
        self.assertNotIn('mode \\in {"Hold", "Drop", "Delay"}', source)
        self.assertIn("DurableStep ==", source)
        self.assertIn("APSDurabilityTemporal == [][DurableStep]_vars", source)
        self.assertIn("CertificateQuorumStep ==", source)
        self.assertIn(
            "APSCertificateQuorumTemporal == [][CertificateQuorumStep]_vars",
            source,
        )
        self.assertIn("CompleteBundleIdentity ==", source)
        self.assertIn("CompletePrepareRegistration ==", source)
        self.assertIn("RegisterCompletePrepareBundle ==", source)
        self.assertIn("OpenCommitWithoutPrepareRegistration ==", source)
        self.assertIn("APSPrepareRegistrationAndCommitBinding ==", source)
        self.assertIn(
            "APSPrepareRegistrationCrashRecoveryTemporal ==", source
        )
        self.assertIn(
            "!.prepareRegistration = CompletePrepareRegistration,", source
        )
        self.assertIn(
            "!.prepareRegistration = EmptyPrepareRegistration,", source
        )
        self.assertIn(
            "st.prepareRegistration = CompletePrepareRegistration", source
        )
        self.assertIn(
            "st'.commitBinding[leg] = CompleteBundleIdentity", source
        )
        self.assertIn("!.daVotes[leg] = {},", source)
        self.assertIn(
            "!.prepareVotes[leg] = [digest \\in Digests |-> {}],", source
        )
        self.assertIn(
            "!.commitVotes[leg] = [digest \\in Digests |-> {}],", source
        )
        self.assertNotIn("crashFloor", source)
        self.assertNotIn("crashedBoundaries", source)
        self.assertNotIn("faultIdentity", source)
        self.assertNotIn("height |->", source)
        self.assertNotIn("invalidRejected", source)

        formal_dir = model_path.parent
        for name, outcome, model in MODULE.CONFIGURATIONS:
            if model != MODULE.INDEXED_MODEL:
                continue
            config = (formal_dir / name).read_text(encoding="utf-8")
            self.assertNotIn("SYMMETRY", config)
            self.assertNotIn("VIEW", config)
            if name.endswith("commit_without_registration_bug.cfg"):
                self.assertIn(
                    "CommitWithoutPrepareRegistration = TRUE", config
                )
            else:
                self.assertIn(
                    "CommitWithoutPrepareRegistration = FALSE", config
                )
            if outcome == "pass":
                self.assertIn("APSDurabilityTemporal", config)
                self.assertIn(
                    "APSPrepareRegistrationCrashRecoveryTemporal", config
                )
                self.assertIn("APSCertificateQuorumTemporal", config)

    def test_count_model_includes_prepare_registration_lifecycle(self) -> None:
        model_path = ROOT / "formal" / "private_settlement" / MODULE.COUNT_MODEL
        source = model_path.read_text(encoding="utf-8")

        self.assertIn('BundleIdentities == {"None", "CompleteBundle"}', source)
        self.assertIn("RegisterCompletePrepareBundle ==", source)
        self.assertIn('prepareRegistration\' = "CompleteBundle"', source)
        self.assertIn('commitBundleIdentity\' = prepareRegistration', source)
        self.assertIn('prepareRegistration\' = "None"', source)
        self.assertIn("APSPrepareRegistrationCrashRecoveryTemporal ==", source)

        formal_dir = model_path.parent
        for name, outcome, model in MODULE.CONFIGURATIONS:
            if model != MODULE.COUNT_MODEL or outcome != "pass":
                continue
            config = (formal_dir / name).read_text(encoding="utf-8")
            self.assertIn("APSPrepareRegistrationCrashRecoveryTemporal", config)

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
        for source_path in MODULE.EVIDENCE_CODE_SOURCE_PATHS:
            (formal_dir / Path(source_path).name).write_text(
                f"# frozen evidence code for {source_path}\n",
                encoding="utf-8",
            )


if __name__ == "__main__":
    unittest.main()
