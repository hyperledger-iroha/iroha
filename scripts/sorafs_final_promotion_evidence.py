"""Replay the independently pinned native final-promotion receipt verifier.

Only exact private copies cross the process boundary. A verified outer receipt
authenticates its custody and operation claims; it does not qualify inner lanes,
cosign provenance, or the complete production approval chain.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import tempfile
from collections.abc import Mapping
from pathlib import Path
from typing import Any

import sorafs_verifier_process as verifier_process
from sorafs_evidence_json import decode_evidence_json, read_evidence_bytes
from sorafs_path_identity import resolve_path_identity

MAX_DOCUMENT_BYTES = 64 * 1024
MAX_STATEMENT_BYTES = 256 * 1024
MAX_VALIDATION_BYTES = 16 * 1024
LOWER_DIGEST = re.compile(r"[0-9a-f]{64}")
VALIDATION_FIELDS = frozenset({
    "schema", "status", "verification_scope", "statement_sha256",
    "statement_size", "signature_sha256", "public_key_fingerprint_sha256",
    "signer_policy_sha256", "custody_trust_sha256",
    "completed_operation_state_sha256", "operation_receipt_sha256",
    "operation_id", "custody_record_digest", "policy_digest", "key_revision",
    "policy_revision", "service_id", "administrator_id", "role",
    "deployment_id", "chain_id", "network_id", "finalized_height",
    "finalized_block_hash", "verified_at_unix_ms",
})
FAILURE = "final promotion requires a verified signer authorization and completed-operation receipt"


def _digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _require_digest(value: object) -> str:
    if not isinstance(value, str) or not LOWER_DIGEST.fullmatch(value) or value == "0" * 64:
        raise ValueError("invalid independent receipt input digest")
    return value


def _positive_integer(value: object) -> bool:
    return type(value) is int and 0 < value < 1 << 64


def validate_native_result(raw: bytes, expected: Mapping[str, Any]) -> None:
    """Require the complete native scope and exact hashes of every captured input."""

    if not isinstance(raw, bytes) or not 0 < len(raw) <= MAX_VALIDATION_BYTES:
        raise ValueError("invalid native receipt result size")
    result = decode_evidence_json(raw)
    if set(result) != VALIDATION_FIELDS:
        raise ValueError("invalid native receipt result schema")
    for key, value in expected.items():
        if type(result.get(key)) is not type(value) or result[key] != value:
            raise ValueError("native receipt result differs from reviewed inputs")
    for key in ("operation_id", "custody_record_digest", "finalized_block_hash"):
        _require_digest(result[key])
    if not _positive_integer(result["finalized_height"]):
        raise ValueError("native receipt result lacks a finalized anchor")


def verify_final_promotion_receipt(
    args: argparse.Namespace,
    statement: bytes,
    signature: bytes,
    trusted_public_key: bytes,
) -> list[str]:
    """Verify current signer receipt evidence; never accept precomputed result JSON."""

    try:
        executable_pin = _require_digest(args.provenance_receipt_verifier_sha256)
        policy_pin = _require_digest(args.provenance_signer_policy_sha256)
        trust_pin = _require_digest(args.provenance_custody_trust_sha256)
        if (
            not isinstance(trusted_public_key, bytes)
            or len(trusted_public_key) != 32
            or not any(trusted_public_key)
            or not _positive_integer(args.now_unix)
            or args.now_unix > ((1 << 64) - 1) // 1000
        ):
            raise ValueError("invalid independent key or clock")
        now_unix_ms = args.now_unix * 1000
        if not isinstance(signature, bytes) or len(signature) != 64 or not any(signature):
            raise ValueError("invalid detached signature")
        if not isinstance(statement, bytes) or not 0 < len(statement) <= MAX_STATEMENT_BYTES:
            raise ValueError("invalid statement length")
        documents = {
            "statement": statement,
            "signature": signature,
            "public-key": trusted_public_key,
        }
        for flag, source in (
            ("signer-policy", args.provenance_signer_policy),
            ("custody-trust", args.provenance_custody_trust),
            ("completed-operation-state", args.provenance_completed_operation_state),
            ("operation-receipt", args.provenance_operation_receipt),
        ):
            documents[flag] = read_evidence_bytes(source, MAX_DOCUMENT_BYTES)
            if not documents[flag]:
                raise ValueError("empty native receipt input")
        if _digest(documents["signer-policy"]) != policy_pin or _digest(documents["custody-trust"]) != trust_pin:
            raise ValueError("receipt inputs differ from independent trust")
        expected = {
            "schema": "sorafs.final_promotion_receipt_verification.v1",
            "status": "verified",
            "verification_scope": "final_promotion_signer_receipt",
            "statement_sha256": _digest(statement),
            "statement_size": len(statement),
            "signature_sha256": _digest(signature),
            "public_key_fingerprint_sha256": _digest(trusted_public_key),
            "signer_policy_sha256": policy_pin,
            "custody_trust_sha256": trust_pin,
            "completed_operation_state_sha256": _digest(documents["completed-operation-state"]),
            "operation_receipt_sha256": _digest(documents["operation-receipt"]),
            "policy_digest": _require_digest(args.provenance_signer_policy_digest_hex),
            "key_revision": args.provenance_signer_key_revision,
            "policy_revision": args.provenance_signer_policy_revision,
            "service_id": args.provenance_signer_service_id,
            "administrator_id": args.provenance_signer_administrator_id,
            "role": "final_promotion_provenance",
            "deployment_id": args.provenance_deployment_id,
            "chain_id": args.provenance_chain_id,
            "network_id": _require_digest(args.provenance_network_id_hex),
            "verified_at_unix_ms": now_unix_ms,
        }
        if not all(_positive_integer(expected[key]) for key in ("key_revision", "policy_revision")):
            raise ValueError("invalid independent signer revision")
        if not all(isinstance(expected[key], str) and expected[key] for key in (
            "service_id", "administrator_id", "deployment_id", "chain_id",
        )):
            raise ValueError("missing independent signer context")
        with tempfile.TemporaryDirectory(prefix="sorafs-final-promotion-") as temporary:
            root = resolve_path_identity(Path(temporary), [])
            if root is None:
                raise OSError("verifier private directory identity unavailable")
            executable = root / "iroha"
            verifier_process.snapshot_executable(args.provenance_receipt_verifier, executable, executable_pin)
            command = [str(executable), "app", "sorafs", "toolkit", "final-promotion-receipt"]
            for flag, document in documents.items():
                path = root / flag
                verifier_process.write_private_input(path, document)
                command.extend([f"--{flag}", str(path)])
            for flag, value in (
                ("public-key-fingerprint", expected["public_key_fingerprint_sha256"]),
                ("signer-policy-sha256", policy_pin),
                ("custody-trust-sha256", trust_pin),
                ("now-unix-ms", str(now_unix_ms)),
            ):
                command.extend([f"--{flag}", value])
            result = verifier_process.run_verifier(
                command, root, max_stdout_bytes=MAX_VALIDATION_BYTES, expected_stderr=b"",
            )
            validate_native_result(result, expected)
    except (OSError, ValueError, TypeError, AttributeError, KeyError, RecursionError, RuntimeError):
        # Candidate bytes, paths and process diagnostics never enter evidence.
        return [FAILURE]
    return []
