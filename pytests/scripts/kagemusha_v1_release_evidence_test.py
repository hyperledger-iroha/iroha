"""Test release-evidence projection; synthetic fixtures are not proof qualification."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
VERIFIER_PATH = SCRIPTS / "verify_kagemusha_v1_release_evidence.py"
sys.path.insert(0, str(SCRIPTS))
SPEC = importlib.util.spec_from_file_location("kagemusha_release_evidence", VERIFIER_PATH)
assert SPEC is not None and SPEC.loader is not None
VERIFIER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = VERIFIER
SPEC.loader.exec_module(VERIFIER)

PHYSICAL_SPEC = importlib.util.spec_from_file_location(
    "kagemusha_physical_test_fixture",
    ROOT / "pytests/scripts/kagemusha_v1_physical_device_test.py",
)
assert PHYSICAL_SPEC is not None and PHYSICAL_SPEC.loader is not None
PHYSICAL_TEST = importlib.util.module_from_spec(PHYSICAL_SPEC)
sys.modules[PHYSICAL_SPEC.name] = PHYSICAL_TEST
PHYSICAL_SPEC.loader.exec_module(PHYSICAL_TEST)

INTERNAL_HELPER_PROOF_LENGTHS = {
    "platform_credential": (8_000, 8_032),
    "guard_bundle": (12_000, 12_032),
    "mint_hash_shard": (8_064, 8_096),
    "mint_hash_claim": (12_064, 12_096),
}


def _digest(value: int) -> str:
    return f"{value:064x}"


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _json_line(value: object) -> bytes:
    return (
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("utf-8")
        + b"\n"
    )


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


@dataclass
class EvidenceFixture:
    root: Path
    manifest_path: Path
    observer_policy_path: Path
    observer_policy_sha256: str
    observer_seed: bytes
    observer_authority_id: str
    trusted_verifier_sha256: str
    manifest: dict[str, Any]
    kinds: dict[str, str]
    commands: list[dict[str, Any]]
    proof_paths: list[str]
    internal_proof_paths: dict[str, tuple[str, str]]
    raw_complete_exchange_paths: list[str]
    text_complete_exchange_paths: list[str]

    def path(self, relative: str) -> Path:
        return self.root / relative

    def write(self, relative: str, payload: bytes, kind: str, *, mode: int = 0o600) -> None:
        path = self.path(relative)
        path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        path.write_bytes(payload)
        path.chmod(mode)
        self.kinds[relative] = kind

    def refresh_files(self) -> None:
        rows = []
        for relative in sorted(self.kinds):
            payload = self.path(relative).read_bytes()
            rows.append(
                {
                    "path": relative,
                    "kind": self.kinds[relative],
                    "sha256": _sha256(payload),
                    "byte_len": len(payload),
                }
            )
        self.manifest["files"] = rows
        self.write_manifest()

    def write_manifest(self) -> str:
        payload = VERIFIER.canonical_json_bytes(self.manifest)
        self.manifest_path.write_bytes(payload)
        self.manifest_path.chmod(0o600)
        return _sha256(payload)

    def candidate_context_digest(self) -> str:
        source = self.manifest["source"]
        source_archive = self.path(source["source_archive"])
        cargo_lock = self.path(source["cargo_lock"])
        artifacts = [
            {
                "role": row["role"],
                "sha256": _sha256(self.path(row["path"]).read_bytes()),
                "byte_len": self.path(row["path"]).stat().st_size,
            }
            for row in self.manifest["artifacts"]
        ]
        protocols = self.manifest["protocols"]
        profile_inputs = [
            {
                "hardware_profile": dict(row["hardware_profile"]),
                "suite_id": row["suite_id"],
            }
            for row in self.manifest["profiles"]
        ]
        _, digest = VERIFIER.release_candidate_context(
            source_archive={
                "sha256": _sha256(source_archive.read_bytes()),
                "byte_len": source_archive.stat().st_size,
            },
            cargo_lock={
                "sha256": _sha256(cargo_lock.read_bytes()),
                "byte_len": cargo_lock.stat().st_size,
            },
            artifacts=artifacts,
            artifact_set_digest=VERIFIER.rust_artifact_set_digest(artifacts),
            vk_digest=VERIFIER.rust_vk_set_digest(artifacts, protocols),
            protocols=protocols,
            profile_inputs=profile_inputs,
            observer_policy={
                "sha256": self.observer_policy_sha256,
                "byte_len": self.observer_policy_path.stat().st_size,
            },
        )
        return digest

    def resign_all_for_candidate_context(self) -> None:
        digest = self.candidate_context_digest()
        for command in self.commands:
            observation_path = command["observation"]
            observation = json.loads(self.path(observation_path).read_text())
            observation["subject"]["candidate_context_digest"] = digest
            self.write(
                observation_path,
                VERIFIER.canonical_json_bytes(observation),
                "observation",
            )
            self.resign_command(command["id"])

    def resign_command(self, command_id: str) -> None:
        command = next(row for row in self.commands if row["id"] == command_id)
        observation_path = command["observation"]
        observation = json.loads(self.path(observation_path).read_text())
        subject = observation["subject"]
        subject["arguments"] = [
            (
                {
                    "file": argument["file"],
                    "sha256": _sha256(self.path(argument["file"]).read_bytes()),
                    "byte_len": self.path(argument["file"]).stat().st_size,
                }
                if "file" in argument
                else dict(argument)
            )
            for argument in command["arguments"]
        ]
        for stream in ("stdout", "stderr"):
            payload = self.path(command[stream]).read_bytes()
            subject[stream] = {"sha256": _sha256(payload), "byte_len": len(payload)}
        signature = _ed25519_sign(self.observer_seed, VERIFIER._approval_message(subject))
        observation["approvals"] = [
            {
                "authority_id": self.observer_authority_id,
                "signature": signature.hex(),
            }
        ]
        self.write(
            observation_path,
            VERIFIER.canonical_json_bytes(observation),
            "observation",
        )

    def resign_commands_for_file(self, relative: str) -> None:
        for command in self.commands:
            if any(argument.get("file") == relative for argument in command["arguments"]):
                self.resign_command(command["id"])


def _fixture(tmp_path: Path) -> EvidenceFixture:
    tmp_path.mkdir(parents=True, exist_ok=True)
    tmp_path.chmod(0o700)
    root = tmp_path / "evidence"
    root.mkdir(mode=0o700)
    observer_seed = bytes(range(1, 33))
    observer_public_key = _ed25519_public_key(observer_seed)
    observer_authority_id = hashlib.sha256(
        VERIFIER.OBSERVER_AUTHORITY_ID_DOMAIN + observer_public_key
    ).hexdigest()
    trusted_verifier_sha256 = _digest(0xA11CE)
    observer_policy_path = tmp_path / "trusted-observer-policy.json"
    observer_policy = {
        "schema": VERIFIER.OBSERVER_POLICY_SCHEMA,
        "schema_version": 1,
        "threshold": 1,
        "authorities": [
            {
                "authority_id": observer_authority_id,
                "ed25519_public_key": observer_public_key.hex(),
            }
        ],
        "verifiers": [
            {
                "id": "kagemusha-release-verifier-v1",
                "sha256": trusted_verifier_sha256,
                "report_schemas": sorted(VERIFIER.REPORT_SCHEMAS),
            },
            {
                "id": VERIFIER.PHYSICAL_VERIFIER_ID,
                "sha256": _sha256(VERIFIER.PHYSICAL_VERIFIER_PATH.read_bytes()),
                "report_schemas": ["iroha.kagemusha_v1.hardware_profile_qualification_report"],
            }
        ],
    }
    observer_policy_bytes = VERIFIER.canonical_json_bytes(observer_policy)
    observer_policy_path.write_bytes(observer_policy_bytes)
    observer_policy_path.chmod(0o600)
    fixture = EvidenceFixture(
        root=root,
        manifest_path=tmp_path / "kagemusha-evidence.json",
        observer_policy_path=observer_policy_path,
        observer_policy_sha256=_sha256(observer_policy_bytes),
        observer_seed=observer_seed,
        observer_authority_id=observer_authority_id,
        trusted_verifier_sha256=trusted_verifier_sha256,
        manifest={},
        kinds={},
        commands=[],
        proof_paths=[],
        internal_proof_paths={},
        raw_complete_exchange_paths=[],
        text_complete_exchange_paths=[],
    )

    fixture.write("source/candidate.tar", b"immutable candidate source archive\n", "source_archive")
    fixture.write("source/Cargo.lock", b"# immutable Cargo.lock\n", "cargo_lock")
    source_sha = _sha256(fixture.path("source/candidate.tar").read_bytes())

    artifact_rows: list[dict[str, str]] = []
    artifact_paths: dict[str, str] = {}
    for index, role in enumerate(VERIFIER.ARTIFACT_ROLES, start=1):
        relative = f"artifacts/{index:02d}-{role}.bin"
        if role.startswith("params_"):
            marker = bytes([index])
            payload = marker + b"P" * (4_194_372 - 1)
        else:
            payload = f"immutable {role} artifact {index}\n".encode()
        fixture.write(relative, payload, "artifact")
        artifact_rows.append({"role": role, "path": relative})
        artifact_paths[role] = relative

    stdout = b"verified\n"
    stderr = b"no diagnostics\n"
    command_index = 0

    def add_report(
        relative: str,
        schema: str,
        body: dict[str, Any],
        inputs: list[str] | tuple[str, ...] = (),
    ) -> str:
        nonlocal command_index
        command_id = body.get("verification_id", f"verify-{command_index:04d}")
        command_index += 1
        report = {
            "schema": schema,
            "schema_version": 1,
            "verification_id": command_id,
            **body,
        }
        fixture.write(relative, VERIFIER.canonical_json_bytes(report), "report")
        required = sorted({relative, *inputs})
        stdout_path = f"transcripts/{command_id}.stdout"
        stderr_path = f"transcripts/{command_id}.stderr"
        observation_path = f"observations/{command_id}.json"
        fixture.write(stdout_path, stdout, "transcript")
        fixture.write(stderr_path, stderr, "transcript")
        arguments = [
            {
                "file": path,
                "sha256": _sha256(fixture.path(path).read_bytes()),
                "byte_len": fixture.path(path).stat().st_size,
            }
            for path in required
        ]
        subject = {
            "command_id": command_id,
            "verifier_id": "kagemusha-release-verifier-v1",
            "verifier_sha256": fixture.trusted_verifier_sha256,
            "report_schema": schema,
            "arguments": arguments,
            "exit_code": 0,
            "stdout": {"sha256": _sha256(stdout), "byte_len": len(stdout)},
            "stderr": {"sha256": _sha256(stderr), "byte_len": len(stderr)},
            "started_at_ms": 1_700_000_000_000 + command_index,
            "duration_ms": 5,
            "cpu_millis": 4,
            "peak_rss_bytes": 64 * 1024 * 1024,
        }
        signature = _ed25519_sign(
            fixture.observer_seed, VERIFIER._approval_message(subject)
        )
        observation = {
            "schema": VERIFIER.OBSERVATION_SCHEMA,
            "schema_version": 1,
            "subject": subject,
            "approvals": [
                {
                    "authority_id": fixture.observer_authority_id,
                    "signature": signature.hex(),
                }
            ],
        }
        fixture.write(
            observation_path,
            VERIFIER.canonical_json_bytes(observation),
            "observation",
        )
        fixture.commands.append(
            {
                "id": command_id,
                "verifier_id": "kagemusha-release-verifier-v1",
                "report_schema": schema,
                "arguments": [{"file": path} for path in required],
                "stdout": stdout_path,
                "stderr": stderr_path,
                "observation": observation_path,
            }
        )
        return relative

    state_eq = _digest(1)
    state_ep = _digest(2)
    helper_protocols = []
    for index, helper in enumerate(VERIFIER.HELPERS):
        eq_proof_bytes, ep_proof_bytes = INTERNAL_HELPER_PROOF_LENGTHS.get(
            helper, (0, 0)
        )
        helper_protocols.append(
            {
                "helper": helper,
                "eq_protocol_digest": _digest(5 + index * 2),
                "ep_protocol_digest": _digest(6 + index * 2),
                "eq_proof_bytes": eq_proof_bytes,
                "ep_proof_bytes": ep_proof_bytes,
            }
        )
    protocols = {
        "state_eq_protocol_digest": state_eq,
        "state_ep_protocol_digest": state_ep,
        "terminal_authorization_eq_protocol_digest": _digest(3),
        "terminal_authorization_ep_protocol_digest": _digest(4),
        "commit_wrapper_eq_protocol_digest": _digest(17),
        "commit_wrapper_ep_protocol_digest": _digest(18),
        "helper_protocols": helper_protocols,
    }
    artifact_projection = [
        {
            "role": role,
            "sha256": _sha256(fixture.path(artifact_paths[role]).read_bytes()),
            "byte_len": fixture.path(artifact_paths[role]).stat().st_size,
        }
        for role in VERIFIER.ARTIFACT_ROLES
    ]
    artifact_set_digest = VERIFIER.rust_artifact_set_digest(artifact_projection)
    vk_digest = VERIFIER.rust_vk_set_digest(artifact_projection, protocols)

    shape_path = add_report(
        "reports/global/circuit-shape.json",
        "iroha.kagemusha_v1.circuit_shape_report",
        {
            "k": 16,
            "relations": [
                {"relation": relation, "eq_circuit_rows": 20_000, "ep_circuit_rows": 20_001}
                for relation in VERIFIER.RELATIONS
            ],
            "helpers": [
                {"helper": helper, "eq_circuit_rows": 10_000, "ep_circuit_rows": 10_001}
                for helper in VERIFIER.HELPERS
            ],
        },
    )
    security_path = add_report(
        "reports/global/security-review.json",
        "iroha.kagemusha_v1.security_review_report",
        {
            "source_tree_sha256": source_sha,
            "artifact_set_digest": artifact_set_digest,
            "approved": True,
        },
    )
    kat_path = add_report(
        "reports/global/kat.json",
        "iroha.kagemusha_v1.kat_report",
        {"positive_cases": 100, "adversarial_cases": 100, "failures": 0},
    )
    fuzz_path = add_report(
        "reports/global/fuzz.json",
        "iroha.kagemusha_v1.fuzz_report",
        {"cases_executed": 10_000_000, "failures": 0},
    )
    resource_path = add_report(
        "reports/global/resource.json",
        "iroha.kagemusha_v1.resource_report",
        {"process_rss_bytes": 64 * 1024 * 1024, "passed": True},
    )

    provider_id = "11" * 32
    policy_epoch = 1
    run_id = _digest(0xF150)
    physical_paths = {
        "transcript": "physical/transcript.json",
        "attestation": "physical/oem-attestation.bin",
        "trust_roots": "physical/oem-trust-roots.bin",
        "observer_policy": "physical/observer-policy.json",
        "oem_report": "reports/profile/oem-attestation.json",
    }
    fixture.write(physical_paths["attestation"], b"synthetic OEM attestation only", "oem_attestation")
    fixture.write(physical_paths["trust_roots"], b"synthetic OEM roots only", "oem_trust_roots")
    fixture.write(physical_paths["observer_policy"], observer_policy_bytes, "observer_policy")
    # Allocate the referenced files before command observations are assembled.
    fixture.write(physical_paths["transcript"], b"{}", "physical_transcript")
    fixture.write(physical_paths["oem_report"], b"{}", "report")
    qualification_path = add_report(
        "reports/profile/qualification.json",
        "iroha.kagemusha_v1.hardware_profile_qualification_report",
        {
            "verification_id": f"physical-{run_id}",
            "provider_id": provider_id,
            "policy_epoch": policy_epoch,
            "physical_checks": list(VERIFIER.PHYSICAL_PROFILE_CHECKS),
            "passed": True,
        },
        inputs=list(physical_paths.values()),
    )
    suite_id = "12" * 32
    p256_base_point = (
        "04"
        "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
        "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
    )
    hardware_profile: dict[str, Any] = {
        "version": 1,
        "protocol_version": 1,
        "hardware_profile_id": "0" * 64,
        "provider_id": provider_id,
        "platform_class": "dedicated_secure_element",
        "product_class_digest": "15" * 32,
        "firmware_policy_digest": "16" * 32,
        "enrollment_attestation_verifier_digest": trusted_verifier_sha256,
        "attestation_trust_roots_digest": _sha256(fixture.path(physical_paths["trust_roots"]).read_bytes()),
        "allowed_suite_commitment": VERIFIER._suite_commitment(suite_id),
        "policy_epoch": policy_epoch,
        "governance_credential_public_key": p256_base_point,
        "capability_mask": 65_535,
        "qualification_report_digest": _sha256(
            fixture.path(qualification_path).read_bytes()
        ),
        "valid_from_ms": 1,
        "expires_at_ms": 1_800_000_000_000,
    }
    hardware_profile["hardware_profile_id"] = VERIFIER.rust_hardware_profile_id(
        hardware_profile
    )
    profile_id = hardware_profile["hardware_profile_id"]

    relation_rows: list[dict[str, Any]] = []
    for index, relation in enumerate(VERIFIER.RELATIONS, start=1):
        proof = f"samples/relation-{index:02d}.proof"
        fixture.write(proof, bytes([index]) * (100 + index), "proof")
        fixture.proof_paths.append(proof)
        if relation == "terminal_authorization":
            eq_protocol = protocols["terminal_authorization_eq_protocol_digest"]
            ep_protocol = protocols["terminal_authorization_ep_protocol_digest"]
            eq_role = "terminal_authorization_vk_eq"
            ep_role = "terminal_authorization_vk_ep"
        elif relation == "commit_wrapper":
            eq_protocol = protocols["commit_wrapper_eq_protocol_digest"]
            ep_protocol = protocols["commit_wrapper_ep_protocol_digest"]
            eq_role = "commit_wrapper_vk_eq"
            ep_role = "commit_wrapper_vk_ep"
        else:
            eq_protocol = state_eq
            ep_protocol = state_ep
            eq_role = "state_vk_eq"
            ep_role = "state_vk_ep"
        report_path = add_report(
            f"reports/profile/relation-{index:02d}.json",
            "iroha.kagemusha_v1.relation_qualification_report",
            {
                "hardware_profile_id": profile_id,
                "relation": relation,
                "eq_protocol_digest": eq_protocol,
                "ep_protocol_digest": ep_protocol,
                "eq_verifying_key": artifact_paths[eq_role],
                "ep_verifying_key": artifact_paths[ep_role],
                "eq_circuit_rows": 20_000,
                "ep_circuit_rows": 20_001,
                "proof": proof,
                "prove_p95_ms": 500,
                "verify_p95_ms": 50,
                "process_rss_bytes": 64 * 1024 * 1024,
                "operation_energy_millijoules": 100,
            },
            [proof, artifact_paths[eq_role], artifact_paths[ep_role]],
        )
        relation_rows.append({"relation": relation, "report": report_path})

    helper_rows: list[dict[str, Any]] = []
    for index, helper in enumerate(VERIFIER.HELPERS, start=1):
        protocol = helper_protocols[index - 1]
        eq_role = f"{helper}_vk_eq"
        ep_role = f"{helper}_vk_ep"
        if helper in VERIFIER.INTERNAL_PROOF_HELPERS:
            eq_proof = f"samples/helper-{index:02d}.eq.proof"
            ep_proof = f"samples/helper-{index:02d}.ep.proof"
            fixture.write(
                eq_proof,
                bytes([20 + index]) * protocol["eq_proof_bytes"],
                "internal_proof",
            )
            fixture.write(
                ep_proof,
                bytes([30 + index]) * protocol["ep_proof_bytes"],
                "internal_proof",
            )
            fixture.internal_proof_paths[helper] = (eq_proof, ep_proof)
            proof_fields = {"eq_proof": eq_proof, "ep_proof": ep_proof}
            proof_inputs = [eq_proof, ep_proof]
        else:
            proof = f"samples/helper-{index:02d}.proof"
            fixture.write(proof, bytes([20 + index]) * (120 + index), "proof")
            fixture.proof_paths.append(proof)
            proof_fields = {"proof": proof}
            proof_inputs = [proof]
        report_path = add_report(
            f"reports/profile/helper-{index:02d}.json",
            "iroha.kagemusha_v1.helper_qualification_report",
            {
                "hardware_profile_id": profile_id,
                "helper": helper,
                "eq_protocol_digest": protocol["eq_protocol_digest"],
                "ep_protocol_digest": protocol["ep_protocol_digest"],
                "eq_verifying_key": artifact_paths[eq_role],
                "ep_verifying_key": artifact_paths[ep_role],
                "eq_circuit_rows": 10_000,
                "ep_circuit_rows": 10_001,
                **proof_fields,
                "prove_p95_ms": 500,
                "verify_p95_ms": 50,
                "process_rss_bytes": 64 * 1024 * 1024,
                "operation_energy_millijoules": 100,
            },
            [*proof_inputs, artifact_paths[eq_role], artifact_paths[ep_role]],
        )
        helper_rows.append({"helper": helper, "report": report_path})

    depth_rows: list[dict[str, Any]] = []
    for depth in (8, 64, 1024, 1025):
        proof = f"samples/depth-{depth}.proof"
        raw = f"samples/depth-{depth}.raw"
        text = f"samples/depth-{depth}.txt"
        log = f"logs/depth-{depth}.jsonl"
        fixture.write(proof, bytes([70 + depth % 10]) * 200, "proof")
        fixture.write(
            raw,
            bytes([80 + depth % 10]) * 400,
            "raw_complete_exchange",
        )
        fixture.write(
            text,
            bytes([90 + depth % 10]) * 500,
            "text_complete_exchange",
        )
        fixture.write(
            log,
            b"".join(_json_line({"index": index, "result": "verified"}) for index in range(1, depth + 1)),
            "event_log",
        )
        fixture.proof_paths.append(proof)
        fixture.raw_complete_exchange_paths.append(raw)
        fixture.text_complete_exchange_paths.append(text)
        report_path = add_report(
            f"reports/profile/depth-{depth}.json",
            "iroha.kagemusha_v1.recursive_depth_report",
            {
                "hardware_profile_id": profile_id,
                "depth": depth,
                "eq_protocol_digest": state_eq,
                "ep_protocol_digest": state_ep,
                "eq_verifying_key": artifact_paths["state_vk_eq"],
                "ep_verifying_key": artifact_paths["state_vk_ep"],
                "proof": proof,
                "raw_complete_exchange": raw,
                "text_complete_exchange": text,
                "handoff_log": log,
            },
            [
                proof,
                raw,
                text,
                log,
                artifact_paths["state_vk_eq"],
                artifact_paths["state_vk_ep"],
            ],
        )
        depth_rows.append({"depth": depth, "report": report_path})

    aggregate_proof = "samples/aggregate.proof"
    aggregate_log = "logs/aggregate.jsonl"
    fixture.write(aggregate_proof, b"A" * 210, "proof")
    fixture.proof_paths.append(aggregate_proof)
    credit_ids = [_digest(1000 + index) for index in range(1, 1001)]
    fixture.write(
        aggregate_log,
        b"".join(
            [
                *(
                    _json_line({"event": "payment_created", "index": index, "credit_id": credit_id})
                    for index, credit_id in enumerate(credit_ids, start=1)
                ),
                *(
                    _json_line({"event": "credit_folded", "index": index, "credit_id": credit_id})
                    for index, credit_id in enumerate(credit_ids, start=1)
                ),
                _json_line({"event": "spend_emitted", "index": 1, "result": "verified"}),
            ]
        ),
        "event_log",
    )
    aggregate_path = add_report(
        "reports/profile/aggregate.json",
        "iroha.kagemusha_v1.aggregate_balance_report",
        {
            "hardware_profile_id": profile_id,
            "eq_protocol_digest": state_eq,
            "ep_protocol_digest": state_ep,
            "eq_verifying_key": artifact_paths["state_vk_eq"],
            "ep_verifying_key": artifact_paths["state_vk_ep"],
            "proof": aggregate_proof,
            "events": aggregate_log,
        },
        [aggregate_proof, aggregate_log, artifact_paths["state_vk_eq"], artifact_paths["state_vk_ep"]],
    )

    thermal_proof = "samples/thermal.proof"
    thermal_log = "logs/thermal.jsonl"
    fixture.write(thermal_proof, b"T" * 220, "proof")
    fixture.proof_paths.append(thermal_proof)
    fixture.write(
        thermal_log,
        b"".join(
            _json_line(
                {"index": index, "credit_id": _digest(3000 + index), "result": "folded"}
            )
            for index in range(1, 1001)
        ),
        "event_log",
    )
    thermal_path = add_report(
        "reports/profile/thermal.json",
        "iroha.kagemusha_v1.thermal_report",
        {
            "hardware_profile_id": profile_id,
            "eq_protocol_digest": state_eq,
            "ep_protocol_digest": state_ep,
            "eq_verifying_key": artifact_paths["state_vk_eq"],
            "ep_verifying_key": artifact_paths["state_vk_ep"],
            "proof": thermal_proof,
            "fold_log": thermal_log,
            "fold_p95_ms": 500,
            "process_rss_bytes": 64 * 1024 * 1024,
            "operation_energy_millijoules": 100,
        },
        [thermal_proof, thermal_log, artifact_paths["state_vk_eq"], artifact_paths["state_vk_ep"]],
    )

    envelope_raw = "samples/envelope.raw"
    envelope_text = "samples/envelope.txt"
    fixture.write(envelope_raw, b"R" * 400, "raw_complete_exchange")
    fixture.write(envelope_text, b"X" * 500, "text_complete_exchange")
    fixture.raw_complete_exchange_paths.append(envelope_raw)
    fixture.text_complete_exchange_paths.append(envelope_text)
    envelope_path = add_report(
        "reports/profile/envelope.json",
        "iroha.kagemusha_v1.envelope_report",
        {
            "hardware_profile_id": profile_id,
            "messages": list(VERIFIER.PUBLIC_MESSAGES),
            "raw_complete_exchange": envelope_raw,
            "text_complete_exchange": envelope_text,
            "handoff_p95_ms": 1_000,
        },
        [envelope_raw, envelope_text],
    )

    acceptance_rows: list[dict[str, Any]] = []
    validator_ids = [_digest(5000 + index) for index in range(4)]
    for index, case in enumerate(VERIFIER.ACCEPTANCE_CASES, start=1):
        path = add_report(
            f"reports/profile/acceptance-{index:02d}.json",
            "iroha.kagemusha_v1.acceptance_case_report",
            {
                "hardware_profile_id": profile_id,
                "case": case,
                "validators": validator_ids if case == "four_peer_activation_restart_replay" else [],
                "passed": True,
            },
        )
        acceptance_rows.append({"case": case, "report": path})

    build_rows: list[dict[str, str]] = []
    build_inputs = ["source/candidate.tar", "source/Cargo.lock", *artifact_paths.values()]
    for index in range(2):
        builder_id = _digest(6000 + index)
        path = add_report(
            f"reports/build-{index + 1}.json",
            "iroha.kagemusha_v1.reproducible_build_report",
            {
                "builder_id": builder_id,
                "source_tree_sha256": source_sha,
                "artifact_set_digest": artifact_set_digest,
                "succeeded": True,
            },
            build_inputs,
        )
        build_rows.append({"builder_id": builder_id, "report": path})

    fixture.manifest = {
        "schema": VERIFIER.MANIFEST_SCHEMA,
        "schema_version": 1,
        "source": {"source_archive": "source/candidate.tar", "cargo_lock": "source/Cargo.lock"},
        "files": [],
        "artifacts": artifact_rows,
        "protocols": protocols,
        "global_reports": {
            "circuit_shape": shape_path,
            "security_review": security_path,
            "kat": kat_path,
            "fuzz": fuzz_path,
            "resource": resource_path,
        },
        "profiles": [
            {
                "hardware_profile": hardware_profile,
                "suite_id": suite_id,
                "qualification_report": qualification_path,
                "physical_evidence": physical_paths,
                "relations": relation_rows,
                "helpers": helper_rows,
                "recursive_depths": depth_rows,
                "aggregate_balance": aggregate_path,
                "thermal": thermal_path,
                "envelope": envelope_path,
                "acceptance_cases": acceptance_rows,
            }
        ],
        "reproducible_builds": build_rows,
        "commands": fixture.commands,
    }
    policy = VERIFIER._load_observer_policy(observer_policy_path, fixture.observer_policy_sha256)
    builder = PHYSICAL_TEST._TranscriptBuilder(policy, {observer_authority_id: observer_seed})
    document = builder.build()
    document["profile"].update({key: hardware_profile[key] for key in (
        "hardware_profile_id", "provider_id", "qualification_report_digest", "policy_epoch", "capability_mask",
    )})
    document["endpoint"].update({
        "hardware_profile_id": profile_id,
        "qualification_report_digest": hardware_profile["qualification_report_digest"],
        "platform_class": hardware_profile["platform_class"],
        "attestation_digest": _sha256(fixture.path(physical_paths["attestation"]).read_bytes()),
    })
    document["run"].update({
        "run_id": run_id, "candidate_context_digest": fixture.candidate_context_digest(),
        "artifact_set_digest": artifact_set_digest,
    })
    builder.approve(document)
    fixture.write(physical_paths["transcript"], VERIFIER.canonical_json_bytes(document), "physical_transcript")
    oem_body = {
        **{key: hardware_profile[key] for key in (
            "hardware_profile_id", "provider_id", "policy_epoch", "capability_mask",
            "product_class_digest", "firmware_policy_digest",
        )},
        **{key: document["endpoint"][key] for key in (
            "hardware_policy_id", "platform_class", "device_id", "product_id", "firmware_digest",
            "os_build_digest", "hardware_backed", "software_fallback", "production_build",
        )},
        **{key: document["run"][key] for key in (
            "candidate_context_digest", "artifact_set_digest", "run_id", "started_at_ms", "ended_at_ms",
        )},
        "challenge_sha256": VERIFIER.physical_oem_challenge(profile_id, document["endpoint"], document["run"]),
        "attestation_verifier_sha256": trusted_verifier_sha256,
        **{name: {
            "sha256": _sha256(fixture.path(physical_paths[name]).read_bytes()),
            "byte_len": fixture.path(physical_paths[name]).stat().st_size,
        } for name in ("attestation", "trust_roots", "transcript", "observer_policy")},
        "passed": True,
    }
    add_report(physical_paths["oem_report"], VERIFIER.OEM_ATTESTATION_REPORT_SCHEMA, oem_body,
               inputs=list(physical_paths.values()))
    fixture.commands.sort(key=lambda command: command["id"])
    fixture.refresh_files()
    fixture.resign_all_for_candidate_context()
    fixture.refresh_files()
    return fixture


def _run(fixture: EvidenceFixture, digest: str | None = None) -> subprocess.CompletedProcess[str]:
    expected = digest or _sha256(fixture.manifest_path.read_bytes())
    return subprocess.run(
        [
            sys.executable,
            str(VERIFIER_PATH),
            "--manifest",
            str(fixture.manifest_path),
            "--manifest-sha256",
            expected,
            "--evidence-root",
            str(fixture.root),
            "--observer-policy",
            str(fixture.observer_policy_path),
            "--observer-policy-sha256",
            fixture.observer_policy_sha256,
        ],
        text=True,
        capture_output=True,
        check=False,
        timeout=60,
    )


def _verify_direct(fixture: EvidenceFixture) -> dict[str, Any]:
    return VERIFIER.verify_evidence(
        manifest_path=fixture.manifest_path,
        expected_manifest_sha256=_sha256(fixture.manifest_path.read_bytes()),
        evidence_root=fixture.root,
        observer_policy_path=fixture.observer_policy_path,
        expected_observer_policy_sha256=fixture.observer_policy_sha256,
    )


def test_acceptance_matrix_is_plan_complete_and_matches_rust_order() -> None:
    cases = VERIFIER.ACCEPTANCE_CASES
    assert len(cases) == len(set(cases))
    assert {
        "missing_sender_authorization",
        "forged_sender_authorization",
        "replayed_sender_authorization",
        "cross_release_sender_authorization",
        "missing_mint_authorization",
        "forged_mint_authorization",
        "replayed_mint_authorization",
        "cross_release_mint_authorization",
        "crash_during_transport",
        "crash_during_ack_recovery",
        "ack_recovery_idempotence",
        "delayed_delivery_across_ordinary_suite_rotation",
        "delayed_delivery_across_credential_rotation",
        "monotonic_lease_expiry",
        "hardware_epoch_rollover",
        "hardware_counter_rollover",
        "positive_exact_request",
        "recipient_key_binding",
        "request_amount_mismatch_rejection",
        "committed_payment_after_request_expiry",
        "distinct_payments_same_request",
        "exact_duplicate_durable_ack",
        "conflicting_credit_id_bytes",
        "transcript_unlinkability",
        "x25519_low_order_public_key_rejection",
        "x25519_zero_dh_rejection",
        "aead_ciphertext_substitution",
        "aead_associated_data_substitution",
        "deterministic_encryption_injected_randomness_kat",
        "receive_fold_single_credit",
        "receive_fold_replay_atomicity",
        "pending_credit_backlog_no_count_rejection",
    } <= set(cases)

    outbound_crash_order = (
        "crash_during_prepare",
        "crash_after_prepare_before_proof",
        "crash_during_proof",
        "crash_after_proof_before_candidate_persistence",
        "crash_during_candidate_persistence",
        "crash_after_candidate_persistence_before_verification",
        "crash_during_candidate_verification",
        "crash_after_candidate_verification_before_hardware_commit",
        "crash_during_hardware_commit",
        "crash_after_hardware_commit_before_terminal_authorization",
        "crash_during_terminal_authorization",
        "crash_after_terminal_authorization_before_final_envelope_persistence",
        "crash_during_final_envelope_persistence",
        "crash_after_final_envelope_persistence_before_exposure",
        "crash_during_exposure",
    )
    start = cases.index(outbound_crash_order[0])
    assert cases[start : start + len(outbound_crash_order)] == outbound_crash_order
    assert {
        "crash_after_transition_intent_stage",
        "crash_after_hardware_commit_before_proof",
        "crash_after_proof_before_state_persistence",
        "crash_after_state_persistence_before_envelope",
        "crash_after_envelope_before_exposure",
    }.isdisjoint(cases)

    rust_source = (
        ROOT
        / "crates/iroha_data_model/src/kagemusha/kagemusha_release_v1.rs"
    ).read_text()
    all_block = rust_source.split("pub const ALL: &[Self] = &[", 1)[1].split(
        "];", 1
    )[0]
    rust_order = re.findall(r"Self::([A-Za-z0-9]+),", all_block)
    python_order_as_rust = [
        "".join(part[:1].upper() + part[1:] for part in case.split("_"))
        for case in cases
    ]
    assert rust_order == python_order_as_rust


@pytest.mark.parametrize(
    ("rust_type", "inventory"),
    [
        ("KagemushaArtifactRoleV1", VERIFIER.ARTIFACT_ROLES),
        ("KagemushaQualifiedRelationV1", VERIFIER.RELATIONS),
        ("KagemushaQualifiedHelperCircuitV1", VERIFIER.HELPERS),
    ],
)
def test_release_inventory_order_matches_current_rust_model(
    rust_type: str, inventory: tuple[str, ...]
) -> None:
    source = (
        ROOT / "crates/iroha_data_model/src/kagemusha/kagemusha_release_v1.rs"
    ).read_text()
    ordered = re.search(
        rf"impl {rust_type} \{{.*?pub const ALL: \[Self; \d+\] = \[(.*?)\];",
        source,
        re.DOTALL,
    )
    assert ordered is not None, f"missing canonical Rust inventory for {rust_type}"
    rust_names = re.findall(r"Self::([A-Za-z0-9]+),", ordered.group(1))
    python_names = [
        "".join(part[:1].upper() + part[1:] for part in name.split("_"))
        for name in inventory
    ]
    assert python_names == rust_names


def test_release_artifact_ordinals_match_current_rust_model() -> None:
    source = (
        ROOT / "crates/iroha_data_model/src/kagemusha/kagemusha_release_v1.rs"
    ).read_text()
    enum = source.split("pub enum KagemushaArtifactRoleV1 {", 1)[1].split("}", 1)[0]
    declarations = re.findall(
        r"^    ([A-Za-z0-9]+)(?: = (\d+))?,$", enum, re.MULTILINE
    )
    rust_roles = []
    next_ordinal = 0
    for name, explicit_ordinal in declarations:
        ordinal = int(explicit_ordinal) if explicit_ordinal else next_ordinal
        rust_roles.append((name, str(ordinal)))
        next_ordinal = ordinal + 1
    expected = [
        (
            "".join(part[:1].upper() + part[1:] for part in role.split("_")),
            str(ordinal),
        )
        for ordinal, role in enumerate(VERIFIER.ARTIFACT_ROLES)
    ]
    assert rust_roles == expected
    assert len(rust_roles) == 50


def test_native_inner_mint_profiles_are_required_and_authenticated() -> None:
    """Pin native requirements without claiming to verify a native profile here."""

    native = (
        ROOT / "crates/iroha_core/src/zk/kagemusha_v1_recursion/native_backend.rs"
    ).read_text()
    config = (ROOT / "crates/iroha_core/src/smartcontracts/isi/kagemusha.rs").read_text()
    native_profile = native.split(
        "pub struct KagemushaRecursiveVerifierProfileV1 {", 1
    )[1].split("}", 1)[0]
    config_profile = config.split(
        "struct KagemushaRecursiveVerifierProfileFileV1 {", 1
    )[1].split("}", 1)[0]
    native_fields = re.findall(r"pub (\w+): BaseCircuitParams,", native_profile)
    config_fields = re.findall(
        r"^    (\w+): KagemushaBaseCircuitProfileFileV1,", config_profile, re.MULTILINE
    )
    assert native_fields == config_fields
    mint_profile_fields = (
        "inner_mint_authorization_eq",
        "inner_mint_authorization_ep",
        "inner_mint_eq",
        "inner_mint_ep",
        "mint_hash_shard_eq",
        "mint_hash_shard_ep",
        "mint_hash_claim_eq",
        "mint_hash_claim_ep",
    )
    assert tuple(native_fields[-8:]) == mint_profile_fields
    digest_impl = native.split("impl KagemushaRecursiveVerifierProfileV1 {", 1)[1].split(
        "    fn validate(&self)", 1
    )[0]
    validation_impl = native.split("    fn validate(&self)", 1)[1].split(
        "        if self.mint_eq_protocol_digest", 1
    )[0]
    for tag, field in enumerate(mint_profile_fields, start=18):
        assert re.search(rf"\({tag}(?:_u8)?, &self\.{field}\)", digest_impl)
        assert f"&self.{field}" in validation_impl


@pytest.mark.parametrize("missing_role", [None, *VERIFIER.ARTIFACT_ROLES[34:]])
def test_release_rejects_missing_inner_mint_artifacts(
    tmp_path: Path, missing_role: str | None
) -> None:
    fixture = _fixture(tmp_path)
    fixture.manifest["artifacts"] = [
        row
        for row in fixture.manifest["artifacts"]
        if (
            row["role"] != missing_role
            if missing_role is not None
            else row["role"] not in VERIFIER.ARTIFACT_ROLES[34:]
        )
    ]
    fixture.write_manifest()
    result = _run(fixture)
    assert result.returncode == 1
    assert "artifact inventory must contain exactly the 50 V1 roles" in result.stderr


def test_release_rejects_reordered_inner_mint_artifacts(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    artifacts = fixture.manifest["artifacts"]
    artifacts[34], artifacts[38] = artifacts[38], artifacts[34]
    fixture.write_manifest()
    result = _run(fixture)
    assert result.returncode == 1
    assert "artifact inventory is not in the canonical V1 role order" in result.stderr


def test_public_message_inventory_matches_current_rust_exchange() -> None:
    source = (
        ROOT / "crates/iroha_data_model/src/kagemusha/kagemusha_v1.rs"
    ).read_text()
    signature = re.search(
        r"pub fn validate_kagemusha_complete_exchange_shape_v1\((.*?)\) ->",
        source,
        re.DOTALL,
    )
    assert signature is not None
    rust_messages = re.findall(r": &(Kagemusha[A-Za-z0-9]+V1)", signature.group(1))
    python_messages = [
        "Kagemusha"
        + "".join(part[:1].upper() + part[1:] for part in name.split("_"))
        + "V1"
        for name in VERIFIER.PUBLIC_MESSAGES
    ]
    assert python_messages == rust_messages
    payment = source.split("pub struct KagemushaPaymentV1 {", 1)[1].split("}", 1)[0]
    assert "pub proof: KagemushaPaymentProofV1" in payment


@pytest.mark.parametrize("suffix", ["pk_eq", "vk_eq", "pk_ep", "vk_ep"])
def test_release_rejects_retired_pre_ticket_proof_artifact_roles(
    tmp_path: Path, suffix: str
) -> None:
    fixture = _fixture(tmp_path)
    row = next(
        row for row in fixture.manifest["artifacts"] if row["role"] == f"commit_wrapper_{suffix}"
    )
    row["role"] = "acceptance_intent_" + "authorization_" + suffix
    fixture.write_manifest()
    result = _run(fixture)
    assert result.returncode == 1
    assert "artifact inventory is not in the canonical V1 role order" in result.stderr


@pytest.mark.parametrize("parity", ["eq", "ep"])
@pytest.mark.parametrize("retain_current", [False, True])
def test_release_rejects_retired_pre_ticket_proof_protocol_keys(
    tmp_path: Path, parity: str, retain_current: bool
) -> None:
    fixture = _fixture(tmp_path)
    protocols = fixture.manifest["protocols"]
    current = f"commit_wrapper_{parity}_protocol_digest"
    retired = "acceptance_intent_" + f"authorization_{parity}_protocol_digest"
    protocols[retired] = protocols[current]
    if not retain_current:
        del protocols[current]
    fixture.write_manifest()
    result = _run(fixture)
    assert result.returncode == 1
    assert "compiled protocols fields must be exactly" in result.stderr


@pytest.mark.parametrize("location", ["matrix", "report"])
def test_release_rejects_retired_pre_ticket_proof_relation(
    tmp_path: Path, location: str
) -> None:
    fixture = _fixture(tmp_path)
    row = next(
        row
        for row in fixture.manifest["profiles"][0]["relations"]
        if row["relation"] == "commit_wrapper"
    )
    retired = "acceptance_intent_" + "authorization"
    if location == "matrix":
        row["relation"] = retired
        fixture.write_manifest()
        expected_error = "profile relation matrix is not in canonical order"
    else:
        report = json.loads(fixture.path(row["report"]).read_text())
        report["relation"] = retired
        fixture.write(row["report"], VERIFIER.canonical_json_bytes(report), "report")
        fixture.resign_commands_for_file(row["report"])
        fixture.refresh_files()
        expected_error = "typed report relation binding differs from its matrix row"
    result = _run(fixture)
    assert result.returncode == 1
    assert expected_error in result.stderr


def test_valid_closure_derives_complete_projection_deterministically(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    first = _run(fixture)
    second = _run(fixture)
    assert first.returncode == 0, first.stderr
    assert second.returncode == 0, second.stderr
    assert first.stdout == second.stdout
    projection = json.loads(first.stdout)
    profile = projection["receipt_projection"]["profile_qualifications"][0]
    assert [row["relation"] for row in profile["relations"]] == list(
        VERIFIER.RELATIONS
    )
    assert len(profile["relations"]) == 8
    assert {
        row["eq_verifying_key"]["role"] for row in profile["relations"]
    } == {
        "state_vk_eq",
        "terminal_authorization_vk_eq",
        "commit_wrapper_vk_eq",
    }
    assert {
        row["ep_verifying_key"]["role"] for row in profile["relations"]
    } == {
        "state_vk_ep",
        "terminal_authorization_vk_ep",
        "commit_wrapper_vk_ep",
    }
    assert len(profile["helper_circuits"]) == 6
    assert [row["helper"] for row in profile["helper_circuits"]] == list(
        VERIFIER.HELPERS
    )
    for helper in profile["helper_circuits"][2:]:
        expected_eq, expected_ep = INTERNAL_HELPER_PROOF_LENGTHS[helper["helper"]]
        assert helper["eq_proof_bytes"] == expected_eq
        assert helper["ep_proof_bytes"] == expected_ep
        assert helper["complete_proof_bytes"] == expected_eq + expected_ep
        assert helper["complete_proof_bytes"] > VERIFIER.PAIRED_PROOF_MAX_BYTES
    assert [row["depth"] for row in profile["recursive_depths"]] == [8, 64, 1024, 1025]
    assert [row["verified_handoffs"] for row in profile["recursive_depths"]] == [8, 64, 1024, 1025]
    assert profile["aggregate_balance"]["independent_payments"] == 1000
    assert profile["aggregate_balance"]["folded_credits"] == 1000
    assert profile["aggregate_balance"]["spend_payments"] == 1
    assert [row["case"] for row in profile["acceptance_cases"]] == list(
        VERIFIER.ACCEPTANCE_CASES
    )
    assert len(profile["acceptance_cases"]) == len(VERIFIER.ACCEPTANCE_CASES)
    assert len(projection["artifact_inventory"]) == 50
    assert [row["role"] for row in projection["artifact_inventory"]][2:10] == [
        "inner_state_pk_eq",
        "inner_state_vk_eq",
        "inner_state_pk_ep",
        "inner_state_vk_ep",
        "state_pk_eq",
        "state_vk_eq",
        "state_pk_ep",
        "state_vk_ep",
    ]
    assert [row["role"] for row in projection["artifact_inventory"]][10:14] == [
        "mint_authorization_pk_eq",
        "mint_authorization_vk_eq",
        "mint_authorization_pk_ep",
        "mint_authorization_vk_ep",
    ]
    assert [row["role"] for row in projection["artifact_inventory"]][34:42] == [
        "inner_mint_authorization_pk_eq",
        "inner_mint_authorization_vk_eq",
        "inner_mint_authorization_pk_ep",
        "inner_mint_authorization_vk_ep",
        "inner_mint_credit_pk_eq",
        "inner_mint_credit_vk_eq",
        "inner_mint_credit_pk_ep",
        "inner_mint_credit_vk_ep",
    ]
    assert [row["role"] for row in projection["artifact_inventory"]][42:50] == [
        "mint_hash_shard_pk_eq",
        "mint_hash_shard_vk_eq",
        "mint_hash_shard_pk_ep",
        "mint_hash_shard_vk_ep",
        "mint_hash_claim_pk_eq",
        "mint_hash_claim_vk_eq",
        "mint_hash_claim_pk_ep",
        "mint_hash_claim_vk_ep",
    ]
    assert len(projection["verifier_commands"]) == len(fixture.commands)
    candidate_context_digest = projection["receipt_projection"]["evidence_closure"][
        "candidate_context_digest"
    ]
    assert projection["candidate_context"]["schema"] == VERIFIER.CANDIDATE_CONTEXT_SCHEMA
    assert {
        command["candidate_context_digest"]
        for command in projection["verifier_commands"]
    } == {candidate_context_digest}
    assert first.stdout.encode() == VERIFIER.canonical_json_bytes(projection)


def test_exact_proof_and_complete_exchange_limits_are_admitted(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    fixture.write(fixture.proof_paths[0], b"P" * 6_528, "proof")
    fixture.resign_commands_for_file(fixture.proof_paths[0])
    wire_helper_row = fixture.manifest["profiles"][0]["helpers"][0]
    wire_helper_report = json.loads(
        fixture.path(wire_helper_row["report"]).read_text()
    )
    wire_helper_proof = wire_helper_report["proof"]
    fixture.write(wire_helper_proof, b"H" * 6_528, "proof")
    fixture.resign_commands_for_file(wire_helper_proof)
    for relative in fixture.raw_complete_exchange_paths:
        fixture.write(relative, b"R" * 9_211, "raw_complete_exchange")
        fixture.resign_commands_for_file(relative)
    for relative in fixture.text_complete_exchange_paths:
        fixture.write(relative, b"T" * 12_288, "text_complete_exchange")
        fixture.resign_commands_for_file(relative)
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 0, result.stderr
    profile = json.loads(result.stdout)["receipt_projection"]["profile_qualifications"][0]
    assert profile["relations"][0]["complete_proof_bytes"] == 6_528
    assert profile["helper_circuits"][0]["complete_proof_bytes"] == 6_528
    assert profile["helper_circuits"][0]["eq_proof_bytes"] == 0
    assert profile["helper_circuits"][0]["ep_proof_bytes"] == 0
    assert {
        row["raw_complete_exchange_bytes"] for row in profile["recursive_depths"]
    } == {9_211}
    assert {
        row["text_complete_exchange_bytes"] for row in profile["recursive_depths"]
    } == {12_288}
    assert profile["envelope"]["raw_complete_exchange_bytes"] == 9_211
    assert profile["envelope"]["text_complete_exchange_bytes"] == 12_288
    retired_measurements = {
        "raw_" + "ses" + "sion_bytes",
        "text_" + "ses" + "sion_bytes",
    }
    assert retired_measurements.isdisjoint(profile["envelope"])
    assert all(
        retired_measurements.isdisjoint(row) for row in profile["recursive_depths"]
    )


@pytest.mark.parametrize(
    "messages",
    [
        [
            "payment_request",
            "acceptance_intent_" + "authorization",
            "payment",
            "acknowledgement",
        ],
        [
            "payment_request",
            "acceptance_intent",
            "payment",
            "acknowledgement",
        ],
        [
            "payment_request",
            "acceptance_ticket",
            "payment",
            "acknowledgement",
        ],
        [
            "payment_request",
            "receipt",
        ],
    ],
)
def test_envelope_requires_exact_three_message_protocol(
    tmp_path: Path, messages: list[str]
) -> None:
    fixture = _fixture(tmp_path)
    envelope_report = fixture.manifest["profiles"][0]["envelope"]
    report = json.loads(fixture.path(envelope_report).read_text())
    report["messages"] = messages
    fixture.write(envelope_report, VERIFIER.canonical_json_bytes(report), "report")
    fixture.resign_commands_for_file(envelope_report)
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "exactly payment_request, payment, and acknowledgement" in result.stderr


@pytest.mark.parametrize("representation", ("raw", "text"))
def test_envelope_rejects_retired_artifact_keys(
    tmp_path: Path, representation: str
) -> None:
    fixture = _fixture(tmp_path)
    envelope_report = fixture.manifest["profiles"][0]["envelope"]
    report = json.loads(fixture.path(envelope_report).read_text())
    current_key = f"{representation}_complete_exchange"
    retired_key = f"{representation}_" + "ses" + "sion"
    report[retired_key] = report.pop(current_key)
    fixture.write(envelope_report, VERIFIER.canonical_json_bytes(report), "report")
    fixture.resign_commands_for_file(envelope_report)
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "fields must be exactly" in result.stderr
    assert "raw_complete_exchange" in result.stderr
    assert "text_complete_exchange" in result.stderr


@pytest.mark.parametrize("representation", ("raw", "text"))
def test_manifest_rejects_retired_complete_exchange_file_kinds(
    tmp_path: Path, representation: str
) -> None:
    fixture = _fixture(tmp_path)
    paths = getattr(fixture, f"{representation}_complete_exchange_paths")
    retired_kind = f"{representation}_" + "ses" + "sion"
    fixture.kinds[paths[0]] = retired_kind
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "has unsupported kind" in result.stderr


@pytest.mark.parametrize(
    ("kind", "limit", "collection"),
    [
        ("proof", 6_528, "proof_paths"),
        ("raw_complete_exchange", 9_211, "raw_complete_exchange_paths"),
        ("text_complete_exchange", 12_288, "text_complete_exchange_paths"),
    ],
)
def test_exact_size_gates_reject_one_byte_over(
    tmp_path: Path, kind: str, limit: int, collection: str
) -> None:
    fixture = _fixture(tmp_path)
    relative = getattr(fixture, collection)[0]
    fixture.write(relative, b"Z" * (limit + 1), kind)
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert str(limit) in result.stderr or "exceeds" in result.stderr


@pytest.mark.parametrize("helper", sorted(VERIFIER.INTERNAL_PROOF_HELPERS))
def test_internal_helper_proof_evidence_must_match_each_pinned_parity(
    tmp_path: Path, helper: str
) -> None:
    fixture = _fixture(tmp_path)
    eq_proof, _ = fixture.internal_proof_paths[helper]
    fixture.write(
        eq_proof,
        fixture.path(eq_proof).read_bytes() + (b"X" * 32),
        "internal_proof",
    )
    fixture.resign_commands_for_file(eq_proof)
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "release-pinned per-parity lengths" in result.stderr


@pytest.mark.parametrize("bad_length", [0, 8_001])
def test_internal_helper_protocol_rejects_zero_or_unaligned_exact_lengths(
    tmp_path: Path, bad_length: int
) -> None:
    fixture = _fixture(tmp_path)
    fixture.manifest["protocols"]["helper_protocols"][2][
        "eq_proof_bytes"
    ] = bad_length
    fixture.resign_all_for_candidate_context()
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "nonzero 32-byte multiples" in result.stderr


def test_wire_helper_cannot_declare_an_internal_exact_length(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    fixture.manifest["protocols"]["helper_protocols"][0]["eq_proof_bytes"] = 32
    fixture.resign_all_for_candidate_context()
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "paired-wire helper" in result.stderr


@pytest.mark.parametrize(
    "matrix",
    [
        "relations",
        "helpers",
        "recursive_depths",
        "acceptance_cases",
    ],
)
def test_closed_matrices_reject_missing_rows(tmp_path: Path, matrix: str) -> None:
    fixture = _fixture(tmp_path)
    fixture.manifest["profiles"][0][matrix].pop()
    digest = fixture.write_manifest()
    result = _run(fixture, digest)
    assert result.returncode == 1
    if matrix == "acceptance_cases":
        assert (
            f"exactly {len(VERIFIER.ACCEPTANCE_CASES)} acceptance cases"
            in result.stderr
        )
    else:
        assert "matrix" in result.stderr


def test_acceptance_matrix_rejects_noncanonical_order(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    rows = fixture.manifest["profiles"][0]["acceptance_cases"]
    rows[0], rows[1] = rows[1], rows[0]
    digest = fixture.write_manifest()
    result = _run(fixture, digest)
    assert result.returncode == 1
    assert (
        f"canonical {len(VERIFIER.ACCEPTANCE_CASES)}-case order" in result.stderr
    )


def test_manifest_digest_and_file_digest_are_both_pinned(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    old_manifest_digest = _sha256(fixture.manifest_path.read_bytes())
    fixture.manifest["protocols"]["state_eq_protocol_digest"] = _digest(9999)
    fixture.write_manifest()
    manifest_result = _run(fixture, old_manifest_digest)
    assert manifest_result.returncode == 1
    assert "explicit immutable identity" in manifest_result.stderr

    fixture = _fixture(tmp_path / "file")
    target = fixture.proof_paths[0]
    fixture.path(target).write_bytes(b"changed")
    file_result = _run(fixture)
    assert file_result.returncode == 1
    assert "digest or length" in file_result.stderr


def test_undeclared_files_symlinks_and_hardlinks_are_rejected(tmp_path: Path) -> None:
    extra_fixture = _fixture(tmp_path / "extra")
    extra_fixture.write("undeclared.bin", b"undeclared\n", "proof")
    del extra_fixture.kinds["undeclared.bin"]
    extra = _run(extra_fixture)
    assert extra.returncode == 1
    assert "undeclared" in extra.stderr

    symlink_fixture = _fixture(tmp_path / "symlink")
    relative = symlink_fixture.proof_paths[0]
    target = tmp_path / "outside-proof"
    target.write_bytes(symlink_fixture.path(relative).read_bytes())
    symlink_fixture.path(relative).unlink()
    try:
        symlink_fixture.path(relative).symlink_to(target)
    except OSError as error:
        pytest.skip(f"symlinks unavailable: {error}")
    symlink = _run(symlink_fixture)
    assert symlink.returncode == 1
    assert "symlink" in symlink.stderr or "regular file" in symlink.stderr

    hardlink_fixture = _fixture(tmp_path / "hardlink")
    relative = hardlink_fixture.proof_paths[0]
    outside = tmp_path / "outside-hardlink"
    try:
        os.link(hardlink_fixture.path(relative), outside)
    except OSError as error:
        pytest.skip(f"hardlinks unavailable: {error}")
    hardlink = _run(hardlink_fixture)
    assert hardlink.returncode == 1
    assert "hard link" in hardlink.stderr


def test_one_measurement_sample_cannot_alias_two_matrix_cells(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    first, second = fixture.manifest["profiles"][0]["recursive_depths"][:2]
    first_report = json.loads(fixture.path(first["report"]).read_text())
    second_report = json.loads(fixture.path(second["report"]).read_text())
    old_proof = second_report["proof"]
    second_report["proof"] = first_report["proof"]
    fixture.write(second["report"], VERIFIER.canonical_json_bytes(second_report), "report")
    command = next(
        row for row in fixture.commands if row["id"] == second_report["verification_id"]
    )
    command["arguments"] = [
        {"file": first_report["proof"]} if arg == {"file": old_proof} else arg
        for arg in command["arguments"]
    ]
    command["arguments"] = sorted(command["arguments"], key=lambda arg: arg["file"])
    fixture.resign_command(command["id"])
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "measurement sample" in result.stderr and "aliased" in result.stderr


def test_receive_fold_batch_occupancies_are_rejected(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    fixture.manifest["profiles"][0]["receive_fold_occupancies"] = []
    fixture.write_manifest()
    result = _run(fixture)
    assert result.returncode == 1
    assert "candidate profile input 0 fields must be exactly" in result.stderr


def test_counts_are_derived_from_typed_logs(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    log = fixture.path("logs/depth-1024.jsonl")
    rows = log.read_bytes().splitlines(keepends=True)
    fixture.write("logs/depth-1024.jsonl", b"".join(rows[:-1]), "event_log")
    fixture.resign_commands_for_file("logs/depth-1024.jsonl")
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "handoff count" in result.stderr


def test_observation_must_be_signed_and_match_its_transcript(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    stdout_path = fixture.commands[0]["stdout"]
    fixture.write(stdout_path, b"substituted transcript\n", "transcript")
    fixture.refresh_files()
    mismatch = _run(fixture)
    assert mismatch.returncode == 1
    assert "substitutes its trusted subject" in mismatch.stderr

    fixture = _fixture(tmp_path / "signature")
    observation_path = fixture.commands[0]["observation"]
    observation = json.loads(fixture.path(observation_path).read_text())
    observation["approvals"][0]["signature"] = "00" * 64
    fixture.write(
        observation_path,
        VERIFIER.canonical_json_bytes(observation),
        "observation",
    )
    fixture.refresh_files()
    failed = _run(fixture)
    assert failed.returncode == 1
    assert "invalid approval" in failed.stderr


def test_direct_relation_rejects_inner_state_verifying_key(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    relation_row = fixture.manifest["profiles"][0]["relations"][-1]
    report_path = fixture.path(relation_row["report"])
    report = json.loads(report_path.read_text())
    old_key = report["eq_verifying_key"]
    report["eq_verifying_key"] = next(
        row["path"]
        for row in fixture.manifest["artifacts"]
        if row["role"] == "inner_state_vk_eq"
    )
    fixture.write(
        relation_row["report"], VERIFIER.canonical_json_bytes(report), "report"
    )
    command = next(
        row
        for row in fixture.commands
        if row["id"] == report["verification_id"]
    )
    command["arguments"] = [
        {"file": report["eq_verifying_key"]}
        if argument == {"file": old_key}
        else argument
        for argument in command["arguments"]
    ]
    command["arguments"] = sorted(command["arguments"], key=lambda value: value["file"])
    fixture.resign_command(command["id"])
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "protocol or verifying-key binding" in result.stderr


def test_artifact_roles_cannot_reuse_identical_bytes(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    first, second = fixture.manifest["artifacts"][2:4]
    fixture.write(
        second["path"], fixture.path(first["path"]).read_bytes(), "artifact"
    )
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "distinct file bytes" in result.stderr


def test_per_profile_four_peer_case_derives_exact_validator_count(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    case_row = next(
        row
        for row in fixture.manifest["profiles"][0]["acceptance_cases"]
        if row["case"] == "four_peer_activation_restart_replay"
    )
    path = fixture.path(case_row["report"])
    report = json.loads(path.read_text())
    report["validators"].pop()
    fixture.write(case_row["report"], VERIFIER.canonical_json_bytes(report), "report")
    fixture.resign_commands_for_file(case_row["report"])
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "wrong validator set" in result.stderr


def test_input_replacement_during_projection_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    original = VERIFIER.EvidenceVerifier._verify_profiles
    invoked = False

    def mutate_after_profile_validation(verifier: Any, *args: Any, **kwargs: Any) -> Any:
        nonlocal invoked
        result = original(verifier, *args, **kwargs)
        if not invoked:
            invoked = True
            fixture.path("source/candidate.tar").write_bytes(b"replaced during replay\n")
        return result

    monkeypatch.setattr(
        VERIFIER.EvidenceVerifier, "_verify_profiles", mutate_after_profile_validation
    )
    with pytest.raises((VERIFIER.KagemushaEvidenceError, VERIFIER.ReleaseArtifactError)):
        VERIFIER.verify_evidence(
            manifest_path=fixture.manifest_path,
            expected_manifest_sha256=_sha256(fixture.manifest_path.read_bytes()),
            evidence_root=fixture.root,
            observer_policy_path=fixture.observer_policy_path,
            expected_observer_policy_sha256=fixture.observer_policy_sha256,
        )


def test_signed_observations_cannot_replay_across_candidate_source_trees(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    kat_report = fixture.manifest["global_reports"]["kat"]
    kat_command_id = json.loads(fixture.path(kat_report).read_text())["verification_id"]
    kat_command = next(row for row in fixture.commands if row["id"] == kat_command_id)
    kat_observation = fixture.path(kat_command["observation"]).read_bytes()

    fixture.write(
        "source/candidate.tar",
        b"different immutable candidate source archive\n",
        "source_archive",
    )
    source_sha = _sha256(fixture.path("source/candidate.tar").read_bytes())
    source_bound_reports = [fixture.manifest["global_reports"]["security_review"]]
    source_bound_reports.extend(
        row["report"] for row in fixture.manifest["reproducible_builds"]
    )
    for report_path in source_bound_reports:
        report = json.loads(fixture.path(report_path).read_text())
        report["source_tree_sha256"] = source_sha
        fixture.write(
            report_path,
            VERIFIER.canonical_json_bytes(report),
            "report",
        )
        fixture.resign_command(report["verification_id"])
    fixture.refresh_files()

    assert fixture.path(kat_command["observation"]).read_bytes() == kat_observation
    result = _run(fixture)
    assert result.returncode == 1
    assert "substitutes its trusted subject" in result.stderr


def test_candidate_selected_executables_are_not_an_evidence_kind(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    fixture.write(
        "tools/candidate-verifier",
        b"#!/bin/sh\nexit 0\n",
        "verifier_executable",
        mode=0o700,
    )
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "unsupported kind" in result.stderr
    source = VERIFIER_PATH.read_text()
    assert "subprocess.Popen" not in source
    assert "process.kill" not in source


def test_arbitrary_proof_requires_a_trusted_signed_observation(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    proof = fixture.proof_paths[0]
    fixture.write(proof, b"arbitrary proof bytes", "proof")
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "substitutes its trusted subject" in result.stderr


def test_untrusted_verifier_and_changed_policy_are_rejected(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path / "verifier")
    fixture.commands[0]["verifier_id"] = "candidate-selected-noop"
    fixture.write_manifest()
    untrusted = _run(fixture)
    assert untrusted.returncode == 1
    assert "trusted local allowlist" in untrusted.stderr

    fixture = _fixture(tmp_path / "policy")
    fixture.observer_policy_path.write_bytes(b"{}")
    changed = _run(fixture)
    assert changed.returncode == 1
    assert "explicit immutable identity" in changed.stderr


def test_projection_derives_full_rust_release_identities(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    projection = _verify_direct(fixture)
    receipt = projection["receipt_projection"]
    qualification = receipt["profile_qualifications"][0]
    enabled = qualification["profile"]
    assert enabled["hardware_profile"] == fixture.manifest["profiles"][0]["hardware_profile"]
    assert enabled["hardware_profile_id"] == VERIFIER.rust_hardware_profile_id(
        enabled["hardware_profile"]
    )
    assert receipt["artifact_set_digest"] == VERIFIER.rust_artifact_set_digest(
        projection["artifact_inventory"]
    )
    assert enabled["vk_digest"] == VERIFIER.rust_vk_set_digest(
        projection["artifact_inventory"], fixture.manifest["protocols"]
    )
    assert enabled["qualification_digest"] == VERIFIER.rust_profile_qualification_digest(
        qualification
    )
    assert receipt["hardware_policy_digest"] == VERIFIER.rust_hardware_policy_digest(
        [enabled]
    )
    assert VERIFIER._crc64_xz(b"123456789") == 0x995DC9BBDF1939FA


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("vk_digest", "aa" * 32),
        ("qualification_digest", "bb" * 32),
        ("hardware_policy_digest", "cc" * 32),
        ("artifact_set_digest", "dd" * 32),
    ],
)
def test_bundle_cannot_declare_authoritative_release_digests(
    tmp_path: Path, field: str, value: str
) -> None:
    fixture = _fixture(tmp_path)
    fixture.manifest["profiles"][0][field] = value
    fixture.write_manifest()
    result = _run(fixture)
    assert result.returncode == 1
    assert "fields must be exactly" in result.stderr


def test_hardware_profile_body_substitution_is_rejected(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    fixture.manifest["profiles"][0]["hardware_profile"]["provider_id"] = "ee" * 32
    fixture.resign_all_for_candidate_context()
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "Rust-derived qualified hardware" in result.stderr


def test_physical_qualification_requires_clock_rollback_even_when_signed(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    report_path = fixture.manifest["profiles"][0]["qualification_report"]
    report = json.loads(fixture.path(report_path).read_text())
    report["physical_checks"].remove("clock_rollback")
    fixture.write(report_path, VERIFIER.canonical_json_bytes(report), "report")
    fixture.resign_commands_for_file(report_path)
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "omits a required physical check" in result.stderr


def test_reproducible_build_observation_must_bind_cargo_lock(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    build_report = fixture.manifest["reproducible_builds"][0]["report"]
    report = json.loads(fixture.path(build_report).read_text())
    command = next(
        row for row in fixture.commands if row["id"] == report["verification_id"]
    )
    command["arguments"] = [
        argument
        for argument in command["arguments"]
        if argument.get("file") != "source/Cargo.lock"
    ]
    fixture.resign_command(command["id"])
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "typed report inputs" in result.stderr


def test_signed_transcript_changes_authority_receipt_projection(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    before = _verify_direct(fixture)["receipt_projection"]["evidence_closure"]
    command = fixture.commands[0]
    fixture.write(command["stdout"], b"verified with another transcript\n", "transcript")
    fixture.resign_command(command["id"])
    fixture.refresh_files()
    after = _verify_direct(fixture)["receipt_projection"]["evidence_closure"]
    assert before["verification_records_digest"] != after["verification_records_digest"]
    assert before["total_transcript_bytes"] != after["total_transcript_bytes"]
    assert after["evidence_manifest"]["sha256"] == _sha256(
        fixture.manifest_path.read_bytes()
    )
    assert after["observer_policy"]["sha256"] == fixture.observer_policy_sha256


def test_aggregate_resource_caps_fail_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path / "transcript")
    monkeypatch.setattr(VERIFIER, "MAX_TOTAL_TRANSCRIPT_BYTES", 1)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="transcripts exceed"):
        _verify_direct(fixture)

    fixture = _fixture(tmp_path / "files")
    monkeypatch.setattr(VERIFIER, "MAX_TOTAL_TRANSCRIPT_BYTES", 64 * 1024 * 1024)
    monkeypatch.setattr(VERIFIER, "MAX_TOTAL_EVIDENCE_BYTES", 1)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="aggregate byte cap"):
        _verify_direct(fixture)

    fixture = _fixture(tmp_path / "commands")
    monkeypatch.setattr(VERIFIER, "MAX_TOTAL_EVIDENCE_BYTES", 6 * 1024 * 1024 * 1024)
    monkeypatch.setattr(VERIFIER, "MAX_COMMANDS", 1)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="commands"):
        _verify_direct(fixture)


def test_retained_payload_cap_is_checked_before_file_reads(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    monkeypatch.setattr(VERIFIER, "MAX_RETAINED_PAYLOAD_BYTES", 1)

    def unexpected_read(*_args: object, **_kwargs: object) -> object:
        raise AssertionError("retained evidence was read before its aggregate cap check")

    monkeypatch.setattr(VERIFIER, "stable_read_relative", unexpected_read)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="retained evidence payloads"):
        _verify_direct(fixture)


def test_evidence_tree_entry_directory_depth_and_empty_caps_fail_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    monkeypatch.setattr(VERIFIER, "MAX_EVIDENCE_TREE_ENTRIES", 1)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="directory-entry cap"):
        _verify_direct(fixture)

    monkeypatch.setattr(VERIFIER, "MAX_EVIDENCE_TREE_ENTRIES", 70_000)
    monkeypatch.setattr(VERIFIER, "MAX_EVIDENCE_DIRECTORIES", 1)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="directory cap"):
        _verify_direct(fixture)

    monkeypatch.setattr(VERIFIER, "MAX_EVIDENCE_DIRECTORIES", 4_096)
    monkeypatch.setattr(VERIFIER, "MAX_EVIDENCE_TREE_DEPTH", 1)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="directory depth"):
        _verify_direct(fixture)

    monkeypatch.setattr(VERIFIER, "MAX_EVIDENCE_TREE_DEPTH", 32)
    (fixture.root / "empty-attacker-fanout").mkdir()
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="empty directory"):
        _verify_direct(fixture)


def test_ed25519_decoder_rejects_noncanonical_and_noncurve_encodings() -> None:
    negative_zero = ((1 << 255) | 1).to_bytes(32, "little")
    noncurve_y = (2).to_bytes(32, "little")
    assert VERIFIER._ed_decode(negative_zero) is None
    assert VERIFIER._ed_decode(noncurve_y) is None


def test_per_observation_and_global_time_caps_fail_closed(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    command = fixture.commands[0]
    observation = json.loads(fixture.path(command["observation"]).read_text())
    observation["subject"]["duration_ms"] = VERIFIER.MAX_OBSERVATION_DURATION_MS + 1
    fixture.write(
        command["observation"],
        VERIFIER.canonical_json_bytes(observation),
        "observation",
    )
    fixture.resign_command(command["id"])
    fixture.refresh_files()
    result = _run(fixture)
    assert result.returncode == 1
    assert "duration exceeds" in result.stderr
