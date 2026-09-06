"""Tests for independently signed SoraFS topology qualification envelopes."""

from __future__ import annotations

import hashlib
import json
import os
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
import build_sorafs_topology_qualification_envelope as ENVELOPE_CLI  # noqa: E402
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
EXPECTED_TOPOLOGY_SIGNING_PAYLOAD_SHA256 = (
    "cfad64a4bcc5c8b20a4766ad3eec2418ff4a8f2f49563ab3cc22f5839e9dee1e"
)


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
        "validator_count": 4,
        "validator_ids": [
            "taira-validator-1",
            "taira-validator-2",
            "taira-validator-3",
            "taira-validator-4",
        ],
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


def topology_envelope_cli_trust_args(summary_path: Path) -> list[str]:
    """Return the public trust tuple shared by all topology CLI phases."""

    return [
        "--topology-qualification-summary",
        str(summary_path),
        "--deployment-id",
        DEPLOYMENT_ID,
        "--environment",
        ENVIRONMENT,
        "--now-unix",
        str(NOW_UNIX),
        "--max-topology-qualification-review-age-secs",
        str(MAX_REVIEW_AGE_SECS),
        "--topology-qualification-verification-public-key-hex",
        PUBLIC_KEY.hex(),
        "--topology-qualification-signer-service-id",
        SIGNER_SERVICE_ID,
        "--topology-qualification-signer-administrator-id",
        SIGNER_ADMINISTRATOR_ID,
        "--topology-qualification-signer-key-revision",
        str(SIGNER_KEY_REVISION),
        "--topology-qualification-signer-policy-revision",
        str(SIGNER_POLICY_REVISION),
        "--topology-qualification-signer-policy-digest-hex",
        SIGNER_POLICY_DIGEST,
    ]


def prepare_topology_envelope_cli(
    tmp_path: Path,
    summary_path: Path,
    *,
    stem: str = "topology",
) -> tuple[Path, Path]:
    """Run the no-private-key prepare phase and return both outputs."""

    prepared = tmp_path / f"{stem}.prepared.json"
    signing_payload = tmp_path / f"{stem}.signing-payload.bin"
    result = ENVELOPE_CLI.main(
        [
            "prepare",
            *topology_envelope_cli_trust_args(summary_path),
            "--reviewed-at-unix",
            str(REVIEWED_AT_UNIX),
            "--prepared-out",
            str(prepared),
            "--signing-payload-out",
            str(signing_payload),
        ]
    )
    assert result == 0
    return prepared, signing_payload


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


def test_cli_prepare_finalize_verify_round_trip_is_deterministic_and_payload_free(
    tmp_path: Path,
) -> None:
    """The public CLI replays exact bytes without receiving a private key."""

    summary_path = tmp_path / "topology-summary.json"
    write_json(summary_path, qualification_summary())
    prepared_a, payload_a = prepare_topology_envelope_cli(
        tmp_path, summary_path, stem="first"
    )
    prepared_b, payload_b = prepare_topology_envelope_cli(
        tmp_path, summary_path, stem="second"
    )
    assert prepared_a.read_bytes() == prepared_b.read_bytes()
    assert payload_a.read_bytes() == payload_b.read_bytes()

    prepared_value = json.loads(prepared_a.read_text(encoding="utf-8"))
    assert set(prepared_value) == ENVELOPE_CLI.PREPARED_ENVELOPE_FIELDS
    independent_unsigned = json.dumps(
        prepared_value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    assert payload_a.read_bytes() == (
        b"sorafs-l1-topology-qualification-envelope-v1\x00"
        + independent_unsigned
    )
    assert len(payload_a.read_bytes()) == 1_132
    assert hashlib.sha256(payload_a.read_bytes()).hexdigest() == (
        EXPECTED_TOPOLOGY_SIGNING_PAYLOAD_SHA256
    )
    signature_path = tmp_path / "topology-signature.bin"
    signature_path.write_bytes(sign(SIGNING_SEED, payload_a.read_bytes()))
    envelope_path = tmp_path / "topology-envelope.json"
    assert (
        ENVELOPE_CLI.main(
            [
                "finalize",
                *topology_envelope_cli_trust_args(summary_path),
                "--reviewed-at-unix",
                str(REVIEWED_AT_UNIX),
                "--prepared",
                str(prepared_a),
                "--signature-file",
                str(signature_path),
                "--envelope-out",
                str(envelope_path),
            ]
        )
        == 0
    )
    verification_a = tmp_path / "topology-verification-a.json"
    verification_b = tmp_path / "topology-verification-b.json"
    for output in (verification_a, verification_b):
        assert (
            ENVELOPE_CLI.main(
                [
                    "verify",
                    *topology_envelope_cli_trust_args(summary_path),
                    "--topology-qualification-envelope",
                    str(envelope_path),
                    "--verification-out",
                    str(output),
                ]
            )
            == 0
        )
    assert verification_a.read_bytes() == verification_b.read_bytes()
    verification = json.loads(verification_a.read_text(encoding="utf-8"))
    assert set(verification) == TOPOLOGY.AUTHENTICATED_TOPOLOGY_BINDING_FIELDS
    assert verification["network"] == "taira"
    assert verification["chain_discriminant"] == 369
    assert "signature_hex" not in verification
    assert "payload" not in verification
    assert envelope_path.stat().st_mode & 0o777 == 0o600


def test_cli_prepare_rejects_stale_pre_chain_binding_summary_without_outputs(
    tmp_path: Path,
) -> None:
    """The old pre-chain summary shape cannot be prepared for signing."""

    summary = qualification_summary()
    del summary["deployment"]["chain_id"]
    del summary["validator_ids"]
    summary_path = tmp_path / "stale-topology-summary.json"
    write_json(summary_path, summary)
    prepared = tmp_path / "prepared.json"
    payload = tmp_path / "signing-payload.bin"

    result = ENVELOPE_CLI.main(
        [
            "prepare",
            *topology_envelope_cli_trust_args(summary_path),
            "--reviewed-at-unix",
            str(REVIEWED_AT_UNIX),
            "--prepared-out",
            str(prepared),
            "--signing-payload-out",
            str(payload),
        ]
    )

    assert result == 1
    assert not prepared.exists()
    assert not payload.exists()


@pytest.mark.parametrize(
    ("option", "replacement"),
    [
        ("--reviewed-at-unix", str(REVIEWED_AT_UNIX - 1)),
        ("--topology-qualification-signer-policy-revision", "12"),
    ],
)
def test_cli_finalize_rejects_review_or_trust_substitution_without_output(
    tmp_path: Path,
    option: str,
    replacement: str,
) -> None:
    """Finalization independently replays time and signer policy inputs."""

    summary_path = tmp_path / "topology-summary.json"
    write_json(summary_path, qualification_summary())
    prepared, payload = prepare_topology_envelope_cli(tmp_path, summary_path)
    signature_path = tmp_path / "topology-signature.bin"
    signature_path.write_bytes(sign(SIGNING_SEED, payload.read_bytes()))
    envelope_path = tmp_path / "topology-envelope.json"
    arguments = [
        "finalize",
        *topology_envelope_cli_trust_args(summary_path),
        "--reviewed-at-unix",
        str(REVIEWED_AT_UNIX),
        "--prepared",
        str(prepared),
        "--signature-file",
        str(signature_path),
        "--envelope-out",
        str(envelope_path),
    ]
    arguments[arguments.index(option) + 1] = replacement

    assert ENVELOPE_CLI.main(arguments) == 1
    assert not envelope_path.exists()


def test_cli_finalize_rejects_invalid_detached_signature_without_output(
    tmp_path: Path,
) -> None:
    """A well-shaped but foreign detached signature cannot publish an envelope."""

    summary_path = tmp_path / "topology-summary.json"
    write_json(summary_path, qualification_summary())
    prepared, _payload = prepare_topology_envelope_cli(tmp_path, summary_path)
    signature_path = tmp_path / "topology-signature.bin"
    signature_path.write_bytes(sign(SIGNING_SEED, b"foreign signing transcript"))
    envelope_path = tmp_path / "topology-envelope.json"

    result = ENVELOPE_CLI.main(
        [
            "finalize",
            *topology_envelope_cli_trust_args(summary_path),
            "--reviewed-at-unix",
            str(REVIEWED_AT_UNIX),
            "--prepared",
            str(prepared),
            "--signature-file",
            str(signature_path),
            "--envelope-out",
            str(envelope_path),
        ]
    )

    assert result == 1
    assert not envelope_path.exists()


def test_cli_finalize_rejects_boolean_revision_in_signed_prepared_object(
    tmp_path: Path,
) -> None:
    """JSON booleans cannot impersonate trusted integer revision one."""

    summary_path = tmp_path / "topology-summary.json"
    write_json(summary_path, qualification_summary())
    trust_args = topology_envelope_cli_trust_args(summary_path)
    trust_args[trust_args.index("--topology-qualification-signer-key-revision") + 1] = (
        "1"
    )
    prepared = tmp_path / "topology.prepared.json"
    payload = tmp_path / "topology.signing-payload.bin"
    assert (
        ENVELOPE_CLI.main(
            [
                "prepare",
                *trust_args,
                "--reviewed-at-unix",
                str(REVIEWED_AT_UNIX),
                "--prepared-out",
                str(prepared),
                "--signing-payload-out",
                str(payload),
            ]
        )
        == 0
    )
    prepared_value = json.loads(prepared.read_text(encoding="utf-8"))
    prepared_value["signer_key_revision"] = True
    write_json(prepared, prepared_value)
    signature_path = tmp_path / "topology-signature.bin"
    signature_path.write_bytes(
        sign(
            SIGNING_SEED,
            ENVELOPE_CLI.prepared_topology_qualification_signing_bytes(
                prepared_value
            ),
        )
    )
    envelope_path = tmp_path / "topology-envelope.json"

    result = ENVELOPE_CLI.main(
        [
            "finalize",
            *trust_args,
            "--reviewed-at-unix",
            str(REVIEWED_AT_UNIX),
            "--prepared",
            str(prepared),
            "--signature-file",
            str(signature_path),
            "--envelope-out",
            str(envelope_path),
        ]
    )

    assert result == 1
    assert not envelope_path.exists()


def test_cli_rejects_secret_signing_arguments_without_echo(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Direct and response-file secret options fail before argparse can echo them."""

    secret = "do-not-render-this-private-value"
    assert ENVELOPE_CLI.main(["prepare", "--private-key", secret]) == 2
    direct_error = capsys.readouterr().err
    assert "secret signing inputs are not accepted" in direct_error
    assert secret not in direct_error

    response = tmp_path / "topology.args"
    response.write_text(f"prepare\n--seed {secret}\n", encoding="utf-8")
    assert ENVELOPE_CLI.main([f"@{response}"]) == 2
    response_error = capsys.readouterr().err
    assert "secret signing inputs are not accepted" in response_error
    assert secret not in response_error

    for option in ("--signing_key", "--password", "--mnemonic", "--token"):
        assert ENVELOPE_CLI.main(["prepare", option, secret]) == 2
        error = capsys.readouterr().err
        assert "secret signing inputs are not accepted" in error
        assert secret not in error


@pytest.mark.parametrize(
    "mutation",
    ["abbreviated", "duplicate"],
)
def test_cli_rejects_abbreviated_or_duplicate_scalar_trust_options(
    tmp_path: Path,
    mutation: str,
) -> None:
    """Expanded scalar trust values have one exact, unambiguous spelling."""

    summary_path = tmp_path / "topology-summary.json"
    write_json(summary_path, qualification_summary())
    arguments = [
        "prepare",
        *topology_envelope_cli_trust_args(summary_path),
        "--reviewed-at-unix",
        str(REVIEWED_AT_UNIX),
        "--prepared-out",
        str(tmp_path / "prepared.json"),
        "--signing-payload-out",
        str(tmp_path / "payload.bin"),
    ]
    if mutation == "abbreviated":
        arguments[arguments.index("--deployment-id")] = "--deploy"
    else:
        arguments.extend(("--environment", "prod"))

    assert ENVELOPE_CLI.main(arguments) == 2
    assert not (tmp_path / "prepared.json").exists()
    assert not (tmp_path / "payload.bin").exists()


@pytest.mark.parametrize("unsafe_kind", ["symlink", "hardlink"])
def test_cli_prepare_rejects_existing_link_outputs_before_writing(
    tmp_path: Path,
    unsafe_kind: str,
) -> None:
    """No prepared or payload output may replace or alias an existing file."""

    summary_path = tmp_path / "topology-summary.json"
    write_json(summary_path, qualification_summary())
    protected = tmp_path / "protected"
    protected.write_bytes(b"unchanged")
    prepared = tmp_path / "prepared.json"
    if unsafe_kind == "symlink":
        prepared.symlink_to(protected)
    else:
        os.link(protected, prepared)
    payload = tmp_path / "signing-payload.bin"

    result = ENVELOPE_CLI.main(
        [
            "prepare",
            *topology_envelope_cli_trust_args(summary_path),
            "--reviewed-at-unix",
            str(REVIEWED_AT_UNIX),
            "--prepared-out",
            str(prepared),
            "--signing-payload-out",
            str(payload),
        ]
    )

    assert result == 2
    assert protected.read_bytes() == b"unchanged"
    assert not payload.exists()


def test_cli_prepare_rejects_non_private_output_parent(tmp_path: Path) -> None:
    """Conditional rollback is offered only in an owner-only directory."""

    summary_path = tmp_path / "topology-summary.json"
    write_json(summary_path, qualification_summary())
    shared = tmp_path / "shared"
    shared.mkdir(mode=0o700)
    shared.chmod(0o770)
    prepared = shared / "prepared.json"
    payload = shared / "signing-payload.bin"

    result = ENVELOPE_CLI.main(
        [
            "prepare",
            *topology_envelope_cli_trust_args(summary_path),
            "--reviewed-at-unix",
            str(REVIEWED_AT_UNIX),
            "--prepared-out",
            str(prepared),
            "--signing-payload-out",
            str(payload),
        ]
    )

    assert result == 2
    assert not prepared.exists()
    assert not payload.exists()


def test_cli_prepare_rolls_back_first_output_when_second_publication_fails(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A handled publication failure cannot leave a lone prepared artifact."""

    summary_path = tmp_path / "topology-summary.json"
    write_json(summary_path, qualification_summary())
    prepared = tmp_path / "prepared.json"
    payload = tmp_path / "signing-payload.bin"
    original_link = ENVELOPE_CLI.os.link
    link_calls = 0

    def fail_second_link(*args: Any, **kwargs: Any) -> None:
        nonlocal link_calls
        link_calls += 1
        if link_calls == 2:
            raise OSError("injected second-publication failure")
        original_link(*args, **kwargs)

    monkeypatch.setattr(ENVELOPE_CLI.os, "link", fail_second_link)

    result = ENVELOPE_CLI.main(
        [
            "prepare",
            *topology_envelope_cli_trust_args(summary_path),
            "--reviewed-at-unix",
            str(REVIEWED_AT_UNIX),
            "--prepared-out",
            str(prepared),
            "--signing-payload-out",
            str(payload),
        ]
    )

    assert result == 2
    assert not prepared.exists()
    assert not payload.exists()
    assert list(tmp_path.glob(".sorafs-topology-*.tmp")) == []


def test_cli_prepare_preserves_racing_foreign_inode_while_rolling_back_its_own(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Rollback preserves a foreign inode already visible at its identity check."""

    summary_path = tmp_path / "topology-summary.json"
    write_json(summary_path, qualification_summary())
    prepared = tmp_path / "prepared.json"
    payload = tmp_path / "signing-payload.bin"
    original_link = ENVELOPE_CLI.os.link
    link_calls = 0

    def race_second_link(
        source: str,
        destination: str,
        *,
        src_dir_fd: int,
        dst_dir_fd: int,
        follow_symlinks: bool,
    ) -> None:
        nonlocal link_calls
        link_calls += 1
        if link_calls == 2:
            descriptor = os.open(
                destination,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL,
                0o600,
                dir_fd=dst_dir_fd,
            )
            try:
                os.write(descriptor, b"foreign-racing-inode")
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        original_link(
            source,
            destination,
            src_dir_fd=src_dir_fd,
            dst_dir_fd=dst_dir_fd,
            follow_symlinks=follow_symlinks,
        )

    monkeypatch.setattr(ENVELOPE_CLI.os, "link", race_second_link)

    result = ENVELOPE_CLI.main(
        [
            "prepare",
            *topology_envelope_cli_trust_args(summary_path),
            "--reviewed-at-unix",
            str(REVIEWED_AT_UNIX),
            "--prepared-out",
            str(prepared),
            "--signing-payload-out",
            str(payload),
        ]
    )

    assert result == 2
    assert not prepared.exists()
    assert payload.read_bytes() == b"foreign-racing-inode"
    assert list(tmp_path.glob(".sorafs-topology-*.tmp")) == []


def test_cli_prepare_rechecks_every_published_output_before_success(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A first output mutated during second-output review fails the whole batch."""

    summary_path = tmp_path / "topology-summary.json"
    write_json(summary_path, qualification_summary())
    prepared = tmp_path / "prepared.json"
    payload = tmp_path / "signing-payload.bin"
    original_identity_check = ENVELOPE_CLI._output_path_identity_matches
    identity_checks = 0

    def mutate_first_during_second(record: Any) -> bool:
        nonlocal identity_checks
        identity_checks += 1
        if identity_checks == 2:
            prepared.write_bytes(b"mutated-after-first-review")
        return original_identity_check(record)

    monkeypatch.setattr(
        ENVELOPE_CLI,
        "_output_path_identity_matches",
        mutate_first_during_second,
    )

    result = ENVELOPE_CLI.main(
        [
            "prepare",
            *topology_envelope_cli_trust_args(summary_path),
            "--reviewed-at-unix",
            str(REVIEWED_AT_UNIX),
            "--prepared-out",
            str(prepared),
            "--signing-payload-out",
            str(payload),
        ]
    )

    assert result == 2
    assert not prepared.exists()
    assert not payload.exists()


def test_cli_operator_workflow_is_documented_and_source_bound() -> None:
    """The runbook keeps the complete no-private-key workflow discoverable."""

    spec = (
        SCRIPT_DIR.parent / "specs/sorafs/l1_deployment_qualification.md"
    ).read_text(encoding="utf-8")
    example = (
        SCRIPT_DIR / "examples/sorafs_l1_topology_qualification_envelope.md"
    ).read_text(encoding="utf-8")
    builder = "scripts/build_sorafs_topology_qualification_envelope.py"

    assert builder in spec
    assert "sorafs_l1_topology_qualification_envelope.md" in spec
    assert "mode-0700 runtime directory" in spec
    assert "does not claim promotion readiness" in spec
    assert "install -d -m 0700 /runtime/evidence" in example
    assert "do not run concurrent writers" in example
    for command in ("prepare", "finalize", "verify"):
        assert f"{builder} {command}" in example
    assert "cmp /runtime/evidence/topology-verification-a.json" in example
