"""Tests for the final read-only SoraFS production-promotion verifier."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
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
import sorafs_l1_lane_evidence_inventory as lane_inventory  # noqa: E402
import sorafs_l1_lane_inventory_integration as inventory_integration  # noqa: E402
import sorafs_topology_qualification as topology_qualification  # noqa: E402
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


def digest(label: str) -> str:
    """Return one deterministic non-zero test digest."""

    return hashlib.sha256(label.encode("ascii")).hexdigest()


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
        "signer_backend": "software",
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
        "signer_backend": "software",
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
        "signer_qualification": "software-key-qualified",
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
            "backend": "software",
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
        "signer_backend": "software",
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
        "signer_qualification": "software-key-qualified",
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
        "backend": "software",
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
        "signing_provider": MODULE.REQUIRED_SIGNING_PROVIDER,
        "signing_backend": MODULE.REQUIRED_SIGNING_BACKEND,
        "signer_qualification": MODULE.REQUIRED_SIGNER_QUALIFICATION,
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
    cosign_raw = MODULE.render_checker_summary(
        {
            "mediaType": "application/vnd.dev.sigstore.bundle+json;version=0.3",
            "verificationMaterial": {"certificate": "public-unit-material"},
        }
    ).encode("utf-8")
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


def test_complete_authenticated_bundle_is_the_only_ready_result(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    args, _paths = build_bundle(tmp_path, monkeypatch)

    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 0
    assert stderr == ""
    assert set(summary) == MODULE.PROMOTION_SUMMARY_FIELDS
    assert summary["status"] == "ready"
    assert summary["externally_authenticated"] is True
    assert summary["promotion_eligible"] is True
    assert summary["signer_qualification"] == "software-key-qualified"
    assert summary["baseline_input_count"] == 22
    assert summary["negative_receipt_count"] == 6
    assert [row["mutation_id"] for row in summary["negative_receipts"]] == [
        case.mutation_id for case in MODULE.negative_runner.MUTATION_CASES
    ]


def test_complete_authenticated_bundle_runs_authoritative_aggregate_validator(
    tmp_path: Path,
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
    exit_code, summary, stderr = run_and_decode(args, capsys)

    assert exit_code == 0
    assert stderr == ""
    assert summary["status"] == "ready"
    assert summary["externally_authenticated"] is True
    assert summary["promotion_eligible"] is True
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
    assert provenance["signing_backend"] == "software"
    assert provenance["signer_qualification"] == "software-key-qualified"
    assert provenance["oidc_identity_status"] == "verified"
    assert provenance["cosign_provenance_status"] == "verified"
    assert provenance["authentication"]["kind"] == "external-ed25519"
    assert provenance["authentication"]["algorithm"] == "ed25519"
    assert provenance["authentication"]["backend"] == "software"
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
        paths["cosign"].write_bytes(
            MODULE.render_checker_summary(
                {"mediaType": "substituted-sigstore-bundle"}
            ).encode("utf-8")
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
            lambda payload: (
                payload.__setitem__("signing_backend", "hsm"),
                payload["authentication"].__setitem__("backend", "hsm"),
            ),
            "signing_backend must be `software`",
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
