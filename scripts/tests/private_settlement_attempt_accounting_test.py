"""Strict benchmark terminal and persistent adapter transport regressions.

Subprocess boundaries are synthetic; all codecs, file publication, terminal
validation and existing successful-measurement checks execute their real code.
No test in this module launches Cargo or a network.
"""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from pathlib import Path
from unittest import mock

FIXTURE_SPEC = importlib.util.spec_from_file_location(
    "benchmark_transport_fixtures",
    Path(__file__).with_name("private_settlement_real_process_harness_test.py"),
)
assert FIXTURE_SPEC is not None and FIXTURE_SPEC.loader is not None
FIXTURES = importlib.util.module_from_spec(FIXTURE_SPEC)
FIXTURE_SPEC.loader.exec_module(FIXTURES)
HARNESS = FIXTURES.MODULE
ACCOUNTING = HARNESS.accounting


def terminal(request: dict, digest: str, kind: str = "succeeded") -> dict:
    """Build a synthetic V1 wire with the same complete successful fixture."""

    outcome = (
        {"kind": kind, "result": FIXTURES.rust_result(request, digest)}
        if kind == "succeeded" else
        {"kind": kind, "stage": "benchmark_worker", "reason": "execution_error"}
        if kind == "failed" else
        {"kind": kind, "stage": "private_receipt", "budget_ms": 1, "elapsed_ms": 1}
    )
    return {
        "version": 1, "protocol": ACCOUNTING.PROTOCOL,
        **{key: request[key] for key in ("request_id", "invocation_nonce", "commit", "participants")},
        "request_sha256": digest, "elapsed_ms": 1, "outcome": outcome,
    }


class BenchmarkTerminalTests(unittest.TestCase):
    """Reject wrong identities, contradictory exits and fabricated deadline causes."""

    def setUp(self) -> None:
        self.request = FIXTURES.request()
        self.digest = "1" * 64

    def validate(self, value: dict, exit_code: int = 0) -> dict:
        return ACCOUNTING.validate_benchmark_terminal(
            value, request=self.request, request_sha256=self.digest, exit_code=exit_code,
        )

    def test_all_declared_outcomes_and_deadline_stages(self) -> None:
        self.validate(terminal(self.request, self.digest))
        for reason in ACCOUNTING.WORKER_FAILURE_REASONS:
            value = terminal(self.request, self.digest, "failed")
            value["outcome"]["reason"] = reason
            self.validate(value, 101)
        for stage in ACCOUNTING.DEADLINE_STAGES:
            value = terminal(self.request, self.digest, "timed_out")
            value["outcome"]["stage"] = stage
            self.validate(value, 101)

    def test_identity_and_full_field_inventory_are_mandatory(self) -> None:
        valid = terminal(self.request, self.digest)
        for key in valid:
            value = copy.deepcopy(valid)
            del value[key]
            with self.subTest(missing=key), self.assertRaises(ACCOUNTING.AccountingError):
                self.validate(value)
        for key in ("request_id", "invocation_nonce", "request_sha256", "commit", "participants"):
            value = copy.deepcopy(valid)
            value[key] = 2 if key == "participants" else "b" * len(value[key])
            with self.subTest(substituted=key), self.assertRaises(ACCOUNTING.AccountingError):
                self.validate(value)
        for owner in (valid, valid["outcome"], valid["outcome"]["result"]):
            owner["unlisted"] = True
            with self.assertRaises(ACCOUNTING.AccountingError):
                self.validate(valid)
            del owner["unlisted"]
        valid["outcome"]["result"]["request_sha256"] = "2" * 64
        with self.assertRaises(ACCOUNTING.AccountingError):
            self.validate(valid)

    def test_exit_and_outcome_must_agree_without_boolean_coercion(self) -> None:
        for kind, code in (("succeeded", 101), ("failed", 0), ("timed_out", 0),
                           ("failed", -9), ("succeeded", False)):
            with self.subTest(kind=kind, code=code), self.assertRaises(ACCOUNTING.AccountingError):
                self.validate(terminal(self.request, self.digest, kind), code)
        for key, replacement in (("version", True), ("version", 1.0), ("participants", 3.0)):
            value = terminal(self.request, self.digest)
            value["outcome"]["result"][key] = replacement
            with self.subTest(key=key, replacement=replacement), self.assertRaises(ACCOUNTING.AccountingError):
                self.validate(value)

    def test_deadline_requires_a_typed_stage_and_actual_exhausted_duration(self) -> None:
        for key, replacement in (
            ("stage", "text says timed out"), ("budget_ms", 0), ("budget_ms", 2),
            ("elapsed_ms", 0), ("elapsed_ms", 2), ("elapsed_ms", True),
            ("budget_ms", 1.0), ("elapsed_ms", 1 << 64),
        ):
            value = terminal(self.request, self.digest, "timed_out")
            value["outcome"][key] = replacement
            with self.subTest(key=key, replacement=replacement), self.assertRaises(ACCOUNTING.AccountingError):
                self.validate(value, 101)
        value = terminal(self.request, self.digest, "failed")
        value["outcome"]["reason"] = "the request timed out"
        with self.assertRaises(ACCOUNTING.AccountingError):
            self.validate(value, 101)

    def test_non_success_cannot_include_measurements(self) -> None:
        for kind in ("failed", "timed_out"):
            value = terminal(self.request, self.digest, kind)
            value["outcome"]["result"] = FIXTURES.rust_result(self.request, self.digest)
            with self.subTest(kind=kind), self.assertRaises(ACCOUNTING.AccountingError):
                self.validate(value, 101)


class BenchmarkTransportTests(unittest.TestCase):
    """Exercise real retained files across unsuccessful synthetic process exits."""

    def exercise(self, root: Path, *, kind: str = "succeeded", build_exit: int = 0,
                 test_exit: int | None = None, missing: bool = False,
                 mutate=None, spawn_error: str | None = None) -> tuple[dict, dict, Path]:
        request = FIXTURES.request()
        raw = (json.dumps(request, sort_keys=True) + "\n").encode()
        digest = hashlib.sha256(raw).hexdigest()
        request_path = root / "request.json"
        request_path.write_bytes(raw)
        evidence = root / "evidence"
        evidence.mkdir(mode=0o700)
        validator = root / "validator"
        validator.write_bytes(b"synthetic executable bytes")
        wire = terminal(request, digest, kind)
        if mutate is not None:
            mutate(wire)
        exit_code = (0 if kind == "succeeded" else 101) if test_exit is None else test_exit

        def run(command, **kwargs):
            self.assertFalse(kwargs["check"])
            stage = command[1]
            if spawn_error == stage:
                raise OSError("synthetic spawn failure")
            if stage == "test" and not missing:
                HARNESS.publish_response(Path(kwargs["env"]["APS_REAL_PROCESS_RESULT"]), wire)
            return subprocess.CompletedProcess(command, build_exit if stage == "build" else exit_code)

        successful = kind == "succeeded" and build_exit == 0 and exit_code == 0 and not missing and mutate is None and spawn_error is None
        with mock.patch.object(HARNESS.subprocess, "run", side_effect=run), mock.patch.object(
            HARNESS, "VALIDATOR_EXECUTABLE", validator
        ), mock.patch.object(HARNESS.time, "monotonic_ns", side_effect=[1_000_000, 6_000_000]):
            if successful:
                result = HARNESS.run_rust_harness(request_path, raw, request, evidence)
                self.assertEqual(result, wire["outcome"]["result"])
            else:
                with self.assertRaises(HARNESS.HarnessError):
                    HARNESS.run_rust_harness(request_path, raw, request, evidence)
        directory = evidence / ACCOUNTING.BENCHMARK_PROTOCOL_DIRECTORY
        receipt = json.loads((directory / ACCOUNTING.ADAPTER_OUTCOME_FILE).read_text())
        ACCOUNTING.validate_adapter_outcome(receipt, request=request, request_sha256=digest)
        if build_exit == 0 and not missing and spawn_error is None:
            self.assertEqual(json.loads((directory / ACCOUNTING.RUST_TERMINAL_FILE).read_text()), wire)
        return request, receipt, evidence

    def test_success_retains_exact_terminal_and_passes_runner_join(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request, receipt, evidence = self.exercise(Path(temporary).resolve())
            result = HARNESS.runner.validate_benchmark_transport(
                evidence, request=request, request_sha256=receipt["request_sha256"],
            )
            self.assertEqual(receipt["status"], "succeeded")
            self.assertEqual(result["request_sha256"], receipt["request_sha256"])

    def test_failed_and_timed_out_terminals_survive_nonzero_process_exit(self) -> None:
        for kind in ("failed", "timed_out"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                request, receipt, evidence = self.exercise(Path(temporary).resolve(), kind=kind)
                self.assertEqual(receipt["status"], kind)
                self.assertEqual(receipt["exit_code"], 101)
                with self.assertRaises(HARNESS.runner.RunnerError):
                    HARNESS.runner.validate_benchmark_transport(
                        evidence, request=request, request_sha256=receipt["request_sha256"],
                    )

    def test_missing_terminal_is_incomplete_for_success_and_failure_exits(self) -> None:
        for code in (0, 101):
            with self.subTest(code=code), tempfile.TemporaryDirectory() as temporary:
                _, receipt, _ = self.exercise(Path(temporary).resolve(), test_exit=code, missing=True)
                self.assertEqual(receipt["status"], "incomplete")
                self.assertIsNone(receipt["rust_terminal"])

    def test_build_and_spawn_failures_have_bounded_retained_causes(self) -> None:
        for options, reason in (({"build_exit": 101}, "validator_build_failed"),
                                ({"spawn_error": "build"}, "validator_build_spawn_failed"),
                                ({"spawn_error": "test"}, "benchmark_process_spawn_failed")):
            with self.subTest(options=options), tempfile.TemporaryDirectory() as temporary:
                _, receipt, _ = self.exercise(Path(temporary).resolve(), **options)
                self.assertEqual((receipt["status"], receipt["reason"]), ("failed", reason))
                self.assertIsNone(receipt["rust_terminal"])

    def test_contradictory_and_malformed_terminals_remain_invalid(self) -> None:
        for options in ({"kind": "failed", "test_exit": 0},
                        {"mutate": lambda value: value.update(unlisted=True)},
                        {"mutate": lambda value: value["outcome"]["result"]["payload"].update(partial_visible_observations=1)}):
            with self.subTest(options=options), tempfile.TemporaryDirectory() as temporary:
                _, receipt, _ = self.exercise(Path(temporary).resolve(), **options)
                self.assertEqual(receipt["status"], "invalid")

    def test_transport_rejects_mutation_wrong_request_and_extra_evidence(self) -> None:
        for change in ("terminal", "request", "extra", "symlink"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                request, receipt, evidence = self.exercise(Path(temporary).resolve())
                directory = evidence / ACCOUNTING.BENCHMARK_PROTOCOL_DIRECTORY
                expected_sha = receipt["request_sha256"]
                if change == "terminal":
                    path = directory / ACCOUNTING.RUST_TERMINAL_FILE
                    path.write_bytes(path.read_bytes() + b" ")
                elif change == "request":
                    expected_sha = "f" * 64
                elif change == "extra":
                    (directory / "extra").write_text("undeclared")
                else:
                    path = directory / ACCOUNTING.RUST_TERMINAL_FILE
                    destination = directory / "held-result"
                    path.rename(destination)
                    path.symlink_to(destination)
                with self.assertRaises(HARNESS.runner.RunnerError):
                    HARNESS.runner.validate_benchmark_transport(
                        evidence, request=request, request_sha256=expected_sha,
                    )

    def test_runner_joins_actual_request_and_response_to_retained_terminal(self) -> None:
        for mutation in (None, "response_measurement", "request_digest"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                attempt = root / "attempt"
                request = FIXTURES.request()

                def launch(command, **kwargs):
                    self.assertTrue(kwargs["start_new_session"])
                    raw = (attempt / "request.json").read_bytes()
                    digest = hashlib.sha256(raw).hexdigest()
                    wire = terminal(request, digest)
                    response = HARNESS.build_response(request, wire["outcome"]["result"])
                    if mutation == "response_measurement":
                        response = copy.deepcopy(response)
                        response["payload"]["cpu_seconds"] = 9000.0
                    if mutation == "request_digest":
                        wire["request_sha256"] = "f" * 64
                        wire["outcome"]["result"]["request_sha256"] = "f" * 64
                    directory = attempt / "evidence" / ACCOUNTING.BENCHMARK_PROTOCOL_DIRECTORY
                    HARNESS.runner.fresh_private_directory(directory)
                    terminal_path = directory / ACCOUNTING.RUST_TERMINAL_FILE
                    HARNESS.publish_response(terminal_path, wire)
                    adapter = {
                        **{key: wire[key] for key in ACCOUNTING.RUST_TERMINAL_FIELDS - {"outcome", "elapsed_ms"}},
                        "elapsed_ms": 5, "phase": "measurement_validation", "exit_code": 0,
                        "rust_terminal": HARNESS.runner.file_binding(terminal_path),
                        "status": "succeeded", "reason": "rust_succeeded",
                    }
                    HARNESS.publish_response(directory / ACCOUNTING.ADAPTER_OUTCOME_FILE, adapter)
                    HARNESS.publish_response(attempt / "response.json", response)
                    return SimpleNamespace(pid=12345, wait=lambda timeout: 0, poll=lambda: 0)

                with mock.patch.object(HARNESS.runner.subprocess, "Popen", side_effect=launch), mock.patch.object(
                    HARNESS.runner, "_process_group_exists", return_value=False
                ):
                    if mutation is None:
                        response, _, _, _ = HARNESS.runner.invoke_harness(
                            root / "synthetic-harness", request, attempt_dir=attempt, timeout_seconds=1,
                            accounting_identity={"scope_sha256": "a" * 64, "campaign_id": "test",
                                                 "plan_sha256": "b" * 64, "attempt_id": ACCOUNTING.registered_attempt_id("a" * 64, "test", "b" * 64, request["request_id"])},
                        )
                        self.assertIs(response["passed"], True)
                    else:
                        with self.assertRaises(HARNESS.runner.RunnerError):
                            HARNESS.runner.invoke_harness(
                                root / "synthetic-harness", request, attempt_dir=attempt, timeout_seconds=1,
                            accounting_identity={"scope_sha256": "a" * 64, "campaign_id": "test",
                                                 "plan_sha256": "b" * 64, "attempt_id": ACCOUNTING.registered_attempt_id("a" * 64, "test", "b" * 64, request["request_id"])},
                            )
                outcome = json.loads((attempt / "response-outcome.json").read_text())
                self.assertEqual(outcome["passed"], mutation is None)


if __name__ == "__main__":
    unittest.main()
