"""Deterministic test fixtures for trusted SoraFS resilience summaries."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

import sccp_release_common as RELEASE_CRYPTO


DEFAULT_SIGNING_SEED = bytes.fromhex("3f" * 32)


def public_key_from_seed(seed: bytes = DEFAULT_SIGNING_SEED) -> bytes:
    """Derive a deterministic test-only Ed25519 public key."""

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
    """Sign one test-only message with a deterministic Ed25519 seed."""

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


def render_summary(payload: dict[str, Any]) -> bytes:
    """Render stable summary bytes used by exact-digest bindings."""

    return (
        json.dumps(
            payload,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")


def resilience_summary(
    checker: Any,
    *,
    deployment_id: str,
    environment: str,
    topology_qualification: dict[str, str],
    generated_at_unix: int,
    captured_at_unix: int,
    seed: bytes = DEFAULT_SIGNING_SEED,
) -> dict[str, Any]:
    """Build one externally authenticated, payload-free resilience summary."""

    public_key = public_key_from_seed(seed)
    artifacts = [
        {
            "requirement": requirement,
            "artifact_path": f"resilience/{index:02d}-{requirement}.json",
            "artifact_sha256": hashlib.sha256(
                f"{requirement}:qualified-observation".encode("ascii")
            ).hexdigest(),
            "captured_at_unix": captured_at_unix,
        }
        for index, requirement in enumerate(
            checker.RESILIENCE_QUALIFICATION_REQUIREMENTS
        )
    ]
    authentication = {
        "kind": "external-ed25519",
        "algorithm": "ed25519",
        "public_key_fingerprint_sha256": hashlib.sha256(public_key).hexdigest(),
        "signature_hex": "00" * 64,
    }
    receipt = {
        "schema": checker.RESILIENCE_QUALIFICATION_RECEIPT_SCHEMA,
        "deployment": {
            "deployment_id": deployment_id,
            "environment": environment,
        },
        "topology_qualification": dict(topology_qualification),
        "generated_at_unix": generated_at_unix,
        "artifacts": artifacts,
        "authentication": authentication,
    }
    unsigned = dict(receipt)
    unsigned_authentication = dict(authentication)
    unsigned_authentication.pop("signature_hex")
    unsigned["authentication"] = unsigned_authentication
    signing_payload = checker.RESILIENCE_QUALIFICATION_SIGNATURE_DOMAIN + json.dumps(
        unsigned,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")
    authentication["signature_hex"] = sign(seed, signing_payload).hex()
    canonical_receipt_bytes = json.dumps(
        receipt,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    receipt_bytes = canonical_receipt_bytes + b"\n"
    return {
        "schema": checker.RESILIENCE_QUALIFICATION_SUMMARY_SCHEMA,
        "status": "evidence-qualified",
        "qualification_scope": "holistic-deployment-resilience",
        "live_evidence_recognized": True,
        "externally_authenticated": True,
        "promotion_eligible": True,
        "readiness_lane_count_delta": 0,
        "receipt_sha256": hashlib.sha256(receipt_bytes).hexdigest(),
        "canonical_receipt_sha256": hashlib.sha256(
            canonical_receipt_bytes
        ).hexdigest(),
        "receipt_generated_at_unix": generated_at_unix,
        "receipt_authentication": dict(authentication),
        "deployment": {
            "deployment_id": deployment_id,
            "environment": environment,
        },
        "topology_qualification": dict(topology_qualification),
        "required_requirements": list(
            checker.RESILIENCE_QUALIFICATION_REQUIREMENTS
        ),
        "recognized_requirement_count": len(
            checker.RESILIENCE_QUALIFICATION_REQUIREMENTS
        ),
        "artifact_bindings": artifacts,
        "earliest_capture_unix": captured_at_unix,
        "latest_capture_unix": captured_at_unix,
        "errors": [],
    }


def resilience_binding(
    checker: Any,
    payload: dict[str, Any],
    raw: bytes,
) -> dict[str, Any]:
    """Build the expected payload-free binding for stable summary bytes."""

    return {
        "schema": checker.RESILIENCE_QUALIFICATION_BINDING_SCHEMA,
        "summary_sha256": hashlib.sha256(raw).hexdigest(),
        "receipt_sha256": payload["receipt_sha256"],
        "canonical_receipt_sha256": payload["canonical_receipt_sha256"],
        "receipt_generated_at_unix": payload["receipt_generated_at_unix"],
        "signer_public_key_fingerprint_sha256": payload[
            "receipt_authentication"
        ]["public_key_fingerprint_sha256"],
    }


def write_resilience_summary(
    checker: Any,
    path: Path,
    *,
    deployment_id: str,
    environment: str,
    topology_qualification: dict[str, str],
    generated_at_unix: int,
    captured_at_unix: int,
    seed: bytes = DEFAULT_SIGNING_SEED,
) -> tuple[Path, bytes, dict[str, Any]]:
    """Write and return one trusted resilience summary, key, and binding."""

    payload = resilience_summary(
        checker,
        deployment_id=deployment_id,
        environment=environment,
        topology_qualification=topology_qualification,
        generated_at_unix=generated_at_unix,
        captured_at_unix=captured_at_unix,
        seed=seed,
    )
    raw = render_summary(payload)
    path.write_bytes(raw)
    return path, public_key_from_seed(seed), resilience_binding(checker, payload, raw)
