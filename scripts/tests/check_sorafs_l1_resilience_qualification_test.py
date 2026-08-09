"""Tests for the holistic SoraFS L1 resilience qualification contract."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import sys
from pathlib import Path

import pytest


SCRIPT_DIR = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_DIR / "check_sorafs_l1_resilience_qualification.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_l1_resilience_qualification",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

import sccp_release_common as RELEASE_CRYPTO  # noqa: E402
import sorafs_topology_qualification as TOPOLOGY  # noqa: E402


NOW_UNIX = 1_800_900_000
CAPTURED_AT_UNIX = NOW_UNIX - 60
GENERATED_AT_UNIX = NOW_UNIX - 30
MAX_AGE_SECS = 3600
DEPLOYMENT_ID = "sorafs-mainnet-2026-07"
ENVIRONMENT = "production"
ARGS_EXAMPLE = (
    SCRIPT_DIR / "examples" / "sorafs_l1_resilience_qualification.args.example"
)


def write_json(path: Path, payload: dict) -> None:
    """Write stable JSON bytes for an exact-digest test artifact."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )


def topology_summary_path(tmp_path: Path) -> Path:
    """Write one exact four-validator topology qualification summary."""

    path = tmp_path / "l1-topology-qualification.json"
    payload = {
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
        "runtime_handle_kinds": ["monitoring", "external_signer", "kms", "webauthn"],
        "runtime_material_policy_valid": True,
        "signed_model_artifact_count": 1,
        "required_lane_slots": list(TOPOLOGY.CANONICAL_READINESS_LANES),
        "recognized_lane_slot_count": 17,
        "errors": [],
    }
    write_json(path, payload)
    return path


def topology_binding(tmp_path: Path) -> tuple[Path, dict[str, str]]:
    """Return the exact binding derived from the topology summary bytes."""

    path = topology_summary_path(tmp_path)
    binding, errors = TOPOLOGY.load_topology_qualification_binding(
        path,
        expected_deployment_id=DEPLOYMENT_ID,
        expected_environment=ENVIRONMENT,
    )
    assert errors == []
    assert binding is not None
    return path, binding


def artifact_payload(
    requirement: str,
    topology: dict[str, str],
    *,
    captured_at_unix: int,
) -> dict:
    """Build one payload-free resilience observation."""

    peer_states = []
    if requirement == "identical_post_recovery_peer_state":
        peer_states = [
            {
                "validator_id": f"validator-{suffix}",
                "finalized_state_sha256": "ab" * 32,
            }
            for suffix in ("a", "b", "c", "d")
        ]
    return {
        "schema": MODULE.ARTIFACT_SCHEMA,
        "requirement": requirement,
        "deployment": {
            "deployment_id": DEPLOYMENT_ID,
            "environment": ENVIRONMENT,
        },
        "topology_qualification": dict(topology),
        "captured_at_unix": captured_at_unix,
        "result": "passed",
        "observation_count": 1,
        "payload_included": False,
        "peer_state_digests": peer_states,
    }


def local_receipt(
    tmp_path: Path,
    *,
    captured_at_unix: int = CAPTURED_AT_UNIX,
) -> tuple[Path, dict[str, str], Path, dict]:
    """Write all 19 artifacts and return one local holistic receipt."""

    topology_path, topology = topology_binding(tmp_path)
    artifact_root = tmp_path / "resilience-artifacts"
    rows = []
    for index, requirement in enumerate(MODULE.REQUIRED_REQUIREMENTS):
        relative = f"observations/{index:02d}-{requirement}.json"
        path = artifact_root.joinpath(*relative.split("/"))
        write_json(
            path,
            artifact_payload(
                requirement,
                topology,
                captured_at_unix=captured_at_unix,
            ),
        )
        rows.append(
            {
                "requirement": requirement,
                "artifact_path": relative,
                "artifact_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "captured_at_unix": captured_at_unix,
            }
        )
    receipt = {
        "schema": MODULE.RECEIPT_SCHEMA,
        "deployment": {
            "deployment_id": DEPLOYMENT_ID,
            "environment": ENVIRONMENT,
        },
        "topology_qualification": dict(topology),
        "generated_at_unix": GENERATED_AT_UNIX,
        "artifacts": rows,
        "authentication": {
            "kind": "local",
            "algorithm": None,
            "public_key_fingerprint_sha256": None,
            "signature_hex": None,
        },
    }
    return topology_path, topology, artifact_root, receipt


def validate(
    receipt: dict,
    artifact_root: Path,
    topology: dict[str, str],
    *,
    trusted_public_key: bytes | None = None,
    now_unix: int = NOW_UNIX,
) -> tuple[dict, list[str]]:
    """Validate a test receipt through the public checker function."""

    digest = hashlib.sha256(
        json.dumps(receipt, sort_keys=True).encode("utf-8")
    ).hexdigest()
    return MODULE.validate_receipt(
        receipt,
        digest,
        artifact_root=artifact_root,
        expected_deployment_id=DEPLOYMENT_ID,
        expected_environment=ENVIRONMENT,
        topology_qualification=topology,
        now_unix=now_unix,
        max_evidence_age_secs=MAX_AGE_SECS,
        trusted_public_key=trusted_public_key,
    )


def public_key_from_seed(seed: bytes) -> bytes:
    """Derive a disposable test-only Ed25519 public key."""

    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    return RELEASE_CRYPTO._ed_encode(  # noqa: SLF001
        RELEASE_CRYPTO._ed_scalar_multiply(  # noqa: SLF001
            RELEASE_CRYPTO._ED_BASE,  # noqa: SLF001
            scalar,
        )
    )


def sign(seed: bytes, message: bytes) -> bytes:
    """Sign with a disposable in-memory seed used only by this test."""

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
    encoded_s = (
        (nonce + challenge * scalar) % RELEASE_CRYPTO._ED_L  # noqa: SLF001
    ).to_bytes(32, "little")
    return encoded_r + encoded_s


def externally_sign(receipt: dict, seed: bytes) -> bytes:
    """Replace local authentication with the trusted external signature."""

    public_key = public_key_from_seed(seed)
    receipt["authentication"] = {
        "kind": "external-ed25519",
        "algorithm": "ed25519",
        "public_key_fingerprint_sha256": hashlib.sha256(public_key).hexdigest(),
        "signature_hex": "00" * 64,
    }
    receipt["authentication"]["signature_hex"] = sign(
        seed,
        MODULE.resilience_signing_payload(receipt),
    ).hex()
    return public_key


def rewrite_bound_artifact(
    artifact_root: Path,
    receipt: dict,
    requirement: str,
    mutation,
) -> None:
    """Mutate one artifact and refresh only its exact receipt digest."""

    row = next(
        row for row in receipt["artifacts"] if row["requirement"] == requirement
    )
    path = artifact_root.joinpath(*row["artifact_path"].split("/"))
    payload = json.loads(path.read_text(encoding="utf-8"))
    mutation(payload)
    write_json(path, payload)
    row["artifact_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()


def test_local_receipt_is_configuration_qualified_and_not_a_lane(
    tmp_path: Path,
) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)

    summary, errors = validate(receipt, artifact_root, topology)

    assert errors == []
    assert set(summary) == MODULE.SUMMARY_FIELDS
    assert summary["status"] == "configuration-qualified"
    assert summary["qualification_scope"] == "holistic-deployment-resilience"
    assert summary["live_evidence_recognized"] is False
    assert summary["externally_authenticated"] is False
    assert summary["promotion_eligible"] is False
    assert summary["readiness_lane_count_delta"] == 0
    assert summary["recognized_requirement_count"] == 19
    assert summary["required_requirements"] == list(MODULE.REQUIRED_REQUIREMENTS)
    assert summary["receipt_generated_at_unix"] == GENERATED_AT_UNIX
    assert summary["receipt_authentication"] == receipt["authentication"]


def test_trusted_external_signature_makes_attachment_evidence_qualified(
    tmp_path: Path,
) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)
    public_key = externally_sign(receipt, os.urandom(32))

    summary, errors = validate(
        receipt,
        artifact_root,
        topology,
        trusted_public_key=public_key,
    )

    assert errors == []
    assert summary["status"] == "evidence-qualified"
    assert summary["live_evidence_recognized"] is True
    assert summary["externally_authenticated"] is True
    assert summary["promotion_eligible"] is True
    assert summary["readiness_lane_count_delta"] == 0
    assert summary["receipt_generated_at_unix"] == GENERATED_AT_UNIX
    assert summary["receipt_authentication"] == receipt["authentication"]


def test_missing_artifact_fails_closed(tmp_path: Path) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)
    first = receipt["artifacts"][0]
    artifact_root.joinpath(*first["artifact_path"].split("/")).unlink()

    summary, errors = validate(receipt, artifact_root, topology)

    assert summary["status"] == "blocked"
    assert summary["recognized_requirement_count"] == 18
    assert any("failed to load evidence JSON" in error for error in errors)


def test_duplicate_artifact_path_and_identity_fail_closed(tmp_path: Path) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)
    receipt["artifacts"][1]["artifact_path"] = receipt["artifacts"][0][
        "artifact_path"
    ]
    receipt["artifacts"][1]["artifact_sha256"] = receipt["artifacts"][0][
        "artifact_sha256"
    ]

    summary, errors = validate(receipt, artifact_root, topology)
    joined = "\n".join(errors)

    assert summary["status"] == "blocked"
    assert "unique canonical file identities" in joined
    assert "duplicate artifact paths" in joined


def test_stale_artifact_and_receipt_timestamps_fail_closed(tmp_path: Path) -> None:
    stale = NOW_UNIX - MAX_AGE_SECS - 1
    _topology_path, topology, artifact_root, receipt = local_receipt(
        tmp_path,
        captured_at_unix=stale,
    )
    receipt["generated_at_unix"] = stale

    summary, errors = validate(receipt, artifact_root, topology)

    assert summary["status"] == "blocked"
    assert any("older than 3600 seconds" in error for error in errors)


def test_exact_artifact_digest_detects_tampering(tmp_path: Path) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)
    first = receipt["artifacts"][0]
    path = artifact_root.joinpath(*first["artifact_path"].split("/"))
    path.write_bytes(path.read_bytes() + b" ")

    summary, errors = validate(receipt, artifact_root, topology)

    assert summary["status"] == "blocked"
    assert any("does not match exact artifact bytes" in error for error in errors)


def test_path_and_digest_swap_fails_embedded_requirement_identity(
    tmp_path: Path,
) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)
    first = receipt["artifacts"][0]
    second = receipt["artifacts"][1]
    for field in ("artifact_path", "artifact_sha256", "captured_at_unix"):
        first[field], second[field] = second[field], first[field]

    summary, errors = validate(receipt, artifact_root, topology)

    assert summary["status"] == "blocked"
    assert sum(
        "artifact requirement identity must match its receipt row" in error
        for error in errors
    ) == 2


def test_unknown_schema_field_and_secret_like_key_are_sanitized(
    tmp_path: Path,
) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)
    receipt["private_key"] = "-----BEGIN PRIVATE KEY-----\nnot-a-real-key"

    summary, errors = validate(receipt, artifact_root, topology)
    joined = "\n".join(errors)

    assert summary["status"] == "blocked"
    assert "schema-closed contract" in joined
    assert "<sensitive-key>" in joined
    assert "not-a-real-key" not in joined


def test_topology_identity_must_match_exact_qualification_bytes(
    tmp_path: Path,
) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)
    receipt["topology_qualification"]["qualification_summary_sha256"] = "cd" * 32

    summary, errors = validate(receipt, artifact_root, topology)

    assert summary["status"] == "blocked"
    assert any(
        "must match the reviewed topology" in error
        for error in errors
    )


def test_peer_state_requires_four_unique_identical_digests(
    tmp_path: Path,
) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)

    def diverge(payload: dict) -> None:
        payload["peer_state_digests"][3]["finalized_state_sha256"] = "cd" * 32

    rewrite_bound_artifact(
        artifact_root,
        receipt,
        "identical_post_recovery_peer_state",
        diverge,
    )

    summary, errors = validate(receipt, artifact_root, topology)

    assert summary["status"] == "blocked"
    assert any(
        "must prove identical post-recovery finalized state" in error
        for error in errors
    )


@pytest.mark.parametrize("requirement", ["repair_outcome", "settlement_outcome"])
def test_repair_and_settlement_require_at_least_one_outcome(
    tmp_path: Path,
    requirement: str,
) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)
    rewrite_bound_artifact(
        artifact_root,
        receipt,
        requirement,
        lambda payload: payload.update(observation_count=0),
    )

    summary, errors = validate(receipt, artifact_root, topology)

    assert summary["status"] == "blocked"
    assert any(
        f"{requirement} artifact observation_count" in error for error in errors
    )


def test_external_receipt_without_trusted_key_is_not_recognized(
    tmp_path: Path,
) -> None:
    _topology_path, topology, artifact_root, receipt = local_receipt(tmp_path)
    externally_sign(receipt, os.urandom(32))

    summary, errors = validate(receipt, artifact_root, topology)

    assert summary["status"] == "blocked"
    assert summary["externally_authenticated"] is False
    assert any(
        "requires --trusted-public-key-hex" in error for error in errors
    )


def test_cli_writes_local_non_promotable_summary(tmp_path: Path) -> None:
    topology_path, _topology, artifact_root, receipt = local_receipt(tmp_path)
    receipt_path = tmp_path / "resilience-receipt.json"
    summary_path = tmp_path / "resilience-summary.json"
    write_json(receipt_path, receipt)

    result = MODULE.main(
        [
            "--receipt",
            str(receipt_path),
            "--artifact-root",
            str(artifact_root),
            "--topology-qualification-summary",
            str(topology_path),
            "--deployment-id",
            DEPLOYMENT_ID,
            "--environment",
            ENVIRONMENT,
            "--now-unix",
            str(NOW_UNIX),
            "--max-evidence-age-secs",
            str(MAX_AGE_SECS),
            "--summary-out",
            str(summary_path),
        ]
    )

    assert result == 0
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["status"] == "configuration-qualified"
    assert summary["promotion_eligible"] is False
    assert summary["recognized_requirement_count"] == 19
    assert summary["readiness_lane_count_delta"] == 0


def test_requirement_contract_is_closed_and_has_no_lane_slot() -> None:
    assert len(MODULE.REQUIRED_REQUIREMENTS) == 19
    assert len(set(MODULE.REQUIRED_REQUIREMENTS)) == 19
    assert MODULE.READINESS_LANE_COUNT_DELTA == 0
    assert set(MODULE.REQUIRED_REQUIREMENTS).isdisjoint(
        TOPOLOGY.CANONICAL_READINESS_LANES
    )


def test_response_file_example_parses_as_local_template() -> None:
    args = MODULE.parse_args([f"@{ARGS_EXAMPLE}"])

    assert args.trusted_public_key_hex is None
    assert args.deployment_id == DEPLOYMENT_ID
    assert args.environment == ENVIRONMENT
    assert args.evidence == [
        Path(
            "artifacts/sorafs/production-readiness/l1-resilience/"
            "resilience-receipt.json"
        )
    ]
