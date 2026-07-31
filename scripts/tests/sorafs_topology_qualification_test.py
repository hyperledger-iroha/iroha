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
SIGNER_IDENTITY = "sorafs-topology-qualification-hsm-primary"
SIGNER_KEY_REVISION = 7
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
        },
        "validator_count": 4,
        "storage_provider_count": 2,
        "gateway_count": 2,
        "governance_dag_instance_count": 2,
        "runtime_handle_kinds": ["monitoring", "hsm", "kms", "webauthn"],
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
        "signer_identity": SIGNER_IDENTITY,
        "signer_key_revision": SIGNER_KEY_REVISION,
        "signer_key_fingerprint_hex": hashlib.sha256(PUBLIC_KEY).hexdigest(),
        "signer_policy_digest_hex": SIGNER_POLICY_DIGEST,
        "reviewed_at_unix": reviewed_at_unix,
        "signature_algorithm": "ed25519",
        "signature_hex": "00" * 64,
    }
    sign_envelope(envelope)
    write_json(envelope_path, envelope)
    return summary_path, envelope_path, binding, envelope


def verify(
    summary_path: Path,
    envelope_path: Path,
    **overrides: Any,
) -> tuple[dict[str, str] | None, list[str]]:
    """Invoke the signed loader with the trusted topology signer context."""

    arguments: dict[str, Any] = {
        "trusted_public_key": PUBLIC_KEY,
        "trusted_signer_identity": SIGNER_IDENTITY,
        "trusted_key_revision": SIGNER_KEY_REVISION,
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
    assert unsigned == expected
    assert errors == []
    assert authenticated == expected
    signing_bytes = TOPOLOGY.topology_qualification_envelope_signing_bytes(envelope)
    assert signing_bytes.startswith(TOPOLOGY.TOPOLOGY_QUALIFICATION_SIGNATURE_DOMAIN)
    assert b"signature_hex" not in signing_bytes


@pytest.mark.parametrize("missing_field", ["signature_hex", "signer_identity"])
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
    ],
)
def test_authenticated_envelope_cannot_bind_another_topology(
    tmp_path: Path,
    field: str,
    replacement: str,
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


@pytest.mark.parametrize(
    ("override", "error_fragment"),
    [
        (
            {"trusted_signer_identity": "another-topology-signer"},
            "signer_identity must match the trusted signer",
        ),
        (
            {"trusted_key_revision": SIGNER_KEY_REVISION + 1},
            "signer_key_revision must match the trusted revision",
        ),
        (
            {
                "trusted_policy_digest_hex": hashlib.sha256(
                    b"another-topology-policy"
                ).hexdigest()
            },
            "signer_policy_digest_hex must match the trusted signer policy",
        ),
        (
            {
                "trusted_public_key": public_key_from_seed(
                    hashlib.sha256(b"another-topology-key").digest()
                )
            },
            "signer_key_fingerprint_hex must match the trusted public key",
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
