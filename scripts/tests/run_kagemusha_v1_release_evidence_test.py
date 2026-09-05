"""Fail-closed tests for the KAGEMUSHA V1 release-evidence runner.

The fixtures use tiny reports and stub executables to test orchestration only;
they are not cryptographic proof or physical-device qualification evidence.
"""

from __future__ import annotations

import contextlib
import copy
import hashlib
import importlib.util
import io
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import types
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
RUNNER_PATH = ROOT / "scripts" / "run_kagemusha_v1_release_evidence.py"
SPEC = importlib.util.spec_from_file_location("run_kagemusha_v1_release_evidence", RUNNER_PATH)
assert SPEC is not None and SPEC.loader is not None
RUNNER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = RUNNER
SPEC.loader.exec_module(RUNNER)
RUNNER._bootstrap_local_modules(
    types.SimpleNamespace(
        python_executable_sha256=hashlib.sha256(
            RUNNER.RESOLVED_PYTHON.read_bytes()
        ).hexdigest(),
        release_verifier_sha256=hashlib.sha256(
            RUNNER.BUNDLED_RELEASE_VERIFIER.read_bytes()
        ).hexdigest(),
        artifact_contract_sha256=hashlib.sha256(
            RUNNER.BUNDLED_ARTIFACT_CONTRACT.read_bytes()
        ).hexdigest(),
    )
)
VERIFIER = RUNNER.release_verifier


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _file_sha256(path: Path) -> str:
    return _sha256(path.read_bytes())


def _write(path: Path, payload: bytes, mode: int = 0o600) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_bytes(payload)
    path.chmod(mode)
    return path


def _canonical(path: Path, value: object, mode: int = 0o600) -> Path:
    return _write(path, RUNNER.canonical_json_bytes(value), mode)


def _ed25519_secret_scalar(seed: bytes) -> tuple[int, bytes]:
    digest = hashlib.sha512(seed).digest()
    scalar_bytes = bytearray(digest[:32])
    scalar_bytes[0] &= 248
    scalar_bytes[31] &= 63
    scalar_bytes[31] |= 64
    return int.from_bytes(scalar_bytes, "little"), digest[32:]


def _ed25519_public_key(seed: bytes) -> bytes:
    scalar, _ = _ed25519_secret_scalar(seed)
    return VERIFIER._ed_encode(VERIFIER._ed_scalarmult(VERIFIER._ED_B, scalar))


def _ed25519_sign(seed: bytes, message: bytes) -> bytes:
    scalar, prefix = _ed25519_secret_scalar(seed)
    public_key = _ed25519_public_key(seed)
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little") % VERIFIER._ED_L
    encoded_r = VERIFIER._ed_encode(VERIFIER._ed_scalarmult(VERIFIER._ED_B, nonce))
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + message).digest(), "little"
    ) % VERIFIER._ED_L
    signature_scalar = (nonce + challenge * scalar) % VERIFIER._ED_L
    return encoded_r + signature_scalar.to_bytes(32, "little")


class _Capture:
    def __init__(self) -> None:
        self.buffer = io.BytesIO()

    def write(self, value: str) -> int:
        encoded = value.encode("utf-8")
        self.buffer.write(encoded)
        return len(value)

    def flush(self) -> None:
        pass

    def text(self) -> str:
        return self.buffer.getvalue().decode("utf-8")


@contextlib.contextmanager
def _small_matrix():
    with (
        mock.patch.object(VERIFIER, "ARTIFACT_ROLES", ("params_eq",)),
        mock.patch.object(VERIFIER, "RELATIONS", ("bootstrap",)),
        mock.patch.object(VERIFIER, "HELPERS", ("mint_authorization",)),
        mock.patch.object(VERIFIER, "RECEIVE_FOLD_BATCH_WIDTH", 16, create=True),
        mock.patch.object(VERIFIER, "ACCEPTANCE_CASES", ("receiver_inbox_pressure",)),
        mock.patch.object(
            RUNNER,
            "_derive_candidate_context",
            return_value=(
                {"schema": VERIFIER.CANDIDATE_CONTEXT_SCHEMA, "schema_version": 1},
                "33" * 32,
            ),
        ),
    ):
        yield


class RunnerFixture:
    def __init__(self, root: Path, *, producer_mode: str = "write") -> None:
        self.root = root
        self.root.mkdir(mode=0o700)
        self.inputs = self.root / "inputs"
        self.inputs.mkdir(mode=0o700)
        self.marker = self.root / "executed.marker"
        native_worker = shutil.which("tee")
        if native_worker is None:
            raise RuntimeError("tests require the platform's native tee executable")
        self.worker = Path(native_worker).resolve(strict=True)
        self.worker_sha256 = _file_sha256(self.worker)
        self.seed = bytes(range(1, 33))
        public_key = _ed25519_public_key(self.seed)
        self.authority_id = _sha256(VERIFIER.OBSERVER_AUTHORITY_ID_DOMAIN + public_key)
        self.policy = {
            "schema": VERIFIER.OBSERVER_POLICY_SCHEMA,
            "schema_version": 1,
            "threshold": 1,
            "authorities": [
                {
                    "authority_id": self.authority_id,
                    "ed25519_public_key": public_key.hex(),
                }
            ],
            "verifiers": [
                {
                    "id": VERIFIER.PHYSICAL_VERIFIER_ID,
                    "sha256": _file_sha256(VERIFIER.PHYSICAL_VERIFIER_PATH),
                    "report_schemas": ["iroha.kagemusha_v1.hardware_profile_qualification_report"],
                },
                {
                    "id": "trusted-stub-v1",
                    "sha256": self.worker_sha256,
                    "report_schemas": sorted(VERIFIER.REPORT_SCHEMAS),
                }
            ],
        }
        self.policy_path = _canonical(self.root / "observer-policy.json", self.policy)
        self.policy_sha256 = _file_sha256(self.policy_path)
        self.template = self._manifest_template()
        self.report_schemas = RUNNER._manifest_report_matrix(self.template)
        self.report_ids = {
            path: f"verify{index:03d}"
            for index, path in enumerate(sorted(self.report_schemas))
        }
        self.produced_report = sorted(self.report_schemas)[0]
        self.plan = self._plan(producer_mode)
        self.plan_path = self.root / "plan.json"
        self.write_plan()
        self.out_dir = self.root / "output"

    def execution_patch(self):
        return mock.patch.object(RUNNER, "_run_process", side_effect=self._simulate_process)

    def _simulate_process(
        self,
        executable: Path,
        arguments: list[str],
        *,
        cwd: Path,
        stdout_path: Path,
        stderr_path: Path,
        timeout_ms: int,
        transcript_limit: int,
        require_nonempty_streams: bool,
    ) -> object:
        del cwd, timeout_ms, transcript_limit, require_nonempty_streams
        if executable != self.worker:
            raise AssertionError("runner did not execute the pinned administrator tool")
        executable_info = executable.stat(follow_symlinks=False)
        if (
            _file_sha256(executable) != self.worker_sha256
            or executable_info.st_nlink != 1
            or executable_info.st_uid != 0
        ):
            raise AssertionError("administrator tool identity is not exact")
        _write(
            self.marker,
            self.marker.read_bytes() + b"x" if self.marker.exists() else b"x",
        )
        _write(stdout_path, b"trusted verifier stdout\n")
        _write(stderr_path, b"trusted verifier stderr\n")
        mode = arguments[0] if arguments else "verify"
        if mode in {"write", "extra", "nonzero", "missing", "timeout", "oversize"}:
            output = Path(arguments[1])
            if mode == "timeout":
                raise RUNNER.KagemushaRunnerError("command exceeded its timeout")
            if mode == "oversize":
                _write(output, b"x" * (4 * 1024 * 1024 + 1))
            elif mode != "missing":
                _canonical(
                    output,
                    {
                        "schema": arguments[3],
                        "schema_version": 1,
                        "verification_id": arguments[2],
                    },
                )
                if mode == "extra":
                    _write(output.with_name(output.name + ".extra"), b"unexpected\n")
            if mode == "nonzero":
                return RUNNER.ProcessResult(
                    7,
                    1,
                    1,
                    1,
                    1,
                    RUNNER.stable_hash_path(stdout_path),
                    RUNNER.stable_hash_path(stderr_path),
                )
        return RUNNER.ProcessResult(
            0,
            1,
            1,
            1,
            1,
            RUNNER.stable_hash_path(stdout_path),
            RUNNER.stable_hash_path(stderr_path),
        )

    def _hardware_profile(self) -> dict[str, object]:
        values: dict[str, object] = {}
        for field in VERIFIER._HARDWARE_PROFILE_FIELDS:
            if field in {
                "version",
                "protocol_version",
                "policy_epoch",
                "capability_mask",
                "valid_from_ms",
                "expires_at_ms",
            }:
                values[field] = 1
            elif field == "hardware_profile_id":
                values[field] = "11" * 32
            else:
                values[field] = "profile-value"
        return values

    def _manifest_template(self) -> dict[str, object]:
        profile = {
            "hardware_profile": self._hardware_profile(),
            "suite_id": "22" * 32,
            "qualification_report": "reports/profile/qualification.json",
            "physical_evidence": {
                "transcript": "physical/transcript.json", "attestation": "physical/attestation.bin",
                "trust_roots": "physical/roots.bin", "observer_policy": "physical/observer-policy.json",
                "oem_report": "reports/profile/oem-attestation.json",
            },
            "relations": [
                {"relation": name, "report": f"reports/profile/relation-{name}.json"}
                for name in VERIFIER.RELATIONS
            ],
            "helpers": [
                {"helper": name, "report": f"reports/profile/helper-{name}.json"}
                for name in VERIFIER.HELPERS
            ],
            "receive_fold_occupancies": [
                {
                    "occupancy": occupancy,
                    "report": f"reports/profile/receive-fold-{occupancy}.json",
                }
                for occupancy in range(1, 17)
            ],
            "recursive_depths": [
                {"depth": depth, "report": f"reports/profile/depth-{depth}.json"}
                for depth in (8, 64, 1024, 1025)
            ],
            "aggregate_balance": "reports/profile/aggregate.json",
            "thermal": "reports/profile/thermal.json",
            "envelope": "reports/profile/envelope.json",
            "acceptance_cases": [
                {"case": name, "report": f"reports/profile/case-{name}.json"}
                for name in VERIFIER.ACCEPTANCE_CASES
            ],
        }
        return {
            "schema": VERIFIER.MANIFEST_SCHEMA,
            "schema_version": 1,
            "source": {
                "source_archive": "source/candidate.tar",
                "cargo_lock": "source/Cargo.lock",
            },
            "artifacts": [
                {"role": role, "path": f"artifacts/{role}.bin"}
                for role in VERIFIER.ARTIFACT_ROLES
            ],
            "protocols": {},
            "global_reports": {
                "circuit_shape": "reports/global/circuit-shape.json",
                "security_review": "reports/global/security-review.json",
                "kat": "reports/global/kat.json",
                "fuzz": "reports/global/fuzz.json",
                "resource": "reports/global/resource.json",
            },
            "profiles": [profile],
            "reproducible_builds": [
                {"builder_id": f"{index:064x}", "report": f"reports/build-{index}.json"}
                for index in (1, 2)
            ],
        }

    def _seed_row(self, evidence_path: str, payload: bytes) -> dict[str, object]:
        source = _write(
            self.inputs / evidence_path.replace("/", "__"),
            payload,
        )
        physical = self.template["profiles"][0]["physical_evidence"]
        physical_kinds = {
            physical[name]: kind for name, kind in (
                ("transcript", "physical_transcript"), ("attestation", "oem_attestation"),
                ("trust_roots", "oem_trust_roots"), ("observer_policy", "observer_policy")
            )
        }
        return {
            "source": str(source),
            "evidence_path": evidence_path,
            "kind": physical_kinds.get(evidence_path) or ("report" if evidence_path in self.report_schemas else (
                "source_archive" if evidence_path.endswith(".tar") else (
                    "cargo_lock" if evidence_path.endswith("Cargo.lock") else "artifact"
                )
            )),
            "sha256": _sha256(payload),
            "byte_len": len(payload),
        }

    def _plan(self, producer_mode: str) -> dict[str, object]:
        seed_payloads: dict[str, bytes] = {
            "source/candidate.tar": b"candidate-source\n",
            "source/Cargo.lock": b"lock-source\n",
        }
        physical = self.template["profiles"][0]["physical_evidence"]
        seed_payloads.update({physical[name]: b"synthetic evidence" for name in (
            "transcript", "attestation", "trust_roots",
        )})
        seed_payloads[physical["observer_policy"]] = self.policy_path.read_bytes()
        for artifact in self.template["artifacts"]:
            seed_payloads[artifact["path"]] = f"{artifact['role']}\n".encode()
        for path, schema in self.report_schemas.items():
            if path != self.produced_report:
                seed_payloads[path] = RUNNER.canonical_json_bytes(
                    {
                        "schema": schema,
                        "schema_version": 1,
                        "verification_id": self.report_ids[path],
                    }
                )
        seed_rows = [
            self._seed_row(path, seed_payloads[path]) for path in sorted(seed_payloads)
        ]
        declared = sorted([*seed_payloads, self.produced_report])
        verification_steps = []
        for path in sorted(self.report_schemas):
            arguments = [{"file": path}]
            if path == sorted(self.report_schemas)[0]:
                arguments = [{"file": value} for value in declared]
            if path == physical["oem_report"]:
                arguments = [{"file": value} for value in sorted(physical.values())]
            elif path == self.template["profiles"][0]["qualification_report"]:
                arguments = [{"file": value} for value in sorted({path, *physical.values()})]
            step_id = self.report_ids[path]
            verification_steps.append(
                {
                    "id": step_id,
                    "verifier_id": "trusted-stub-v1",
                    "executable": {
                        "path": str(self.worker),
                        "sha256": self.worker_sha256,
                    },
                    "report_schema": self.report_schemas[path],
                    "report": path,
                    "arguments": arguments,
                    "stdout": f"transcripts/{step_id}.stdout",
                    "stderr": f"transcripts/{step_id}.stderr",
                    "observation": f"observations/{step_id}.json",
                    "timeout_ms": 2_000,
                }
            )
        return {
            "schema": RUNNER.PLAN_SCHEMA,
            "schema_version": 1,
            "manifest_template": self.template,
            "seed_files": seed_rows,
            "producer_steps": [
                {
                    "id": "producer000",
                    "executable": {
                        "path": str(self.worker),
                        "sha256": self.worker_sha256,
                    },
                    "arguments": [
                        {"literal": producer_mode},
                        {"output": self.produced_report},
                        {"literal": self.report_ids[self.produced_report]},
                        {"literal": self.report_schemas[self.produced_report]},
                    ],
                    "outputs": [
                        {
                            "evidence_path": self.produced_report,
                            "kind": "report",
                            "max_bytes": VERIFIER.MAX_REPORT_BYTES,
                        }
                    ],
                    "timeout_ms": 2_000 if producer_mode != "timeout" else 50,
                }
            ],
            "verification_steps": verification_steps,
        }

    def write_plan(self) -> None:
        _canonical(self.plan_path, self.plan)
        self.plan_sha256 = _file_sha256(self.plan_path)

    def common_args(self, command: str, *, out_dir: Path | None = None) -> list[str]:
        return [
            command,
            "--plan",
            str(self.plan_path),
            "--plan-sha256",
            self.plan_sha256,
            "--observer-policy",
            str(self.policy_path),
            "--observer-policy-sha256",
            self.policy_sha256,
            "--out-dir",
            str(out_dir or self.out_dir),
            "--python-executable-sha256",
            _file_sha256(RUNNER.RESOLVED_PYTHON),
            "--release-verifier-sha256",
            _file_sha256(RUNNER.BUNDLED_RELEASE_VERIFIER),
            "--artifact-contract-sha256",
            _file_sha256(RUNNER.BUNDLED_ARTIFACT_CONTRACT),
        ]

    def collect(self, *, dry_run: bool = False) -> tuple[int, str, str]:
        args = self.common_args("collect") + (["--dry-run"] if dry_run else [])
        return _call_main(args)

    def write_approvals(self, directory: Path, *, wrong_subject: bool = False) -> None:
        directory.mkdir(mode=0o700)
        requests = self.out_dir / "control" / "signing-requests"
        for request_path in sorted(requests.glob("*.json")):
            request = json.loads(request_path.read_text())
            subject = request["subject"]
            subject_sha256 = request["subject_sha256"]
            if wrong_subject and request_path == sorted(requests.glob("*.json"))[0]:
                subject_sha256 = "44" * 32
            signature = _ed25519_sign(self.seed, VERIFIER._approval_message(subject))
            approval = {
                "schema": RUNNER.DETACHED_APPROVAL_SCHEMA,
                "schema_version": 1,
                "collection_id": request["collection_id"],
                "command_id": request["command_id"],
                "subject_sha256": subject_sha256,
                "authority_id": self.authority_id,
                "signature": signature.hex(),
            }
            _canonical(directory / f"{request['command_id']}.json", approval)


def _call_main(args: list[str]) -> tuple[int, str, str]:
    stdout = _Capture()
    stderr = _Capture()
    with mock.patch.object(RUNNER.sys, "stdout", stdout), mock.patch.object(
        RUNNER.sys, "stderr", stderr
    ):
        code = RUNNER.main(args)
    return code, stdout.text(), stderr.text()


def _fake_projection(**kwargs: object) -> dict[str, object]:
    manifest_sha256 = kwargs["manifest_sha256"]
    return {
        "schema": VERIFIER.PROJECTION_SCHEMA,
        "schema_version": 1,
        "manifest_sha256": manifest_sha256,
        "receipt_projection": {
            "evidence_closure": {"candidate_context_digest": "33" * 32}
        },
    }


class KagemushaReleaseEvidenceRunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory(prefix=".kagemusha-runner-test-", dir=ROOT)
        self.temp = Path(self._temporary.name)
        self.temp.chmod(0o700)

    def tearDown(self) -> None:
        self._temporary.cleanup()

    def fixture(self, name: str, *, producer_mode: str = "write") -> RunnerFixture:
        return RunnerFixture(self.temp / name, producer_mode=producer_mode)

    def test_dry_run_is_canonical_and_executes_nothing(self) -> None:
        with _small_matrix():
            fixture = self.fixture("dry")
            with fixture.execution_patch() as execution:
                code, stdout, stderr = fixture.collect(dry_run=True)
            execution.assert_not_called()
        self.assertEqual(code, 0, stderr)
        self.assertFalse(fixture.marker.exists())
        self.assertFalse(fixture.out_dir.exists())
        projection = json.loads(stdout)
        self.assertEqual(projection["status"], "dry_run")
        self.assertEqual(stdout.encode(), RUNNER.canonical_json_bytes(projection))

    def test_physical_raw_evidence_is_required_before_any_execution(self) -> None:
        with _small_matrix():
            fixture = self.fixture("missing-physical")
            path = fixture.template["profiles"][0]["physical_evidence"]["attestation"]
            fixture.plan["seed_files"] = [row for row in fixture.plan["seed_files"] if row["evidence_path"] != path]
            fixture.write_plan()
            code, _, stderr = fixture.collect()
            self.assertEqual(code, 1)
            self.assertIn("physical evidence attestation must be declared", stderr)
            self.assertFalse(fixture.marker.exists())

    def test_physical_verification_inputs_must_be_exact_before_execution(self) -> None:
        with _small_matrix():
            fixture = self.fixture("wrong-physical-inputs")
            path = fixture.template["profiles"][0]["physical_evidence"]["oem_report"]
            step = next(row for row in fixture.plan["verification_steps"] if row["report"] == path)
            step["arguments"] = [{"file": path}]
            fixture.write_plan()
            code, _, stderr = fixture.collect()
            self.assertEqual(code, 1)
            self.assertIn("exactly its physical evidence inputs", stderr)
            self.assertFalse(fixture.marker.exists())

    def test_unknown_field_and_duplicate_job_fail_before_execution(self) -> None:
        with _small_matrix():
            fixture = self.fixture("schema")
            fixture.plan["unexpected"] = True
            fixture.write_plan()
            code, _, stderr = fixture.collect()
            self.assertEqual(code, 1)
            self.assertIn("fields must be exactly", stderr)
            self.assertFalse(fixture.marker.exists())
            self.assertFalse(fixture.out_dir.exists())

            fixture.plan.pop("unexpected")
            duplicate = copy.deepcopy(fixture.plan["verification_steps"][0])
            fixture.plan["verification_steps"].append(duplicate)
            fixture.write_plan()
            code, _, stderr = fixture.collect()
            self.assertEqual(code, 1)
            self.assertIn("more than one verification step", stderr)
            self.assertFalse(fixture.marker.exists())
            self.assertFalse(fixture.out_dir.exists())

    def test_executable_hash_and_symlink_are_rejected_before_execution(self) -> None:
        with _small_matrix():
            fixture = self.fixture("executable")
            fixture.plan["producer_steps"][0]["executable"]["sha256"] = "aa" * 32
            fixture.write_plan()
            code, _, stderr = fixture.collect()
            self.assertEqual(code, 1)
            self.assertIn("pinned SHA-256", stderr)
            self.assertFalse(fixture.marker.exists())

            fixture.plan["producer_steps"][0]["executable"]["sha256"] = fixture.worker_sha256
            link = fixture.root / "worker-link.py"
            link.symlink_to(fixture.worker)
            fixture.plan["producer_steps"][0]["executable"]["path"] = str(link)
            fixture.write_plan()
            code, _, stderr = fixture.collect()
            self.assertEqual(code, 1)
            self.assertIn("symlink", stderr)
            self.assertFalse(fixture.marker.exists())

    def test_script_writable_and_operator_owned_executables_fail_closed(self) -> None:
        with _small_matrix():
            script_fixture = self.fixture("script")
            script = _write(
                script_fixture.root / "candidate-script",
                b"#!/usr/bin/env python3\nraise SystemExit(0)\n",
                0o700,
            )
            script_fixture.plan["producer_steps"][0]["executable"] = {
                "path": str(script),
                "sha256": _file_sha256(script),
            }
            script_fixture.write_plan()
            code, _, stderr = script_fixture.collect()
            self.assertEqual(code, 1)
            self.assertIn("native executable", stderr)
            self.assertFalse(script_fixture.marker.exists())

            writable_fixture = self.fixture("writable")
            writable = writable_fixture.root / "writable-native"
            shutil.copyfile(writable_fixture.worker, writable)
            writable.chmod(0o722)
            writable_fixture.plan["producer_steps"][0]["executable"] = {
                "path": str(writable),
                "sha256": _file_sha256(writable),
            }
            writable_fixture.write_plan()
            code, _, stderr = writable_fixture.collect()
            self.assertEqual(code, 1)
            self.assertIn("writable", stderr)
            self.assertFalse(writable_fixture.marker.exists())

            operator_fixture = self.fixture("operator-owned")
            operator_tool = operator_fixture.root / "operator-native"
            shutil.copyfile(operator_fixture.worker, operator_tool)
            operator_tool.chmod(0o700)
            operator_fixture.plan["producer_steps"][0]["executable"] = {
                "path": str(operator_tool),
                "sha256": _file_sha256(operator_tool),
            }
            operator_fixture.write_plan()
            code, _, stderr = operator_fixture.collect()
            self.assertEqual(code, 1)
            self.assertIn("administrator-owned", stderr)
            self.assertFalse(operator_fixture.out_dir.exists())

    def test_producer_failures_publish_no_output(self) -> None:
        expected = {
            "nonzero": "exited with 7",
            "timeout": "timeout",
            "missing": "omitted output",
            "extra": "evidence tree differs",
            "oversize": "exceeds",
        }
        for mode, message in expected.items():
            with self.subTest(mode=mode), _small_matrix():
                fixture = self.fixture(f"producer-{mode}", producer_mode=mode)
                with fixture.execution_patch():
                    code, _, stderr = fixture.collect()
                self.assertEqual(code, 1)
                self.assertIn(message, stderr)
                self.assertFalse(fixture.out_dir.exists())

    def test_collect_stops_pending_and_finalize_rejects_substitution_and_mutation(self) -> None:
        with _small_matrix():
            fixture = self.fixture("rejection")
            with fixture.execution_patch():
                code, stdout, stderr = fixture.collect()
            self.assertEqual(code, RUNNER.EXIT_PENDING_APPROVALS, stderr)
            self.assertEqual(json.loads(stdout)["status"], "awaiting_approvals")
            self.assertFalse((fixture.out_dir / "manifest.json").exists())
            self.assertFalse((fixture.out_dir / "projection.json").exists())
            self.assertFalse((fixture.out_dir / "evidence" / "observations").exists())

            empty_approvals = fixture.root / "empty-approvals"
            empty_approvals.mkdir(mode=0o700)
            code, _, stderr = _call_main(
                fixture.common_args("finalize")
                + ["--approvals-dir", str(empty_approvals)]
            )
            self.assertEqual(code, 1)
            self.assertIn("invalid file count", stderr)
            self.assertFalse((fixture.out_dir / "manifest.json").exists())

            approvals = fixture.root / "bad-approvals"
            fixture.write_approvals(approvals, wrong_subject=True)
            code, _, stderr = _call_main(
                fixture.common_args("finalize") + ["--approvals-dir", str(approvals)]
            )
            self.assertEqual(code, 1)
            self.assertIn("substitutes its verification subject", stderr)
            self.assertFalse((fixture.out_dir / "manifest.json").exists())

            shutil.rmtree(approvals)
            fixture.write_approvals(approvals)
            changed = fixture.out_dir / "evidence" / "source" / "candidate.tar"
            changed.write_bytes(b"mutated-source\n")
            code, _, stderr = _call_main(
                fixture.common_args("finalize") + ["--approvals-dir", str(approvals)]
            )
            self.assertEqual(code, 1)
            self.assertIn("changed", stderr)
            self.assertFalse((fixture.out_dir / "manifest.json").exists())
            self.assertFalse((fixture.out_dir / "projection.json").exists())

    def test_tiny_approved_collection_publishes_only_after_projection_verification(self) -> None:
        with _small_matrix():
            fixture = self.fixture("happy")
            with fixture.execution_patch():
                code, _, stderr = fixture.collect()
            self.assertEqual(code, RUNNER.EXIT_PENDING_APPROVALS, stderr)
            approvals = fixture.root / "approvals"
            fixture.write_approvals(approvals)
            with mock.patch.object(
                RUNNER, "_run_projection_verifier", side_effect=_fake_projection
            ) as projector:
                code, stdout, stderr = _call_main(
                    fixture.common_args("finalize")
                    + ["--approvals-dir", str(approvals)]
                )
            self.assertEqual(code, 0, stderr)
            projector.assert_called_once()
            result = json.loads(stdout)
            self.assertEqual(result["status"], "verified")
            manifest_path = fixture.out_dir / "manifest.json"
            projection_path = fixture.out_dir / "projection.json"
            self.assertTrue(manifest_path.is_file())
            self.assertTrue(projection_path.is_file())
            manifest = json.loads(manifest_path.read_text())
            self.assertEqual(len(manifest["commands"]), len(fixture.report_schemas))
            self.assertEqual(
                len(list((fixture.out_dir / "evidence" / "observations").glob("*.json"))),
                len(fixture.report_schemas),
            )
            for command in manifest["commands"]:
                observation = json.loads(
                    (fixture.out_dir / "evidence" / command["observation"]).read_text()
                )
                self.assertEqual(len(observation["approvals"]), 1)
                self.assertEqual(
                    observation["approvals"][0]["authority_id"], fixture.authority_id
                )

    def test_fresh_main_authenticates_before_loading_local_modules(self) -> None:
        with _small_matrix():
            fixture = self.fixture("fresh-bootstrap")
            module_name = "run_kagemusha_v1_release_evidence_fresh_test"
            spec = importlib.util.spec_from_file_location(module_name, RUNNER_PATH)
            assert spec is not None and spec.loader is not None
            fresh = importlib.util.module_from_spec(spec)
            sys.modules[module_name] = fresh
            try:
                spec.loader.exec_module(fresh)
                self.assertIsNone(fresh.release_verifier)
                args = fixture.common_args("collect") + ["--dry-run"]
                digest_index = args.index("--python-executable-sha256") + 1
                args[digest_index] = "00" * 32
                stdout = _Capture()
                stderr = _Capture()
                with mock.patch.object(fresh.sys, "stdout", stdout), mock.patch.object(
                    fresh.sys, "stderr", stderr
                ):
                    code = fresh.main(args)
                self.assertEqual(code, 1)
                self.assertIn("must be canonical nonzero", stderr.text())
                self.assertIsNone(fresh.release_verifier)
            finally:
                sys.modules.pop(module_name, None)

    def test_direct_execution_requires_isolated_interpreter_flags(self) -> None:
        completed = subprocess.run(
            [str(RUNNER.RESOLVED_PYTHON), str(RUNNER_PATH)],
            cwd=ROOT,
            env=RUNNER.MINIMAL_ENVIRONMENT,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=10,
        )
        self.assertEqual(completed.returncode, 1)
        self.assertIn(b"requires an absolute Python interpreter", completed.stderr)
        self.assertNotIn(b"usage:", completed.stderr)

    def test_isolated_projector_bootstrap_loads_the_real_pinned_sources(self) -> None:
        command = [
            str(RUNNER.RESOLVED_PYTHON),
            "-I",
            "-B",
            "-S",
            "-c",
            RUNNER.PROJECTOR_BOOTSTRAP,
            str(RUNNER.SCRIPT_DIR),
            str(RUNNER.RESOLVED_PYTHON),
            _file_sha256(RUNNER.RESOLVED_PYTHON),
            str(RUNNER.BUNDLED_RELEASE_VERIFIER),
            _file_sha256(RUNNER.BUNDLED_RELEASE_VERIFIER),
            str(RUNNER.BUNDLED_ARTIFACT_CONTRACT),
            _file_sha256(RUNNER.BUNDLED_ARTIFACT_CONTRACT),
            "--help",
        ]
        completed = subprocess.run(
            command,
            cwd=ROOT,
            env=RUNNER.MINIMAL_ENVIRONMENT,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=10,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        self.assertIn(b"--manifest", completed.stdout)
        self.assertNotIn(b"ModuleNotFoundError", completed.stderr)

    def test_real_projection_path_rejects_an_invalid_manifest_after_loading(self) -> None:
        with _small_matrix():
            fixture = self.fixture("real-projector")
        out_dir = fixture.root / "projection-attempt"
        control = out_dir / "control"
        evidence = out_dir / "evidence"
        control.mkdir(parents=True, mode=0o700)
        evidence.mkdir(mode=0o700)
        manifest = _canonical(out_dir / "manifest.json", {})
        runtime_args = types.SimpleNamespace(
            observer_policy=fixture.policy_path,
            observer_policy_sha256=fixture.policy_sha256,
            python_executable_sha256=_file_sha256(RUNNER.RESOLVED_PYTHON),
            release_verifier_sha256=_file_sha256(
                RUNNER.BUNDLED_RELEASE_VERIFIER
            ),
            artifact_contract_sha256=_file_sha256(
                RUNNER.BUNDLED_ARTIFACT_CONTRACT
            ),
        )
        with self.assertRaisesRegex(
            RUNNER.KagemushaRunnerError, "release verifier rejected"
        ):
            RUNNER._run_projection_verifier(
                out_dir=out_dir,
                manifest_path=manifest,
                manifest_sha256=_file_sha256(manifest),
                evidence_root=evidence,
                observer_policy_path=fixture.policy_path,
                observer_policy_sha256=fixture.policy_sha256,
                toolchain=RUNNER._toolchain_closure(runtime_args),
            )
        stderr = (control / "final-verifier.stderr").read_bytes()
        self.assertNotIn(b"ModuleNotFoundError", stderr)
        self.assertIn(b"manifest", stderr)

    def test_transcript_capture_rejects_path_replacement(self) -> None:
        path = self.temp / "transcript.out"
        descriptor = os.open(path, os.O_RDWR | os.O_CREAT | os.O_EXCL, 0o600)
        try:
            os.write(descriptor, b"actual child output\n")
            os.fsync(descriptor)
            path.rename(self.temp / "unlinked-original.out")
            _write(path, b"substituted output\n")
            with self.assertRaisesRegex(
                RUNNER.KagemushaRunnerError, "replaced or mutated"
            ):
                RUNNER._stable_transcript_from_fd(
                    descriptor,
                    path,
                    transcript_limit=1024,
                    require_nonempty=True,
                )
        finally:
            os.close(descriptor)

    @unittest.skipUnless(hasattr(os, "fork"), "requires POSIX process groups")
    def test_real_process_capture_stops_background_descendants(self) -> None:
        marker = self.temp / "escaped-child.marker"
        source = (
            "import os,sys,time\n"
            "print('leader stdout', flush=True)\n"
            "print('leader stderr', file=sys.stderr, flush=True)\n"
            "if os.fork() == 0:\n"
            "    time.sleep(0.5)\n"
            "    open(sys.argv[1], 'wb').write(b'escaped')\n"
            "    os._exit(0)\n"
        )
        stdout_path = self.temp / "real.stdout"
        stderr_path = self.temp / "real.stderr"
        result = RUNNER._run_process(
            RUNNER.RESOLVED_PYTHON,
            ["-I", "-B", "-S", "-c", source, str(marker)],
            cwd=self.temp,
            stdout_path=stdout_path,
            stderr_path=stderr_path,
            timeout_ms=5_000,
            transcript_limit=1024,
            require_nonempty_streams=False,
        )
        self.assertEqual(result.exit_code, 0)
        self.assertEqual(result.stdout, RUNNER.stable_hash_path(stdout_path))
        self.assertEqual(result.stderr, RUNNER.stable_hash_path(stderr_path))
        time.sleep(0.7)
        self.assertFalse(marker.exists())


if __name__ == "__main__":
    unittest.main()
