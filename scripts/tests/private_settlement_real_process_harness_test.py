"""Focused tests for the fail-closed private-settlement process harness."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "private_settlement_real_process_harness.py"
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_real_process_harness", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

COMMIT = "a" * 40
HARDWARE = "b" * 64
HARDWARE_PROFILE = "c" * 64
NONCE = "d" * 64


def request(
    participants: int = 3,
    *,
    profile: str = "private",
    warmup: bool = False,
    run: int = 0,
) -> dict[str, Any]:
    """Build one exact canonical benchmark request."""

    seeds = list(range(10))
    configuration = MODULE.runner.build_configuration(
        participants,
        seeds=seeds,
        warmups=MODULE.runner.MIN_WARMUPS,
        measured=MODULE.runner.MIN_MEASURED,
    )
    configuration_bytes = (
        json.dumps(configuration, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode()
    configuration_sha = hashlib.sha256(configuration_bytes).hexdigest()
    seed = seeds[run % len(seeds)]
    job = {
        "kind": "benchmark",
        "profile": profile,
        "participants": participants,
        "seed": seed,
        "run": run,
        "warmup": warmup,
        "configuration_sha256": configuration_sha,
    }
    return {
        "version": MODULE.runner.VERSION,
        "protocol": MODULE.runner.PROTOCOL,
        "request_id": MODULE.runner.object_digest(job),
        "invocation_nonce": NONCE,
        "kind": "benchmark",
        "commit": COMMIT,
        "hardware_sha256": HARDWARE,
        "hardware_profile_sha256": HARDWARE_PROFILE,
        "configuration_sha256": configuration_sha,
        "participants": participants,
        "validators_per_dataspace": 4,
        "global_validators": 4,
        "quorum": "3-of-4",
        "mandatory_signed_rs16_da_rbc": True,
        "minimum_signed_rs16_da_observations": (participants + 1) * 4,
        "authenticated_message_control": True,
        "seed": seed,
        "run": run,
        "configuration": configuration,
        "payload": {
            "profile": profile,
            "warmup": warmup,
            "stages": list(MODULE.benchmark_stages(profile)),
            "resources": list(MODULE.runner.benchmark_report.RESOURCE_FIELDS),
        },
    }


def inventory(participants: int) -> list[dict[str, Any]]:
    identities: list[tuple[str, int | None, int | None]] = [
        ("coordinator", None, None)
    ]
    identities.extend(("global_validator", None, ordinal) for ordinal in range(4))
    identities.extend(
        ("dataspace_validator", dataspace, ordinal)
        for dataspace in range(participants)
        for ordinal in range(4)
    )
    return [
        {
            "role": role,
            "dataspace_ordinal": dataspace,
            "validator_ordinal": validator,
            "pid": 10_000 + index,
            "executable_sha256": ("e" if role == "coordinator" else "f") * 64,
            "revision": COMMIT,
            "health_observed": True,
        }
        for index, (role, dataspace, validator) in enumerate(identities)
    ]


def rust_result(bound_request: dict[str, Any], request_sha: str) -> dict[str, Any]:
    participants = bound_request["participants"]
    profile = bound_request["payload"]["profile"]
    return {
        "version": 1,
        "protocol": MODULE.runner.PROTOCOL,
        "request_id": bound_request["request_id"],
        "invocation_nonce": bound_request["invocation_nonce"],
        "request_sha256": request_sha,
        "commit": bound_request["commit"],
        "participants": participants,
        "mandatory_signed_rs16_da_rbc": True,
        "signed_rs16_da_observations": (participants + 1) * 4,
        "authenticated_message_control": True,
        "process_inventory": inventory(participants),
        "payload": {
            "stages_ms": {
                stage: 1.0
                for stage in MODULE.benchmark_stages(profile)
            },
            "throughput_bundles_per_second": 1.0,
            "cpu_seconds": 1.0,
            "peak_rss_bytes": 1,
            "network_bytes": 1,
            "proof_bytes": 1 if profile == "private" else 0,
            "receipt_bytes": 1,
            "storage_growth_bytes": 0,
            "finalized_receipt_observed": True,
            "successful_leg_applications": participants,
            "each_leg_applied_exactly_once": True,
            "partial_visible_observations": 0,
            "partial_spendable_observations": 0,
        },
    }


class PrivateSettlementRealProcessHarnessTests(unittest.TestCase):
    """Exercise strict request/result bindings without launching Cargo."""

    def test_exact_argument_contract_rejects_reordering_and_extras(self) -> None:
        parsed = MODULE.parse_exact_arguments(
            [
                "--aps-request",
                "request.json",
                "--aps-response",
                "response.json",
                "--aps-evidence-dir",
                "evidence",
            ]
        )
        self.assertEqual(parsed, (Path("request.json"), Path("response.json"), Path("evidence")))
        with self.assertRaisesRegex(MODULE.HarnessError, "expected exactly"):
            MODULE.parse_exact_arguments(
                [
                    "--aps-response",
                    "response.json",
                    "--aps-request",
                    "request.json",
                    "--aps-evidence-dir",
                    "evidence",
                ]
            )
        with self.assertRaisesRegex(MODULE.HarnessError, "expected exactly"):
            MODULE.parse_exact_arguments(
                [
                    "--aps-request",
                    "request.json",
                    "--aps-response",
                    "response.json",
                    "--aps-evidence-dir",
                    "evidence",
                    "extra",
                ]
            )

    def test_all_release_participant_shapes_validate_deterministically(self) -> None:
        for profile in MODULE.runner.PROFILES:
            for participants in MODULE.runner.PARTICIPANTS:
                fixture = request(participants, profile=profile)
                self.assertEqual(MODULE.validate_request(copy.deepcopy(fixture)), fixture)

    def test_unsupported_kinds_fail_before_execution(self) -> None:
        fault = request()
        fault["kind"] = "fault"
        with self.assertRaisesRegex(MODULE.HarnessError, "benchmark requests"):
            MODULE.validate_request(fault)
        leakage = request()
        leakage["kind"] = "leakage"
        with self.assertRaisesRegex(MODULE.HarnessError, "benchmark requests"):
            MODULE.validate_request(leakage)

    def test_each_profile_requires_its_exact_stage_inventory(self) -> None:
        private = request()
        private["payload"]["stages"] = ["global_finality", "end_to_end"]
        with self.assertRaisesRegex(MODULE.HarnessError, "canonical profile"):
            MODULE.validate_request(private)
        transparent = request(profile="transparent_control")
        transparent["payload"]["stages"] = list(
            MODULE.runner.benchmark_report.REQUIRED_PRIVATE_STAGES
        )
        with self.assertRaisesRegex(MODULE.HarnessError, "canonical profile"):
            MODULE.validate_request(transparent)

    def test_configuration_job_and_hardware_bindings_reject_substitution(self) -> None:
        malformed = request()
        malformed["hardware_profile_sha256"] = "0" * 64
        with self.assertRaisesRegex(MODULE.HarnessError, "must be non-zero"):
            MODULE.validate_request(malformed)
        configuration = request()
        configuration["configuration"]["participants"] = 4
        with self.assertRaisesRegex(MODULE.HarnessError, "not the canonical"):
            MODULE.validate_request(configuration)
        job = request()
        job["request_id"] = "1" * 64
        with self.assertRaisesRegex(MODULE.HarnessError, "does not bind"):
            MODULE.validate_request(job)

    def test_rust_result_cannot_be_reused_across_request_or_nonce(self) -> None:
        for profile in MODULE.runner.PROFILES:
            bound = request(profile=profile)
            raw = (json.dumps(bound, sort_keys=True) + "\n").encode()
            request_sha = hashlib.sha256(raw).hexdigest()
            result = rust_result(bound, request_sha)
            MODULE.validate_rust_result(result, request=bound, request_sha=request_sha)
            stale = copy.deepcopy(result)
            stale["request_sha256"] = "1" * 64
            with self.assertRaisesRegex(MODULE.HarnessError, "exact invocation"):
                MODULE.validate_rust_result(stale, request=bound, request_sha=request_sha)
            stale_nonce = copy.deepcopy(result)
            stale_nonce["invocation_nonce"] = "2" * 64
            with self.assertRaisesRegex(MODULE.HarnessError, "exact invocation"):
                MODULE.validate_rust_result(
                    stale_nonce, request=bound, request_sha=request_sha
                )

    def test_transparent_control_accepts_zero_proof_bytes_but_private_does_not(self) -> None:
        transparent = request(profile="transparent_control")
        transparent_result = rust_result(transparent, "3" * 64)
        MODULE.validate_rust_result(
            transparent_result, request=transparent, request_sha="3" * 64
        )
        private = request()
        private_result = rust_result(private, "4" * 64)
        private_result["payload"]["proof_bytes"] = 0
        with self.assertRaisesRegex(MODULE.HarnessError, "proof_bytes"):
            MODULE.validate_rust_result(
                private_result, request=private, request_sha="4" * 64
            )

    def test_rust_result_requires_validator_scaled_da_and_complete_inventory(self) -> None:
        bound = request(8)
        request_sha = "3" * 64
        too_few = rust_result(bound, request_sha)
        too_few["signed_rs16_da_observations"] = 1
        with self.assertRaisesRegex(MODULE.HarnessError, "per-validator"):
            MODULE.validate_rust_result(too_few, request=bound, request_sha=request_sha)
        missing = rust_result(bound, request_sha)
        missing["process_inventory"].pop()
        with self.assertRaisesRegex(MODULE.HarnessError, "topology mismatch"):
            MODULE.validate_rust_result(missing, request=bound, request_sha=request_sha)

    def test_atomic_publication_never_replaces_existing_response(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            response_path = Path(temporary) / "response.json"
            MODULE.publish_response(response_path, {"passed": True})
            original = response_path.read_bytes()
            with self.assertRaisesRegex(MODULE.HarnessError, "appeared"):
                MODULE.publish_response(response_path, {"passed": False})
            self.assertEqual(response_path.read_bytes(), original)

    def test_canonical_paths_pin_a_symlinked_ancestor_before_use(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            real = root / "real"
            real.mkdir()
            (real / "request.json").write_text("{}", encoding="utf-8")
            (real / "evidence").mkdir()
            alias = root / "alias"
            alias.symlink_to(real, target_is_directory=True)
            request_path, response_path, evidence_path = MODULE.canonicalize_paths(
                alias / "request.json",
                alias / "response.json",
                alias / "evidence",
            )
            canonical_real = real.resolve()
            self.assertEqual(request_path, canonical_real / "request.json")
            self.assertEqual(response_path, canonical_real / "response.json")
            self.assertEqual(evidence_path, canonical_real / "evidence")

    def test_canonical_paths_reject_a_symlinked_final_component(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = root / "request.json"
            request.write_text("{}", encoding="utf-8")
            request_alias = root / "request-alias.json"
            request_alias.symlink_to(request)
            evidence = root / "evidence"
            evidence.mkdir()
            with self.assertRaisesRegex(MODULE.HarnessError, "symbolic links"):
                MODULE.canonicalize_paths(
                    request_alias,
                    root / "response.json",
                    evidence,
                )

    def test_static_rust_path_uses_real_processes_controller_and_signed_da(self) -> None:
        localnet = (
            ROOT
            / "integration_tests/tests/nexus/atomic_private_settlement_localnet.rs"
        ).read_text(encoding="utf-8")
        harness = (
            ROOT
            / "integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs"
        ).read_text(encoding="utf-8")
        self.assertIn("atomic_private_settlement_real_process_benchmark_harness", harness)
        self.assertIn("with_consensus_message_control", harness)
        self.assertIn("wait_until_ready", harness)
        self.assertIn("process_id()", harness)
        self.assertIn("get_bridge_finality_anchor", harness)
        self.assertIn("SumeragiV2GenesisContextParameters::recommended().da_layout", harness)
        self.assertIn("private_settlement_committee_proof_v1", harness)
        self.assertIn("impl Drop for ProcessResourceSampler", harness)
        self.assertIn("impl Drop for TransparentControlAtomicityObserver", harness)
        self.assertIn("classify_transparent_control_values", harness)
        self.assertIn("atomicity_clients.len() == shape.peer_count()", harness)
        self.assertIn("atomicity_observer.begin()", harness)
        self.assertIn("atomicity_observer.finish(3)", harness)
        self.assertIn("run_real_process_transparent_control_benchmark", harness)
        self.assertIn("DvpIsi::new", harness)
        self.assertNotIn("Transfer::asset", harness)
        self.assertIn("request.participants - 1", harness)
        self.assertIn("CanExecuteSettlement", localnet)
        self.assertIn("wait_for_identical_native_amx_receipt", harness)
        self.assertIn("proof_bytes: 0", harness)
        self.assertIn(
            'include!("atomic_private_settlement_real_process_harness.rs")', localnet
        )


if __name__ == "__main__":
    unittest.main()
