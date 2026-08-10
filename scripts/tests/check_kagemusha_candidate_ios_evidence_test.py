"""Tests for strict signed physical-iOS Kagemusha evidence."""

from __future__ import annotations

import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from typing import Any, Callable


SCRIPT_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_kagemusha_candidate_ios_evidence as checker  # noqa: E402
import kagemusha_candidate_ios_evidence as evidence_lib  # noqa: E402
import sign_kagemusha_candidate_ios_evidence as signer  # noqa: E402


def sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def write_private(path: Path, payload: bytes) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    path.parent.chmod(0o700)
    path.write_bytes(payload)
    path.chmod(0o600)


def write_json(path: Path, value: Any) -> None:
    write_private(path, evidence_lib.canonical_json_bytes(value))


def digest(path: Path) -> str:
    return sha256(path.read_bytes())


def nonzero_digest(label: str) -> str:
    return sha256(label.encode("ascii"))


def network_samples(base: int) -> list[dict[str, Any]]:
    labels = [
        "callback",
        "before",
        "through_before_native",
        "through_after_native",
        "after",
    ]
    return [
        {
            "label": label,
            "monotonic_nanos": base + index,
            "status": "unsatisfied",
            "expensive": False,
            "constrained": False,
            "wifi": False,
            "cellular": False,
            "wired_ethernet": False,
            "loopback": False,
        }
        for index, label in enumerate(labels)
    ]


def device(session: dict[str, Any]) -> dict[str, Any]:
    return {
        "physical": True,
        "simulator": False,
        "platform": "ios",
        "hardware_model": session["expected_hardware_model"],
        "board_config": session["expected_board_config"],
        "os_version": session["expected_os_version"],
        "os_build": session["expected_os_build"],
        "udid_sha256": session["device_udid_sha256"],
        "ecid_sha256": session["device_ecid_sha256"],
        "serial_sha256": session["device_serial_sha256"],
        "identifier_for_vendor_sha256": nonzero_digest("identifier-for-vendor"),
        "boot_session_sha256": nonzero_digest("boot-session"),
    }


def code_identity() -> dict[str, Any]:
    return {
        "app_bundle_id": "org.hyperledger.iroha.KagemushaCandidateEvidenceLab",
        "app_version": "1.0",
        "app_build": "1",
        "app_executable_sha256": nonzero_digest("app-executable"),
        "test_bundle_id": "org.hyperledger.iroha.KagemushaCandidateEvidenceLabTests",
        "test_executable_sha256": nonzero_digest("test-executable"),
    }


class Fixture:
    def __init__(self, base: Path, private_key: Path, public_key: Path) -> None:
        self.base = base
        self.raw = base / "raw"
        self.signed_dir = base / "signed"
        self.evidence = self.signed_dir / "signed-evidence-v1.json"
        self.private_key = private_key
        self.public_key = public_key
        self.key_id = "ios-lab-key-1"
        self.raw.mkdir(mode=0o700)
        self.signed_dir.mkdir(mode=0o700)
        for relative in evidence_lib.EXPECTED_RAW_ARTIFACT_PATHS:
            write_private(self.raw / relative, f"fixture:{relative}\n".encode("ascii"))
        for path in self.raw.rglob("*"):
            if path.is_dir():
                path.chmod(0o700)
        self._populate_structured_artifacts()

    def _populate_structured_artifacts(self) -> None:
        source_commit = "a" * 40
        source_tree = nonzero_digest("source-tree")
        tracked_diff = sha256(b"")
        untracked_manifest_digest = sha256(b"")
        combined = hashlib.sha256()
        combined.update(evidence_lib.SOURCE_DIFF_DOMAIN)
        combined.update(evidence_lib.TRACKED_DIFF_DOMAIN)
        combined.update(bytes.fromhex(tracked_diff))
        combined.update(evidence_lib.UNTRACKED_MANIFEST_DOMAIN)
        combined.update(bytes.fromhex(untracked_manifest_digest))
        write_json(
            self.raw / "input/reviewed-source-closure-v1.json",
            {
                "schema": evidence_lib.REVIEWED_SOURCE_CLOSURE_SCHEMA,
                "base_commit": source_commit,
                "source_commit": source_commit,
                "source_repo_dirty": False,
                "source_tree_sha256": source_tree,
                "tracked_binary_diff_sha256": tracked_diff,
                "untracked_file_count": 0,
                "untracked_path_mode_blob_oid_manifest": [],
                "untracked_path_mode_blob_oid_manifest_sha256": (
                    untracked_manifest_digest
                ),
                "ignored_cargo_lock_size_bytes": 1,
                "ignored_cargo_lock_sha256": nonzero_digest("cargo-lock"),
                "combined_source_fingerprint_sha256": combined.hexdigest(),
            },
        )
        native_files = {
            manifest_relative: digest(self.raw / raw_relative)
            for manifest_relative, raw_relative in (
                evidence_lib.NATIVE_BUILD_RAW_BINDINGS.items()
            )
        }
        library_digest = digest(self.raw / "build/libNoritoBridgeCandidateLab.a")
        native_library_key = (
            "NoritoBridgeCandidateLab.xcframework/ios-arm64/"
            "libNoritoBridgeCandidateLab.a"
        )
        native_files[native_library_key] = library_digest
        reviewed_closure = digest(
            self.raw / "input/reviewed-source-closure-v1.json"
        )
        native_manifest = {
            "schema": evidence_lib.NATIVE_BUILD_SCHEMA,
            "version": 1,
            "profile": "physical-ios-candidate-evidence-lab",
            "do_not_ship_marker": "KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2",
            "candidate_feature_enabled": True,
            "production_capability_enabled": False,
            "bridge_abi_version": 22,
            "target_triple": "aarch64-apple-ios",
            "architectures": ["arm64"],
            "simulator_slice_present": False,
            "minimum_ios_version": "15.0",
            "candidate_record_sha256": digest(
                self.raw / "input/candidate-v4.norito"
            ),
            "source_commit": source_commit,
            "source_tree_sha256": source_tree,
            "source_repo_dirty": False,
            "reviewed_source_closure_descriptor_sha256": reviewed_closure,
            "iphoneos_sdk_version": "26.0",
            "xcode_version": "Xcode 26.0\nBuild version 17A1",
            "cargo_version_verbose": "cargo 1.93.1\nrelease: 1.93.1",
            "rustc_version_verbose": "rustc 1.93.1\ncommit-hash: fixture",
            "required_symbols": [
                "connect_norito_kagemusha_recursive_spend_candidate_lab_apple_proof_phase_v1",
                "connect_norito_kagemusha_recursive_spend_candidate_lab_apple_restart_phase_v1",
                "CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2",
            ],
            "files": native_files,
        }
        write_json(self.raw / "input/native-build-manifest.json", native_manifest)

        scenario_hasher = hashlib.sha256()
        scenario_hasher.update(evidence_lib.SCENARIO_INVENTORY_DOMAIN)
        scenario_hasher.update(
            len(evidence_lib.SCENARIO_FILES).to_bytes(4, "big")
        )
        for name in sorted(evidence_lib.SCENARIO_FILES):
            artifact = self.raw / "input/scenario" / name
            relative = f"scenario/{name}".encode("utf-8")
            scenario_hasher.update(len(relative).to_bytes(4, "big"))
            scenario_hasher.update(relative)
            scenario_hasher.update(artifact.stat().st_size.to_bytes(8, "big"))
            scenario_hasher.update(bytes.fromhex(digest(artifact)))
        session = {
            "schema": evidence_lib.SESSION_SCHEMA,
            "version": 1,
            "candidate_record_sha256": native_manifest["candidate_record_sha256"],
            "candidate_manifest_sha256": digest(
                self.raw / "input/candidate-manifest-v4.norito"
            ),
            "topup_finality_roster_sha256": digest(
                self.raw / "input/topup-finality-roster-v4.norito"
            ),
            "scenario_inventory_sha256": scenario_hasher.hexdigest(),
            "native_build_manifest_sha256": digest(
                self.raw / "input/native-build-manifest.json"
            ),
            "native_library_sha256": library_digest,
            "source_commit": source_commit,
            "source_tree_sha256": source_tree,
            "source_repo_dirty": False,
            "reviewed_source_closure_descriptor_sha256": reviewed_closure,
            "device_udid_sha256": nonzero_digest("udid"),
            "device_ecid_sha256": nonzero_digest("ecid"),
            "device_serial_sha256": nonzero_digest("serial"),
            "expected_hardware_model": "iPhone17,1",
            "expected_board_config": "D93AP",
            "expected_os_version": "26.0",
            "expected_os_build": "23A1",
        }
        write_json(self.raw / "input/session-v1.json", session)

        checkpoint = self.raw / "output/checkpoint-v1.norito"
        install = self.raw / "output/install-identity-v1.bin"
        write_private(install, bytes(range(32)))
        common = {
            "schema": evidence_lib.LAUNCH_RECEIPT_SCHEMA,
            "version": 1,
            "recorded_at_utc": "2026-07-30T00:00:00Z",
            "resource_ceiling_bytes": evidence_lib.RESOURCE_CEILING_BYTES,
            "candidate_record_sha256": session["candidate_record_sha256"],
            "candidate_manifest_sha256": session["candidate_manifest_sha256"],
            "topup_finality_roster_sha256": session[
                "topup_finality_roster_sha256"
            ],
            "scenario_inventory_sha256": session["scenario_inventory_sha256"],
            "native_build_manifest_sha256": session[
                "native_build_manifest_sha256"
            ],
            "native_library_sha256": session["native_library_sha256"],
            "source_commit": session["source_commit"],
            "source_tree_sha256": session["source_tree_sha256"],
            "source_repo_dirty": False,
            "reviewed_source_closure_descriptor_sha256": session[
                "reviewed_source_closure_descriptor_sha256"
            ],
            "install_identity_sha256": digest(install),
            "checkpoint_size_bytes": checkpoint.stat().st_size,
            "checkpoint_sha256": digest(checkpoint),
            "device": device(session),
            "code_identity": code_identity(),
            "network_monitor": "NWPathMonitor",
            "url_protocol_observed_request_count": 0,
            "device_attestation_policy": evidence_lib.DEVICE_POLICY,
            "app_attest_used": False,
        }
        proof = {
            **common,
            "phase": "proof",
            "process_id": 41001,
            "launch_nonce_sha256": nonzero_digest("proof-nonce"),
            "monotonic_nanos": 1_000_000,
            "network_samples": network_samples(1_000),
        }
        write_json(self.raw / "output/proof-launch-receipt-v1.json", proof)

        inventory = []
        for role, filename in evidence_lib.NATIVE_ARTIFACTS:
            artifact = self.raw / "input/artifacts" / filename
            inventory.append(
                {
                    "role": role,
                    "framed_size_bytes": artifact.stat().st_size,
                    "framed_sha256": digest(artifact),
                    "payload_size_bytes": 1,
                    "payload_sha256": nonzero_digest(f"payload:{role}"),
                }
            )
        native_inventory_hasher = hashlib.sha256()
        for item in inventory:
            native_inventory_hasher.update(item["role"].encode("utf-8"))
            native_inventory_hasher.update(b"\0")
            native_inventory_hasher.update(
                str(item["framed_size_bytes"]).encode("ascii")
            )
            native_inventory_hasher.update(b"\0")
            native_inventory_hasher.update(item["framed_sha256"].encode("ascii"))
            native_inventory_hasher.update(b"\0")
            native_inventory_hasher.update(
                str(item["payload_size_bytes"]).encode("ascii")
            )
            native_inventory_hasher.update(b"\0")
            native_inventory_hasher.update(item["payload_sha256"].encode("ascii"))
            native_inventory_hasher.update(b"\n")
        events = []
        for index, operation in enumerate(evidence_lib.CAUSAL_OPERATIONS, start=1):
            rejected = operation == "duplicate_input_rejection"
            events.append(
                {
                    "sequence": index,
                    "phase": "proof_launch" if index <= 7 else "restart_launch",
                    "operation": operation,
                    "outcome": "rejected" if rejected else "succeeded",
                    "duration_nanos": index,
                    "input_sha256": nonzero_digest(f"input:{operation}"),
                    "output_sha256": nonzero_digest(f"output:{operation}"),
                    "output_size_bytes": 4 if rejected else 1,
                    "rejection_classification": (
                        "duplicate_input" if rejected else None
                    ),
                    "exception_class": None,
                    "error_message_sha256": (
                        nonzero_digest("duplicate-error") if rejected else None
                    ),
                }
            )
        transcript = {
            "schema": evidence_lib.NATIVE_TRANSCRIPT_SCHEMA,
            "version": 1,
            "platform": "ios",
            "physical_device_required": True,
            "simulator_accepted": False,
            "source_repo_dirty": False,
            "production_capability_observed": False,
            "process_restart_observed": True,
            "init_succeeded": True,
            "two_hop_append_succeeded": True,
            "all_branches_restored": True,
            "recipient_proofs_verified": True,
            "all_branches_fully_redeemed": True,
            "duplicate_input_rejected": True,
            "generation": "fixture-generation",
            "source_commit": session["source_commit"],
            "bridge_abi_version": 22,
            "source_tree_sha256": session["source_tree_sha256"],
            "reviewed_source_closure_descriptor_sha256": session[
                "reviewed_source_closure_descriptor_sha256"
            ],
            "candidate_record_sha256": session["candidate_record_sha256"],
            "candidate_manifest_sha256": session["candidate_manifest_sha256"],
            "native_accepted_inventory_sha256": native_inventory_hasher.hexdigest(),
            "scenario_inventory_sha256": session["scenario_inventory_sha256"],
            "checkpoint_sha256": digest(checkpoint),
            "init_result_sha256": nonzero_digest("init-result"),
            "split_hop_01_result_sha256": nonzero_digest("hop-01-result"),
            "split_hop_02_result_sha256": nonzero_digest("hop-02-result"),
            "proof_launch_nonce_sha256": proof["launch_nonce_sha256"],
            "restart_launch_nonce_sha256": nonzero_digest("restart-nonce"),
            "proof_process_id": proof["process_id"],
            "restart_process_id": 41002,
            "resource_ceiling_bytes": evidence_lib.RESOURCE_CEILING_BYTES,
            "proof_peak_rss_bytes": 256 * 1024 * 1024,
            "restart_peak_rss_bytes": 384 * 1024 * 1024,
            **{key: 1 for key in evidence_lib.TRANSCRIPT_DURATION_FIELDS},
            "proof_hops": 2,
            "exact_operation_count": 28,
            "initial_atomic_units": "100",
            "first_recipient_atomic_units": "30",
            "second_recipient_atomic_units": "20",
            "sender_change_atomic_units": "50",
            "redeemed_atomic_units": "100",
            "final_unspent_atomic_units": "0",
            "asset_scale": 2,
            "duplicate_error_code": -311,
            "artifact_inventory": inventory,
            "causal_events": events,
        }
        write_json(self.raw / "output/native-transcript-v1.json", transcript)
        transcript_path = self.raw / "output/native-transcript-v1.json"
        proof_path = self.raw / "output/proof-launch-receipt-v1.json"
        restart = {
            **common,
            "phase": "restart",
            "process_id": transcript["restart_process_id"],
            "launch_nonce_sha256": transcript["restart_launch_nonce_sha256"],
            "recorded_at_utc": "2026-07-30T00:00:01Z",
            "monotonic_nanos": 2_000_000,
            "network_samples": network_samples(1_500_000),
            "native_transcript_size_bytes": transcript_path.stat().st_size,
            "native_transcript_sha256": digest(transcript_path),
            "proof_launch_receipt_sha256": digest(proof_path),
        }
        write_json(self.raw / "output/restart-launch-receipt-v1.json", restart)
        identity = common["code_identity"]
        code_sign_measurements = {
            "schema": evidence_lib.CODE_SIGN_MEASUREMENTS_SCHEMA,
            "version": 1,
            "app": {
                "bundle_id": identity["app_bundle_id"],
                "version": identity["app_version"],
                "build": identity["app_build"],
                "identifier": identity["app_bundle_id"],
                "team_id": "A1B2C3D4E5",
                "cdhash": "1" * 40,
                "executable_sha256": identity["app_executable_sha256"],
                "entitlements_sha256": nonzero_digest("app-entitlements"),
                "provisioning_profile_sha256": nonzero_digest("app-profile"),
            },
            "test": {
                "bundle_id": identity["test_bundle_id"],
                "identifier": identity["test_bundle_id"],
                "team_id": "A1B2C3D4E5",
                "cdhash": "2" * 40,
                "executable_sha256": identity["test_executable_sha256"],
                "entitlements_sha256": nonzero_digest("test-entitlements"),
                "provisioning_profile_sha256": nonzero_digest("test-profile"),
            },
            "native": {
                "kind": "static_library_bound_into_signed_test_bundle",
                "sha256": digest(
                    self.raw / "build/libNoritoBridgeCandidateLab.a"
                ),
                "build_manifest_sha256": digest(
                    self.raw / "input/native-build-manifest.json"
                ),
                "architectures": ["arm64"],
                "simulator_slice_used": False,
            },
        }
        write_json(
            self.raw / "build/code-sign-measurements-v1.json",
            code_sign_measurements,
        )
        proof_test_result = {
            "schema": evidence_lib.TEST_RESULT_SCHEMA,
            "version": 1,
            "phase": "proof",
            "test_status": "passed",
            "test_identifier": (
                "KagemushaCandidateEvidenceLabTests/"
                "KagemushaCandidateEvidenceLabTests/testProofPhase"
            ),
            "launch_receipt_sha256": digest(proof_path),
            "native_transcript_sha256": None,
        }
        restart_test_result = {
            "schema": evidence_lib.TEST_RESULT_SCHEMA,
            "version": 1,
            "phase": "restart",
            "test_status": "passed",
            "test_identifier": (
                "KagemushaCandidateEvidenceLabTests/"
                "KagemushaCandidateEvidenceLabTests/testRestartPhase"
            ),
            "launch_receipt_sha256": digest(
                self.raw / "output/restart-launch-receipt-v1.json"
            ),
            "native_transcript_sha256": digest(transcript_path),
        }
        write_json(
            self.raw / "run/proof-test-result-v1.json",
            proof_test_result,
        )
        write_json(
            self.raw / "run/restart-test-result-v1.json",
            restart_test_result,
        )

    def sign(self) -> None:
        result = signer.main(
            [
                "--artifact-root",
                str(self.raw),
                "--private-key",
                str(self.private_key),
                "--public-key",
                str(self.public_key),
                "--signer-key-id",
                self.key_id,
                "--output",
                str(self.evidence),
            ]
        )
        if result != 0:
            raise AssertionError("fixture signing failed")

    def errors(self) -> list[str]:
        return evidence_lib.validate_signed_evidence(
            self.evidence,
            self.raw,
            self.key_id,
            self.public_key,
        )

    def mutate_json(
        self, relative: str, mutator: Callable[[dict[str, Any]], None]
    ) -> None:
        path = self.raw / relative
        value = json.loads(path.read_text(encoding="utf-8"))
        mutator(value)
        write_json(path, value)

    def resign_without_semantic_preflight(self) -> None:
        value = json.loads(self.evidence.read_text(encoding="utf-8"))
        digests, sizes = evidence_lib.scan_raw_artifacts(self.raw)
        value["artifact_digests"] = {
            relative: {"size_bytes": sizes[relative], "sha256": artifact_digest}
            for relative, artifact_digest in digests.items()
        }
        payload = evidence_lib.canonical_signature_payload(value)
        value["signature_payload_sha256"] = sha256(payload)
        value["signature"] = evidence_lib.sign_ed25519(
            self.private_key, payload
        ).hex()
        evidence_lib.write_private_json(self.evidence, value)

    def rebind_restart_receipt(self) -> None:
        receipt = self.raw / "output/restart-launch-receipt-v1.json"
        result = self.raw / "run/restart-test-result-v1.json"
        value = json.loads(result.read_text(encoding="utf-8"))
        value["launch_receipt_sha256"] = digest(receipt)
        write_json(result, value)

    def rebind_transcript(self) -> None:
        transcript = self.raw / "output/native-transcript-v1.json"
        restart = self.raw / "output/restart-launch-receipt-v1.json"
        restart_value = json.loads(restart.read_text(encoding="utf-8"))
        restart_value["native_transcript_size_bytes"] = transcript.stat().st_size
        restart_value["native_transcript_sha256"] = digest(transcript)
        write_json(restart, restart_value)
        result = self.raw / "run/restart-test-result-v1.json"
        result_value = json.loads(result.read_text(encoding="utf-8"))
        result_value["native_transcript_sha256"] = digest(transcript)
        result_value["launch_receipt_sha256"] = digest(restart)
        write_json(result, result_value)

    def mutate_signed_envelope(
        self, mutator: Callable[[dict[str, Any]], None], *, resign: bool
    ) -> None:
        value = json.loads(self.evidence.read_text(encoding="utf-8"))
        mutator(value)
        if resign:
            payload = evidence_lib.canonical_signature_payload(value)
            value["signature_payload_sha256"] = sha256(payload)
            value["signature"] = evidence_lib.sign_ed25519(
                self.private_key, payload
            ).hex()
        evidence_lib.write_private_json(self.evidence, value)


class IosCandidateEvidenceTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        openssl = shutil.which("openssl")
        if openssl is None:
            raise unittest.SkipTest("openssl is unavailable")
        cls.key_temp = tempfile.TemporaryDirectory()
        key_root = Path(cls.key_temp.name)
        cls.private_key = key_root / "signing-key.pem"
        cls.public_key = key_root / "signing-key.pub.pem"
        subprocess.run(
            [
                openssl,
                "genpkey",
                "-algorithm",
                "ED25519",
                "-out",
                str(cls.private_key),
            ],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        cls.private_key.chmod(0o600)
        subprocess.run(
            [
                openssl,
                "pkey",
                "-in",
                str(cls.private_key),
                "-pubout",
                "-out",
                str(cls.public_key),
            ],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        cls.public_key.chmod(0o600)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.key_temp.cleanup()

    def fixture(self, temporary: str) -> Fixture:
        return Fixture(
            Path(temporary),
            self.private_key,
            self.public_key,
        )

    def assert_error_contains(self, fixture: Fixture, expected: str) -> None:
        errors = fixture.errors()
        self.assertTrue(
            any(expected in error for error in errors),
            (expected, errors),
        )

    def test_valid_signing_and_cli_verification(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            self.assertEqual(fixture.errors(), [])
            self.assertEqual(
                checker.main(
                    [
                        "--evidence",
                        str(fixture.evidence),
                        "--artifact-root",
                        str(fixture.raw),
                        "--trusted-key-id",
                        fixture.key_id,
                        "--trusted-public-key",
                        str(fixture.public_key),
                    ]
                ),
                0,
            )
            value = json.loads(fixture.evidence.read_text(encoding="utf-8"))
            self.assertEqual(set(value), evidence_lib.SIGNED_EVIDENCE_FIELDS)
            self.assertEqual(
                set(value["artifact_digests"]),
                evidence_lib.EXPECTED_RAW_ARTIFACT_PATHS,
            )

    def test_isolated_cli_signing_and_verification(self) -> None:
        for script_name in (
            "sign_kagemusha_candidate_ios_evidence.py",
            "check_kagemusha_candidate_ios_evidence.py",
        ):
            subprocess.run(
                [
                    sys.executable,
                    "-I",
                    str(SCRIPT_DIR / script_name),
                    "--help",
                ],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            subprocess.run(
                [
                    sys.executable,
                    "-I",
                    str(SCRIPT_DIR / "sign_kagemusha_candidate_ios_evidence.py"),
                    "--artifact-root",
                    str(fixture.raw),
                    "--private-key",
                    str(fixture.private_key),
                    "--public-key",
                    str(fixture.public_key),
                    "--signer-key-id",
                    fixture.key_id,
                    "--output",
                    str(fixture.evidence),
                ],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            subprocess.run(
                [
                    sys.executable,
                    "-I",
                    str(SCRIPT_DIR / "check_kagemusha_candidate_ios_evidence.py"),
                    "--evidence",
                    str(fixture.evidence),
                    "--artifact-root",
                    str(fixture.raw),
                    "--trusted-key-id",
                    fixture.key_id,
                    "--trusted-public-key",
                    str(fixture.public_key),
                ],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

    def test_unknown_signed_field_is_rejected_even_when_resigned(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            fixture.mutate_signed_envelope(
                lambda value: value.__setitem__("unexpected", True),
                resign=True,
            )
            self.assert_error_contains(fixture, "contains unknown fields")

    def test_signature_and_payload_digest_tamper_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            fixture.mutate_signed_envelope(
                lambda value: value.__setitem__(
                    "signature", ("00" if value["signature"][:2] != "00" else "01")
                    + value["signature"][2:]
                ),
                resign=False,
            )
            self.assert_error_contains(fixture, "signature verification failed")
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            fixture.mutate_signed_envelope(
                lambda value: value.__setitem__(
                    "signature_payload_sha256", nonzero_digest("wrong-payload")
                ),
                resign=False,
            )
            self.assert_error_contains(
                fixture, "signature_payload_sha256 mismatch"
            )

    def test_raw_artifact_digest_tamper_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            write_private(
                fixture.raw / "output/checkpoint-v1.norito",
                b"tampered checkpoint\n",
            )
            self.assert_error_contains(fixture, "artifact digest mismatch")

    def test_manifest_bound_xcframework_support_file_is_rejected_when_changed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            relative = (
                "build/NoritoBridgeCandidateLab.xcframework/"
                "ios-arm64/Headers/module.modulemap"
            )
            write_private(fixture.raw / relative, b"tampered module map\n")
            fixture.resign_without_semantic_preflight()
            self.assert_error_contains(
                fixture,
                "native build manifest file digest mismatch",
            )

    def test_simulator_receipt_is_rejected_with_valid_signature(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()

            def mutate(value: dict[str, Any]) -> None:
                value["device"]["physical"] = False
                value["device"]["simulator"] = True

            fixture.mutate_json("output/proof-launch-receipt-v1.json", mutate)
            fixture.resign_without_semantic_preflight()
            self.assert_error_contains(fixture, "simulator must be false")

    def test_dirty_true_is_rejected_with_valid_signature(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            fixture.mutate_json(
                "output/native-transcript-v1.json",
                lambda value: value.__setitem__("source_repo_dirty", True),
            )
            fixture.resign_without_semantic_preflight()
            self.assert_error_contains(fixture, "source_repo_dirty must be false")

    def test_satisfied_network_sample_is_rejected_with_valid_signature(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()

            def mutate(value: dict[str, Any]) -> None:
                value["network_samples"][2]["status"] = "satisfied"

            fixture.mutate_json("output/restart-launch-receipt-v1.json", mutate)
            fixture.resign_without_semantic_preflight()
            self.assert_error_contains(fixture, "status must be unsatisfied")

    def test_host_measurements_results_and_reviewed_closure_are_bound(self) -> None:
        def add_tracked_source_diff(value: dict[str, Any]) -> None:
            tracked = nonzero_digest("unexpected-tracked-source-diff")
            manifest = value["untracked_path_mode_blob_oid_manifest_sha256"]
            combined = hashlib.sha256()
            combined.update(evidence_lib.SOURCE_DIFF_DOMAIN)
            combined.update(evidence_lib.TRACKED_DIFF_DOMAIN)
            combined.update(bytes.fromhex(tracked))
            combined.update(evidence_lib.UNTRACKED_MANIFEST_DOMAIN)
            combined.update(bytes.fromhex(manifest))
            value["tracked_binary_diff_sha256"] = tracked
            value["combined_source_fingerprint_sha256"] = combined.hexdigest()

        cases: tuple[
            tuple[str, str, Callable[[dict[str, Any]], None], str], ...
        ] = (
            (
                "code-sign-unknown",
                "build/code-sign-measurements-v1.json",
                lambda value: value["app"].__setitem__("unexpected", True),
                "code-sign measurements app contains unknown fields",
            ),
            (
                "runtime-executable",
                "build/code-sign-measurements-v1.json",
                lambda value: value["app"].__setitem__(
                    "executable_sha256",
                    nonzero_digest("different-app-executable"),
                ),
                "must match proof receipt code_identity app_executable_sha256",
            ),
            (
                "simulator-slice",
                "build/code-sign-measurements-v1.json",
                lambda value: value["native"].__setitem__(
                    "simulator_slice_used",
                    True,
                ),
                "simulator_slice_used must be false",
            ),
            (
                "test-result-receipt",
                "run/proof-test-result-v1.json",
                lambda value: value.__setitem__(
                    "launch_receipt_sha256",
                    nonzero_digest("different-proof-receipt"),
                ),
                "launch_receipt_sha256 does not match",
            ),
            (
                "test-result-transcript",
                "run/restart-test-result-v1.json",
                lambda value: value.__setitem__(
                    "native_transcript_sha256",
                    nonzero_digest("different-transcript"),
                ),
                "native_transcript_sha256 does not match",
            ),
            (
                "reviewed-closure",
                "input/reviewed-source-closure-v1.json",
                lambda value: value.__setitem__("source_repo_dirty", True),
                "reviewed source closure source_repo_dirty must be false",
            ),
            (
                "reviewed-closure-tracked-diff",
                "input/reviewed-source-closure-v1.json",
                add_tracked_source_diff,
                "tracked_binary_diff_sha256 must identify an empty diff",
            ),
        )
        for label, relative, mutator, expected in cases:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                fixture = self.fixture(temporary)
                fixture.sign()
                fixture.mutate_json(relative, mutator)
                fixture.resign_without_semantic_preflight()
                self.assert_error_contains(fixture, expected)

    def test_wrong_event_duplicate_rss_and_amount_are_rejected(self) -> None:
        cases: tuple[
            tuple[str, Callable[[dict[str, Any]], None], str], ...
        ] = (
            (
                "event",
                lambda value: value["causal_events"][0].__setitem__(
                    "operation", "wrong_operation"
                ),
                "operation must be candidate_install",
            ),
            (
                "duplicate",
                lambda value: value.__setitem__("duplicate_error_code", -310),
                "duplicate_error_code must be -311",
            ),
            (
                "rss",
                lambda value: value.__setitem__(
                    "proof_peak_rss_bytes",
                    evidence_lib.RESOURCE_CEILING_BYTES + 1,
                ),
                "exceeds the fixed RSS ceiling",
            ),
            (
                "amount",
                lambda value: value.__setitem__("redeemed_atomic_units", "99"),
                "redeemed_atomic_units must equal initial_atomic_units",
            ),
        )
        for label, mutator, expected in cases:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                fixture = self.fixture(temporary)
                fixture.sign()
                fixture.mutate_json(
                    "output/native-transcript-v1.json",
                    mutator,
                )
                fixture.resign_without_semantic_preflight()
                self.assert_error_contains(fixture, expected)

    def test_rfc8032_vector_and_fake_path_cannot_replace_crypto(self) -> None:
        seed = bytes.fromhex(
            "9d61b19deffd5a60ba844af492ec2cc"
            "44449c5697b326919703bac031cae7f60"
        )
        public = bytes.fromhex(
            "d75a980182b10ab7d54bfed3c964073a"
            "0ee172f3daa62325af021a68f707511a"
        )
        expected_signature = bytes.fromhex(
            "e5564300c360ac729086e2cc806e828a"
            "84877f1eb8e5d974d873e06522490155"
            "5fb8821590a33bacc61e39701cf9b46b"
            "d25bf5f0595bbe24655141438e7a100b"
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            private = root / "rfc8032-private.pem"
            public_path = root / "rfc8032-public.pem"
            private_der = evidence_lib.ED25519_PKCS8_SEED_PREFIX + seed
            public_der = evidence_lib.ED25519_SPKI_PREFIX + public
            write_private(
                private,
                b"-----BEGIN PRIVATE KEY-----\n"
                + base64.b64encode(private_der)
                + b"\n-----END PRIVATE KEY-----\n",
            )
            write_private(
                public_path,
                b"-----BEGIN PUBLIC KEY-----\n"
                + base64.b64encode(public_der)
                + b"\n-----END PUBLIC KEY-----\n",
            )
            signature = evidence_lib.sign_ed25519(private, b"")
            self.assertEqual(signature, expected_signature)
            evidence_lib.verify_ed25519(public_path, b"", signature)

        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fake_root = Path(temporary) / "fake-bin"
            fake_root.mkdir(mode=0o700)
            fake_openssl = fake_root / "openssl"
            write_private(fake_openssl, b"#!/bin/sh\nexit 0\n")
            fake_openssl.chmod(0o700)
            with mock.patch.dict(os.environ, {"PATH": str(fake_root)}):
                fixture.sign()
                self.assertEqual(fixture.errors(), [])

    def test_scan_to_semantic_swap_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            transcript_path = fixture.raw / "output/native-transcript-v1.json"
            valid_transcript = transcript_path.read_bytes()
            fixture.mutate_json(
                "output/native-transcript-v1.json",
                lambda value: value.__setitem__("source_repo_dirty", True),
            )
            fixture.rebind_transcript()
            fixture.resign_without_semantic_preflight()
            original_snapshot = evidence_lib.snapshot_raw_artifacts
            swapped = False

            def snapshot_then_swap(root: Path) -> evidence_lib.RawArtifactSnapshot:
                nonlocal swapped
                snapshot = original_snapshot(root)
                if not swapped:
                    swapped = True
                    write_private(transcript_path, valid_transcript)
                return snapshot

            with mock.patch.object(
                evidence_lib,
                "snapshot_raw_artifacts",
                side_effect=snapshot_then_swap,
            ):
                errors = fixture.errors()
            self.assertTrue(
                any("source_repo_dirty must be false" in error for error in errors),
                errors,
            )
            self.assertTrue(
                any("changed after its validated immutable snapshot" in error for error in errors),
                errors,
            )

    def test_key_path_swap_after_snapshot_is_rejected(self) -> None:
        for phase in ("sign", "verify"):
            with self.subTest(phase=phase), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                key_root = root / "keys"
                key_root.mkdir(mode=0o700)
                private = key_root / "private.pem"
                public = key_root / "public.pem"
                write_private(private, self.private_key.read_bytes())
                write_private(public, self.public_key.read_bytes())
                fixture_root = root / "fixture"
                fixture_root.mkdir(mode=0o700)
                fixture = Fixture(fixture_root, private, public)
                if phase == "verify":
                    fixture.sign()
                original_snapshot = evidence_lib._snapshot_key_file
                swapped = False

                def snapshot_then_swap(
                    path: Path,
                    label: str,
                    *,
                    private: bool,
                ) -> evidence_lib.FileSnapshot:
                    nonlocal swapped
                    snapshot = original_snapshot(path, label, private=private)
                    target_phase = (
                        phase == "sign" and private
                        or phase == "verify" and not private
                    )
                    if target_phase and not swapped:
                        swapped = True
                        replacement = bytearray(snapshot.payload)
                        replacement[-10] ^= 1
                        write_private(path, bytes(replacement))
                    return snapshot

                with mock.patch.object(
                    evidence_lib,
                    "_snapshot_key_file",
                    side_effect=snapshot_then_swap,
                ):
                    if phase == "sign":
                        with self.assertRaisesRegex(
                            evidence_lib.EvidenceError,
                            "changed after its immutable snapshot",
                        ):
                            evidence_lib.build_signed_evidence(
                                fixture.raw,
                                private,
                                public,
                                fixture.key_id,
                            )
                    else:
                        self.assert_error_contains(
                            fixture,
                            "changed after its immutable snapshot",
                        )

    def test_noncanonical_signed_and_raw_json_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            signed = json.loads(fixture.evidence.read_text(encoding="utf-8"))
            write_private(
                fixture.evidence,
                (json.dumps(signed, indent=2) + "\n").encode("utf-8"),
            )
            self.assert_error_contains(fixture, "bytes are not canonical JSON")

        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            transcript = fixture.raw / "output/native-transcript-v1.json"
            value = json.loads(transcript.read_text(encoding="utf-8"))
            write_private(
                transcript,
                (json.dumps(value, indent=2) + "\n").encode("utf-8"),
            )
            fixture.rebind_transcript()
            fixture.resign_without_semantic_preflight()
            self.assert_error_contains(
                fixture,
                "native transcript bytes are not canonical JSON",
            )

    def test_network_sample_duplicates_extras_and_order_are_rejected(self) -> None:
        def duplicate(value: dict[str, Any]) -> None:
            value["network_samples"][0]["label"] = "before"

        def extra(value: dict[str, Any]) -> None:
            value["network_samples"].append(dict(value["network_samples"][-1]))

        def reorder(value: dict[str, Any]) -> None:
            value["network_samples"][1], value["network_samples"][2] = (
                value["network_samples"][2],
                value["network_samples"][1],
            )

        for label, mutator in (
            ("duplicate", duplicate),
            ("extra", extra),
            ("reorder", reorder),
        ):
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                fixture = self.fixture(temporary)
                fixture.sign()
                fixture.mutate_json(
                    "output/restart-launch-receipt-v1.json",
                    mutator,
                )
                fixture.rebind_restart_receipt()
                fixture.resign_without_semantic_preflight()
                self.assert_error_contains(fixture, "network_samples")

    def test_launch_timestamp_reversal_and_malformed_utc_are_rejected(self) -> None:
        cases: tuple[
            tuple[str, Callable[[dict[str, Any]], None], str], ...
        ] = (
            (
                "monotonic-reversal",
                lambda value: value.__setitem__("monotonic_nanos", 1),
                "proof launch monotonic_nanos must be strictly before restart",
            ),
            (
                "utc-reversal",
                lambda value: value.__setitem__(
                    "recorded_at_utc", "2026-07-29T23:59:59Z"
                ),
                "recorded_at_utc must be strictly before restart",
            ),
            (
                "utc-malformed",
                lambda value: value.__setitem__(
                    "recorded_at_utc", "2026-07-30 00:00:01Z"
                ),
                "canonical UTC ISO-8601",
            ),
        )
        for label, mutator, expected in cases:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                fixture = self.fixture(temporary)
                fixture.sign()
                fixture.mutate_json(
                    "output/restart-launch-receipt-v1.json",
                    mutator,
                )
                fixture.rebind_restart_receipt()
                fixture.resign_without_semantic_preflight()
                self.assert_error_contains(fixture, expected)

    def test_missing_raw_artifact_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.fixture(temporary)
            fixture.sign()
            (
                fixture.raw
                / "input/scenario"
                / evidence_lib.SCENARIO_FILES[-1]
            ).unlink()
            self.assert_error_contains(fixture, "raw artifact tree is missing files")


if __name__ == "__main__":
    unittest.main()
