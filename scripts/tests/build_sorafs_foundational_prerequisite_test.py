"""Tests for the two-phase SoraFS foundational prerequisite builder."""

from __future__ import annotations

import dataclasses
import hashlib
import importlib.util
import json
import os
import shlex
import stat
import sys
import time
from pathlib import Path

import pytest


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "build_sorafs_foundational_prerequisite.py"
)
SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_foundational_prerequisite",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

import check_sorafs_production_readiness as CHECKER  # noqa: E402
import sorafs_production_readiness_contract as CONTRACT  # noqa: E402
import sorafs_software_signer_receipt as RECEIPT  # noqa: E402
import sccp_release_common as RELEASE_CRYPTO  # noqa: E402
import sorafs_l1_lane_evidence_inventory as LANE_INVENTORY  # noqa: E402
from sorafs_l1_lane_inventory_test_support import (  # noqa: E402
    inventory_cli_args,
    write_signed_inventory,
)
from sorafs_resilience_test_support import (  # noqa: E402
    DEFAULT_SIGNING_SEED as RESILIENCE_SIGNING_SEED,
    public_key_from_seed as resilience_public_key_from_seed,
    render_summary as render_resilience_summary,
    resilience_summary as build_resilience_summary,
    write_resilience_summary,
)
from check_sorafs_production_readiness_test import (  # noqa: E402
    gate_summary as complete_gate_summary,
)
from sorafs_rollout_runner_test_support import signed_topology_cli_args  # noqa: E402


NOW_UNIX = 1_800_900_000
GENERATED_AT_UNIX = NOW_UNIX - 30
EVIDENCE_AT_UNIX = NOW_UNIX - 60
MAX_AGE_SECS = 3600
DEPLOYMENT_ID = "sorafs-mainnet-2026-07"
ENVIRONMENT = "production"
RELEASE_SEQUENCE = 1
PREDECESSOR_SHA256 = "00" * 32
SIGNER_SERVICE_ID = "sorafs-promotion-signer-a"
SIGNER_ADMINISTRATOR_ID = "sorafs-promotion-admin-b"
SIGNER_KEY_REVISION = 7
SIGNER_POLICY_REVISION = 11
SIGNER_POLICY_DIGEST_SHA256 = hashlib.sha256(b"reviewed promotion policy").hexdigest()
SIGNER_OPERATION_ID = hashlib.sha256(b"promotion operation one").hexdigest()
RESILIENCE_SIGNER_PUBLIC_KEY = resilience_public_key_from_seed(
    RESILIENCE_SIGNING_SEED
)


def topology_qualification_path(
    tmp_path: Path,
    *,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
) -> Path:
    """Write one exact non-promotable four-validator qualification summary."""

    path = tmp_path / "l1-topology-qualification.json"
    payload = {
        "schema": "sorafs.l1.deployment_qualification.summary.v1",
        "status": "configuration-qualified",
        "qualification_scope": "pre-deployment-configuration",
        "live_evidence_recognized": False,
        "promotion_eligible": False,
        "manifest_sha256": hashlib.sha256(b"exact-manifest").hexdigest(),
        "canonical_manifest_sha256": hashlib.sha256(
            b"canonical-manifest"
        ).hexdigest(),
        "deployment": {
            "deployment_id": deployment_id,
            "environment": environment,
            "network": "taira",
            "chain_id": "fc56984b-2be7-431d-840e-21514d1883f0",
            "chain_discriminant": 369,
        },
        "validator_count": 4, "validator_ids": ["taira-validator-1", "taira-validator-2", "taira-validator-3", "taira-validator-4"],
        "storage_provider_count": 2,
        "gateway_count": 2,
        "governance_dag_instance_count": 2,
        "runtime_handle_kinds": [
            "monitoring",
            "external_signer",
            "key_custody",
            "webauthn",
        ],
        "runtime_material_policy_valid": True,
        "signed_model_artifact_count": 1,
        "required_lane_slots": list(CHECKER.DEFAULT_REQUIRED_GATES),
        "recognized_lane_slot_count": 17,
        "errors": [],
    }
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    return path


def topology_binding(
    tmp_path: Path,
    *,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
) -> dict[str, str]:
    """Return the exact binding derived from the test qualification bytes."""

    path = topology_qualification_path(
        tmp_path,
        deployment_id=deployment_id,
        environment=environment,
    )
    return {
        "qualification_summary_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "manifest_sha256": hashlib.sha256(b"exact-manifest").hexdigest(),
        "canonical_manifest_sha256": hashlib.sha256(
            b"canonical-manifest"
        ).hexdigest(),
        "deployment_id": deployment_id,
        "environment": environment,
        "network": "taira", "chain_id": "fc56984b-2be7-431d-840e-21514d1883f0", "chain_discriminant": 369, "validator_ids_sha256": "15e4cadecf176094a1791a33ee75ce9315e1d953018affa5df2762fcba52d6f2",
    }


def topology_cli_args(
    tmp_path: Path,
    *,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    now_unix: int = NOW_UNIX,
) -> list[str]:
    """Write and return the independently signed topology trust tuple."""

    summary_path = topology_qualification_path(
        tmp_path,
        deployment_id=deployment_id,
        environment=environment,
    )
    return signed_topology_cli_args(
        summary_path,
        deployment_id=deployment_id,
        environment=environment,
        now_unix=now_unix,
    )


def resilience_qualification_path(
    tmp_path: Path,
    *,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    generated_at_unix: int = GENERATED_AT_UNIX,
) -> Path:
    """Write one trusted resilience summary bound to the test topology."""

    path, public_key, _binding = write_resilience_summary(
        CHECKER,
        tmp_path / "l1-resilience-qualification.summary",
        deployment_id=deployment_id,
        environment=environment,
        topology_qualification=topology_binding(
            tmp_path,
            deployment_id=deployment_id,
            environment=environment,
        ),
        generated_at_unix=generated_at_unix,
        captured_at_unix=generated_at_unix - 1,
    )
    assert public_key == RESILIENCE_SIGNER_PUBLIC_KEY
    return path


def public_key_from_seed(seed: bytes) -> bytes:
    """Derive a temporary Ed25519 public key for one test invocation."""

    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    return RELEASE_CRYPTO._ed_encode(  # noqa: SLF001 - test-only signer
        RELEASE_CRYPTO._ed_scalar_multiply(  # noqa: SLF001
            RELEASE_CRYPTO._ED_BASE,  # noqa: SLF001
            scalar,
        )
    )


def sign(seed: bytes, message: bytes) -> bytes:
    """Sign with a temporary in-memory Ed25519 seed used only by tests."""

    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    prefix = digest[32:]
    public_key = public_key_from_seed(seed)
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little")
    nonce %= RELEASE_CRYPTO._ED_L  # noqa: SLF001
    encoded_r = RELEASE_CRYPTO._ed_encode(  # noqa: SLF001
        RELEASE_CRYPTO._ed_scalar_multiply(  # noqa: SLF001
            RELEASE_CRYPTO._ED_BASE,  # noqa: SLF001
            nonce,
        )
    )
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + message).digest(),
        "little",
    ) % RELEASE_CRYPTO._ED_L  # noqa: SLF001
    scalar_bytes = (
        (nonce + challenge * scalar) % RELEASE_CRYPTO._ED_L  # noqa: SLF001
    ).to_bytes(32, "little")
    return encoded_r + scalar_bytes


def signer_receipt_args(
    tmp_path: Path,
    *,
    payload_name: str,
    signature_name: str,
) -> list[str]:
    """Create a pinned test verifier plus exact public binding and receipt."""

    verifier = tmp_path / "test-external-software-signer"
    settings = {
        "service_id": SIGNER_SERVICE_ID,
        "administrator_id": SIGNER_ADMINISTRATOR_ID,
        "key_revision": SIGNER_KEY_REVISION,
        "policy_revision": SIGNER_POLICY_REVISION,
        "policy_digest_sha256": SIGNER_POLICY_DIGEST_SHA256,
    }
    if not verifier.exists():
        verifier.write_text(
            f"""#!/usr/bin/env python3
import argparse
import hashlib
import json
import sys
from pathlib import Path

SETTINGS = {settings!r}
if len(sys.argv) < 2 or sys.argv[1] != "verify-receipt":
    raise SystemExit(2)
parser = argparse.ArgumentParser()
for flag in ("binding", "payload", "signature", "receipt", "expected-operation-id", "validation-out"):
    parser.add_argument("--" + flag, required=True)
args = parser.parse_args(sys.argv[2:])
binding = Path(args.binding).read_bytes()
payload = Path(args.payload).read_bytes()
signature = Path(args.signature).read_bytes()
receipt_raw = Path(args.receipt).read_bytes()
receipt = json.loads(receipt_raw)
expected_receipt = dict(
    schema="test.external_software_signer.signature_receipt.v1",
    operation_id_hex=args.expected_operation_id,
    binding_sha256=hashlib.sha256(binding).hexdigest(),
    payload_sha256=hashlib.sha256(payload).hexdigest(),
    signature_sha256=hashlib.sha256(signature).hexdigest(),
)
if receipt != expected_receipt or receipt_raw != json.dumps(receipt, sort_keys=True, separators=(",", ":")).encode("ascii"):
    raise SystemExit(1)
validation = dict(
    schema="sorafs.external_software_signer.signature_receipt_validation.v1",
    status="valid",
    operation_id_hex=args.expected_operation_id,
    payload_digest_blake3_hex="11" * 32,
    payload_length=len(payload),
    signature_digest_blake3_hex="22" * 32,
    binding_digest_blake3_hex="33" * 32,
    backend="software",
    service_id=SETTINGS["service_id"],
    administrator_id=SETTINGS["administrator_id"],
    role="promotion",
    domain="sorafs.production-readiness.foundational-prerequisites.v1",
    signature_algorithm="ed25519",
    key_revision=SETTINGS["key_revision"],
    policy_revision=SETTINGS["policy_revision"],
    policy_digest_sha256=SETTINGS["policy_digest_sha256"],
    public_key_digest_blake3_hex="44" * 32,
    commit_sequence=7,
    commit_audit_head_blake3_hex="55" * 32,
    audit_sequence=7,
    audit_head_blake3_hex="55" * 32,
    replayed=False,
    revoked=False,
    payload_signature_valid=True,
    provenance_attestation_valid=True,
    response_attestation_valid=True,
)
Path(args.validation_out).write_bytes(json.dumps(validation, sort_keys=True, separators=(",", ":")).encode("ascii"))
""",
            encoding="utf-8",
        )
        verifier.chmod(0o500)
    binding = tmp_path / "promotion.binding.norito"
    if not binding.exists():
        binding.write_bytes(b"test-canonical-norito-promotion-binding-v1")
    payload = tmp_path / payload_name
    signature = tmp_path / signature_name
    receipt = tmp_path / f"{payload_name}.signature-receipt.json"
    receipt.write_bytes(
        json.dumps(
            {
                "schema": "test.external_software_signer.signature_receipt.v1",
                "operation_id_hex": SIGNER_OPERATION_ID,
                "binding_sha256": hashlib.sha256(binding.read_bytes()).hexdigest(),
                "payload_sha256": hashlib.sha256(
                    payload.read_bytes() if payload.exists() else b""
                ).hexdigest(),
                "signature_sha256": hashlib.sha256(
                    signature.read_bytes() if signature.exists() else b""
                ).hexdigest(),
            },
            sort_keys=True,
            separators=(",", ":"),
        ).encode("ascii")
    )
    return [
        "--signer-binding",
        str(binding),
        "--signer-receipt",
        str(receipt),
        "--signer-verifier",
        str(verifier),
        "--expected-signer-verifier-sha256",
        hashlib.sha256(verifier.read_bytes()).hexdigest(),
        "--expected-signer-operation-id",
        SIGNER_OPERATION_ID,
    ]


@pytest.fixture
def signer() -> tuple[bytes, bytes]:
    """Return temporary signing material that is never written to disk."""

    seed = os.urandom(32)
    return seed, public_key_from_seed(seed)


def readiness_summary_paths(
    tmp_path: Path,
    *,
    evidence_at_unix: int = EVIDENCE_AT_UNIX,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
) -> dict[str, Path]:
    """Write one authoritative ready summary for every canonical lane."""

    paths: dict[str, Path] = {}
    for gate_name in CHECKER.DEFAULT_REQUIRED_GATES:
        summary_path = tmp_path / (
            "authoritative-" + gate_name.replace("_", "-") + "-summary.json"
        )
        if not summary_path.exists():
            payload = (
                gateway_load_summary(
                    tmp_path,
                    deployment_id=deployment_id,
                    environment=environment,
                    generated_at_unix=evidence_at_unix,
                )
                if gate_name == "gateway_load"
                else complete_gate_summary(
                    gate_name,
                    generated_at_unix=evidence_at_unix,
                    deployment_id=deployment_id,
                    environment=environment,
                )
            )
            payload["topology_qualification"] = topology_binding(
                tmp_path,
                deployment_id=deployment_id,
                environment=environment,
            )
            summary_path.write_bytes(LANE_INVENTORY.canonical_file_bytes(payload))
        paths[gate_name] = summary_path
    return paths


def prerequisite_specs(
    tmp_path: Path,
    *,
    evidence_at_unix: int = EVIDENCE_AT_UNIX,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
) -> list[str]:
    """Write exact mapped prerequisite manifests and return ID=PATH inputs."""

    summary_paths = readiness_summary_paths(
        tmp_path,
        evidence_at_unix=evidence_at_unix,
        deployment_id=deployment_id,
        environment=environment,
    )
    summary_digests = {
        gate_name: hashlib.sha256(summary_path.read_bytes()).hexdigest()
        for gate_name, summary_path in summary_paths.items()
    }
    values: list[str] = []
    for prerequisite_id in MODULE.FOUNDATIONAL_PREREQUISITE_IDS:
        package_path = tmp_path / (
            "prerequisite-" + prerequisite_id.lower().replace("-", "_") + ".json"
        )
        if not package_path.exists():
            package = {
                "schema": MODULE.FOUNDATIONAL_PREREQUISITE_EVIDENCE_PACKAGE_SCHEMA,
                "prerequisite_id": prerequisite_id,
                "status": "verified",
                "deployment": {
                    "deployment_id": deployment_id,
                    "environment": environment,
                },
                "evidence_generated_at_unix": evidence_at_unix,
                "topology_qualification": topology_binding(
                    tmp_path,
                    deployment_id=deployment_id,
                    environment=environment,
                ),
                "readiness_summaries": [
                    {
                        "gate": gate_name,
                        "path": summary_paths[gate_name].name,
                        "sha256": summary_digests[gate_name],
                    }
                    for gate_name in MODULE.FOUNDATIONAL_PREREQUISITE_LANES[
                        prerequisite_id
                    ]
                ],
                "errors": [],
            }
            package_path.write_text(
                json.dumps(package, sort_keys=True),
                encoding="utf-8",
            )
        values.append(f"{prerequisite_id}={package_path}")
    lane_inventory_args(
        tmp_path,
        deployment_id=deployment_id,
        environment=environment,
    )
    return values


def legacy_gateway_load_prerequisite_specs(
    tmp_path: Path,
    *,
    evidence_at_unix: int = EVIDENCE_AT_UNIX,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
) -> list[str]:
    """Write the retired nine-times-gateway-load package set."""

    summary_path = readiness_summary_paths(
        tmp_path,
        evidence_at_unix=evidence_at_unix,
        deployment_id=deployment_id,
        environment=environment,
    )["gateway_load"]
    summary_sha256 = hashlib.sha256(summary_path.read_bytes()).hexdigest()
    values: list[str] = []
    for prerequisite_id in MODULE.FOUNDATIONAL_PREREQUISITE_IDS:
        package_path = tmp_path / (
            "legacy-prerequisite-"
            + prerequisite_id.lower().replace("-", "_")
            + ".json"
        )
        package_path.write_text(
            json.dumps(
                {
                    "schema": (
                        MODULE.FOUNDATIONAL_PREREQUISITE_EVIDENCE_PACKAGE_SCHEMA
                    ),
                    "prerequisite_id": prerequisite_id,
                    "status": "verified",
                    "deployment": {
                        "deployment_id": deployment_id,
                        "environment": environment,
                    },
                    "evidence_generated_at_unix": evidence_at_unix,
                    "topology_qualification": topology_binding(
                        tmp_path,
                        deployment_id=deployment_id,
                        environment=environment,
                    ),
                    "readiness_summaries": [
                        {
                            "gate": "gateway_load",
                            "path": summary_path.name,
                            "sha256": summary_sha256,
                        }
                    ],
                    "errors": [],
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )
        values.append(f"{prerequisite_id}={package_path}")
    return values


def lane_summary_paths(
    tmp_path: Path,
    *,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
) -> list[tuple[str, Path]]:
    """Create exact temporary lane-summary bytes for the signing fixture."""

    paths = readiness_summary_paths(
        tmp_path,
        deployment_id=deployment_id,
        environment=environment,
    )
    return [(gate_name, paths[gate_name]) for gate_name in CHECKER.DEFAULT_REQUIRED_GATES]


def lane_inventory_args(
    tmp_path: Path,
    *,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    now_unix: int = NOW_UNIX,
) -> list[str]:
    """Create and trust one independently signed exact-17 inventory."""

    path = tmp_path / "l1-lane-evidence.inventory"
    if path.exists():
        return inventory_cli_args(path)
    inventory_root = tmp_path
    inventory_deployment_id = deployment_id
    inventory_environment = environment
    if deployment_id != DEPLOYMENT_ID or environment != ENVIRONMENT:
        inventory_root = tmp_path / "inventory-baseline"
        inventory_root.mkdir(exist_ok=True)
        inventory_deployment_id = DEPLOYMENT_ID
        inventory_environment = ENVIRONMENT
    write_signed_inventory(
        path,
        lane_summary_paths(
            inventory_root,
            deployment_id=inventory_deployment_id,
            environment=inventory_environment,
        ),
        topology_binding(
            inventory_root,
            deployment_id=inventory_deployment_id,
            environment=inventory_environment,
        ),
        deployment_id=inventory_deployment_id,
        environment=inventory_environment,
        now_unix=now_unix,
    )
    return inventory_cli_args(path)


def prepare_args(
    tmp_path: Path,
    public_key: bytes,
    *,
    output_name: str = "foundational-signing-payload.bin",
    specs: list[str] | None = None,
    generated_at_unix: int = GENERATED_AT_UNIX,
    now_unix: int = NOW_UNIX,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    release_sequence: int = RELEASE_SEQUENCE,
    predecessor_sha256: str = PREDECESSOR_SHA256,
    previous_envelope_name: str | None = None,
) -> list[str]:
    """Build one complete prepare command."""

    values = [
        "prepare",
        *topology_cli_args(
            tmp_path,
            deployment_id=deployment_id,
            environment=environment,
            now_unix=now_unix,
        ),
        *lane_inventory_args(
            tmp_path,
            deployment_id=deployment_id,
            environment=environment,
            now_unix=now_unix,
        ),
        "--resilience-qualification-summary",
        str(
            resilience_qualification_path(
                tmp_path,
                deployment_id=deployment_id,
                environment=environment,
            )
        ),
        "--resilience-qualification-signer-public-key-hex",
        RESILIENCE_SIGNER_PUBLIC_KEY.hex(),
        "--deployment-id",
        deployment_id,
        "--environment",
        environment,
        "--generated-at-unix",
        str(generated_at_unix),
        "--now-unix",
        str(now_unix),
        "--max-evidence-age-secs",
        str(MAX_AGE_SECS),
        "--release-sequence",
        str(release_sequence),
        "--previous-envelope-sha256",
        predecessor_sha256,
        "--trusted-public-key-hex",
        public_key.hex(),
        "--signer-service-id",
        SIGNER_SERVICE_ID,
        "--signer-administrator-id",
        SIGNER_ADMINISTRATOR_ID,
        "--signer-key-revision",
        str(SIGNER_KEY_REVISION),
        "--signer-policy-revision",
        str(SIGNER_POLICY_REVISION),
        "--signer-policy-digest-sha256",
        SIGNER_POLICY_DIGEST_SHA256,
        "--signing-payload-out",
        str(tmp_path / output_name),
    ]
    if previous_envelope_name is not None:
        values.extend(
            ["--previous-envelope", str(tmp_path / previous_envelope_name)]
        )
    selected_specs = (
        prerequisite_specs(
            tmp_path,
            evidence_at_unix=min(EVIDENCE_AT_UNIX, generated_at_unix),
            deployment_id=deployment_id,
            environment=environment,
        )
        if specs is None
        else specs
    )
    for spec in selected_specs:
        values.extend(["--prerequisite", spec])
    for gate_name, path in lane_summary_paths(
        tmp_path,
        deployment_id=deployment_id,
        environment=environment,
    ):
        values.extend(["--lane-summary", f"{gate_name}={path}"])
    return values


def finalize_args(
    tmp_path: Path,
    public_key: bytes,
    *,
    payload_name: str = "foundational-signing-payload.bin",
    signature_name: str = "foundational-signature.bin",
    output_name: str = "foundational-prerequisites.json",
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    release_sequence: int = RELEASE_SEQUENCE,
    predecessor_sha256: str = PREDECESSOR_SHA256,
    now_unix: int = NOW_UNIX,
    previous_envelope_name: str | None = None,
) -> list[str]:
    """Build one complete finalize command."""

    values = [
        "finalize",
        *topology_cli_args(
            tmp_path,
            deployment_id=deployment_id,
            environment=environment,
            now_unix=now_unix,
        ),
        *lane_inventory_args(
            tmp_path,
            deployment_id=deployment_id,
            environment=environment,
            now_unix=now_unix,
        ),
        "--resilience-qualification-summary",
        str(
            resilience_qualification_path(
                tmp_path,
                deployment_id=deployment_id,
                environment=environment,
            )
        ),
        "--resilience-qualification-signer-public-key-hex",
        RESILIENCE_SIGNER_PUBLIC_KEY.hex(),
        "--signing-payload",
        str(tmp_path / payload_name),
        "--signature-file",
        str(tmp_path / signature_name),
        "--trusted-public-key-hex",
        public_key.hex(),
        "--expected-signer-service-id",
        SIGNER_SERVICE_ID,
        "--expected-signer-administrator-id",
        SIGNER_ADMINISTRATOR_ID,
        "--expected-signer-key-revision",
        str(SIGNER_KEY_REVISION),
        "--expected-signer-policy-revision",
        str(SIGNER_POLICY_REVISION),
        "--expected-signer-policy-digest-sha256",
        SIGNER_POLICY_DIGEST_SHA256,
        "--expected-deployment-id",
        deployment_id,
        "--expected-environment",
        environment,
        "--expected-release-sequence",
        str(release_sequence),
        "--expected-previous-envelope-sha256",
        predecessor_sha256,
        "--now-unix",
        str(now_unix),
        "--max-evidence-age-secs",
        str(MAX_AGE_SECS),
        "--envelope-out",
        str(tmp_path / output_name),
    ]
    values.extend(
        signer_receipt_args(
            tmp_path,
            payload_name=payload_name,
            signature_name=signature_name,
        )
    )
    if previous_envelope_name is not None:
        values.extend(
            ["--previous-envelope", str(tmp_path / previous_envelope_name)]
        )
    for gate_name, path in lane_summary_paths(
        tmp_path,
        deployment_id=deployment_id,
        environment=environment,
    ):
        values.extend(["--lane-summary", f"{gate_name}={path}"])
    return values


def prepare_and_sign(
    tmp_path: Path,
    seed: bytes,
    public_key: bytes,
) -> bytes:
    """Prepare one request and write its temporary raw detached signature."""

    assert MODULE.main(prepare_args(tmp_path, public_key)) == 0
    payload = (tmp_path / "foundational-signing-payload.bin").read_bytes()
    (tmp_path / "foundational-signature.bin").write_bytes(sign(seed, payload))
    return payload


def decode_unsigned(payload: bytes) -> dict:
    """Decode the unsigned canonical body from one signing payload."""

    assert payload.startswith(MODULE.FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN)
    return json.loads(
        payload[len(MODULE.FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN) :]
    )


def write_signing_payload(path: Path, unsigned: dict) -> bytes:
    """Write one canonical binary signing payload used by negative tests."""

    payload = MODULE.foundational_signing_payload(unsigned)
    path.write_bytes(payload)
    return payload


def gateway_load_summary(
    tmp_path: Path,
    *,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    generated_at_unix: int = GENERATED_AT_UNIX,
) -> dict:
    """Build one temporary ready lane summary for direct aggregate acceptance."""

    gate = CHECKER.GATE_BY_NAME["gateway_load"]
    required_rows: dict[str, dict] = {}
    for kind_name in gate.required_kinds:
        kind_schema = CHECKER.GATE_REQUIRED_KIND_SCHEMAS["gateway_load"][kind_name]
        fingerprint = {
            "generated_at_unix": generated_at_unix,
            "deployment_id": deployment_id,
            "environment": environment,
            "deployment_context_reviewed": True,
            "metric_count": len(CHECKER.GATEWAY_LOAD_REQUIRED_METRICS),
            "metrics": list(CHECKER.GATEWAY_LOAD_REQUIRED_METRICS),
            "policy_digest_hex": "ab" * 32,
            "staging_report_digest_hex": "bc" * 32,
            "suite_report_digest_hex": "cd" * 32,
        }
        if kind_name == "staging_load":
            streams = [
                {"name": f"gateway-load-stream-{index:04d}"}
                for index in range(1_200)
            ]
            providers = [
                {"name": "gateway-load-provider-a"},
                {"name": "gateway-load-provider-b"},
                {"name": "gateway-load-provider-c"},
                {"name": "gateway-load-provider-d"},
            ]
            fingerprint.update(
                {
                    "schema": kind_schema,
                    "fixture_bundle_digest_hex": "ef" * 32,
                    "gateway_version": "iroha-gateway 1.0.0-rc.1",
                    "hardware_profile": {
                        "name": "gateway-load-hardware-c6i-2xlarge"
                    },
                    "cache_coverage": {
                        "cold_cache_exercised": True,
                        "warm_cache_exercised": True,
                        "mixed_cache_exercised": True,
                    },
                    "duration_seconds": 86_400,
                    "stream_count": len(streams),
                    "streams": streams,
                    "peak_concurrent_range_streams": 1_000,
                    "provider_count": len(providers),
                    "providers": providers,
                    "load_conditions": {
                        "corruption_injection_bps": 100,
                        "revocation_exercised": True,
                        "malformed_flood_exercised": True,
                        "denylist_pressure_exercised": True,
                        "rate_limit_pressure_exercised": True,
                        "failover_exercised": True,
                    },
                    "success_rate_bps": 9_950,
                    "error_rate_bps": 50,
                    "p95_latency_ms": 1_200,
                    "p99_latency_ms": 2_200,
                }
            )
        required_rows[kind_name] = {
            "schema": kind_schema,
            "present": True,
            "valid": True,
            "artifact_count": 1,
            "artifacts": [
                {
                    "path": f"artifacts/gateway_load/{kind_name}.json",
                    "sha256": "de" * 32,
                    "schema": kind_schema,
                    "status": "passed",
                    "fingerprint": fingerprint,
                    "valid": True,
                    "errors": [],
                }
            ],
            "errors": [],
        }
    recognized_artifacts = []
    for kind_name, row in required_rows.items():
        artifact = dict(row["artifacts"][0])
        artifact["kind"] = kind_name
        recognized_artifacts.append(artifact)
    return {
        "schema": gate.schema,
        "status": "ready",
        "topology_qualification": topology_binding(
            tmp_path,
            deployment_id=deployment_id,
            environment=environment,
        ),
        "required_kinds": list(gate.required_kinds),
        "thresholds": {"max_evidence_bytes": 2_097_152},
        "evidence_file_count": len(gate.required_kinds),
        "recognized_artifact_count": len(gate.required_kinds),
        "recognized_artifacts": recognized_artifacts,
        "required": required_rows,
        "metric_count_values": [len(CHECKER.GATEWAY_LOAD_REQUIRED_METRICS)],
        "metrics": sorted(CHECKER.GATEWAY_LOAD_REQUIRED_METRICS),
        "valid_policy_digests": ["ab" * 32],
        "valid_staging_report_digests": ["bc" * 32],
        "valid_suite_report_digests": ["cd" * 32],
        "errors": [],
    }


def test_prepare_and_finalize_external_signer_roundtrip(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
) -> None:
    """Bind one external software signer and pass the aggregate contract."""

    seed, public_key = signer
    signing_payload = prepare_and_sign(tmp_path, seed, public_key)
    assert stat.S_IMODE(
        (tmp_path / "foundational-signing-payload.bin").stat().st_mode
    ) == 0o600

    unsigned = decode_unsigned(signing_payload)
    assert set(unsigned) == MODULE.UNSIGNED_FOUNDATIONAL_FIELDS
    assert set(unsigned["signature"]) == MODULE.UNSIGNED_SIGNATURE_FIELDS
    assert "signature_hex" not in unsigned["signature"]
    assert unsigned["signature"] == {
        "administrator_id": SIGNER_ADMINISTRATOR_ID,
        "algorithm": "ed25519",
        "backend": "software",
        "key_revision": SIGNER_KEY_REVISION,
        "policy_digest_sha256": SIGNER_POLICY_DIGEST_SHA256,
        "policy_revision": SIGNER_POLICY_REVISION,
        "public_key_fingerprint_sha256": hashlib.sha256(public_key).hexdigest(),
        "service_id": SIGNER_SERVICE_ID,
    }
    assert [row["id"] for row in unsigned["prerequisites"]] == list(
        MODULE.FOUNDATIONAL_PREREQUISITE_IDS
    )
    assert all(
        set(row) == MODULE.FOUNDATIONAL_PREREQUISITE_ROW_FIELDS
        for row in unsigned["prerequisites"]
    )
    expected_prerequisite_hashes = [
        hashlib.sha256(Path(spec.partition("=")[2]).read_bytes()).hexdigest()
        for spec in prerequisite_specs(tmp_path)
    ]
    assert [
        row["evidence_anchor_sha256"] for row in unsigned["prerequisites"]
    ] == expected_prerequisite_hashes
    assert {
        row["id"]: tuple(
            summary["gate"] for summary in row["readiness_summary_sha256"]
        )
        for row in unsigned["prerequisites"]
    } == {
        "SFM-1": ("reputation",),
        "SF-1": ("reference_sdk_release",),
        "SF-2": ("pdp",),
        "SF-2c": ("por", "potr"),
        "SF-3": ("gateway_compliance",),
        "SF-4": ("repair",),
        "SF-5b": ("gateway_load",),
        "SF-6": (
            "appeal_finance",
            "governance_dag",
            "hedging_billing",
            "orderbook",
            "reserve_rent",
        ),
        "SF-8a": (
            "ai_prescreen",
            "moderation_panel",
            "pop_credentials",
            "transparency",
        ),
    } == MODULE.FOUNDATIONAL_PREREQUISITE_LANES
    assert {
        summary["gate"]
        for row in unsigned["prerequisites"]
        for summary in row["readiness_summary_sha256"]
    } == set(CHECKER.DEFAULT_REQUIRED_GATES)
    assert [row["gate"] for row in unsigned["lane_summaries"]] == list(
        CHECKER.DEFAULT_REQUIRED_GATES
    )
    assert (
        set(unsigned["resilience_qualification"])
        == CHECKER.RESILIENCE_QUALIFICATION_BINDING_FIELDS
    )
    assert signing_payload == MODULE.foundational_signing_payload(unsigned)

    finalize_values = finalize_args(tmp_path, public_key)
    assert MODULE.main(finalize_values) == 0
    envelope_path = tmp_path / "foundational-prerequisites.json"
    envelope = json.loads(envelope_path.read_text(encoding="utf-8"))
    assert stat.S_IMODE(envelope_path.stat().st_mode) == 0o600
    assert envelope["signature"]["algorithm"] == "ed25519"
    assert bytes.fromhex(envelope["signature"]["signature_hex"]) == sign(
        seed,
        signing_payload,
    )
    assert MODULE.foundational_signing_payload(envelope) == signing_payload
    assert set(envelope["signer_receipt_bundle"]) == (
        CONTRACT.FOUNDATIONAL_SIGNER_RECEIPT_BUNDLE_FIELDS
    )

    verifier = Path(finalize_values[finalize_values.index("--signer-verifier") + 1])
    verifier_sha256 = finalize_values[
        finalize_values.index("--expected-signer-verifier-sha256") + 1
    ]

    _summary, errors, context = CHECKER.validate_foundational_prerequisite_summary(
        envelope,
        CHECKER.ValidationOptions(
            now_unix=NOW_UNIX,
            max_summary_artifact_age_secs=MAX_AGE_SECS,
            deployment_id=DEPLOYMENT_ID,
            environment=ENVIRONMENT,
            foundational_signer_public_key=public_key,
            foundational_release_sequence=RELEASE_SEQUENCE,
            foundational_previous_envelope_sha256=PREDECESSOR_SHA256,
            foundational_signer_verifier=verifier,
            foundational_signer_verifier_sha256=verifier_sha256,
        ),
    )
    assert errors == []
    assert context == (DEPLOYMENT_ID, ENVIRONMENT)


def test_aggregate_rejects_signed_envelope_without_external_signer_receipt(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
) -> None:
    """A raw signature and self-declared software metadata are insufficient."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    finalize_values = finalize_args(tmp_path, public_key)
    assert MODULE.main(finalize_values) == 0
    envelope = json.loads(
        (tmp_path / "foundational-prerequisites.json").read_text(encoding="utf-8")
    )
    envelope.pop("signer_receipt_bundle")
    verifier = Path(finalize_values[finalize_values.index("--signer-verifier") + 1])
    options = CHECKER.ValidationOptions(
        now_unix=NOW_UNIX,
        max_summary_artifact_age_secs=MAX_AGE_SECS,
        deployment_id=DEPLOYMENT_ID,
        environment=ENVIRONMENT,
        foundational_signer_public_key=public_key,
        foundational_release_sequence=RELEASE_SEQUENCE,
        foundational_previous_envelope_sha256=PREDECESSOR_SHA256,
        foundational_signer_verifier=verifier,
        foundational_signer_verifier_sha256=hashlib.sha256(verifier.read_bytes()).hexdigest(),
    )
    _summary, errors, _context = CHECKER.validate_foundational_prerequisite_summary(
        envelope, options
    )
    assert "foundational prerequisite requires a signer receipt bundle" in errors


def test_aggregate_replays_receipt_and_independent_verifier_trust(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
) -> None:
    """Post-sign receipt bytes remain replayed against a separately pinned tool."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    finalize_values = finalize_args(tmp_path, public_key)
    assert MODULE.main(finalize_values) == 0
    envelope = json.loads(
        (tmp_path / "foundational-prerequisites.json").read_text(encoding="utf-8")
    )
    verifier = Path(finalize_values[finalize_values.index("--signer-verifier") + 1])
    options = CHECKER.ValidationOptions(
        now_unix=NOW_UNIX,
        max_summary_artifact_age_secs=MAX_AGE_SECS,
        deployment_id=DEPLOYMENT_ID,
        environment=ENVIRONMENT,
        foundational_signer_public_key=public_key,
        foundational_release_sequence=RELEASE_SEQUENCE,
        foundational_previous_envelope_sha256=PREDECESSOR_SHA256,
        foundational_signer_verifier=verifier,
        foundational_signer_verifier_sha256=hashlib.sha256(verifier.read_bytes()).hexdigest(),
    )
    tampered = json.loads(json.dumps(envelope))
    encoded = tampered["signer_receipt_bundle"]["receipt_base64"]
    tampered["signer_receipt_bundle"]["receipt_base64"] = (
        ("A" if encoded[0] != "A" else "B") + encoded[1:]
    )
    assert MODULE.foundational_signing_payload(tampered) == (
        MODULE.foundational_signing_payload(envelope)
    )
    _summary, errors, _context = CHECKER.validate_foundational_prerequisite_summary(
        tampered, options
    )
    assert "external software signer receipt verification failed" in errors

    untrusted_options = dataclasses.replace(
        options, foundational_signer_verifier_sha256="ab" * 32
    )
    _summary, errors, _context = CHECKER.validate_foundational_prerequisite_summary(
        envelope, untrusted_options
    )
    assert any("independently reviewed digest" in error for error in errors)


def _run_test_receipt_verifier(verifier: bytes) -> tuple[bytes | None, list[str]]:
    """Run a synthetic verifier with inert exact-byte inputs."""

    return RECEIPT.run_offline_receipt_verifier(
        verifier=verifier,
        binding=b"binding",
        payload=b"payload",
        signature=b"signature",
        receipt=b"receipt",
        operation_id_hex="11" * 32,
    )


@pytest.mark.parametrize("descriptor", [1, 2])
def test_receipt_verifier_rejects_bounded_payload_free_diagnostics(
    monkeypatch: pytest.MonkeyPatch,
    descriptor: int,
) -> None:
    """Noisy stdout/stderr is bounded, killed, and never reflected in errors."""

    secret = b"verifier-controlled-sensitive-diagnostic"
    monkeypatch.setattr(RECEIPT, "MAX_RECEIPT_VERIFIER_DIAGNOSTIC_BYTES", 64)
    monkeypatch.setattr(RECEIPT, "RECEIPT_VERIFIER_TIMEOUT_SECS", 0.5)
    validation, errors = _run_test_receipt_verifier(
        b"#!/usr/bin/env python3\n"
        b"import os\n"
        + f"os.write({descriptor}, {secret!r} * 4096)\n".encode("ascii")
    )
    assert validation is None
    assert errors == ["external software signer receipt verification failed"]
    assert secret.decode("ascii") not in " ".join(errors)


def test_receipt_verifier_hard_timeout_reaps_process(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A silent verifier cannot outlive the hard replay deadline."""

    monkeypatch.setattr(RECEIPT, "RECEIPT_VERIFIER_TIMEOUT_SECS", 0.15)
    started = time.monotonic()
    validation, errors = _run_test_receipt_verifier(
        b"#!/usr/bin/env python3\nimport time\ntime.sleep(60)\n"
    )
    assert time.monotonic() - started < 2
    assert validation is None
    assert errors == ["external software signer receipt verifier could not run"]


def test_receipt_verifier_timeout_kills_inherited_pipe_descendant(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A descendant holding diagnostics open is killed with its process group."""

    process_ids = tmp_path / "verifier-process-ids"
    monkeypatch.setattr(RECEIPT, "RECEIPT_VERIFIER_TIMEOUT_SECS", 1.0)
    validation, errors = _run_test_receipt_verifier(
        (
            "#!/usr/bin/env python3\n"
            "import os\n"
            "import subprocess\n"
            "import sys\n"
            "from pathlib import Path\n"
            "child = subprocess.Popen([sys.executable, '-c', "
            "'import time; time.sleep(60)'])\n"
            f"Path({str(process_ids)!r}).write_text("
            "f'{os.getpgrp()} {child.pid}', encoding='ascii')\n"
        ).encode("ascii")
    )
    assert validation is None
    assert errors == ["external software signer receipt verifier could not run"]
    process_group, child_pid = map(
        int, process_ids.read_text(encoding="ascii").split()
    )
    deadline = time.monotonic() + 1
    while time.monotonic() < deadline:
        try:
            os.killpg(process_group, 0)
            os.kill(child_pid, 0)
        except ProcessLookupError:
            break
        time.sleep(0.01)
    with pytest.raises(ProcessLookupError):
        os.killpg(process_group, 0)
    with pytest.raises(ProcessLookupError):
        os.kill(child_pid, 0)


@pytest.mark.parametrize("output_kind", ["fifo", "public-file"])
def test_receipt_verifier_requires_private_regular_output(
    output_kind: str,
) -> None:
    """Verifier output must be nonblocking, regular, and owner-only."""

    output_action = {
        "fifo": "os.mkfifo(output)",
        "public-file": "output.write_bytes(b'{}'); output.chmod(0o644)",
    }[output_kind]
    started = time.monotonic()
    validation, errors = _run_test_receipt_verifier(
        (
            "#!/usr/bin/env python3\n"
            "import os\n"
            "import sys\n"
            "from pathlib import Path\n"
            "output = Path(sys.argv[sys.argv.index('--validation-out') + 1])\n"
            f"{output_action}\n"
        ).encode("ascii")
    )
    assert time.monotonic() - started < 2
    assert validation is None
    assert errors == ["external software signer receipt verifier could not run"]


@pytest.mark.parametrize(
    ("flag", "substitution"),
    [
        ("--expected-signer-service-id", "sorafs-promotion-signer-b"),
        ("--expected-signer-administrator-id", "sorafs-promotion-admin-c"),
        ("--expected-signer-key-revision", "8"),
        ("--expected-signer-policy-revision", "12"),
        ("--expected-signer-policy-digest-sha256", "ab" * 32),
    ],
)
def test_finalize_rejects_software_signer_binding_substitution(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    flag: str,
    substitution: str,
) -> None:
    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    args = finalize_args(tmp_path, public_key)
    args[args.index(flag) + 1] = substitution
    assert MODULE.main(args) == 2
    assert "software-signer binding must match" in capsys.readouterr().err
    assert not (tmp_path / "foundational-prerequisites.json").exists()


@pytest.mark.parametrize(
    ("case", "expected"),
    [
        ("missing_receipt", "--signer-receipt must be an existing regular file"),
        ("tampered_receipt", "receipt verification failed"),
        ("substituted_binding", "receipt verification failed"),
        ("wrong_verifier_digest", "verifier SHA-256 does not match"),
        ("wrong_operation", "receipt verification failed"),
        ("revoked", "revoked does not match"),
        ("wrong_role", "role does not match"),
        ("sequence_drift", "commit and audit sequences must match"),
        ("head_drift", "commit and audit heads must match"),
    ],
)
def test_finalize_requires_exact_external_software_signer_receipt(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    case: str,
    expected: str,
) -> None:
    """Reject missing, substituted, revoked, or internally stale receipts."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    args = finalize_args(tmp_path, public_key)
    receipt = Path(args[args.index("--signer-receipt") + 1])
    binding = Path(args[args.index("--signer-binding") + 1])
    verifier = Path(args[args.index("--signer-verifier") + 1])
    if case == "missing_receipt":
        receipt.unlink()
    elif case == "tampered_receipt":
        receipt.write_bytes(receipt.read_bytes() + b" ")
    elif case == "substituted_binding":
        binding.write_bytes(b"substituted-public-binding")
    elif case == "wrong_verifier_digest":
        args[args.index("--expected-signer-verifier-sha256") + 1] = "ab" * 32
    elif case == "wrong_operation":
        args[args.index("--expected-signer-operation-id") + 1] = "cd" * 32
    else:
        source = verifier.read_text(encoding="utf-8")
        substitutions = {
            "revoked": ("revoked=False", "revoked=True"),
            "wrong_role": ('role="promotion"', 'role="repair"'),
            "sequence_drift": ("audit_sequence=7", "audit_sequence=8"),
            "head_drift": (
                'audit_head_blake3_hex="55" * 32',
                'audit_head_blake3_hex="66" * 32',
            ),
        }
        before, after = substitutions[case]
        assert before in source
        verifier.chmod(0o700)
        verifier.write_text(source.replace(before, after, 1), encoding="utf-8")
        verifier.chmod(0o500)
        args[args.index("--expected-signer-verifier-sha256") + 1] = (
            hashlib.sha256(verifier.read_bytes()).hexdigest()
        )
    assert MODULE.main(args) == 1
    assert expected in capsys.readouterr().err
    assert not (tmp_path / "foundational-prerequisites.json").exists()


@pytest.mark.parametrize(
    ("case", "expected"),
    [
        ("missing", "failed to load evidence JSON"),
        ("stale", "exceeds max summary artifact age"),
        ("tampered", "receipt signature verification failed"),
        ("wrong_deployment", "deployment_id must match --deployment-id"),
    ],
)
def test_prepare_rejects_invalid_resilience_qualification(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    case: str,
    expected: str,
) -> None:
    """Foundational prepare must authenticate the exact resilience attachment."""

    _seed, public_key = signer
    args = prepare_args(tmp_path, public_key)
    resilience_path = tmp_path / "l1-resilience-qualification.summary"
    if case == "missing":
        resilience_path.unlink()
    elif case == "stale":
        stale_generated_at = NOW_UNIX - MAX_AGE_SECS - 1
        stale = build_resilience_summary(
            CHECKER,
            deployment_id=DEPLOYMENT_ID,
            environment=ENVIRONMENT,
            topology_qualification=topology_binding(tmp_path),
            generated_at_unix=stale_generated_at,
            captured_at_unix=stale_generated_at,
        )
        resilience_path.write_bytes(render_resilience_summary(stale))
    elif case == "tampered":
        tampered = json.loads(resilience_path.read_text(encoding="utf-8"))
        signature = tampered["receipt_authentication"]["signature_hex"]
        tampered["receipt_authentication"]["signature_hex"] = (
            ("0" if signature[0] != "0" else "1") + signature[1:]
        )
        resilience_path.write_bytes(render_resilience_summary(tampered))
    else:
        foreign = build_resilience_summary(
            CHECKER,
            deployment_id="sorafs-mainnet-foreign",
            environment=ENVIRONMENT,
            topology_qualification=topology_binding(tmp_path),
            generated_at_unix=GENERATED_AT_UNIX,
            captured_at_unix=EVIDENCE_AT_UNIX,
        )
        resilience_path.write_bytes(render_resilience_summary(foreign))

    assert MODULE.main(args) == 2
    assert expected in capsys.readouterr().err


def test_prepare_and_finalize_reject_substituted_topology_summary(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Reject a valid-looking qualification whose exact binding was not reviewed."""

    seed, public_key = signer
    original = topology_qualification_path(tmp_path)
    substitute = tmp_path / "substituted-topology-qualification.json"
    substituted_payload = json.loads(original.read_text(encoding="utf-8"))
    substituted_payload["manifest_sha256"] = hashlib.sha256(
        b"substituted-exact-manifest"
    ).hexdigest()
    substitute.write_text(
        json.dumps(substituted_payload, sort_keys=True),
        encoding="utf-8",
    )

    prepare = prepare_args(tmp_path, public_key)
    prepare[prepare.index("--topology-qualification-summary") + 1] = str(substitute)
    assert MODULE.main(prepare) == 2
    assert "must match the exact qualification binding" in capsys.readouterr().err

    prepare_and_sign(tmp_path, seed, public_key)
    finalize = finalize_args(tmp_path, public_key)
    finalize[finalize.index("--topology-qualification-summary") + 1] = str(
        substitute
    )
    assert MODULE.main(finalize) == 2
    assert "must match the exact qualification binding" in capsys.readouterr().err


def test_prepare_rejects_missing_and_reordered_lane_summary_inventory(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Fail closed unless all 17 exact summary files use canonical lane order."""

    _seed, public_key = signer
    missing = prepare_args(
        tmp_path,
        public_key,
        output_name="missing-lane.bin",
    )
    last_flag = len(missing) - 2
    assert missing[last_flag] == "--lane-summary"
    del missing[last_flag:]
    assert MODULE.main(missing) == 2
    assert "exactly 17 --lane-summary values are required" in capsys.readouterr().err

    reordered = prepare_args(
        tmp_path,
        public_key,
        output_name="reordered-lanes.bin",
    )
    value_indexes = [
        index + 1
        for index, value in enumerate(reordered)
        if value == "--lane-summary"
    ]
    reordered[value_indexes[0]], reordered[value_indexes[1]] = (
        reordered[value_indexes[1]],
        reordered[value_indexes[0]],
    )
    assert MODULE.main(reordered) == 2
    assert (
        "--lane-summary values must match all 17 readiness lanes in canonical order"
        in capsys.readouterr().err
    )
    assert not (tmp_path / "missing-lane.bin").exists()
    assert not (tmp_path / "reordered-lanes.bin").exists()


def test_later_sequence_requires_and_validates_immediate_signed_predecessor(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Bind sequence two to the exact verified sequence-one envelope bytes."""

    seed, public_key = signer
    previous_payload_name = "previous-signing-payload.bin"
    previous_signature_name = "previous-signature.bin"
    previous_envelope_name = "previous-envelope.json"
    previous_prepare = prepare_args(
        tmp_path,
        public_key,
        output_name=previous_payload_name,
        generated_at_unix=GENERATED_AT_UNIX - 15,
    )
    assert MODULE.main(previous_prepare) == 0
    previous_payload = (tmp_path / previous_payload_name).read_bytes()
    (tmp_path / previous_signature_name).write_bytes(sign(seed, previous_payload))
    assert (
        MODULE.main(
            finalize_args(
                tmp_path,
                public_key,
                payload_name=previous_payload_name,
                signature_name=previous_signature_name,
                output_name=previous_envelope_name,
            )
        )
        == 0
    )
    previous_bytes = (tmp_path / previous_envelope_name).read_bytes()
    previous_sha256 = hashlib.sha256(previous_bytes).hexdigest()

    current_prepare = prepare_args(
        tmp_path,
        public_key,
        output_name="current-signing-payload.bin",
        release_sequence=2,
        predecessor_sha256=previous_sha256,
        previous_envelope_name=previous_envelope_name,
    )
    assert MODULE.main(current_prepare) == 0
    current_payload = (tmp_path / "current-signing-payload.bin").read_bytes()
    (tmp_path / "current-signature.bin").write_bytes(sign(seed, current_payload))
    assert (
        MODULE.main(
            finalize_args(
                tmp_path,
                public_key,
                payload_name="current-signing-payload.bin",
                signature_name="current-signature.bin",
                output_name="current-envelope.json",
                release_sequence=2,
                predecessor_sha256=previous_sha256,
                previous_envelope_name=previous_envelope_name,
            )
        )
        == 0
    )

    missing_previous = prepare_args(
        tmp_path,
        public_key,
        output_name="missing-previous.bin",
        release_sequence=2,
        predecessor_sha256=previous_sha256,
    )
    assert MODULE.main(missing_previous) == 2
    assert "--previous-envelope is required" in capsys.readouterr().err

    wrong_digest = prepare_args(
        tmp_path,
        public_key,
        output_name="wrong-predecessor.bin",
        release_sequence=2,
        predecessor_sha256=hashlib.sha256(b"wrong-predecessor").hexdigest(),
        previous_envelope_name=previous_envelope_name,
    )
    assert MODULE.main(wrong_digest) == 2
    assert "does not match the reviewed predecessor" in capsys.readouterr().err

    forged = json.loads(previous_bytes)
    forged_signature = bytearray.fromhex(
        forged["signature"]["signature_hex"]
    )
    forged_signature[0] ^= 0x01
    forged["signature"]["signature_hex"] = forged_signature.hex()
    forged_name = "forged-previous-envelope.json"
    forged_bytes = MODULE.render_envelope(forged)
    (tmp_path / forged_name).write_bytes(forged_bytes)
    forged_prepare = prepare_args(
        tmp_path,
        public_key,
        output_name="forged-predecessor.bin",
        release_sequence=2,
        predecessor_sha256=hashlib.sha256(forged_bytes).hexdigest(),
        previous_envelope_name=forged_name,
    )
    assert MODULE.main(forged_prepare) == 2
    assert "signature verification failed" in capsys.readouterr().err


def test_finalized_envelope_is_accepted_by_direct_aggregate_gate(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
) -> None:
    """Pass the produced file through the real aggregate discovery path."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 0

    evidence_dir = tmp_path / "aggregate-evidence"
    evidence_dir.mkdir()
    (evidence_dir / "foundational-prerequisites.json").write_bytes(
        (tmp_path / "foundational-prerequisites.json").read_bytes()
    )
    (evidence_dir / "gateway-load.json").write_bytes(
        readiness_summary_paths(tmp_path)["gateway_load"].read_bytes(),
    )
    aggregate_out = tmp_path / "aggregate-summary.json"
    assert (
        CHECKER.main(
                        [
                            *topology_cli_args(tmp_path),
                            *lane_inventory_args(tmp_path),
                        "--resilience-qualification-summary",
                        str(resilience_qualification_path(tmp_path)),
                            "--resilience-qualification-signer-public-key-hex",
                            RESILIENCE_SIGNER_PUBLIC_KEY.hex(),
                        "--evidence-dir",
                str(evidence_dir),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--max-summary-artifact-age-secs",
                str(MAX_AGE_SECS),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--foundational-prerequisite-signer-public-key-hex",
                public_key.hex(),
                "--foundational-prerequisite-signer-verifier",
                str(tmp_path / "test-external-software-signer"),
                "--foundational-prerequisite-signer-verifier-sha256",
                hashlib.sha256(
                    (tmp_path / "test-external-software-signer").read_bytes()
                ).hexdigest(),
                "--foundational-prerequisite-release-sequence",
                str(RELEASE_SEQUENCE),
                "--foundational-prerequisite-previous-envelope-sha256",
                PREDECESSOR_SHA256,
                "--summary-out",
                            str(aggregate_out),
                            *(
                                argument
                                for gate_name, path in lane_summary_paths(tmp_path)
                                for argument in (
                                    "--l1-lane-summary",
                                    f"{gate_name}={path}",
                                )
                            ),
                        ]
        )
        == 0
    )
    aggregate = json.loads(aggregate_out.read_text(encoding="utf-8"))
    assert aggregate["status"] == CHECKER.NON_PROMOTABLE_STATUS
    assert aggregate["recognized_summary_count"] == 1
    assert aggregate["foundational_prerequisites"]["valid"] is True


def test_prepare_is_byte_deterministic_and_supports_reviewed_response_files(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
) -> None:
    """Equivalent direct and response-file inputs produce byte-identical payloads."""

    _seed, public_key = signer
    direct_args = prepare_args(
        tmp_path,
        public_key,
        output_name="direct-signing-payload.bin",
    )
    assert MODULE.main(direct_args) == 0

    response_args = prepare_args(
        tmp_path,
        public_key,
        output_name="response-signing-payload.bin",
    )
    response_path = tmp_path / "prepare-foundational.args"
    response_path.write_text(
        "\n".join(shlex.join([value]) for value in response_args) + "\n",
        encoding="utf-8",
    )
    assert MODULE.main([f"@{response_path}"]) == 0
    assert (tmp_path / "direct-signing-payload.bin").read_bytes() == (
        tmp_path / "response-signing-payload.bin"
    ).read_bytes()


@pytest.mark.parametrize(
    ("case", "expected"),
    [
        ("missing", "exactly nine --prerequisite values are required"),
        ("wrong_id", "prerequisite_id must match its ordered command-line id"),
        ("reordered", "canonical order"),
        (
            "wrong_schema",
            "evidence package schema must match the foundational prerequisite",
        ),
        (
            "underlying_schema",
            "schema must match its gate",
        ),
        ("path_swap", "prerequisite_id must match its ordered command-line id"),
        ("stale", "exceeds max summary artifact age"),
        ("tamper", "digest does not match the exact file"),
    ],
)
def test_prepare_rejects_invalid_prerequisite_evidence_packages(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    case: str,
    expected: str,
) -> None:
    """Reject wrong identities, ordering, schema, paths, freshness, and bytes."""

    _seed, public_key = signer
    evidence_at_unix = (
        NOW_UNIX - MAX_AGE_SECS - 1 if case == "stale" else EVIDENCE_AT_UNIX
    )
    specs = prerequisite_specs(
        tmp_path,
        evidence_at_unix=evidence_at_unix,
    )
    if case == "missing":
        specs = specs[:-1]
    elif case == "wrong_id":
        package_path = Path(specs[0].partition("=")[2])
        package = json.loads(package_path.read_text(encoding="utf-8"))
        package["prerequisite_id"] = "SF-1"
        package_path.write_text(json.dumps(package, sort_keys=True), encoding="utf-8")
    elif case == "reordered":
        specs[0], specs[1] = specs[1], specs[0]
    elif case == "wrong_schema":
        package_path = Path(specs[0].partition("=")[2])
        package = json.loads(package_path.read_text(encoding="utf-8"))
        package["schema"] = "sorafs.production_readiness.unknown.v1"
        package_path.write_text(json.dumps(package, sort_keys=True), encoding="utf-8")
    elif case == "underlying_schema":
        summary_path = tmp_path / "authoritative-gateway-load-summary.json"
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        summary["schema"] = "sorafs.production_readiness.unknown.v1"
        summary_path.write_text(json.dumps(summary, sort_keys=True), encoding="utf-8")
        summary_sha256 = hashlib.sha256(summary_path.read_bytes()).hexdigest()
        for spec in specs:
            package_path = Path(spec.partition("=")[2])
            package = json.loads(package_path.read_text(encoding="utf-8"))
            for reference in package["readiness_summaries"]:
                if reference["gate"] == "gateway_load":
                    reference["sha256"] = summary_sha256
            package_path.write_text(
                json.dumps(package, sort_keys=True),
                encoding="utf-8",
            )
    elif case == "path_swap":
        first_id, _, first_path = specs[0].partition("=")
        second_id, _, second_path = specs[1].partition("=")
        specs[0] = f"{first_id}={second_path}"
        specs[1] = f"{second_id}={first_path}"
    elif case == "tamper":
        summary_path = tmp_path / "authoritative-gateway-load-summary.json"
        summary_path.write_bytes(summary_path.read_bytes() + b"\n")
    args = prepare_args(tmp_path, public_key, specs=specs)
    assert MODULE.main(args) == 2
    captured = capsys.readouterr()
    assert expected in captured.err
    assert not (tmp_path / "foundational-signing-payload.bin").exists()


def test_prepare_rejects_legacy_gateway_load_for_all_nine_prerequisites(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """A valid gateway-load summary cannot stand in for unrelated foundations."""

    _seed, public_key = signer
    specs = legacy_gateway_load_prerequisite_specs(tmp_path)
    assert MODULE.main(prepare_args(tmp_path, public_key, specs=specs)) == 2
    diagnostics = capsys.readouterr().err
    assert "must match the exact lanes for SFM-1: reputation" in diagnostics
    assert "must match the exact lanes for SF-8a" in diagnostics
    assert not (tmp_path / "foundational-signing-payload.bin").exists()


def test_prepare_rejects_retired_singular_summary_field(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """The V1 hard cut rejects the retired singular compatibility shape."""

    _seed, public_key = signer
    specs = prerequisite_specs(tmp_path)
    package_path = Path(specs[0].partition("=")[2])
    package = json.loads(package_path.read_text(encoding="utf-8"))
    package["readiness_summary"] = package.pop("readiness_summaries")[0]
    package_path.write_text(json.dumps(package, sort_keys=True), encoding="utf-8")

    assert MODULE.main(prepare_args(tmp_path, public_key, specs=specs)) == 2
    diagnostics = capsys.readouterr().err
    assert ".readiness_summary is not allowed" in diagnostics
    assert "readiness_summaries must be an array" in diagnostics
    assert not (tmp_path / "foundational-signing-payload.bin").exists()


@pytest.mark.parametrize(
    ("case", "expected"),
    [
        ("missing", "are missing required gates"),
        ("extra", "must contain exactly 5 entries for SF-6"),
        ("duplicate", "must contain exactly 2 entries for SF-2c"),
        ("reordered", "must match the exact lanes for SF-6"),
        ("wrong_valid_gate", "contain gates outside the prerequisite mapping"),
        ("digest_substitution", "digest does not match the exact file"),
    ],
)
def test_prepare_rejects_mapped_multi_lane_package_attacks(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    case: str,
    expected: str,
) -> None:
    """Reject structural and byte substitutions within grouped lane evidence."""

    _seed, public_key = signer
    specs = prerequisite_specs(tmp_path)
    target_id = "SF-2c" if case in {"missing", "duplicate"} else "SF-6"
    target_spec = next(spec for spec in specs if spec.startswith(f"{target_id}="))
    package_path = Path(target_spec.partition("=")[2])
    package = json.loads(package_path.read_text(encoding="utf-8"))
    references = package["readiness_summaries"]
    summary_paths = readiness_summary_paths(tmp_path)

    if case == "missing":
        references.pop()
    elif case == "extra":
        gateway_path = summary_paths["gateway_load"]
        references.append(
            {
                "gate": "gateway_load",
                "path": gateway_path.name,
                "sha256": hashlib.sha256(gateway_path.read_bytes()).hexdigest(),
            }
        )
    elif case == "duplicate":
        references.append(dict(references[0]))
    elif case == "reordered":
        references.reverse()
    elif case == "wrong_valid_gate":
        gateway_path = summary_paths["gateway_load"]
        references[0] = {
            "gate": "gateway_load",
            "path": gateway_path.name,
            "sha256": hashlib.sha256(gateway_path.read_bytes()).hexdigest(),
        }
    else:
        substitute_path = summary_paths["gateway_load"]
        references[0]["sha256"] = hashlib.sha256(
            substitute_path.read_bytes()
        ).hexdigest()

    package_path.write_text(json.dumps(package, sort_keys=True), encoding="utf-8")
    assert MODULE.main(prepare_args(tmp_path, public_key, specs=specs)) == 2
    assert expected in capsys.readouterr().err
    assert not (tmp_path / "foundational-signing-payload.bin").exists()


def test_prepare_rejects_wrong_reference_count_before_summary_io(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    monkeypatch,
    capsys,
) -> None:
    """A malformed reference count cannot amplify authoritative-summary reads."""

    _seed, public_key = signer
    specs = prerequisite_specs(tmp_path)
    target_spec = next(spec for spec in specs if spec.startswith("SF-6="))
    package_path = Path(target_spec.partition("=")[2])
    package = json.loads(package_path.read_text(encoding="utf-8"))
    package["readiness_summaries"].extend(
        dict(package["readiness_summaries"][0]) for _ in range(64)
    )
    package_path.write_text(json.dumps(package, sort_keys=True), encoding="utf-8")

    original_read = MODULE.read_bounded_regular_file
    observed_labels: list[str] = []

    def tracking_read(*args, **kwargs):
        observed_labels.append(kwargs.get("label", ""))
        return original_read(*args, **kwargs)

    monkeypatch.setattr(MODULE, "read_bounded_regular_file", tracking_read)
    assert MODULE.main(prepare_args(tmp_path, public_key, specs=specs)) == 2
    diagnostics = capsys.readouterr().err
    assert "must contain exactly 5 entries for SF-6" in diagnostics
    assert not any(
        label.startswith(
            "--prerequisite[7] evidence package readiness_summaries["
        )
        for label in observed_labels
    )
    assert not (tmp_path / "foundational-signing-payload.bin").exists()


def test_prepare_rejects_digest_only_prerequisite_rows(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Do not retain the former caller-supplied digest/timestamp production path."""

    _seed, public_key = signer
    legacy = [
        f"{prerequisite_id}:{'ab' * 32}:{EVIDENCE_AT_UNIX}"
        for prerequisite_id in MODULE.FOUNDATIONAL_PREREQUISITE_IDS
    ]
    assert MODULE.main(prepare_args(tmp_path, public_key, specs=legacy)) == 2
    assert "must use ID=PATH" in capsys.readouterr().err
    assert not (tmp_path / "foundational-signing-payload.bin").exists()


@pytest.mark.parametrize(
    ("overrides", "expected"),
    [
        (
            {"generated_at_unix": NOW_UNIX + 1},
            "generated_at_unix must not be future",
        ),
        (
            {"generated_at_unix": NOW_UNIX - MAX_AGE_SECS - 1},
            "generated_at_unix exceeds max summary artifact age",
        ),
        (
            {"deployment_id": "sorafs-staging-2026-07"},
            "must not contain non-production deployment markers",
        ),
        (
            {"deployment_id": "bearer_token=runtime-only-value"},
            "topology qualification deployment_id must be",
        ),
        (
            {"environment": "development"},
            "environment must be production",
        ),
        (
            {
                "release_sequence": 1,
                "predecessor_sha256": hashlib.sha256(b"not-the-root").hexdigest(),
            },
            "requires the zero predecessor",
        ),
        (
            {"release_sequence": 2, "predecessor_sha256": "00" * 32},
            "requires a non-zero predecessor",
        ),
    ],
)
def test_prepare_rejects_bad_context_freshness_and_continuity(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    overrides: dict,
    expected: str,
) -> None:
    """Fail closed on non-production context, clock, and chain discontinuity."""

    _seed, public_key = signer
    assert MODULE.main(prepare_args(tmp_path, public_key, **overrides)) == 2
    captured = capsys.readouterr()
    assert expected in captured.err
    assert "runtime-only-value" not in captured.err


def test_prepare_rejects_untrusted_key_and_never_clobbers_output(
    tmp_path: Path,
    capsys,
) -> None:
    """Reject zero trust anchors and preserve an existing destination exactly."""

    output = tmp_path / "foundational-signing-payload.bin"
    output.write_bytes(b"preserve-me")
    assert MODULE.main(prepare_args(tmp_path, b"\x00" * 32)) == 2
    captured = capsys.readouterr()
    assert "must not be the all-zero key" in captured.err
    assert "must not already exist" in captured.err
    assert output.read_bytes() == b"preserve-me"


def test_prepare_rejects_symlink_parent_and_secret_looking_output(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Do not follow output links or render secret-looking path material."""

    _seed, public_key = signer
    target = tmp_path / "target"
    target.mkdir()
    linked = tmp_path / "linked"
    linked.symlink_to(target, target_is_directory=True)
    args = prepare_args(tmp_path, public_key)
    output_index = args.index("--signing-payload-out") + 1
    args[output_index] = str(linked / "payload.bin")
    assert MODULE.main(args) == 2
    captured = capsys.readouterr()
    assert "parent" in captured.err
    assert "must not be a symlink" in captured.err

    secret_args = prepare_args(
        tmp_path,
        public_key,
        output_name="private_key-material.bin",
    )
    assert MODULE.main(secret_args) == 2
    captured = capsys.readouterr()
    assert "canonical safe artifact path" in captured.err
    assert "private_key" not in captured.err


def test_prepare_detects_parent_swap_during_atomic_publication(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    monkeypatch,
    capsys,
) -> None:
    """Pin the output parent and remove the artifact if its pathname is swapped."""

    _seed, public_key = signer
    live_parent = tmp_path / "live"
    pinned_parent = tmp_path / "pinned"
    live_parent.mkdir()
    original_write = MODULE.write_all_checker_summary_bytes
    swapped = False

    def swap_parent_then_write(fd: int, payload: bytes) -> None:
        nonlocal swapped
        if not swapped:
            live_parent.rename(pinned_parent)
            live_parent.mkdir()
            swapped = True
        original_write(fd, payload)

    monkeypatch.setattr(
        MODULE,
        "write_all_checker_summary_bytes",
        swap_parent_then_write,
    )
    assert (
        MODULE.main(
            prepare_args(
                tmp_path,
                public_key,
                output_name="live/foundational-signing-payload.bin",
            )
        )
        == 2
    )
    assert "path changed during atomic publication" in capsys.readouterr().err
    assert not (live_parent / "foundational-signing-payload.bin").exists()
    assert not (pinned_parent / "foundational-signing-payload.bin").exists()


def test_bounded_input_read_detects_parent_swap(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Reject bytes read through a parent FD when the reviewed pathname moves."""

    live_parent = tmp_path / "live-input"
    pinned_parent = tmp_path / "pinned-input"
    live_parent.mkdir()
    source = live_parent / "signature.bin"
    source.write_bytes(b"\x01" * 64)
    original_read = MODULE.os.read
    swapped = False

    def swap_parent_then_read(fd: int, size: int) -> bytes:
        nonlocal swapped
        if not swapped:
            live_parent.rename(pinned_parent)
            live_parent.mkdir()
            (live_parent / "signature.bin").write_bytes(b"\x02" * 64)
            swapped = True
        return original_read(fd, size)

    monkeypatch.setattr(MODULE.os, "read", swap_parent_then_read)
    payload, errors = MODULE.read_bounded_regular_file(
        source,
        label="--signature-file",
        maximum_bytes=64,
    )
    assert payload is None
    assert errors == ["--signature-file path changed while it was read"]


def test_atomic_publication_removes_destination_when_parent_fsync_fails(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    monkeypatch,
    capsys,
) -> None:
    """Do not leave a publishable artifact after uncertain directory durability."""

    _seed, public_key = signer
    original_fsync = MODULE.os.fsync
    fsync_calls = 0

    def fail_parent_fsync(fd: int) -> None:
        nonlocal fsync_calls
        fsync_calls += 1
        if fsync_calls == 2:
            raise OSError("injected parent fsync failure")
        original_fsync(fd)

    monkeypatch.setattr(MODULE.os, "fsync", fail_parent_fsync)
    assert MODULE.main(prepare_args(tmp_path, public_key)) == 2
    assert "cannot be written" in capsys.readouterr().err
    assert not (tmp_path / "foundational-signing-payload.bin").exists()


@pytest.mark.parametrize(
    ("signature_factory", "expected", "exit_code"),
    [
        (lambda _payload: b"\x00" * 64, "all-zero signature", 2),
        (lambda _payload: b"\x01" * 63, "exactly 64 raw bytes", 2),
        (
            lambda payload: bytes([payload[0] ^ 1, *payload[1:64]]),
            "signature verification failed",
            1,
        ),
    ],
)
def test_finalize_rejects_zero_malformed_and_forged_signatures(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    signature_factory,
    expected: str,
    exit_code: int,
) -> None:
    """Only a strict detached Ed25519 signature can cross finalization."""

    _seed, public_key = signer
    assert MODULE.main(prepare_args(tmp_path, public_key)) == 0
    payload = (tmp_path / "foundational-signing-payload.bin").read_bytes()
    (tmp_path / "foundational-signature.bin").write_bytes(
        signature_factory(payload)
    )
    assert MODULE.main(finalize_args(tmp_path, public_key)) == exit_code
    captured = capsys.readouterr()
    assert expected in captured.err
    assert not (tmp_path / "foundational-prerequisites.json").exists()


def test_finalize_rejects_self_selected_key_and_expected_continuity_mismatch(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Bind the signature to both reviewed trust and continuity inputs."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    alternate_public_key = public_key_from_seed(os.urandom(32))
    assert MODULE.main(finalize_args(tmp_path, alternate_public_key)) == 2
    captured = capsys.readouterr()
    assert "fingerprint must match the operator-trusted key" in captured.err

    assert (
        MODULE.main(
            finalize_args(
                tmp_path,
                public_key,
                release_sequence=RELEASE_SEQUENCE + 1,
                predecessor_sha256=hashlib.sha256(
                    b"missing-reviewed-predecessor"
                ).hexdigest(),
            )
        )
        == 2
    )
    captured = capsys.readouterr()
    assert "--previous-envelope is required" in captured.err

    assert (
        MODULE.main(
            finalize_args(
                tmp_path,
                public_key,
                predecessor_sha256=hashlib.sha256(b"wrong").hexdigest(),
            )
        )
        == 2
    )
    captured = capsys.readouterr()
    assert "--expected-release-sequence 1 requires the zero predecessor" in (
        captured.err
    )


@pytest.mark.parametrize(
    ("payload_factory", "expected"),
    [
        (
            lambda unsigned: b"wrong-domain\x00"
            + MODULE.canonical_json_bytes(unsigned),
            "wrong signature domain",
        ),
        (
            lambda unsigned: MODULE.FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN
            + json.dumps(unsigned, indent=2, sort_keys=True).encode("ascii"),
            "exact canonical encoding",
        ),
        (
            lambda _unsigned: (
                MODULE.FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN
                + b'{"schema":"a","schema":"b"}'
            ),
            "strict and duplicate-free",
        ),
    ],
)
def test_finalize_rejects_wrong_domain_noncanonical_and_duplicate_json(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    payload_factory,
    expected: str,
) -> None:
    """Reject altered encodings before detached signature verification."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    payload_path = tmp_path / "foundational-signing-payload.bin"
    unsigned = decode_unsigned(payload_path.read_bytes())
    payload_path.unlink()
    payload_path.write_bytes(payload_factory(unsigned))
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert expected in captured.err


def test_finalize_rejects_secret_fields_without_leaking_values(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Keep unknown sensitive fields and their values out of diagnostics."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    payload_path = tmp_path / "foundational-signing-payload.bin"
    unsigned = decode_unsigned(payload_path.read_bytes())
    secret_value = "bearer runtime-only-sensitive-material"
    unsigned["private_key"] = secret_value
    payload_path.unlink()
    write_signing_payload(payload_path, unsigned)
    (tmp_path / "foundational-signature.bin").write_bytes(
        sign(seed, payload_path.read_bytes())
    )
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert "<sensitive-key>" in captured.err
    assert secret_value not in captured.err
    assert "private_key" not in captured.err


def test_finalize_rejects_symlinked_signature_and_preserves_existing_envelope(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Reject input symlinks and never replace an existing final envelope."""

    seed, public_key = signer
    payload = prepare_and_sign(tmp_path, seed, public_key)
    signature_path = tmp_path / "foundational-signature.bin"
    signature_path.unlink()
    signature_target = tmp_path / "signature-target.bin"
    signature_target.write_bytes(sign(seed, payload))
    signature_path.symlink_to(signature_target)
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert "--signature-file must not be a symlink" in captured.err

    signature_path.unlink()
    signature_path.write_bytes(sign(seed, payload))
    envelope = tmp_path / "foundational-prerequisites.json"
    envelope.write_bytes(b"preserve-final-envelope")
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert "--envelope-out must not already exist" in captured.err
    assert envelope.read_bytes() == b"preserve-final-envelope"


def test_finalize_rejects_symlinked_input_parent_hardlinks_and_writable_inputs(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Keep every public input behind the stable regular-file preflight."""

    seed, public_key = signer
    payload = prepare_and_sign(tmp_path, seed, public_key)

    target_parent = tmp_path / "input-target"
    target_parent.mkdir()
    (target_parent / "signature.bin").write_bytes(sign(seed, payload))
    linked_parent = tmp_path / "input-linked"
    linked_parent.symlink_to(target_parent, target_is_directory=True)
    args = finalize_args(tmp_path, public_key)
    args[args.index("--signature-file") + 1] = str(
        linked_parent / "signature.bin"
    )
    assert MODULE.main(args) == 2
    captured = capsys.readouterr()
    assert "--signature-file parent" in captured.err
    assert "must not be a symlink" in captured.err

    signature_path = tmp_path / "foundational-signature.bin"
    signature_path.unlink()
    hardlink_source = tmp_path / "signature-source.bin"
    hardlink_source.write_bytes(sign(seed, payload))
    os.link(hardlink_source, signature_path)
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert "--signature-file must not be hardlinked" in captured.err

    signature_path.unlink()
    hardlink_source.unlink()
    signature_path.write_bytes(sign(seed, payload))
    signature_path.chmod(0o666)
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert "--signature-file must not be group- or world-writable" in captured.err


def test_cli_has_no_private_signing_key_input() -> None:
    """Keep every private-key option outside the builder boundary."""

    parser = MODULE.build_parser()
    options: set[str] = set()
    pending = [parser]
    while pending:
        current = pending.pop()
        for action in current._actions:  # noqa: SLF001 - parser contract test
            options.update(action.option_strings)
            choices = getattr(action, "choices", None)
            if isinstance(choices, dict):
                pending.extend(choices.values())
    assert not any("private" in option or "seed" in option for option in options)
