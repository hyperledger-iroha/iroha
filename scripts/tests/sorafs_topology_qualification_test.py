"""Tests for independently signed SoraFS topology qualification envelopes."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any

import pytest


SCRIPT_DIR = Path(__file__).resolve().parents[1]
TEST_DIR = Path(__file__).resolve().parent
for import_root in (SCRIPT_DIR, TEST_DIR):
    if str(import_root) not in sys.path:
        sys.path.insert(0, str(import_root))

import sorafs_topology_qualification as TOPOLOGY  # noqa: E402
from sorafs_resilience_test_support import (  # noqa: E402
    public_key_from_seed,
    sign,
)


NOW_UNIX = 1_800_900_000
MAX_REVIEW_AGE_SECS = 3_600
REVIEWED_AT_UNIX = NOW_UNIX - 60
DEPLOYMENT_ID = "sorafs-mainnet-2026-07"
ENVIRONMENT = "production"
SIGNER_SERVICE_ID = "sorafs-topology-signer-a"
SIGNER_ADMINISTRATOR_ID = "sorafs-topology-admin-b"
SIGNER_KEY_REVISION = 7
SIGNER_POLICY_REVISION = 11
SIGNER_POLICY_DIGEST = hashlib.sha256(
    b"sorafs-topology-qualification-policy-v1"
).hexdigest()
SIGNING_SEED = hashlib.sha256(b"sorafs-topology-qualification-test-key").digest()
PUBLIC_KEY = public_key_from_seed(SIGNING_SEED)


def write_json(path: Path, payload: dict[str, Any]) -> None:
    """Write deterministic strict JSON used by exact-digest tests."""

    path.write_text(
        json.dumps(
            payload,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
            allow_nan=False,
        )
        + "\n",
        encoding="utf-8",
    )


def qualification_summary() -> dict[str, Any]:
    """Return one valid non-promotable four-validator topology summary."""

    return {
        "schema": TOPOLOGY.SUMMARY_SCHEMA,
        "status": "configuration-qualified",
        "qualification_scope": "pre-deployment-configuration",
        "live_evidence_recognized": False,
        "promotion_eligible": False,
        "manifest_sha256": hashlib.sha256(b"exact-topology-manifest").hexdigest(),
        "canonical_manifest_sha256": hashlib.sha256(
            b"canonical-topology-manifest"
        ).hexdigest(),
        "deployment": {
            "deployment_id": DEPLOYMENT_ID,
            "environment": ENVIRONMENT,
            "network": TOPOLOGY.taira_constants.NETWORK_NAME,
            "chain_id": TOPOLOGY.taira_constants.CHAIN_ID,
            "chain_discriminant": TOPOLOGY.taira_constants.CHAIN_DISCRIMINANT,
        },
        "validator_count": 4, "validator_ids": ["taira-validator-1", "taira-validator-2", "taira-validator-3", "taira-validator-4"],
        "storage_provider_count": 2,
        "gateway_count": 2,
        "governance_dag_instance_count": 2,
        "runtime_handle_kinds": [
            "monitoring",
            "external_signer",
            "kms",
            "webauthn",
        ],
        "runtime_material_policy_valid": True,
        "signed_model_artifact_count": 1,
        "required_lane_slots": list(TOPOLOGY.CANONICAL_READINESS_LANES),
        "recognized_lane_slot_count": len(TOPOLOGY.CANONICAL_READINESS_LANES),
        "errors": [],
    }


def sign_envelope(
    envelope: dict[str, Any],
    *,
    signing_bytes: bytes | None = None,
) -> None:
    """Attach a deterministic test-only signature to an envelope."""

    message = (
        TOPOLOGY.topology_qualification_envelope_signing_bytes(envelope)
        if signing_bytes is None
        else signing_bytes
    )
    envelope["signature_hex"] = sign(SIGNING_SEED, message).hex()


def signed_fixture(
    tmp_path: Path,
    *,
    reviewed_at_unix: int = REVIEWED_AT_UNIX,
) -> tuple[Path, Path, dict[str, str], dict[str, Any]]:
    """Write a valid summary and its independently signed companion."""

    summary_path = tmp_path / "topology-summary.json"
    envelope_path = tmp_path / "topology-summary.ed25519.json"
    write_json(summary_path, qualification_summary())
    binding, errors = TOPOLOGY.load_topology_qualification_binding(
        summary_path,
        expected_deployment_id=DEPLOYMENT_ID,
        expected_environment=ENVIRONMENT,
    )
    assert errors == []
    assert binding is not None
    envelope: dict[str, Any] = {
        "schema": TOPOLOGY.SIGNED_QUALIFICATION_ENVELOPE_SCHEMA,
        **binding,
        "signer_authentication_kind": "external-ed25519",
        "signer_backend": "software",
        "signer_service_id": SIGNER_SERVICE_ID,
        "signer_administrator_id": SIGNER_ADMINISTRATOR_ID,
        "signer_key_revision": SIGNER_KEY_REVISION,
        "signer_policy_revision": SIGNER_POLICY_REVISION,
        "signer_public_key_fingerprint_sha256": hashlib.sha256(PUBLIC_KEY).hexdigest(),
        "signer_policy_digest_sha256": SIGNER_POLICY_DIGEST,
        "reviewed_at_unix": reviewed_at_unix,
        "signature_algorithm": "ed25519",
        "signature_hex": "00" * 64,
    }
    sign_envelope(envelope)
    write_json(envelope_path, envelope)
    authenticated_binding = {
        field: envelope[field]
        for field in TOPOLOGY.AUTHENTICATED_TOPOLOGY_BINDING_FIELDS
    }
    return summary_path, envelope_path, authenticated_binding, envelope


def verify(
    summary_path: Path,
    envelope_path: Path,
    **overrides: Any,
) -> tuple[dict[str, str] | None, list[str]]:
    """Invoke the signed loader with the trusted topology signer context."""

    arguments: dict[str, Any] = {
        "trusted_public_key": PUBLIC_KEY,
        "trusted_signer_service_id": SIGNER_SERVICE_ID,
        "trusted_signer_administrator_id": SIGNER_ADMINISTRATOR_ID,
        "trusted_key_revision": SIGNER_KEY_REVISION,
        "trusted_policy_revision": SIGNER_POLICY_REVISION,
        "trusted_policy_digest_hex": SIGNER_POLICY_DIGEST,
        "now_unix": NOW_UNIX,
        "max_review_age_secs": MAX_REVIEW_AGE_SECS,
        "expected_deployment_id": DEPLOYMENT_ID,
        "expected_environment": ENVIRONMENT,
    }
    arguments.update(overrides)
    return TOPOLOGY.load_signed_topology_qualification_binding(
        summary_path,
        envelope_path,
        **arguments,
    )


def test_signed_envelope_authenticates_exact_unsigned_binding(tmp_path: Path) -> None:
    """The additive signed API returns the existing exact binding shape."""

    summary_path, envelope_path, expected, envelope = signed_fixture(tmp_path)

    unsigned, unsigned_errors = TOPOLOGY.load_topology_qualification_binding(
        summary_path,
        expected_deployment_id=DEPLOYMENT_ID,
        expected_environment=ENVIRONMENT,
    )
    authenticated, errors = verify(summary_path, envelope_path)

    assert unsigned_errors == []
    assert unsigned == {field: expected[field] for field in TOPOLOGY.TOPOLOGY_BINDING_FIELDS}
    assert errors == []
    assert authenticated == expected
    assert authenticated["validator_ids_sha256"] == (
        TOPOLOGY.CANONICAL_TAIRA_VALIDATOR_IDS_SHA256
    )
    signing_bytes = TOPOLOGY.topology_qualification_envelope_signing_bytes(envelope)
    assert signing_bytes.startswith(TOPOLOGY.TOPOLOGY_QUALIFICATION_SIGNATURE_DOMAIN)
    assert b'"signer_backend":"software"' in signing_bytes
    assert b'"signer_administrator_id":"sorafs-topology-admin-b"' in signing_bytes
    assert b'"signature_algorithm":"ed25519"' in signing_bytes
    assert b"signature_hex" not in signing_bytes


@pytest.mark.parametrize(
    "missing_field", ["signature_hex", "signer_service_id", "signer_backend"]
)
def test_unsigned_or_incomplete_envelope_fails_closed(
    tmp_path: Path,
    missing_field: str,
) -> None:
    """No alias or unsigned companion can satisfy the schema-closed API."""

    summary_path, envelope_path, _binding, envelope = signed_fixture(tmp_path)
    del envelope[missing_field]
    write_json(envelope_path, envelope)

    authenticated, errors = verify(summary_path, envelope_path)

    assert authenticated is None
    assert any("schema-closed contract" in error for error in errors)


def test_exact_summary_bytes_are_bound(tmp_path: Path) -> None:
    """Semantically equivalent summary whitespace still changes the binding."""

    summary_path, envelope_path, _binding, _envelope = signed_fixture(tmp_path)
    summary_path.write_bytes(summary_path.read_bytes() + b"\n")

    authenticated, errors = verify(summary_path, envelope_path)

    assert authenticated is None
    assert any(
        "qualification_summary_sha256 must match the exact qualification binding"
        in error
        for error in errors
    )


@pytest.mark.parametrize("backend", ["local", "hsm", "pkcs11", "hardware"])
def test_non_software_signer_backend_is_rejected(
    tmp_path: Path,
    backend: str,
) -> None:
    """The signed topology policy accepts only the revised software backend."""

    summary_path, envelope_path, _binding, envelope = signed_fixture(tmp_path)
    envelope["signer_backend"] = backend
    sign_envelope(envelope)
    write_json(envelope_path, envelope)

    authenticated, errors = verify(summary_path, envelope_path)

    assert authenticated is None
    assert any("signer backend must be `software`" in error for error in errors)
    assert not any("signature must authenticate" in error for error in errors)


def test_same_signer_service_and_administrator_is_rejected(tmp_path: Path) -> None:
    summary_path, envelope_path, _binding, envelope = signed_fixture(tmp_path)
    envelope["signer_administrator_id"] = envelope["signer_service_id"]
    sign_envelope(envelope)
    write_json(envelope_path, envelope)

    authenticated, errors = verify(summary_path, envelope_path)

    assert authenticated is None
    assert any("service_id and administrator_id must differ" in error for error in errors)


def test_topology_key_must_not_overlap_an_independent_signer(tmp_path: Path) -> None:
    summary_path, envelope_path, _binding, _envelope = signed_fixture(tmp_path)

    authenticated, errors = verify(
        summary_path,
        envelope_path,
        independent_public_keys={"resilience signer key": PUBLIC_KEY},
    )

    assert authenticated is None
    assert "trusted topology public key must differ from resilience signer key" in errors


def test_topology_administrator_must_not_overlap_an_independent_signer(
    tmp_path: Path,
) -> None:
    summary_path, envelope_path, _binding, _envelope = signed_fixture(tmp_path)

    authenticated, errors = verify(
        summary_path,
        envelope_path,
        independent_administrator_ids={
            "resilience signer administrator": SIGNER_ADMINISTRATOR_ID
        },
    )

    assert authenticated is None
    assert (
        "trusted topology administrator must differ from resilience signer administrator"
        in errors
    )


def test_topology_domain_comparator_rejects_admin_and_key_overlap(
    tmp_path: Path,
) -> None:
    _summary, _envelope, binding, _payload = signed_fixture(tmp_path)
    peer = {
        "signer_administrator_id": binding["signer_administrator_id"],
        "signer_public_key_fingerprint_sha256": binding[
            "signer_public_key_fingerprint_sha256"
        ],
    }

    errors = TOPOLOGY.validate_independent_topology_signer_domains(
        binding, ("resilience signer", peer)
    )

    assert "topology signer administrator must differ from resilience signer" in errors
    assert "topology signer public key must differ from resilience signer" in errors


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("manifest_sha256", hashlib.sha256(b"other-manifest").hexdigest()),
        (
            "canonical_manifest_sha256",
            hashlib.sha256(b"other-canonical-manifest").hexdigest(),
        ),
        ("deployment_id", "other-production-deployment"),
        ("environment", "prod"),
        ("network", "minamoto"),
        ("chain_id", "00000000-0000-0000-0000-000000000000"),
        ("chain_discriminant", 0),
        ("validator_ids_sha256", hashlib.sha256(b"other-roster").hexdigest()),
    ],
)
def test_authenticated_envelope_cannot_bind_another_topology(
    tmp_path: Path,
    field: str,
    replacement: Any,
) -> None:
    """A valid signature over the wrong topology tuple remains unacceptable."""

    summary_path, envelope_path, _binding, envelope = signed_fixture(tmp_path)
    envelope[field] = replacement
    sign_envelope(envelope)
    write_json(envelope_path, envelope)

    authenticated, errors = verify(summary_path, envelope_path)

    assert authenticated is None
    assert any(
        f"{field} must match the exact qualification binding" in error
        for error in errors
    )
    assert not any("signature must authenticate" in error for error in errors)


def test_minamoto_summary_is_rejected_before_signature_review(tmp_path: Path) -> None:
    """A mainnet label cannot enter the Taira qualification binding."""

    payload = qualification_summary()
    payload["deployment"]["network"] = "minamoto"
    summary_path = tmp_path / "minamoto-topology-summary.json"
    write_json(summary_path, payload)

    binding, errors = TOPOLOGY.load_topology_qualification_binding(summary_path)

    assert binding is None
    assert any("Minamoto evidence is not accepted" in error for error in errors)


def test_noncanonical_validator_roster_is_rejected_before_signature_review(
    tmp_path: Path,
) -> None:
    """The summary cannot replace or reorder the exact four Taira validators."""

    payload = qualification_summary()
    payload["validator_ids"] = list(reversed(payload["validator_ids"]))
    summary_path = tmp_path / "wrong-validator-roster.json"
    write_json(summary_path, payload)

    binding, errors = TOPOLOGY.load_topology_qualification_binding(summary_path)

    assert binding is None
    assert any("canonical ordered Taira validator identities" in error for error in errors)


@pytest.mark.parametrize(
    ("override", "error_fragment"),
    [
        (
            {"trusted_signer_service_id": "another-topology-signer"},
            "signer_service_id must match the trusted external software signer",
        ),
        (
            {"trusted_signer_administrator_id": "another-topology-admin"},
            "signer_administrator_id must match the trusted external software signer",
        ),
        (
            {"trusted_key_revision": SIGNER_KEY_REVISION + 1},
            "signer_key_revision must match the trusted external software signer",
        ),
        (
            {"trusted_policy_revision": SIGNER_POLICY_REVISION + 1},
            "signer_policy_revision must match the trusted external software signer",
        ),
        (
            {
                "trusted_policy_digest_hex": hashlib.sha256(
                    b"another-topology-policy"
                ).hexdigest()
            },
            "signer_policy_digest_sha256 must match the trusted external software signer",
        ),
        (
            {
                "trusted_public_key": public_key_from_seed(
                    hashlib.sha256(b"another-topology-key").digest()
                )
            },
            "signer public-key fingerprint must match the trusted public key",
        ),
    ],
)
def test_substituted_trust_context_is_rejected(
    tmp_path: Path,
    override: dict[str, Any],
    error_fragment: str,
) -> None:
    """The signer identity, revision, policy, and key are independent anchors."""

    summary_path, envelope_path, _binding, _envelope = signed_fixture(tmp_path)

    authenticated, errors = verify(summary_path, envelope_path, **override)

    assert authenticated is None
    assert any(error_fragment in error for error in errors)


@pytest.mark.parametrize(
    ("reviewed_at_unix", "error_fragment"),
    [
        (
            NOW_UNIX - MAX_REVIEW_AGE_SECS - 1,
            "review exceeds the maximum age",
        ),
        (NOW_UNIX + 1, "reviewed_at_unix must not be in the future"),
    ],
)
def test_stale_or_future_review_is_rejected(
    tmp_path: Path,
    reviewed_at_unix: int,
    error_fragment: str,
) -> None:
    """A correct signature cannot override the independent freshness clock."""

    summary_path, envelope_path, _binding, _envelope = signed_fixture(
        tmp_path,
        reviewed_at_unix=reviewed_at_unix,
    )

    authenticated, errors = verify(summary_path, envelope_path)

    assert authenticated is None
    assert any(error_fragment in error for error in errors)


def test_mutated_signature_and_cross_domain_signature_are_rejected(
    tmp_path: Path,
) -> None:
    """Only the topology-specific domain can authenticate this companion."""

    summary_path, envelope_path, _binding, envelope = signed_fixture(tmp_path)
    signature = bytes.fromhex(envelope["signature_hex"])
    mutated_signature = bytes((signature[0] ^ 1,)) + signature[1:]
    envelope["signature_hex"] = mutated_signature.hex()
    write_json(envelope_path, envelope)
    authenticated, errors = verify(summary_path, envelope_path)
    assert authenticated is None
    assert any("signature must authenticate" in error for error in errors)

    envelope["signature_hex"] = "00" * 64
    topology_bytes = TOPOLOGY.topology_qualification_envelope_signing_bytes(envelope)
    cross_domain_bytes = (
        b"sorafs-reference-sdk-provenance-verification-receipt-v1\x00"
        + topology_bytes[len(TOPOLOGY.TOPOLOGY_QUALIFICATION_SIGNATURE_DOMAIN) :]
    )
    sign_envelope(envelope, signing_bytes=cross_domain_bytes)
    write_json(envelope_path, envelope)
    authenticated, errors = verify(summary_path, envelope_path)
    assert authenticated is None
    assert any("signature must authenticate" in error for error in errors)
