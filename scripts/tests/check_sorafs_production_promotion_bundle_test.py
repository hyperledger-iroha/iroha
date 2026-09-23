"""Tests for the final read-only SoraFS production-promotion verifier."""

from __future__ import annotations

import argparse
import hashlib
import base64
import copy
import importlib.util
import json
import os
import re
import sys
from pathlib import Path
from typing import Any, Callable

import pytest


SCRIPT_DIR = Path(__file__).resolve().parents[1]
TEST_SUPPORT_DIR = Path(__file__).resolve().parent
for import_path in (SCRIPT_DIR, TEST_SUPPORT_DIR):
    if str(import_path) not in sys.path:
        sys.path.insert(0, str(import_path))

MODULE_PATH = SCRIPT_DIR / "check_sorafs_production_promotion_bundle.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_production_promotion_bundle",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

import check_sorafs_production_readiness as aggregate_checker  # noqa: E402
import check_sorafs_production_readiness_test as readiness_fixture  # noqa: E402
import sorafs_l1_lane_evidence_inventory as lane_inventory  # noqa: E402
import sorafs_l1_lane_inventory_integration as inventory_integration  # noqa: E402
import sorafs_l1_lane_inventory_test_support as inventory_support  # noqa: E402
import sorafs_topology_qualification as topology_qualification  # noqa: E402
import sorafs_rollout_runner_test_support as rollout_support  # noqa: E402
import sorafs_resilience_test_support as resilience_support  # noqa: E402
from sorafs_resilience_test_support import (  # noqa: E402
    public_key_from_seed,
    sign,
)


SIGNING_SEED = bytes.fromhex("6d" * 32)
SIGNING_PUBLIC_KEY = public_key_from_seed(SIGNING_SEED)
SIGNER_SERVICE_ID = "sorafs-promotion-signer-a"
SIGNER_ADMINISTRATOR_ID = "sorafs-promotion-admin-b"
SIGNER_KEY_REVISION = 11
SIGNER_POLICY_REVISION = 19
SIGNER_POLICY_DIGEST = "a7" * 32
CERTIFICATE_IDENTITY = "https://github.com/hyperledger-iroha/iroha"
OIDC_ISSUER = "https://token.actions.githubusercontent.com"
NOW_UNIX = 1_900_000_000
CHAIN_ID = "promotion-chain"
NETWORK_ID_HEX = "11" * 32
DEPLOYMENT_ID = "production-primary"


@pytest.mark.parametrize("identity", ["attester", "attestation", "latest", "contest", "Account-Attester", "a" * 128])
def test_signer_identity_preserves_real_words_and_exact_bytes(identity: str) -> None:
    assert MODULE._canonical_identity(identity) == identity


@pytest.mark.parametrize("reserved", ["null", "mock", "test", "dev", "demo", "fake", "dummy", "placeholder"])
@pytest.mark.parametrize("delimiter", [".", "_", "-", ":"])
def test_signer_identity_rejects_reserved_components(reserved: str, delimiter: str) -> None:
    assert MODULE._canonical_identity(f"production{delimiter}{reserved.upper()}{delimiter}primary") is None


@pytest.mark.parametrize("identity", [None, 1, "", " a", "a ", "a/b", "a@b", "a?b", "a#b", "a%2fb", "a\\nb", "é", "a" * 129])
def test_signer_identity_rejects_malformed_bytes_and_size(identity: Any) -> None:
    assert MODULE._canonical_identity(identity) is None


def digest(label: str) -> str:
    """Return one deterministic non-zero test digest."""

    return hashlib.sha256(label.encode("ascii")).hexdigest()


def synthetic_cosign_bundle() -> dict[str, Any]:
    """Use a public upstream proof for another subject; it cannot prove this promotion."""

    fixture = SCRIPT_DIR.parent / "fixtures/sorafs/final_promotion_cosign/bundle.sigstore.json"
    return json.loads(fixture.read_bytes())


def promotion_payload() -> dict[str, Any]:
    """Return the explicit fields checked after aggregate schema validation."""

    return {
        "schema": MODULE.promotion_runner.SUMMARY_SCHEMA,
        "status": "ready",
        "required_gates": list(MODULE.promotion_runner.DEFAULT_REQUIRED_GATES),
        "summary_file_count": 17,
        "recognized_summary_count": 17,
        "resilience_qualification": {
            "present": True,
            "valid": True,
            "binding": {"schema": "synthetic-unit-binding"},
            "errors": [],
        },
        "l1_lane_evidence_inventory": {
            "present": True,
            "valid": True,
            "binding": {"schema": "synthetic-unit-inventory"},
            "errors": [],
        },
        "required": {
            gate: {"present": True, "valid": True, "errors": []}
            for gate in MODULE.promotion_runner.DEFAULT_REQUIRED_GATES
        },
        "foundational_prerequisites": {
            "present": True,
            "valid": True,
            "errors": [],
        },
        "errors": [],
    }


def schema_valid_synthetic_promotion_payload() -> dict[str, Any]:
    """Return a synthetic payload accepted by the authoritative aggregate validator.

    This is test structure only: its deterministic digests and signer identities are
    neither captured deployment evidence nor eligible production provenance.
    """

    deployment_id = "sorafs-mainnet-2026-08"
    environment = "production"
    generated_at_unix = NOW_UNIX - 120
    base_topology = {
        "qualification_summary_sha256": digest("topology-summary"),
        "manifest_sha256": digest("topology-manifest"),
        "canonical_manifest_sha256": digest("topology-manifest-canonical"),
        "deployment_id": deployment_id,
        "environment": environment,
        "network": "taira",
        "chain_id": "fc56984b-2be7-431d-840e-21514d1883f0",
        "chain_discriminant": 369,
        "validator_ids_sha256": (
            topology_qualification.CANONICAL_TAIRA_VALIDATOR_IDS_SHA256
        ),
    }
    authenticated_topology = {
        **base_topology,
        "signer_authentication_kind": "external-ed25519",
        "signer_service_id": "sorafs-topology-signer-a",
        "signer_administrator_id": "sorafs-topology-admin-b",
        "signer_key_revision": 3,
        "signer_policy_revision": 5,
        "signer_policy_digest_sha256": digest("topology-policy"),
        "signer_public_key_fingerprint_sha256": digest("topology-key"),
    }
    resilience_binding = {
        "schema": aggregate_checker.RESILIENCE_QUALIFICATION_BINDING_SCHEMA,
        "summary_sha256": digest("resilience-summary"),
        "receipt_sha256": digest("resilience-receipt"),
        "canonical_receipt_sha256": digest("resilience-canonical-receipt"),
        "receipt_generated_at_unix": generated_at_unix,
        "signer_service_id": "sorafs-resilience-signer-a",
        "signer_administrator_id": "sorafs-resilience-admin-b",
        "signer_key_revision": 5,
        "signer_policy_revision": 8,
        "signer_policy_digest_sha256": digest("resilience-policy"),
        "signer_public_key_fingerprint_sha256": digest("resilience-key"),
    }
    inventory_sha256 = digest("lane-inventory")
    inventory_binding = {
        "schema": lane_inventory.VERIFICATION_SCHEMA,
        "status": "ready",
        "inventory_sha256": inventory_sha256,
        "summary_file_count": 17,
        "recognized_summary_count": 17,
        "deployment": {
            "deployment_id": deployment_id,
            "environment": environment,
            "network": "taira",
            "chain_id": base_topology["chain_id"],
            "chain_discriminant": base_topology["chain_discriminant"],
        },
        "anchors": {
            "topology_qualification_summary_sha256": base_topology[
                "qualification_summary_sha256"
            ],
            "topology_manifest_sha256": base_topology["manifest_sha256"],
            "topology_canonical_manifest_sha256": base_topology[
                "canonical_manifest_sha256"
            ],
            "validator_ids_sha256": base_topology["validator_ids_sha256"],
            "oldest_evidence_generated_at_unix": generated_at_unix,
            "newest_evidence_generated_at_unix": generated_at_unix,
        },
        "signer": {
            "role": lane_inventory.SIGNER_ROLE,
            "service_kind": lane_inventory.SIGNER_KIND,
            "algorithm": "ed25519",
            "service_id": "sorafs-inventory-signer-a",
            "administrator_id": "sorafs-inventory-admin-b",
            "key_revision": 13,
            "policy_revision": 17,
            "policy_digest_sha256": digest("inventory-policy"),
            "public_key_fingerprint_sha256": digest("inventory-key"),
        },
    }
    lane_sha256 = {
        gate: digest(f"lane:{gate}")
        for gate in MODULE.promotion_runner.DEFAULT_REQUIRED_GATES
    }
    required = {}
    for gate_name in MODULE.promotion_runner.DEFAULT_REQUIRED_GATES:
        gate = MODULE.promotion_runner.GATE_BY_NAME[gate_name]
        artifact_count = len(gate.required_kinds)
        required[gate_name] = {
            "schema": gate.schema,
            "present": True,
            "valid": True,
            "required_kind_count": artifact_count,
            "expected_required_kind_count": artifact_count,
            "evidence_file_count": artifact_count,
            "recognized_artifact_count": artifact_count,
            "artifact_count": artifact_count,
            "thresholds": {"synthetic_threshold": 1},
            "oldest_generated_at_unix": generated_at_unix,
            "newest_generated_at_unix": generated_at_unix,
            "deployment_id": deployment_id,
            "environment": environment,
            "expected_required_kinds": list(gate.required_kinds),
            "topology_qualification": base_topology,
            "errors": [],
            "path": f"{gate_name}.json",
            "sha256": lane_sha256[gate_name],
        }
    foundational = {
        "schema": MODULE.promotion_runner.FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "present": True,
        "valid": True,
        "required_ids": list(
            MODULE.promotion_runner.FOUNDATIONAL_PREREQUISITE_IDS
        ),
        "prerequisite_count": len(
            MODULE.promotion_runner.FOUNDATIONAL_PREREQUISITE_IDS
        ),
        "generated_at_unix": generated_at_unix,
        "oldest_evidence_generated_at_unix": generated_at_unix - 1,
        "newest_evidence_generated_at_unix": generated_at_unix - 1,
        "deployment_id": deployment_id,
        "environment": environment,
        "release_sequence": 7,
        "previous_envelope_sha256": digest("previous-foundation"),
        "signer_service_id": "sorafs-foundation-signer-a",
        "signer_administrator_id": "sorafs-foundation-admin-b",
        "signer_key_revision": 7,
        "signer_policy_revision": 11,
        "signer_policy_digest_sha256": digest("foundation-policy"),
        "signer_public_key_fingerprint_sha256": digest("foundation-key"),
        "evidence_anchor_sha256": [
            digest(f"anchor:{prerequisite_id}")
            for prerequisite_id in (
                MODULE.promotion_runner.FOUNDATIONAL_PREREQUISITE_IDS
            )
        ],
        "prerequisite_readiness_summary_sha256": [
            {
                "id": prerequisite_id,
                "readiness_summary_sha256": [
                    {"gate": gate, "sha256": lane_sha256[gate]}
                    for gate in aggregate_checker.FOUNDATIONAL_PREREQUISITE_LANES[
                        prerequisite_id
                    ]
                ],
            }
            for prerequisite_id in (
                MODULE.promotion_runner.FOUNDATIONAL_PREREQUISITE_IDS
            )
        ],
        "lane_summary_sha256": [
            {"gate": gate, "sha256": lane_sha256[gate]}
            for gate in MODULE.promotion_runner.DEFAULT_REQUIRED_GATES
        ],
        "l1_lane_evidence_inventory_sha256": inventory_sha256,
        "topology_qualification": authenticated_topology,
        "resilience_qualification": resilience_binding,
        "path": "foundational-prerequisites.json",
        "sha256": digest("foundation-envelope"),
        "errors": [],
    }
    return {
        "schema": MODULE.promotion_runner.SUMMARY_SCHEMA,
        "status": "ready",
        "required_gates": list(MODULE.promotion_runner.DEFAULT_REQUIRED_GATES),
        "thresholds": {"max_summary_artifact_age_secs": 1_209_600},
        "summary_file_count": 17,
        "recognized_summary_count": 17,
        "deployment": {
            "deployment_id": deployment_id,
            "environment": environment,
        },
        "topology_qualification": authenticated_topology,
        "resilience_qualification": {
            "schema": (
                aggregate_checker.AGGREGATE_RESILIENCE_QUALIFICATION_SCHEMA
            ),
            "present": True,
            "valid": True,
            "binding": resilience_binding,
            "errors": [],
        },
        "l1_lane_evidence_inventory": {
            "schema": (
                inventory_integration.AGGREGATE_L1_LANE_EVIDENCE_INVENTORY_SCHEMA
            ),
            "present": True,
            "valid": True,
            "binding": inventory_binding,
            "errors": [],
        },
        "foundational_prerequisites": foundational,
        "required": required,
        "errors": [],
    }


def write_positive_replay(
    root: Path,
    *,
    aggregate_payload: dict[str, Any] | None = None,
) -> tuple[dict[str, Path], dict[str, Any]]:
    """Write one byte-identical two-run/22-input synthetic replay."""

    payload = promotion_payload() if aggregate_payload is None else aggregate_payload
    aggregate_raw = MODULE.render_checker_summary(payload).encode("utf-8")
    first = root / "aggregate-first.json"
    second = root / "aggregate-second.json"
    first.write_bytes(aggregate_raw)
    second.write_bytes(aggregate_raw)
    snapshot = tuple(
        (slot, digest(f"input:{slot}"))
        for slot in MODULE.promotion_runner.REPLAY_INPUT_SLOTS
    )
    aggregate_sha256 = hashlib.sha256(aggregate_raw).hexdigest()
    replay = MODULE.promotion_runner.ReplayAggregate(
        payload=payload,
        first_sha256=aggregate_sha256,
        second_sha256=aggregate_sha256,
        semantic_sha256=aggregate_sha256,
    )
    manifest = MODULE.promotion_runner.build_replay_manifest(snapshot, replay)
    manifest_path = root / "replay-manifest.json"
    manifest_raw = MODULE.render_checker_summary(manifest).encode("utf-8")
    manifest_path.write_bytes(manifest_raw)
    return (
        {
            "first": first,
            "second": second,
            "manifest": manifest_path,
        },
        {
            "input_count": len(snapshot),
            "input_set_sha256": MODULE.promotion_runner.input_set_sha256(snapshot),
            "positive_output_sha256": {
                "first_aggregate_sha256": aggregate_sha256,
                "second_aggregate_sha256": aggregate_sha256,
                "aggregate_semantic_sha256": aggregate_sha256,
                "replay_manifest_sha256": hashlib.sha256(manifest_raw).hexdigest(),
            },
        },
    )


def write_negative_archive(
    root: Path,
    positive: dict[str, Any],
    *,
    input_set_sha256: str | None = None,
) -> tuple[Path, dict[str, Any], bytes]:
    """Write a schema-valid local-only six-receipt synthetic archive."""

    archive = root / "negative-archive"
    archive.mkdir()
    baseline_input_set_sha256 = (
        input_set_sha256 or positive["input_set_sha256"]
    )
    runner_sha256 = digest("negative-runner")
    checker_sha256 = digest("aggregate-checker")
    toolchain_sha256 = digest("negative-toolchain")
    rows: list[dict[str, str]] = []
    for index, case in enumerate(MODULE.negative_runner.MUTATION_CASES, start=1):
        receipt = {
            "schema": MODULE.negative_runner.RECEIPT_SCHEMA,
            "mutation_id": case.mutation_id,
            "baseline_input_set_sha256": baseline_input_set_sha256,
            "aggregate_checker_sha256": checker_sha256,
            "aggregate_toolchain_sha256": toolchain_sha256,
            "expected_rejection": {
                "checker_exit_code": 1,
                "aggregate_status": "blocked",
                "diagnostic_class": case.diagnostic_class,
            },
            "observed_diagnostic_class": case.diagnostic_class,
            "output_sha256": {
                field: digest(f"{case.mutation_id}:{field}")
                for field in MODULE.negative_runner.OUTPUT_HASH_FIELDS
            },
            "errors": [],
        }
        filename = f"{index:02d}-{case.mutation_id}.json"
        raw = MODULE.render_checker_summary(receipt).encode("utf-8")
        (archive / filename).write_bytes(raw)
        rows.append(
            {
                "mutation_id": case.mutation_id,
                "receipt_file": filename,
                "sha256": hashlib.sha256(raw).hexdigest(),
            }
        )
    positive_hashes = positive["positive_output_sha256"]
    python_runtime = {
        "implementation": "cpython",
        "version": "3.12.7",
        "executable_sha256": digest("python-runtime"),
    }
    manifest = {
        "schema": MODULE.negative_runner.ARCHIVE_SCHEMA,
        "status": MODULE.negative_runner.ARCHIVE_STATUS,
        "attestation_scope": MODULE.negative_runner.ARCHIVE_ATTESTATION_SCOPE,
        "externally_authenticated": False,
        "promotion_eligible": False,
        "baseline_input_count": positive["input_count"],
        "baseline_input_set_sha256": baseline_input_set_sha256,
        "aggregate_runner_sha256": runner_sha256,
        "aggregate_checker_sha256": checker_sha256,
        "aggregate_toolchain_sha256": toolchain_sha256,
        "python_runtime": python_runtime,
        "baseline_output_sha256": {
            "aggregate_summary_sha256": positive_hashes[
                "first_aggregate_sha256"
            ],
            "replay_summary_sha256": positive_hashes[
                "second_aggregate_sha256"
            ],
            "replay_manifest_sha256": positive_hashes[
                "replay_manifest_sha256"
            ],
            "stdout_sha256": digest("positive-stdout"),
            "stderr_sha256": digest("positive-stderr"),
        },
        "mutation_count": len(MODULE.negative_runner.MUTATION_CASES),
        "mutation_ids": [
            case.mutation_id for case in MODULE.negative_runner.MUTATION_CASES
        ],
        "receipts": rows,
        "errors": [],
    }
    manifest_raw = MODULE.render_checker_summary(manifest).encode("utf-8")
    (archive / MODULE.negative_runner.ARCHIVE_MANIFEST_FILENAME).write_bytes(
        manifest_raw
    )
    return archive, manifest, manifest_raw


def write_provenance(
    path: Path,
    *,
    positive: dict[str, Any],
    negative_manifest: dict[str, Any],
    negative_manifest_raw: bytes,
    cosign_raw: bytes,
    mutate: Callable[[dict[str, Any]], None] | None = None,
    valid_signature: bool = True,
) -> dict[str, Any]:
    """Write a test-only signed final provenance receipt."""

    authentication = {
        "kind": "external-ed25519",
        "algorithm": "ed25519",
        "service_id": SIGNER_SERVICE_ID,
        "administrator_id": SIGNER_ADMINISTRATOR_ID,
        "key_revision": SIGNER_KEY_REVISION,
        "policy_revision": SIGNER_POLICY_REVISION,
        "policy_digest_sha256": SIGNER_POLICY_DIGEST,
        "public_key_fingerprint_sha256": hashlib.sha256(
            SIGNING_PUBLIC_KEY
        ).hexdigest(),
        "signature_hex": "00" * 64,
    }
    payload = {
        "schema": MODULE.PROMOTION_PROVENANCE_SCHEMA,
        "status": "verified",
        "attestation_scope": MODULE.PROMOTION_ATTESTATION_SCOPE,
        "generated_at_unix": NOW_UNIX - 60,
        "chain_id": CHAIN_ID,
        "network_id_hex": NETWORK_ID_HEX,
        "deployment_id": DEPLOYMENT_ID,
        "signing_provider": MODULE.REQUIRED_SIGNING_PROVIDER,
        "baseline_input_count": positive["input_count"],
        "baseline_input_set_sha256": negative_manifest[
            "baseline_input_set_sha256"
        ],
        "negative_archive_manifest_sha256": hashlib.sha256(
            negative_manifest_raw
        ).hexdigest(),
        "negative_receipts": negative_manifest["receipts"],
        "aggregate_runner_sha256": negative_manifest[
            "aggregate_runner_sha256"
        ],
        "aggregate_checker_sha256": negative_manifest[
            "aggregate_checker_sha256"
        ],
        "aggregate_toolchain_sha256": negative_manifest[
            "aggregate_toolchain_sha256"
        ],
        "python_runtime": negative_manifest["python_runtime"],
        "positive_output_sha256": positive["positive_output_sha256"],
        "cosign_bundle_sha256": hashlib.sha256(cosign_raw).hexdigest(),
        "provenance_certificate_identity": CERTIFICATE_IDENTITY,
        "provenance_oidc_issuer": OIDC_ISSUER,
        "oidc_identity_status": "verified",
        "cosign_provenance_status": "verified",
        "authentication": authentication,
        "errors": [],
    }
    if mutate is not None:
        mutate(payload)
    payload["authentication"]["signature_hex"] = sign(
        SIGNING_SEED,
        MODULE.promotion_provenance_signing_payload(payload),
    ).hex()
    if not valid_signature:
        signature = payload["authentication"]["signature_hex"]
        payload["authentication"]["signature_hex"] = (
            ("1" if signature[0] == "0" else "0") + signature[1:]
        )
    path.write_bytes(MODULE.render_checker_summary(payload).encode("utf-8"))
    return payload


def build_bundle(
    root: Path,
    monkeypatch: pytest.MonkeyPatch | None,
    *,
    aggregate_payload: dict[str, Any] | None = None,
    archive_input_set_sha256: str | None = None,
    provenance_mutator: Callable[[dict[str, Any]], None] | None = None,
    valid_signature: bool = True,
) -> tuple[list[str], dict[str, Path]]:
    """Build one completely synthetic but internally valid promotion bundle."""

    root.mkdir(parents=True, exist_ok=True)
    if monkeypatch is not None:
        monkeypatch.setattr(
            MODULE.promotion_runner,
            "validate_aggregate_summary_output",
            lambda payload, required_gates, errors: None,
        )
    positive_paths, positive = write_positive_replay(
        root,
        aggregate_payload=aggregate_payload,
    )
    archive, negative_manifest, negative_manifest_raw = write_negative_archive(
        root,
        positive,
        input_set_sha256=archive_input_set_sha256,
    )
    cosign = root / "promotion.sigstore.json"
    cosign_raw = MODULE.render_checker_summary(synthetic_cosign_bundle()).encode("utf-8")
    cosign.write_bytes(cosign_raw)
    provenance = root / "promotion-provenance.json"
    write_provenance(
        provenance,
        positive=positive,
        negative_manifest=negative_manifest,
        negative_manifest_raw=negative_manifest_raw,
        cosign_raw=cosign_raw,
        mutate=provenance_mutator,
        valid_signature=valid_signature,
    )
    args = [
        "--first-aggregate",
        str(positive_paths["first"]),
        "--second-aggregate",
        str(positive_paths["second"]),
        "--replay-manifest",
        str(positive_paths["manifest"]),
        "--negative-archive-dir",
        str(archive),
        "--promotion-provenance",
        str(provenance),
        "--cosign-bundle",
        str(cosign),
        "--provenance-verification-public-key-hex",
        SIGNING_PUBLIC_KEY.hex(),
        "--provenance-signer-service-id",
        SIGNER_SERVICE_ID,
        "--provenance-signer-administrator-id",
        SIGNER_ADMINISTRATOR_ID,
        "--provenance-signer-key-revision",
        str(SIGNER_KEY_REVISION),
        "--provenance-signer-policy-revision",
        str(SIGNER_POLICY_REVISION),
        "--provenance-signer-policy-digest-hex",
        SIGNER_POLICY_DIGEST,
        "--provenance-certificate-identity",
        CERTIFICATE_IDENTITY,
        "--provenance-oidc-issuer",
        OIDC_ISSUER,
        "--provenance-receipt-verifier", str(root / "native-verifier"),
        "--provenance-receipt-verifier-sha256", digest("native-verifier"),
        "--provenance-signer-policy", str(root / "policy.norito"),
        "--provenance-signer-policy-sha256", digest("policy"),
        "--provenance-custody-trust", str(root / "trust.norito"),
        "--provenance-custody-trust-sha256", digest("trust"),
        "--provenance-cosign-verifier", str(root / "cosign"),
        "--provenance-cosign-verifier-sha256", digest("cosign"),
        "--provenance-cosign-trusted-root", str(root / "sigstore-root.json"),
        "--provenance-cosign-trusted-root-sha256", digest("sigstore-root"),
        "--provenance-completed-operation-state", str(root / "state.norito"),
        "--provenance-operation-receipt", str(root / "receipt.norito"),
        "--provenance-chain-id", CHAIN_ID,
        "--provenance-network-id-hex", NETWORK_ID_HEX,
        "--provenance-deployment-id", DEPLOYMENT_ID,
        "--now-unix",
        str(NOW_UNIX),
    ]
    return args, {
        "archive": archive,
        "first": positive_paths["first"],
        "second": positive_paths["second"],
        "manifest": positive_paths["manifest"],
        "provenance": provenance,
        "cosign": cosign,
    }


def remove_options(args: list[str], *options: str) -> list[str]:
    """Remove named two-token options from a test command."""

    stripped: list[str] = []
    index = 0
    while index < len(args):
        if args[index] in options:
            index += 2
            continue
        stripped.append(args[index])
        index += 1
    return stripped


def replace_option(args: list[str], option: str, value: str) -> list[str]:
    """Replace one present two-token option in a test command."""

    updated = list(args)
    index = updated.index(option)
    updated[index + 1] = value
    return updated


def run_and_decode(args: list[str], capsys) -> tuple[int, dict[str, Any], str]:
    """Run the checker and decode its one summary object."""

    exit_code = MODULE.main(args)
    captured = capsys.readouterr()
    return exit_code, json.loads(captured.out), captured.err


def mock_cosign_verification(monkeypatch):
    """Record the adapter boundary without asserting cryptographic qualification."""

    calls = []

    def verify(arguments, subject, bundle):
        calls.append((arguments, subject, bundle))
        return []

    monkeypatch.setattr(MODULE.final_promotion_cosign, "verify_final_promotion_cosign", verify)
    return calls


def inner_approval_inputs(root: Path) -> tuple[argparse.Namespace, MODULE.PositiveReplayEvidence]:
    """Build isolated exact-byte inputs; their bodies are never production approvals."""

    values: dict[str, Any] = {
        "now_unix": NOW_UNIX,
        "inner_max_summary_artifact_age_secs": 14 * 24 * 60 * 60,
        "inner_foundational_release_sequence": 1,
        "inner_foundational_previous_envelope_sha256": "00" * 32,
    }
    digests: dict[str, str] = {}
    for slot, attribute in MODULE.INNER_APPROVAL_INPUTS:
        path = root / f"{slot}.json"
        raw = MODULE.render_checker_summary({"slot": slot}).encode("utf-8")
        path.write_bytes(raw)
        values[attribute] = path
        digests[slot] = hashlib.sha256(raw).hexdigest()
    values["inner_lane_summary"] = []
    for lane in MODULE.promotion_runner.DEFAULT_REQUIRED_GATES:
        path = root / f"lane-{lane}.json"
        raw = MODULE.render_checker_summary({"lane": lane}).encode("utf-8")
        path.write_bytes(raw)
        values["inner_lane_summary"].append(f"{lane}={path}")
        digests[lane] = hashlib.sha256(raw).hexdigest()
    trust = {
        "schema": MODULE.INNER_APPROVAL_TRUST_SCHEMA,
        "foundational_receipt_verifier_sha256": digest("reviewed-foundation-verifier"),
    }
    for index, name in enumerate(("topology", "resilience", "lane_inventory", "foundational"), start=1):
        trust[name] = {
            "public_key_hex": public_key_from_seed(bytes([index]) * 32).hex(),
            "service_id": f"sorafs-{name}-signer",
            "administrator_id": f"sorafs-{name}-administrator",
            "key_revision": index,
            "policy_revision": index + 1,
            "policy_digest_sha256": digest(f"{name}-policy"),
        }
    trust_path = root / "inner-trust.json"
    trust_raw = MODULE.render_checker_summary(trust).encode("utf-8")
    trust_path.write_bytes(trust_raw)
    values["inner_approval_trust"] = trust_path
    values["inner_approval_trust_sha256"] = hashlib.sha256(trust_raw).hexdigest()
    values["inner_foundational_receipt_verifier"] = root / "foundation-verifier"
    return argparse.Namespace(**values), MODULE.PositiveReplayEvidence(
        input_count=22,
        input_set_sha256=digest("inner-input-set"),
        input_sha256=digests,
        aggregate={"thresholds": {"max_summary_artifact_age_secs": values["inner_max_summary_artifact_age_secs"]}},
        output_sha256={},
    )


def signed_inner_approval_inputs(
    root: Path,
) -> tuple[argparse.Namespace, MODULE.PositiveReplayEvidence]:
    """Build genuine, separately signed test inputs without native completion claims."""

    deployment_id = readiness_fixture.DEPLOYMENT_ID
    environment = readiness_fixture.ENVIRONMENT
    now_unix = readiness_fixture.NOW_UNIX
    inventory_path, lane_paths, _base_topology = readiness_fixture.lane_inventory_fixture(root)
    topology_path = readiness_fixture.write_topology_qualification(root)
    rollout_support.signed_topology_cli_args(
        topology_path, deployment_id=deployment_id,
        environment=environment, now_unix=now_unix,
    )
    envelope_path = topology_path.with_name(f"{topology_path.name}.ed25519")
    topology, topology_errors = topology_qualification.load_signed_topology_qualification_binding(
        topology_path, envelope_path,
        trusted_public_key=rollout_support.TOPOLOGY_VERIFICATION_PUBLIC_KEY,
        trusted_signer_service_id=rollout_support.TOPOLOGY_SIGNER_SERVICE_ID,
        trusted_signer_administrator_id=rollout_support.TOPOLOGY_SIGNER_ADMINISTRATOR_ID,
        trusted_key_revision=rollout_support.TOPOLOGY_SIGNER_KEY_REVISION,
        trusted_policy_revision=rollout_support.TOPOLOGY_SIGNER_POLICY_REVISION,
        trusted_policy_digest_hex=rollout_support.TOPOLOGY_SIGNER_POLICY_DIGEST,
        now_unix=now_unix,
        expected_deployment_id=deployment_id,
        expected_environment=environment,
    )
    assert topology_errors == [] and topology is not None
    resilience_path, resilience_key, resilience = resilience_support.write_resilience_summary(
        aggregate_checker, root / "l1-resilience-qualification.summary",
        deployment_id=deployment_id, environment=environment,
        topology_qualification=topology,
        generated_at_unix=readiness_fixture.GENERATED_AT,
        captured_at_unix=readiness_fixture.GENERATED_AT - 1,
    )
    inventory, _raw = lane_inventory.load_canonical_inventory_file(inventory_path)
    verification = lane_inventory.verify_inventory(
        inventory, lane_paths,
        **inventory_support._trust(  # noqa: SLF001 - test support's exact reviewed tuple
            topology, deployment_id=deployment_id,
            environment=environment, now_unix=now_unix,
        ),
    )
    verified_inventory = inventory_integration.VerifiedLaneInventory(
        verification,
        {row["lane"]: row["summary_sha256"] for row in inventory["summaries"]},
    )
    foundation = readiness_fixture.foundational_summary(
        lane_summary_sha256={
            lane: hashlib.sha256(path.read_bytes()).hexdigest()
            for lane, path in lane_paths
        },
        resilience_qualification=resilience,
        l1_lane_evidence_inventory_sha256=hashlib.sha256(inventory_path.read_bytes()).hexdigest(),
    )
    foundation["topology_qualification"] = topology
    readiness_fixture.resign_foundational_summary(foundation)
    verifier_path = readiness_fixture.RECEIPT_SUPPORT.attach_bundle(foundation, root)
    foundation_path = readiness_fixture.write_json(
        root / "foundational_prerequisites.json", foundation,
    )
    foundation_row, foundation_errors, _context = MODULE.validate_foundational_prerequisite_summary(
        foundation,
        MODULE.ValidationOptions(
            now_unix=now_unix,
            max_summary_artifact_age_secs=aggregate_checker.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
            deployment_id=deployment_id,
            environment=environment,
            foundational_signer_public_key=readiness_fixture.FOUNDATIONAL_SIGNER_PUBLIC_KEY,
            foundational_release_sequence=readiness_fixture.FOUNDATIONAL_RELEASE_SEQUENCE,
            foundational_previous_envelope_sha256=readiness_fixture.FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256,
            foundational_signer_verifier=verifier_path,
            foundational_signer_verifier_sha256=hashlib.sha256(verifier_path.read_bytes()).hexdigest(),
            topology_qualification=topology,
            resilience_qualification=resilience,
            l1_lane_evidence_inventory=verified_inventory,
        ),
    )
    assert foundation_errors == []

    def signer(public_key: bytes, service_id: str, administrator_id: str,
               key_revision: int, policy_revision: int, policy_digest: str) -> dict[str, Any]:
        return {
            "public_key_hex": public_key.hex(),
            "service_id": service_id,
            "administrator_id": administrator_id,
            "key_revision": key_revision,
            "policy_revision": policy_revision,
            "policy_digest_sha256": policy_digest,
        }

    trust = {
        "schema": MODULE.INNER_APPROVAL_TRUST_SCHEMA,
        "topology": signer(
            rollout_support.TOPOLOGY_VERIFICATION_PUBLIC_KEY,
            rollout_support.TOPOLOGY_SIGNER_SERVICE_ID,
            rollout_support.TOPOLOGY_SIGNER_ADMINISTRATOR_ID,
            rollout_support.TOPOLOGY_SIGNER_KEY_REVISION,
            rollout_support.TOPOLOGY_SIGNER_POLICY_REVISION,
            rollout_support.TOPOLOGY_SIGNER_POLICY_DIGEST,
        ),
        "resilience": signer(
            resilience_key, resilience_support.SIGNER_SERVICE_ID,
            resilience_support.SIGNER_ADMINISTRATOR_ID,
            resilience_support.SIGNER_KEY_REVISION,
            resilience_support.SIGNER_POLICY_REVISION,
            resilience_support.SIGNER_POLICY_DIGEST_SHA256,
        ),
        "lane_inventory": signer(
            inventory_support.PUBLIC_KEY, inventory_support.SERVICE_ID,
            inventory_support.ADMINISTRATOR_ID,
            inventory_support.KEY_REVISION,
            inventory_support.POLICY_REVISION,
            inventory_support.POLICY_DIGEST_SHA256,
        ),
        "foundational": signer(
            readiness_fixture.FOUNDATIONAL_SIGNER_PUBLIC_KEY,
            readiness_fixture.FOUNDATIONAL_SIGNER_SERVICE_ID,
            readiness_fixture.FOUNDATIONAL_SIGNER_ADMINISTRATOR_ID,
            readiness_fixture.FOUNDATIONAL_SIGNER_KEY_REVISION,
            readiness_fixture.FOUNDATIONAL_SIGNER_POLICY_REVISION,
            readiness_fixture.FOUNDATIONAL_SIGNER_POLICY_DIGEST,
        ),
        "foundational_receipt_verifier_sha256": hashlib.sha256(
            verifier_path.read_bytes()
        ).hexdigest(),
    }
    trust_path = root / "inner-trust.json"
    trust_raw = MODULE.render_checker_summary(trust).encode("utf-8")
    trust_path.write_bytes(trust_raw)
    paths = {
        "topology_qualification": topology_path,
        "topology_qualification_envelope": envelope_path,
        "resilience_qualification": resilience_path,
        "l1_lane_evidence_inventory": inventory_path,
        "foundational_prerequisite": foundation_path,
        **dict(lane_paths),
    }
    input_sha256 = {
        slot: hashlib.sha256(path.read_bytes()).hexdigest()
        for slot, path in paths.items()
    }
    snapshot = tuple(
        (slot, input_sha256[slot])
        for slot in MODULE.promotion_runner.REPLAY_INPUT_SLOTS
    )
    aggregate = {
        "deployment": {"deployment_id": deployment_id, "environment": environment},
        "thresholds": {
            "max_summary_artifact_age_secs": aggregate_checker.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
        },
        "topology_qualification": topology,
        "resilience_qualification": {"binding": resilience},
        "l1_lane_evidence_inventory": {"binding": verification},
        "foundational_prerequisites": {
            **foundation_row,
            "path": "foundational_prerequisites.json",
            "sha256": input_sha256["foundational_prerequisite"],
        },
    }
    args = argparse.Namespace(
        now_unix=now_unix,
        inner_topology_qualification=topology_path,
        inner_topology_qualification_envelope=envelope_path,
        inner_resilience_qualification=resilience_path,
        inner_l1_lane_evidence_inventory=inventory_path,
        inner_foundational_prerequisite=foundation_path,
        inner_lane_summary=[f"{lane}={path}" for lane, path in lane_paths],
        inner_approval_trust=trust_path,
        inner_approval_trust_sha256=hashlib.sha256(trust_raw).hexdigest(),
        inner_foundational_receipt_verifier=verifier_path,
        inner_max_summary_artifact_age_secs=aggregate_checker.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
        inner_foundational_release_sequence=readiness_fixture.FOUNDATIONAL_RELEASE_SEQUENCE,
        inner_foundational_previous_envelope_sha256=(
            readiness_fixture.FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256
        ),
        provenance_verification_public_key_hex=SIGNING_PUBLIC_KEY.hex(),
        provenance_signer_administrator_id="sorafs-final-promotion-administrator",
    )
    positive = MODULE.PositiveReplayEvidence(
        input_count=len(snapshot),
        input_set_sha256=MODULE.promotion_runner.input_set_sha256(snapshot),
        input_sha256=input_sha256,
        aggregate=aggregate,
        output_sha256={},
    )
    return args, positive


def assert_missing_inner_approval_errors(errors: list[str]) -> None:
    """Require explicit missing-input failures and the unchanged final release block."""

    assert errors[-1] == MODULE.INNER_APPROVAL_RELEASE_BLOCKER
    assert "inner approvals require an independently pinned trust document" in errors
    for slot, _attribute in MODULE.INNER_APPROVAL_INPUTS:
        assert f"inner approval {slot} requires its exact replay input" in errors
    assert "inner approval requires exactly 17 ordered lane summary inputs" in errors


def test_inner_approval_gate_requires_explicit_positive_replay_and_trust() -> None:
    errors = MODULE.validate_inner_approval_chain(argparse.Namespace(), None)
    assert "inner approvals require a validated positive replay" in errors
    assert "inner approvals require an independently pinned trust document" in errors
    assert errors[-1] == MODULE.INNER_APPROVAL_RELEASE_BLOCKER
    with pytest.raises(TypeError):
        MODULE.validate_inner_approval_chain()


def test_inner_approval_dispatch_requires_all_22_exact_replay_inputs(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, positive = inner_approval_inputs(tmp_path)
    observed: list[dict[str, dict[str, Any]]] = []
    monkeypatch.setattr(
        MODULE, "_verify_inner_approval_signatures",
        lambda _args, _positive, inputs, _trust, _reviewed: observed.append(dict(inputs)) or [],
    )

    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert len(observed) == 1
    assert set(observed[0]) == {slot for slot, _attribute in MODULE.INNER_APPROVAL_INPUTS}
    assert errors == [MODULE.INNER_APPROVAL_RELEASE_BLOCKER]
    assert "inner approval chain" in errors[-1]


def test_inner_approval_verifiers_consume_snapshotted_replay_bytes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, positive = inner_approval_inputs(tmp_path)
    original = MODULE._load_inner_approval_inputs
    original_lanes = MODULE._load_inner_lane_summaries
    source = args.inner_topology_qualification
    first_lane = MODULE.promotion_runner.DEFAULT_REQUIRED_GATES[0]
    lane_source = Path(args.inner_lane_summary[0].partition("=")[2])
    captured: list[tuple[bytes, bytes]] = []

    def replace_after_read(arguments, replay, errors):
        inputs = original(arguments, replay, errors)
        source.write_bytes(b'{"slot":"replaced-after-read"}')
        return inputs

    def replace_lane_after_read(arguments, replay, errors):
        inputs = original_lanes(arguments, replay, errors)
        lane_source.write_bytes(b'{"lane":"replaced-after-read"}')
        return inputs

    def inspect_snapshots(arguments, _replay, _inputs, _trust, _reviewed):
        lane_snapshot = Path(arguments.inner_lane_summary[0].partition("=")[2])
        captured.append((
            arguments.inner_topology_qualification.read_bytes(),
            lane_snapshot.read_bytes(),
        ))
        assert arguments.inner_topology_qualification != source
        assert lane_snapshot != lane_source
        assert arguments.inner_lane_summary[0].partition("=")[0] == first_lane
        snapshot_root = arguments.inner_topology_qualification.parent.resolve(strict=True)
        source_root = MODULE.SCRIPT_DIR.parent.resolve(strict=True)
        assert snapshot_root != source_root
        assert source_root not in snapshot_root.parents
        return []

    monkeypatch.setattr(MODULE, "_load_inner_approval_inputs", replace_after_read)
    monkeypatch.setattr(MODULE, "_load_inner_lane_summaries", replace_lane_after_read)
    monkeypatch.setattr(MODULE, "_verify_inner_approval_signatures", inspect_snapshots)
    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert captured == [(
        MODULE.render_checker_summary({"slot": "topology_qualification"}).encode("utf-8"),
        MODULE.render_checker_summary({"lane": first_lane}).encode("utf-8"),
    )]
    assert errors == [MODULE.INNER_APPROVAL_RELEASE_BLOCKER]


def test_inner_approval_rejects_a_source_tree_temporary_root_without_creating_it(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, positive = inner_approval_inputs(tmp_path)
    created: list[object] = []
    monkeypatch.setattr(
        MODULE.tempfile, "gettempdir", lambda: str(MODULE.SCRIPT_DIR.parent),
    )
    monkeypatch.setattr(
        MODULE.tempfile, "TemporaryDirectory",
        lambda *_args, **_kwargs: created.append(True),
    )

    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert created == []
    assert "inner approval signature verification failed closed" in errors
    assert "inner approval chain" in errors[-1]


@pytest.mark.parametrize("failure", ["missing", "tampered", "substituted"])
def test_inner_approval_missing_tampered_or_substituted_bytes_fail_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, failure: str,
) -> None:
    args, positive = inner_approval_inputs(tmp_path)
    observed: list[object] = []
    monkeypatch.setattr(
        MODULE, "_verify_inner_approval_signatures",
        lambda *_args: observed.append(True) or [],
    )
    if failure == "missing":
        args.inner_resilience_qualification.unlink()
    elif failure == "tampered":
        args.inner_resilience_qualification.write_bytes(b'{"slot":"tampered"}')
    else:
        args.inner_resilience_qualification = args.inner_topology_qualification

    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert observed == []
    assert any("inner approval resilience_qualification" in error for error in errors)
    assert "inner approval chain" in errors[-1]


@pytest.mark.parametrize("failure", ["missing", "tampered", "reordered", "substituted"])
def test_inner_lane_replay_requires_all_ordered_exact_bytes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, failure: str,
) -> None:
    args, positive = inner_approval_inputs(tmp_path)
    observed: list[object] = []
    monkeypatch.setattr(
        MODULE, "_verify_inner_approval_signatures",
        lambda *_args: observed.append(True) or [],
    )
    first_lane = MODULE.promotion_runner.DEFAULT_REQUIRED_GATES[0]
    if failure == "missing":
        args.inner_lane_summary.pop()
    elif failure == "tampered":
        Path(args.inner_lane_summary[0].partition("=")[2]).write_bytes(b'{"lane":"tampered"}')
    elif failure == "reordered":
        args.inner_lane_summary.reverse()
    else:
        second_path = args.inner_lane_summary[1].partition("=")[2]
        args.inner_lane_summary[0] = f"{first_lane}={second_path}"

    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert observed == []
    if failure in {"missing", "reordered"}:
        assert "inner approval requires exactly 17 ordered lane summary inputs" in errors
    else:
        assert any(f"inner approval lane {first_lane}" in error for error in errors)
    assert errors[-1] == MODULE.INNER_APPROVAL_RELEASE_BLOCKER


@pytest.mark.parametrize("failure", ["missing_age", "mismatched_age", "missing_sequence", "wrong_predecessor"])
def test_inner_reviewed_policy_is_independent_and_matches_the_replay(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, failure: str,
) -> None:
    args, positive = inner_approval_inputs(tmp_path)
    observed: list[object] = []
    monkeypatch.setattr(
        MODULE, "_verify_inner_approval_signatures",
        lambda *_args: observed.append(True) or [],
    )
    if failure == "missing_age":
        args.inner_max_summary_artifact_age_secs = None
    elif failure == "mismatched_age":
        args.inner_max_summary_artifact_age_secs += 1
    elif failure == "missing_sequence":
        args.inner_foundational_release_sequence = None
    else:
        args.inner_foundational_previous_envelope_sha256 = "11" * 32

    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert observed == []
    assert any("inner approval" in error for error in errors[:-1])
    assert errors[-1] == MODULE.INNER_APPROVAL_RELEASE_BLOCKER


def test_inner_verifier_dispatches_full_lane_and_foundational_replay(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, base = inner_approval_inputs(tmp_path)
    trust = json.loads(args.inner_approval_trust.read_bytes())
    topology = {
        field: digest(field)
        for field in (
            "qualification_summary_sha256", "manifest_sha256",
            "canonical_manifest_sha256", "validator_ids_sha256",
        )
    }
    resilience = {
        f"signer_{field}": value
        for field, value in trust["resilience"].items()
        if field != "public_key_hex"
    }
    resilience["signer_public_key_fingerprint_sha256"] = hashlib.sha256(
        bytes.fromhex(trust["resilience"]["public_key_hex"])
    ).hexdigest()
    foundation_summary = {
        f"signer_{field}": value
        for field, value in trust["foundational"].items()
        if field != "public_key_hex"
    }
    foundation_summary["signer_public_key_fingerprint_sha256"] = hashlib.sha256(
        bytes.fromhex(trust["foundational"]["public_key_hex"])
    ).hexdigest()
    inventory_raw = b"canonical inventory"
    verification = {"inventory_sha256": hashlib.sha256(inventory_raw).hexdigest()}
    inventory = {
        "summaries": [
            {"lane": lane, "summary_sha256": base.input_sha256[lane]}
            for lane in MODULE.promotion_runner.DEFAULT_REQUIRED_GATES
        ],
    }
    aggregate = {
        "deployment": {"deployment_id": DEPLOYMENT_ID, "environment": "production"},
        "topology_qualification": topology,
        "resilience_qualification": {"binding": resilience},
        "l1_lane_evidence_inventory": {"binding": verification},
        "foundational_prerequisites": {
            **foundation_summary,
            "path": "foundational.json",
            "sha256": base.input_sha256["foundational_prerequisite"],
        },
    }
    positive = MODULE.PositiveReplayEvidence(
        input_count=base.input_count,
        input_set_sha256=base.input_set_sha256,
        input_sha256=base.input_sha256,
        aggregate=aggregate,
        output_sha256=base.output_sha256,
    )
    monkeypatch.setattr(
        MODULE.topology_qualification, "load_signed_topology_qualification_binding",
        lambda *_args, **_kwargs: (topology, []),
    )
    monkeypatch.setattr(
        MODULE.promotion_runner, "load_resilience_qualification_binding",
        lambda *_args, **_kwargs: (resilience, []),
    )
    monkeypatch.setattr(
        MODULE.lane_inventory, "load_canonical_inventory_file",
        lambda *_args: (inventory, inventory_raw),
    )
    lane_calls: list[tuple[tuple[str, ...], dict[str, Any]]] = []

    def replay_inventory(_inventory, specs, **kwargs):
        lane_calls.append((tuple(lane for lane, _path in specs), kwargs))
        return verification

    monkeypatch.setattr(MODULE.lane_inventory, "verify_inventory", replay_inventory)
    foundation_calls: list[aggregate_checker.ValidationOptions] = []

    def replay_foundation(_payload, options):
        foundation_calls.append(options)
        return foundation_summary, [], None

    monkeypatch.setattr(MODULE, "validate_foundational_prerequisite_summary", replay_foundation)
    errors = MODULE._verify_inner_approval_signatures(
        args, positive, {"foundational_prerequisite": {}}, trust,
        (args.inner_max_summary_artifact_age_secs, 1, "00" * 32),
    )

    assert errors == [MODULE.TOPOLOGY_NATIVE_AUTHORITY_BLOCKER]
    assert lane_calls[0][0] == MODULE.promotion_runner.DEFAULT_REQUIRED_GATES
    assert lane_calls[0][1]["expected_topology_manifest_sha256"] == topology["manifest_sha256"]
    assert lane_calls[0][1]["verification_public_key_hex"] == trust["lane_inventory"]["public_key_hex"]
    assert len(foundation_calls) == 1
    assert foundation_calls[0].foundational_release_sequence == 1
    assert foundation_calls[0].foundational_previous_envelope_sha256 == "00" * 32
    assert foundation_calls[0].l1_lane_evidence_inventory.verification == verification
    assert foundation_calls[0].topology_qualification == topology
    assert foundation_calls[0].resilience_qualification == resilience


def test_signed_inner_chain_replays_all_prerequisites_but_stays_blocked(
    tmp_path: Path,
) -> None:
    args, positive = signed_inner_approval_inputs(tmp_path)
    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert errors == [
        MODULE.TOPOLOGY_NATIVE_AUTHORITY_BLOCKER,
        MODULE.INNER_APPROVAL_RELEASE_BLOCKER,
    ]


def test_topology_envelope_cannot_claim_a_completed_native_operation(
    tmp_path: Path,
) -> None:
    args, positive = signed_inner_approval_inputs(tmp_path)
    envelope_path = args.inner_topology_qualification_envelope
    envelope = json.loads(envelope_path.read_bytes())
    envelope["completed_operation_state_sha256"] = digest("claimed-topology-completion")
    raw = MODULE.render_checker_summary(envelope).encode("utf-8")
    envelope_path.write_bytes(raw)
    positive.input_sha256["topology_qualification_envelope"] = hashlib.sha256(raw).hexdigest()

    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert any("signed topology qualification envelope fields must match" in error for error in errors)
    assert MODULE.TOPOLOGY_NATIVE_AUTHORITY_BLOCKER not in errors
    assert errors[-1] == MODULE.INNER_APPROVAL_RELEASE_BLOCKER


def test_signed_inner_chain_rejects_a_rebound_lane_summary(
    tmp_path: Path,
) -> None:
    args, positive = signed_inner_approval_inputs(tmp_path)
    lane, _separator, path_text = args.inner_lane_summary[0].partition("=")
    path = Path(path_text)
    value = json.loads(path.read_bytes())
    value["recognized_artifacts"][0]["fingerprint"]["deployment_id"] = "production-other"
    path.write_bytes(lane_inventory.canonical_file_bytes(value))
    positive.input_sha256[lane] = hashlib.sha256(path.read_bytes()).hexdigest()

    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert any("inner approval lane-inventory full replay could not be verified" in error for error in errors)
    assert errors[-1] == MODULE.INNER_APPROVAL_RELEASE_BLOCKER


def test_signed_inner_chain_rejects_re_signed_invalid_foundational_semantics(
    tmp_path: Path,
) -> None:
    args, positive = signed_inner_approval_inputs(tmp_path)
    path = args.inner_foundational_prerequisite
    foundation = json.loads(path.read_bytes())
    foundation["prerequisites"][0]["status"] = "blocked"
    readiness_fixture.resign_foundational_summary(foundation)
    path.write_bytes(lane_inventory.canonical_file_bytes(foundation))
    positive.input_sha256["foundational_prerequisite"] = hashlib.sha256(
        path.read_bytes()
    ).hexdigest()

    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert any("foundational prerequisites[0].status must be `verified`" in error for error in errors)
    assert errors[-1] == MODULE.INNER_APPROVAL_RELEASE_BLOCKER


def test_inner_approval_trust_substitution_or_signer_reuse_fails_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, positive = inner_approval_inputs(tmp_path)
    observed: list[object] = []
    monkeypatch.setattr(
        MODULE, "_verify_inner_approval_signatures",
        lambda *_args: observed.append(True) or [],
    )
    args.inner_approval_trust.write_bytes(b'{"schema":"substituted"}')
    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert any("independently reviewed SHA-256" in error for error in errors)
    assert observed == []

    args, positive = inner_approval_inputs(tmp_path)
    trust = json.loads(args.inner_approval_trust.read_bytes())
    trust["resilience"]["public_key_hex"] = trust["topology"]["public_key_hex"]
    raw = MODULE.render_checker_summary(trust).encode("utf-8")
    args.inner_approval_trust.write_bytes(raw)
    args.inner_approval_trust_sha256 = hashlib.sha256(raw).hexdigest()
    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert any("four independent signer keys" in error for error in errors)
    assert observed == []

    args, positive = inner_approval_inputs(tmp_path)
    trust = json.loads(args.inner_approval_trust.read_bytes())
    args.provenance_verification_public_key_hex = trust["topology"]["public_key_hex"]
    args.provenance_signer_administrator_id = trust["topology"]["administrator_id"]
    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert any("outer promotion key" in error for error in errors)
    assert any("outer promotion administrator" in error for error in errors)
    assert observed == []


def test_unverified_inner_approvals_never_make_promotion_ready(tmp_path: Path) -> None:
    args, positive = inner_approval_inputs(tmp_path)
    errors = MODULE.validate_inner_approval_chain(args, positive)
    assert errors
    assert any("inner approval" in error for error in errors[:-1])
    assert "inner approval chain" in errors[-1]


def test_complete_outer_receipt_verification_cannot_qualify_the_inner_chain(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    cosign_calls = mock_cosign_verification(monkeypatch)
    calls = []
    def verify_native(arguments, statement, signature, key):
        calls.append((arguments, statement, signature, key))
        return []
    monkeypatch.setattr(MODULE.final_promotion_evidence, "verify_final_promotion_receipt", verify_native)

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert "inner approval chain" in stderr
    assert len(calls) == 1
    provenance = json.loads(paths["provenance"].read_bytes())
    assert calls[0][1] == MODULE.promotion_provenance_signing_payload(provenance)
    assert calls[0][2] == bytes.fromhex(provenance["authentication"]["signature_hex"])
    assert calls[0][3] == SIGNING_PUBLIC_KEY
    assert MODULE.verify_ed25519(calls[0][3], calls[0][2], calls[0][1])
    assert len(cosign_calls) == 1
    assert cosign_calls[0][2] == paths["cosign"].read_bytes()
    assert_missing_inner_approval_errors(summary["errors"])
    assert set(summary) == MODULE.PROMOTION_SUMMARY_FIELDS
    assert summary["status"] == "blocked"
    assert summary["externally_authenticated"] is False
    assert summary["promotion_eligible"] is False
    assert "signer_qualification" not in summary
    assert summary["baseline_input_count"] == 22
    assert summary["negative_receipt_count"] == 6
    assert [row["mutation_id"] for row in summary["negative_receipts"]] == [
        case.mutation_id for case in MODULE.negative_runner.MUTATION_CASES
    ]


def test_complete_authenticated_bundle_runs_authoritative_aggregate_validator(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    aggregate_payload = schema_valid_synthetic_promotion_payload()
    assert (
        MODULE.promotion_runner.validate_aggregate_summary_output
        is aggregate_checker.validate_aggregate_summary_output
    )
    aggregate_errors: list[str] = []
    MODULE.promotion_runner.validate_aggregate_summary_output(
        aggregate_payload,
        MODULE.promotion_runner.DEFAULT_REQUIRED_GATES,
        aggregate_errors,
    )
    assert aggregate_errors == []

    args, paths = build_bundle(
        tmp_path,
        None,
        aggregate_payload=aggregate_payload,
    )
    calls = []
    mock_cosign_verification(monkeypatch)
    def verify_native(arguments, statement, signature, key):
        calls.append((arguments, statement, signature, key))
        return []
    monkeypatch.setattr(MODULE.final_promotion_evidence, "verify_final_promotion_receipt", verify_native)
    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert "inner approval chain" in stderr
    assert len(calls) == 1
    assert_missing_inner_approval_errors(summary["errors"])
    assert summary["status"] == "blocked"
    assert summary["externally_authenticated"] is False
    assert summary["promotion_eligible"] is False
    assert summary["baseline_input_count"] == 22
    assert paths["first"].read_bytes() == paths["second"].read_bytes()

    replay_manifest = json.loads(paths["manifest"].read_bytes())
    assert replay_manifest["status"] == "verified"
    assert replay_manifest["execution_count"] == 2
    assert replay_manifest["input_count"] == 22
    assert replay_manifest["first_aggregate_sha256"] == replay_manifest[
        "second_aggregate_sha256"
    ]

    negative_manifest = json.loads(
        (
            paths["archive"]
            / MODULE.negative_runner.ARCHIVE_MANIFEST_FILENAME
        ).read_bytes()
    )
    assert negative_manifest["mutation_count"] == 6
    assert negative_manifest["mutation_ids"] == [
        case.mutation_id for case in MODULE.negative_runner.MUTATION_CASES
    ]
    assert len(negative_manifest["receipts"]) == 6

    provenance = json.loads(paths["provenance"].read_bytes())
    assert provenance["signing_provider"] == MODULE.REQUIRED_SIGNING_PROVIDER
    assert "signing_backend" not in provenance
    assert "signer_qualification" not in provenance
    assert provenance["oidc_identity_status"] == "verified"
    assert provenance["cosign_provenance_status"] == "verified"
    assert provenance["authentication"]["kind"] == "external-ed25519"
    assert provenance["authentication"]["algorithm"] == "ed25519"
    assert "backend" not in provenance["authentication"]
    assert summary["cosign_bundle_sha256"] == hashlib.sha256(
        paths["cosign"].read_bytes()
    ).hexdigest()


@pytest.mark.parametrize(
    "field",
    ("summary_file_count", "recognized_summary_count"),
)
def test_float_positive_aggregate_counts_block_the_final_bundle(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
    field: str,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    payload = json.loads(paths["first"].read_text(encoding="utf-8"))
    payload[field] = 17.0
    raw = MODULE.render_checker_summary(payload).encode("utf-8")
    paths["first"].write_bytes(raw)
    paths["second"].write_bytes(raw)

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert f"replayed aggregate {field} must be 17" in stderr


@pytest.mark.parametrize(
    ("field", "substituted"),
    (
        ("input_count", 22.0),
        ("execution_count", 2.0),
        ("all_required_rows_valid", 1),
    ),
)
def test_python_equal_replay_manifest_numeric_types_block_the_final_bundle(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
    field: str,
    substituted: object,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    manifest = json.loads(paths["manifest"].read_text(encoding="utf-8"))
    manifest[field] = substituted
    paths["manifest"].write_bytes(
        MODULE.render_checker_summary(manifest).encode("utf-8")
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "must match the verified immutable inputs" in stderr


def test_boolean_receipt_exit_code_blocks_the_final_bundle(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    receipt_path = paths["archive"] / "01-tampered-lane-summary-bytes.json"
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    receipt["expected_rejection"]["checker_exit_code"] = True
    receipt_path.write_bytes(MODULE.render_checker_summary(receipt).encode("utf-8"))

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "expected rejection must match the matrix" in stderr


@pytest.mark.parametrize(
    ("field", "substituted", "diagnostic"),
    (
        ("baseline_input_count", 22.0, "baseline input count must match"),
        ("mutation_count", 6.0, "mutation count must be six"),
    ),
)
def test_float_negative_archive_counts_block_the_final_bundle(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
    field: str,
    substituted: float,
    diagnostic: str,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    manifest_path = (
        paths["archive"] / MODULE.negative_runner.ARCHIVE_MANIFEST_FILENAME
    )
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest[field] = substituted
    manifest_path.write_bytes(
        MODULE.render_checker_summary(manifest).encode("utf-8")
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert diagnostic in stderr


def test_float_signed_provenance_binding_blocks_the_final_bundle(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, _paths = build_bundle(
        tmp_path,
        monkeypatch,
        provenance_mutator=lambda payload: payload.__setitem__(
            "baseline_input_count", 22.0
        ),
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "baseline_input_count must match the verified bundle" in stderr


def test_local_archive_without_external_provenance_remains_non_promotable(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, _paths = build_bundle(tmp_path, monkeypatch)
    args = remove_options(
        args,
        "--promotion-provenance",
        "--cosign-bundle",
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["status"] == "blocked"
    assert summary["externally_authenticated"] is False
    assert summary["promotion_eligible"] is False
    assert "requires externally authenticated provenance" in stderr
    assert "requires an exact cosign bundle" in stderr


def test_receipt_byte_tamper_blocks_the_bundle(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    receipt = paths["archive"] / "01-tampered-lane-summary-bytes.json"
    receipt.write_bytes(receipt.read_bytes() + b" ")

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "must match its manifest binding" in stderr


@pytest.mark.parametrize(
    ("mutate", "diagnostic"),
    (
        (
            lambda manifest: manifest["receipts"].reverse(),
            "receipt rows must use matrix order",
        ),
        (
            lambda manifest: manifest.__setitem__("mutation_count", 5),
            "mutation count must be six",
        ),
        (
            lambda manifest: manifest["receipts"].pop(),
            "must contain six receipt rows",
        ),
    ),
)
def test_archive_receipt_order_and_count_are_revalidated(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
    mutate: Callable[[dict[str, Any]], None],
    diagnostic: str,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    manifest_path = (
        paths["archive"] / MODULE.negative_runner.ARCHIVE_MANIFEST_FILENAME
    )
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    mutate(manifest)
    manifest_path.write_bytes(
        MODULE.render_checker_summary(manifest).encode("utf-8")
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert diagnostic in stderr


def test_missing_negative_archive_blocks(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, _paths = build_bundle(tmp_path, monkeypatch)
    args = replace_option(
        args,
        "--negative-archive-dir",
        str(tmp_path / "missing-negative-archive"),
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["negative_receipt_count"] == 0
    assert summary["promotion_eligible"] is False
    assert "must be an existing directory" in stderr


@pytest.mark.parametrize("swap", ("second", "manifest", "cosign"))
def test_positive_replay_and_cosign_swaps_block(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
    swap: str,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    if swap == "second":
        second = Path(args[args.index("--second-aggregate") + 1])
        second.write_bytes(MODULE.render_checker_summary({}).encode("utf-8"))
        expected = "aggregate replay outputs must be byte-identical"
    elif swap == "manifest":
        args = replace_option(
            args,
            "--replay-manifest",
            str(paths["cosign"]),
        )
        expected = "input inventory must be an ordered digest array"
    else:
        substituted = synthetic_cosign_bundle()
        substituted["verificationMaterial"]["timestampVerificationData"]["rfc3161Timestamps"][0]["signedTimestamp"] = "c3Vi"
        paths["cosign"].write_bytes(
            MODULE.render_checker_summary(substituted).encode("utf-8")
        )
        expected = "cosign_bundle_sha256 must match the verified bundle"

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert expected in stderr


def test_positive_and_negative_input_set_mismatch_blocks_even_when_signed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, _paths = build_bundle(
        tmp_path,
        monkeypatch,
        archive_input_set_sha256=digest("different-22-input-set"),
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "input-set digests must match" in stderr


@pytest.mark.parametrize(
    ("generated_at_unix", "diagnostic"),
    (
        (
            NOW_UNIX - MODULE.DEFAULT_MAX_PROVENANCE_AGE_SECS - 1,
            "exceeds the reviewed age bound",
        ),
        (NOW_UNIX + 1, "must not be future-dated"),
    ),
)
def test_stale_or_future_provenance_blocks(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
    generated_at_unix: int,
    diagnostic: str,
) -> None:
    args, _paths = build_bundle(
        tmp_path,
        monkeypatch,
        provenance_mutator=lambda payload: payload.__setitem__(
            "generated_at_unix", generated_at_unix
        ),
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert diagnostic in stderr


@pytest.mark.parametrize(
    ("mutator", "diagnostic"),
    (
        (
            lambda payload: payload["negative_receipts"].reverse(),
            "negative_receipts must match the verified bundle",
        ),
        (
            lambda payload: payload["positive_output_sha256"].__setitem__(
                "first_aggregate_sha256", digest("substituted-positive")
            ),
            "positive_output_sha256 must match the verified bundle",
        ),
        (
            lambda payload: payload.__setitem__("signing_provider", "self_asserted"),
            "signing_provider must be `authenticated_external_signer`",
        ),
        (
            lambda payload: payload.__setitem__(
                "oidc_identity_status", "failed"
            ),
            "oidc_identity_status must be `verified`",
        ),
    ),
)
def test_signed_binding_substitutions_block(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
    mutator: Callable[[dict[str, Any]], None],
    diagnostic: str,
) -> None:
    args, _paths = build_bundle(
        tmp_path,
        monkeypatch,
        provenance_mutator=mutator,
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert diagnostic in stderr


def test_invalid_external_signature_blocks(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, _paths = build_bundle(
        tmp_path,
        monkeypatch,
        valid_signature=False,
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "signature verification failed" in stderr


@pytest.mark.parametrize(
    ("option", "replacement", "diagnostic"),
    (
        (
            "--provenance-signer-service-id",
            "sorafs-promotion-signer-c",
            "signer_service_id must match operator trust",
        ),
        (
            "--provenance-signer-administrator-id",
            "sorafs-promotion-admin-c",
            "signer_administrator_id must match operator trust",
        ),
        (
            "--provenance-verification-public-key-hex",
            public_key_from_seed(bytes.fromhex("7e" * 32)).hex(),
            "authentication key must match operator trust",
        ),
        (
            "--provenance-signer-key-revision",
            str(SIGNER_KEY_REVISION + 1),
            "signer_key_revision must match operator trust",
        ),
        (
            "--provenance-signer-policy-revision",
            str(SIGNER_POLICY_REVISION + 1),
            "signer_policy_revision must match operator trust",
        ),
        (
            "--provenance-signer-policy-digest-hex",
            "b8" * 32,
            "signer_policy_digest_sha256 must match operator trust",
        ),
    ),
)
def test_operator_signer_trust_substitution_blocks(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
    option: str,
    replacement: str,
    diagnostic: str,
) -> None:
    args, _paths = build_bundle(tmp_path, monkeypatch)
    args = replace_option(args, option, replacement)

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert diagnostic in stderr


def test_provenance_unknown_field_is_rejected_without_echoing_value(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    provenance = json.loads(paths["provenance"].read_text(encoding="utf-8"))
    provenance["raw_payload"] = "must-not-be-reported"
    paths["provenance"].write_bytes(
        MODULE.render_checker_summary(provenance).encode("utf-8")
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "schema-closed contract" in stderr
    assert "must-not-be-reported" not in stderr


def test_duplicate_provenance_key_blocks_without_echoing_value(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    raw = paths["provenance"].read_bytes()
    paths["provenance"].write_bytes(
        raw.replace(
            b"{\n",
            (
                b'{\n  "schema": "duplicate-must-not-be-reported",\n'
            ),
            1,
        )
    )

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "bounded strict JSON object" in stderr
    assert "duplicate-must-not-be-reported" not in stderr


def test_symlinked_provenance_and_hardlinked_cosign_are_rejected(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    symlink_args, symlink_paths = build_bundle(
        tmp_path / "symlink-case",
        monkeypatch,
    )
    provenance_link = tmp_path / "symlink-case" / "provenance-link.json"
    provenance_link.symlink_to(symlink_paths["provenance"].name)
    symlink_args = replace_option(
        symlink_args,
        "--promotion-provenance",
        str(provenance_link),
    )

    exit_code, summary, stderr = run_and_decode(symlink_args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "bounded strict JSON object" in stderr

    hardlink_args, hardlink_paths = build_bundle(
        tmp_path / "hardlink-case",
        monkeypatch,
    )
    cosign_hardlink = tmp_path / "hardlink-case" / "cosign-hardlink.json"
    os.link(hardlink_paths["cosign"], cosign_hardlink)
    hardlink_args = replace_option(
        hardlink_args,
        "--cosign-bundle",
        str(cosign_hardlink),
    )

    exit_code, summary, stderr = run_and_decode(hardlink_args, capsys)

    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "bounded strict JSON object" in stderr


@pytest.mark.parametrize("option", (
    "--provenance-receipt-verifier", "--provenance-receipt-verifier-sha256",
    "--provenance-signer-policy", "--provenance-signer-policy-sha256",
    "--provenance-custody-trust", "--provenance-custody-trust-sha256",
    "--provenance-completed-operation-state", "--provenance-operation-receipt",
    "--provenance-chain-id", "--provenance-network-id-hex", "--provenance-deployment-id",
    "--provenance-cosign-verifier", "--provenance-cosign-verifier-sha256",
    "--provenance-cosign-trusted-root", "--provenance-cosign-trusted-root-sha256",
))
def test_every_native_artifact_and_context_flag_is_required_before_eligibility(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys, option: str,
) -> None:
    args, _paths = build_bundle(tmp_path, monkeypatch)
    calls = []
    monkeypatch.setattr(MODULE.final_promotion_evidence, "verify_final_promotion_receipt", lambda *args: calls.append(args))
    assert MODULE.main(remove_options(args, option)) == 2
    captured = capsys.readouterr()
    assert captured.out == ""
    assert option in captured.err
    assert calls == []


@pytest.mark.parametrize(("option", "replacement", "field"), (
    ("--provenance-chain-id", "another-chain", "chain_id"),
    ("--provenance-network-id-hex", "22" * 32, "network_id_hex"),
    ("--provenance-deployment-id", "another-deployment", "deployment_id"),
))
def test_signed_context_substitution_fails_before_native_execution(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys,
    option: str, replacement: str, field: str,
) -> None:
    args, _paths = build_bundle(tmp_path, monkeypatch)
    calls = []
    monkeypatch.setattr(MODULE.final_promotion_evidence, "verify_final_promotion_receipt", lambda *args: calls.append(args))
    exit_code, summary, stderr = run_and_decode(replace_option(args, option, replacement), capsys)
    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert f"{field} must match operator trust" in stderr
    assert calls == []


@pytest.mark.parametrize("old_schema", (False, True))
@pytest.mark.parametrize("backend", ("software", "hardware"))
def test_signed_obsolete_backend_claims_are_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys, old_schema: bool, backend: str,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    payload = json.loads(paths["provenance"].read_bytes())
    payload["signing_backend"] = backend
    payload["signer_qualification"] = f"{backend}-key-qualified"
    payload["authentication"]["backend"] = backend
    if old_schema:
        for field in ("chain_id", "network_id_hex", "deployment_id"):
            del payload[field]
    # Preserve the actual retired signed preimage independently of the current closed-schema helper.
    unsigned = dict(payload)
    unsigned["authentication"] = dict(payload["authentication"])
    del unsigned["authentication"]["signature_hex"]
    message = MODULE.PROMOTION_PROVENANCE_SIGNATURE_DOMAIN + json.dumps(
        unsigned, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False,
    ).encode("ascii")
    signature = sign(SIGNING_SEED, message)
    assert MODULE.verify_ed25519(SIGNING_PUBLIC_KEY, signature, message)
    payload["authentication"]["signature_hex"] = signature.hex()
    paths["provenance"].write_bytes(MODULE.render_checker_summary(payload).encode())
    calls = []
    monkeypatch.setattr(MODULE.final_promotion_evidence, "verify_final_promotion_receipt", lambda *args: calls.append(args))
    exit_code, summary, stderr = run_and_decode(args, capsys)
    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "schema-closed contract" in stderr
    if old_schema:
        assert "schema-closed contract" in stderr
    assert calls == []


def test_native_receipt_failure_cannot_be_overridden_by_valid_outer_signature(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys,
) -> None:
    args, _paths = build_bundle(tmp_path, monkeypatch)
    calls = []
    def reject(arguments, statement, signature, key):
        calls.append((arguments, statement, signature, key))
        return ["native final promotion receipt failed exact current custody verification"]
    monkeypatch.setattr(MODULE.final_promotion_evidence, "verify_final_promotion_receipt", reject)
    exit_code, summary, stderr = run_and_decode(args, capsys)
    assert exit_code == 1
    assert len(calls) == 1
    assert summary["promotion_eligible"] is False
    assert "failed exact current custody" in stderr
    assert "inner approval chain" in stderr


def test_relabelled_inner_aggregate_cannot_supply_missing_custody_proofs(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys,
) -> None:
    payload = schema_valid_synthetic_promotion_payload()
    def relabel(value):
        if isinstance(value, dict):
            return {key: relabel(item) for key, item in value.items()}
        if isinstance(value, list):
            return [relabel(item) for item in value]
        if value == "software":
            return "hardware"
        if value == "software-key-qualified":
            return "hardware-key-qualified"
        return value
    # Isolate the final gate even if an upstream aggregate validator were fooled by relabeling.
    args, _paths = build_bundle(tmp_path, monkeypatch, aggregate_payload=relabel(payload))
    mock_cosign_verification(monkeypatch)
    calls = []
    def verify_native(*args):
        calls.append(args)
        return []
    monkeypatch.setattr(MODULE.final_promotion_evidence, "verify_final_promotion_receipt", verify_native)
    exit_code, summary, stderr = run_and_decode(args, capsys)
    assert exit_code == 1
    assert len(calls) == 1
    assert summary["status"] == "blocked"
    assert summary["promotion_eligible"] is False
    assert_missing_inner_approval_errors(summary["errors"])
    for owner in ("foundational", "topology", "resilience", "lane-inventory"):
        assert owner in stderr


def test_arbitrary_cosign_json_stays_blocked_after_other_verification_boundaries(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    arbitrary_bundle = json.dumps(synthetic_cosign_bundle()).encode()
    paths["cosign"].write_bytes(arbitrary_bundle)
    provenance = json.loads(paths["provenance"].read_bytes())
    provenance["cosign_bundle_sha256"] = hashlib.sha256(arbitrary_bundle).hexdigest()
    statement = MODULE.promotion_provenance_signing_payload(provenance)
    signature = sign(SIGNING_SEED, statement)
    provenance["authentication"]["signature_hex"] = signature.hex()
    paths["provenance"].write_bytes(MODULE.render_checker_summary(provenance).encode())
    calls = []

    def verify_native(arguments, actual_statement, actual_signature, key):
        assert actual_statement == statement
        assert actual_signature == signature
        assert MODULE.verify_ed25519(key, actual_signature, actual_statement)
        calls.append(arguments)
        return []

    # Scope this regression to the separate cosign gate; these mocks supply no production proof.
    monkeypatch.setattr(
        MODULE.final_promotion_evidence, "verify_final_promotion_receipt", verify_native,
    )
    monkeypatch.setattr(MODULE, "validate_inner_approval_chain", lambda *_args: [])
    cosign_calls = []
    def reject_cosign(arguments, subject, bundle):
        assert bundle == arbitrary_bundle
        cosign_calls.append((arguments, subject, bundle))
        return [MODULE.final_promotion_cosign.FAILURE]
    monkeypatch.setattr(MODULE.final_promotion_cosign, "verify_final_promotion_cosign", reject_cosign)
    exit_code, summary, stderr = run_and_decode(args, capsys)
    assert exit_code == 1
    assert len(calls) == 1
    assert len(cosign_calls) == 1
    assert summary["status"] == "blocked"
    assert summary["externally_authenticated"] is False
    assert summary["promotion_eligible"] is False
    assert "signer_qualification" not in summary
    assert summary["errors"] == [MODULE.final_promotion_cosign.FAILURE]
    assert "cosign verification failed for the exact subject and independent trust" in stderr


def test_python_signing_preimage_matches_independent_rust_statement_golden() -> None:
    golden = (SCRIPT_DIR.parent / "crates/sorafs_manifest/src/signer/final_promotion/tests/statement_fixture.message").read_bytes()
    unsigned = json.loads(golden.removeprefix(MODULE.PROMOTION_PROVENANCE_SIGNATURE_DOMAIN))
    assert len(unsigned) == 24
    assert len(unsigned["authentication"]) == 8
    unsigned["authentication"]["signature_hex"] = "01" * 64
    assert MODULE.promotion_provenance_signing_payload(unsigned) == golden
    unsigned["authentication"]["unrecognized"] = "not-a-wire-field"
    with pytest.raises(ValueError, match="wrong exact schema"):
        MODULE.promotion_provenance_signing_payload(unsigned)


def unsigned_cosign_fixture() -> dict[str, Any]:
    """Project the independently stored Rust statement into the initial cosign input."""

    golden = (SCRIPT_DIR.parent / "crates/sorafs_manifest/src/signer/final_promotion/tests/statement_fixture.message").read_bytes()
    unsigned = json.loads(golden.removeprefix(MODULE.PROMOTION_PROVENANCE_SIGNATURE_DOMAIN))
    del unsigned["cosign_bundle_sha256"]
    return unsigned


def test_cosign_subject_matches_exact_non_circular_golden_projection() -> None:
    golden = (SCRIPT_DIR.parent / "crates/sorafs_manifest/src/signer/final_promotion/tests/statement_fixture.message").read_bytes()
    raw_body = golden.removeprefix(MODULE.PROMOTION_PROVENANCE_SIGNATURE_DOMAIN)
    projected, count = re.subn(br'"cosign_bundle_sha256":"[0-9a-f]{64}",', b"", raw_body)
    assert count == 1
    unsigned = unsigned_cosign_fixture()
    original = copy.deepcopy(unsigned)
    actual = MODULE.promotion_cosign_subject_bytes(unsigned)
    assert actual == MODULE.PROMOTION_COSIGN_SUBJECT_DOMAIN + projected
    assert hashlib.sha256(actual).hexdigest() == "0f8eeebca94bc78ee583c8e270b62d4adeb94945cf2b9abf5dd7098b7aac2bf3"
    assert unsigned == original
    assert len(unsigned) == 23
    assert len(unsigned["authentication"]) == 8
    assert b'"cosign_bundle_sha256"' not in actual
    assert b'"signature_hex"' not in actual


@pytest.mark.parametrize("field", sorted(MODULE.PROMOTION_COSIGN_SUBJECT_FIELDS - {"authentication"}))
def test_cosign_subject_binds_every_unsigned_root_field(field: str) -> None:
    unsigned = unsigned_cosign_fixture()
    baseline = MODULE.promotion_cosign_subject_bytes(unsigned)
    unsigned[field] = {"changed-bound-value": unsigned[field]}
    assert MODULE.promotion_cosign_subject_bytes(unsigned) != baseline


@pytest.mark.parametrize("field", sorted(MODULE.AUTHENTICATION_FIELDS - {"signature_hex"}))
def test_cosign_subject_binds_each_authentication_field(field: str) -> None:
    unsigned = unsigned_cosign_fixture()
    baseline = MODULE.promotion_cosign_subject_bytes(unsigned)
    unsigned["authentication"][field] = {"changed-bound-value": unsigned["authentication"][field]}
    assert MODULE.promotion_cosign_subject_bytes(unsigned) != baseline


@pytest.mark.parametrize("mutation", ("extra_root", "missing_root", "bundle_hash", "signature", "extra_auth", "missing_auth", "wrong_auth", "nonfinite", "oversized"))
def test_cosign_subject_rejects_noncanonical_shape_or_encoding(mutation: str) -> None:
    unsigned = unsigned_cosign_fixture()
    if mutation == "extra_root": unsigned["extra"] = "no wire aliases"
    elif mutation == "missing_root": del unsigned["chain_id"]
    elif mutation == "bundle_hash": unsigned["cosign_bundle_sha256"] = "00" * 32
    elif mutation == "signature": unsigned["authentication"]["signature_hex"] = "00" * 64
    elif mutation == "extra_auth": unsigned["authentication"]["extra"] = "unit"
    elif mutation == "missing_auth": del unsigned["authentication"]["kind"]
    elif mutation == "wrong_auth": unsigned["authentication"] = []
    elif mutation == "nonfinite": unsigned["generated_at_unix"] = float("nan")
    elif mutation == "oversized": unsigned["chain_id"] = "x" * (256 * 1024)
    with pytest.raises(ValueError):
        MODULE.promotion_cosign_subject_bytes(unsigned)


def test_cosign_uses_once_captured_bundle_when_source_is_replaced_after_native_verification(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    original = paths["cosign"].read_bytes()
    provenance = json.loads(paths["provenance"].read_bytes())
    projected = MODULE._unsigned_promotion_body(provenance)
    del projected["cosign_bundle_sha256"]
    expected_subject = MODULE.promotion_cosign_subject_bytes(projected)
    def verify_native(arguments, statement, signature, key):
        assert MODULE.verify_ed25519(key, signature, statement)
        paths["cosign"].write_bytes(b"replacement candidate must not be read")
        return []
    monkeypatch.setattr(MODULE.final_promotion_evidence, "verify_final_promotion_receipt", verify_native)
    calls = mock_cosign_verification(monkeypatch)
    exit_code, summary, _ = run_and_decode(args, capsys)
    assert exit_code == 1
    assert len(calls) == 1
    assert calls[0][1:] == (expected_subject, original)
    assert summary["cosign_bundle_sha256"] == hashlib.sha256(original).hexdigest()
    assert_missing_inner_approval_errors(summary["errors"])
    assert summary["promotion_eligible"] is False


@pytest.mark.parametrize("mutation", ("old_media_type", "dsse", "unknown", "managed_key", "old_log"))
def test_invalid_cosign_profile_blocks_before_native_or_cosign_execution(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys, mutation: str,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    payload = synthetic_cosign_bundle()
    if mutation == "old_media_type": payload["mediaType"] = "application/vnd.dev.sigstore.bundle+json;version=0.3"
    elif mutation == "dsse": payload["dsseEnvelope"] = payload.pop("messageSignature")
    elif mutation == "unknown": payload["unknown"] = "unit"
    elif mutation == "managed_key": payload["verificationMaterial"]["publicKey"] = payload["verificationMaterial"].pop("certificate")
    elif mutation == "old_log": payload["verificationMaterial"]["tlogEntries"][0]["kindVersion"]["version"] = "0.0.1"
    paths["cosign"].write_bytes(json.dumps(payload).encode())
    native_calls = []
    monkeypatch.setattr(MODULE.final_promotion_evidence, "verify_final_promotion_receipt", lambda *args: native_calls.append(args))
    cosign_calls = mock_cosign_verification(monkeypatch)
    exit_code, summary, stderr = run_and_decode(args, capsys)
    assert exit_code == 1
    assert summary["promotion_eligible"] is False
    assert "Sigstore v0.3 format" in stderr
    assert native_calls == []
    assert cosign_calls == []


def test_invalid_x509_version_returns_closed_summary_before_either_verifier(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys,
) -> None:
    args, paths = build_bundle(tmp_path, monkeypatch)
    payload = synthetic_cosign_bundle()
    certificate = payload["verificationMaterial"]["certificate"]
    raw = base64.b64decode(certificate["rawBytes"])
    original = b"\xa0\x03\x02\x01\x02"
    assert original in raw
    certificate["rawBytes"] = base64.b64encode(raw.replace(original, original[:-1] + b"\x03", 1)).decode()
    paths["cosign"].write_bytes(json.dumps(payload).encode())
    native_calls = []
    monkeypatch.setattr(MODULE.final_promotion_evidence, "verify_final_promotion_receipt", lambda *args: native_calls.append(args))
    cosign_calls = mock_cosign_verification(monkeypatch)
    exit_code, summary, stderr = run_and_decode(args, capsys)
    assert exit_code == 1
    assert summary["status"] == "blocked"
    assert summary["promotion_eligible"] is False
    assert "Sigstore v0.3 format" in stderr
    assert "InvalidVersion" not in stderr
    assert set(summary) == MODULE.PROMOTION_SUMMARY_FIELDS
    assert native_calls == []
    assert cosign_calls == []


@pytest.mark.parametrize(("field", "invalid"), (
    ("service_id", "promotion/service"), ("service_id", "attestation-test"),
    ("administrator_id", "é"), ("key_revision", True), ("key_revision", 1.0),
    ("policy_revision", 0), ("policy_revision", 1 << 64),
    ("policy_digest_sha256", "00" * 32), ("policy_digest_sha256", "AB" * 32),
))
def test_signer_coordinates_use_exact_manifest_grammar(field: str, invalid: Any) -> None:
    row = {
        "service_id": SIGNER_SERVICE_ID,
        "administrator_id": SIGNER_ADMINISTRATOR_ID, "key_revision": SIGNER_KEY_REVISION,
        "policy_revision": SIGNER_POLICY_REVISION, "policy_digest_sha256": SIGNER_POLICY_DIGEST,
    }
    errors = []
    MODULE._validate_signer(row, errors)
    assert errors == []
    row[field] = invalid
    MODULE._validate_signer(row, errors)
    assert errors


def test_chain_identity_and_revision_boundaries_are_exact() -> None:
    for accepted in ("a", "CHAIN.a_b:c-1", "a" * 128):
        assert MODULE._canonical_chain_id(accepted) == accepted
    for invalid in ("", "-chain", "chain_", "x" * 129, "é", "a b", "a\0b"):
        assert MODULE._canonical_chain_id(invalid) is None
    row, errors = MODULE._validate_operator_signer_tuple(
        service_id=SIGNER_SERVICE_ID, administrator_id=SIGNER_ADMINISTRATOR_ID,
        key_revision=(1 << 64) - 1, policy_revision=(1 << 64) - 1,
        policy_digest_sha256=SIGNER_POLICY_DIGEST,
    )
    assert errors == []
    assert "signer_backend" not in row
    _, errors = MODULE._validate_operator_signer_tuple(
        service_id=SIGNER_SERVICE_ID, administrator_id=SIGNER_SERVICE_ID,
        key_revision=1, policy_revision=1, policy_digest_sha256=SIGNER_POLICY_DIGEST,
    )
    assert "must differ" in errors[0]
