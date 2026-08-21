#!/usr/bin/env python3
"""Verify source-bound evidence for a deployed public-Taira 24-hour soak.

This is an offline validator, not a workload runner or observation authority.
Its public API and CLI refuse before caller-controlled path I/O while the
independent authority, durable replay broker, and native evidence verifier are
unprovisioned. The private structural validator defines the closed v1 receipt
that those independent components authenticate.
"""

from __future__ import annotations

import argparse
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import sys
from typing import NoReturn

try:
    from scripts import taira_constants
    from scripts import taira_public_soak_authority_contract as soak_authority
except ModuleNotFoundError as error:
    if error.name != "scripts":
        raise
    import taira_constants
    import taira_public_soak_authority_contract as soak_authority


SCHEMA = "iroha.taira.deployed-public-v2-24h-soak-completion.v1"
SCHEMA_VERSION = 1
RESULT = "complete"
COMPLETION_FILENAME = "TAIRA_PUBLIC_V2_24H_COMPLETED.json"
PARTIAL_FILENAME = ".TAIRA_PUBLIC_V2_24H_COMPLETED.partial.json"
HANDOFF_SCHEMA = "iroha.taira.public-v2-24h-prerequisite-handoff.v1"
WORKLOAD_SCHEMA = "iroha.taira.public-v2-24h-signed-transfer-inventory.v1"
SUBMISSION_SCHEMA = "iroha.taira.public-v2-24h-submission-receipts.v1"
STATUS_SCHEMA = "iroha.taira.public-v2-24h-applied-statuses.v1"
BLOCK_SCHEMA = "iroha.taira.public-v2-24h-block-evidence.v1"
LIFECYCLE_SCHEMA = "iroha.taira.public-v2-24h-lifecycle-evidence.v1"
LIFECYCLE_JOURNAL_SCHEMA = (
    "iroha.taira.public-v2-24h-lifecycle-journal.v1"
)
LIFECYCLE_JOURNAL_RECEIPT_SCHEMA = (
    "iroha.taira.public-v2-24h-native-journal-verifier-receipt.v1"
)
AUTHORITY_SCHEMA = soak_authority.AUTHORITY_SCHEMA

DURATION_SECS = 86_400
DURATION_MS = DURATION_SECS * 1_000
VALIDATORS = tuple(taira_constants.SLUGS)
VALIDATOR_COUNT = 4
QUORUM = 3
TARGET_TPS = 5
SLOT_INTERVAL_MS = 1_000 // TARGET_TPS
REQUIRED_TRANSFER_COUNT = TARGET_TPS * DURATION_SECS
SAMPLE_INTERVAL_MS = 60_000
MAX_OBSERVATION_START_LATENESS_MS = 30_000
MAXIMUM_SAMPLE_GAP_MS = SAMPLE_INTERVAL_MS + MAX_OBSERVATION_START_LATENESS_MS
MAX_OBSERVATION_WINDOW_MS = 30_000
MAX_ANCHOR_TO_WORKLOAD_GAP_MS = 30_000
MAX_SUBMISSION_START_LATENESS_MS = 1_000
MAX_SUBMISSION_REQUEST_WINDOW_MS = 30_000
MAX_CONFIRMATION_DRAIN_MS = 15 * 60 * 1_000
MAX_WALL_CLOCK_SKEW_MS = 5_000
PROTOCOL_VERSION = 4
FAULT_INJECTION = "none"
WORKLOAD = "valid-signed-transfers"
IROHA_HASH_ALGORITHM = "blake2b-32"
BLOCK_HASH_TYPE = "HashOf<BlockHeader>"
SIGNED_TRANSACTION_HASH_TYPE = "HashOf<SignedTransaction>"
ENTRYPOINT_HASH_TYPE = "HashOf<TransactionEntrypoint>"
NATIVE_VERIFIER_PROTOCOL = "iroha-taira-public-soak-native-verifier-v1"
NATIVE_JOURNAL_VERIFIER_PROTOCOL = (
    "iroha-taira-public-soak-native-journal-verifier-v1"
)

MAX_RECEIPT_BYTES = 64 * 1024 * 1024
MAX_HANDOFF_BYTES = 32 * 1024 * 1024
MAX_AUTHORITY_BYTES = 1024 * 1024
MAX_ADMISSION_RECEIPT_BYTES = 1024 * 1024
MAX_WORKLOAD_BYTES = 512 * 1024 * 1024
MAX_SUBMISSION_BYTES = 512 * 1024 * 1024
MAX_STATUS_BYTES = 512 * 1024 * 1024
MAX_BLOCK_BYTES = 512 * 1024 * 1024
MAX_LIFECYCLE_BYTES = 16 * 1024 * 1024
MAX_LIFECYCLE_JOURNAL_BYTES = 512 * 1024 * 1024
MAX_LIFECYCLE_JOURNAL_RECEIPT_BYTES = 1024 * 1024
MAX_BLOCK_COUNT = 1_000_000
MAX_SAMPLE_COUNT = 100_000
MAX_TIMESTAMP_MS = 9_999_999_999_999

SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
HEX_32_RE = re.compile(r"[0-9a-f]{64}")
SECP256K1_KEY_RE = re.compile(r"[0-9a-f]{66}")
IDENTITY_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:@+-]{7,255}")
RECEIPT_NODE_ID_DOMAIN = b"iroha.taira.receipt-signer.node-id.v1\x00"
RECEIPT_NODE_ID_PREFIX = "taira-node:receipt-signer:secp256k1:sha256:"
RECEIPT_PUBLIC_KEY_PREFIX = "e70121"
TERMINAL_UNHEALTHY_SCHEMA = "taira-terminal-unhealthy-v1"
LIFECYCLE_STATE_SCHEMA = "iroha.taira.peer-supervisor-lifecycle-state.v1"
LIFECYCLE_BINDING_DOMAIN = (
    b"iroha.taira.peer-supervisor-lifecycle-binding.v1\x00"
)
SECP256K1_FIELD_MODULUS = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F
STABLE_FIELDS = (
    "st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
    "st_size", "st_mtime_ns", "st_ctime_ns",
)

TOP_LEVEL_FIELDS = {
    "schema", "schema_version", "result", "network", "source", "profile",
    "candidate_handoff", "publication_handoff", "deploy_handoff",
    "native_verifier", "soak_anchor", "samples", "workload_inventory",
    "submission_receipt_inventory", "applied_status_inventory",
    "block_evidence_inventory", "lifecycle", "completion",
}
NETWORK_FIELDS = {
    "name", "deployment", "chain_id", "network_id", "protocol_version",
    "genesis_block_hash",
}
SOURCE_FIELDS = {
    "commit", "dpn_validator_release_commit", "cargo_lock_sha256",
    "workspace_source_manifest_sha256",
}
PROFILE_FIELDS = {
    "duration_ms", "validator_count", "quorum", "target_tps",
    "slot_interval_ms", "required_transfer_slots", "sample_interval_ms",
    "maximum_sample_gap_ms", "maximum_observation_start_lateness_ms",
    "maximum_observation_window_ms", "maximum_submission_start_lateness_ms",
    "maximum_anchor_to_workload_gap_ms",
    "maximum_submission_request_window_ms", "maximum_confirmation_drain_ms",
    "maximum_wall_clock_skew_ms", "workload", "fault_injection",
}
HANDOFF_REFERENCE_FIELDS = {"kind", "schema", "sha256", "size_bytes", "source"}
HANDOFF_DOCUMENT_FIELDS = {"schema", "schema_version", "kind", "source", "identity"}
CANDIDATE_IDENTITY_FIELDS = {
    "qualification_receipt_id", "handoff_inventory_sha256",
    "admission_archive_sha256", "admission_authority_manifest_sha256",
    "validator_binary_sha256",
}
PUBLICATION_IDENTITY_FIELDS = {
    "qualification_receipt_id", "candidate_handoff_sha256",
    "handoff_inventory_sha256", "admission_archive_sha256",
    "validator_binary_sha256", "publication_receipt_sha256",
    "publication_signature_sha256", "publication_public_key_sha256",
    "published_primary_oci_manifest_sha256",
    "published_receipt_oci_manifest_sha256", "publisher_controller_sha256",
}
DEPLOY_IDENTITY_FIELDS = {
    "qualification_receipt_id", "candidate_handoff_sha256",
    "publication_handoff_sha256", "handoff_inventory_sha256",
    "publication_receipt_sha256", "published_primary_oci_manifest_sha256",
    "admission_receipt_id",
    "admission_archive_sha256", "deploy_receipt_sha256",
    "deploy_handoff_manifest_sha256", "controller_host_id",
    "controller_installation_id", "controller_sha256",
    "validator_binary_sha256", "signed_genesis_sha256", "topology_sha256",
    "config_set_sha256", "supervisor_sha256", "restart_generation",
    "network_name", "chain_id", "network_id", "protocol_version",
    "genesis_block_hash", "deployment_completed_at_unix_ms",
    "start_height", "end_height", "end_block_hash", "receipt_signers",
}
RECEIPT_SIGNER_FIELDS = {
    "node_id", "public_key", "binary_stat_seal", "config_sha256",
    "runtime_binding_sha256", "lifecycle_binding_sha256",
    "native_verifier_binary_sha256",
    "native_verifier_source_sha256", "native_verifier_receipt_sha256",
    "native_verifier_receipt_size_bytes", "verification_result",
}
PUBLIC_KEY_FIELDS = {"algorithm", "payload_hex"}
NATIVE_VERIFIER_FIELDS = {"protocol", "binary_sha256", "source_sha256"}
IROHA_HASH_FIELDS = {"algorithm", "type", "value"}
INVENTORY_REFERENCE_FIELDS = {
    "kind", "schema", "sha256", "size_bytes", "record_count",
    "records_sha256",
}
WORKLOAD_REFERENCE_FIELDS = INVENTORY_REFERENCE_FIELDS | {
    "first_signed_transaction_hash", "last_signed_transaction_hash",
}
INVENTORY_HEADER_FIELDS = {"schema", "schema_version", "record_count"}
BLOCK_RECORD_FIELDS = {
    "index", "height", "block_hash", "parent_block_hash",
    "signed_block_wire_sha256", "signed_block_wire_size_bytes",
    "finality_proof_sha256", "finality_proof_size_bytes",
    "finality_validators", "native_verifier_receipt_sha256",
    "native_verifier_receipt_size_bytes", "verification_result",
}
ANCHOR_FIELDS = {
    "schema", "observation_started_at_unix_ms",
    "observation_completed_at_unix_ms", "controller_host_id",
    "controller_installation_id", "controller_sha256",
    "controller_signing_key_id", "controller_receipt_sha256",
    "controller_signature_sha256", "deploy_end_height",
    "deploy_end_block_hash", "common_start_block_evidence_index", "validators",
}
ANCHOR_VALIDATOR_FIELDS = {
    "validator_id", "node_id", "challenge_hex", "attestation_sha256",
    "attestation_size_bytes", "attested_at_unix_ms",
    "tip_block_evidence_index", "ancestry_proof_sha256",
    "ancestry_proof_size_bytes", "native_verifier_receipt_sha256",
    "native_verifier_receipt_size_bytes", "verification_result",
}
SAMPLE_FIELDS = {
    "scheduled_elapsed_ms", "observation_started_at_unix_ms",
    "observation_completed_at_unix_ms", "applied_transfer_count",
    "failed_transfer_count", "common_block_evidence_index", "validators",
}
VALIDATOR_SAMPLE_FIELDS = {
    "validator_id", "node_id", "challenge_hex", "attestation_sha256",
    "attestation_size_bytes", "attested_at_unix_ms",
    "tip_block_evidence_index", "queue_depth", "queue_capacity",
    "queue_saturated", "queue_dropped_total", "restart_count",
    "supervisor_generation", "process_generation", "unexpected_exit_total",
    "restart_required", "last_restart_successful", "healthy",
    "native_verifier_receipt_sha256", "native_verifier_receipt_size_bytes",
    "verification_result",
}
SUBMISSION_RECORD_FIELDS = {
    "index", "signed_transaction_hash", "entrypoint_hash",
    "receipt_sha256", "receipt_size_bytes", "submitted_at_unix_ms",
    "submitted_at_height", "receipt_signer_validator_id",
    "receipt_signer_node_id", "receipt_signer_public_key",
    "native_verifier_receipt_sha256", "native_verifier_receipt_size_bytes",
    "verification_result",
}
STATUS_RECORD_FIELDS = {
    "index", "signed_transaction_hash", "entrypoint_hash", "result",
    "observed_at_unix_ms", "observation_index", "block_evidence_index",
    "response_sha256", "response_size_bytes",
    "native_verifier_receipt_sha256", "native_verifier_receipt_size_bytes",
    "verification_result",
}
WORKLOAD_RECORD_FIELDS = {
    "sequence", "operation", "scheduled_elapsed_ms",
    "request_started_elapsed_ms", "request_completed_elapsed_ms",
    "signed_transaction_hash", "entrypoint_hash",
    "versioned_signed_transaction_sha256",
    "versioned_signed_transaction_size_bytes", "submission_receipt_index",
    "applied_status_index", "block_evidence_index",
}
LIFECYCLE_REFERENCE_FIELDS = {
    "kind", "schema", "sha256", "size_bytes", "identity_sha256",
}
LIFECYCLE_FIELDS = {
    "schema", "schema_version", "deployment_completed_at_unix_ms",
    "restart_generation", "config_set_sha256", "topology_sha256",
    "signed_genesis_sha256", "supervisor_sha256", "genesis_block_hash",
    "raw_windows", "journal_inventory", "native_journal_verifier_receipt", "baseline",
    "terminal", "unexpected_exit_events", "restart_events",
}
LIFECYCLE_RAW_WINDOW_FIELDS = {
    "validator_id", "node_id", "binding_sha256", "artifact_sha256",
    "artifact_size_bytes", "records_sha256", "record_count",
    "baseline_sequence", "terminal_sequence",
}
ARTIFACT_IDENTITY_FIELDS = {"sha256", "size_bytes"}
LIFECYCLE_CHECKPOINT_FIELDS = {
    "captured_at_unix_ms", "journal_sequence", "journal_chain_sha256",
    "validators",
}
LIFECYCLE_VALIDATOR_FIELDS = {
    "validator_id", "node_id", "restart_count", "supervisor_generation",
    "process_generation", "unexpected_exit_total",
}
LIFECYCLE_JOURNAL_RECORD_FIELDS = {
    "index", "journal_sequence", "observed_at_unix_ms", "validator_id",
    "node_id", "event", "restart_count", "supervisor_generation",
    "process_generation", "unexpected_exit_total",
}
LIFECYCLE_JOURNAL_RECEIPT_FIELDS = {
    "schema", "schema_version", "protocol", "verifier_binary_sha256",
    "verifier_source_sha256", "journal_artifact_sha256",
    "journal_artifact_size_bytes", "journal_records_sha256",
    "journal_record_count", "lifecycle_window_sha256", "verification_result",
}
COMPLETION_FIELDS = {
    "state", "publication", "natural_completion",
    "workload_started_at_unix_ms", "workload_ended_at_unix_ms",
    "evidence_completed_at_unix_ms", "workload_duration_ms",
    "confirmation_drain_ms", "transfer_slot_count", "sample_count",
    "anchor_to_workload_gap_ms",
    "maximum_observed_sample_gap_ms", "maximum_observation_window_ms",
    "maximum_submission_start_lateness_ms",
    "maximum_submission_request_window_ms", "applied_transfer_count",
    "failed_transfer_count", "queue_drop_events", "unhealthy_samples",
    "restart_events", "unexpected_exit_events", "source_tuple_sha256",
    "candidate_handoff_sha256", "publication_handoff_sha256",
    "deploy_handoff_sha256", "native_verifier_identity_sha256",
    "anchor_sha256", "sample_set_sha256", "workload_inventory_sha256",
    "workload_records_sha256", "submission_inventory_sha256",
    "submission_records_sha256", "status_inventory_sha256",
    "status_records_sha256", "block_inventory_sha256",
    "block_records_sha256", "lifecycle_artifact_sha256",
    "lifecycle_identity_sha256", "lifecycle_journal_artifact_sha256",
    "lifecycle_journal_records_sha256", "lifecycle_journal_record_count",
    "lifecycle_native_verifier_receipt_sha256", "lifecycle_window_sha256",
}


class EvidenceError(RuntimeError):
    """The public-Taira evidence does not satisfy the closed contract."""


@dataclass(frozen=True)
class Artifact:
    """Stable bytes and identity for one bounded regular evidence file."""

    path: Path
    payload: bytes
    sha256: str
    size: int
    device: int
    inode: int


@dataclass(frozen=True)
class BlockEvidence:
    """Compact identity of one verified canonical block."""

    index: int
    height: int
    block_hash: str
    parent_hash: str


@dataclass(frozen=True)
class SubmissionEvidence:
    """Compact identity of one verified Torii submission receipt."""

    index: int
    signed_hash: str
    entrypoint_hash: str
    submitted_ms: int
    submitted_height: int
    signer_validator: str


@dataclass(frozen=True)
class StatusEvidence:
    """Compact identity of one verified global Applied observation."""

    index: int
    signed_hash: str
    entrypoint_hash: str
    observed_ms: int
    observation_index: int
    block_index: int


@dataclass(frozen=True)
class StructuralResult:
    """Validated evidence identities awaiting independent authentication."""

    authority_subject_core: Mapping[str, object]
    completed_at_unix_ms: int


def _fail(message: str) -> NoReturn:
    raise EvidenceError(message)


def _require(condition: bool, message: str) -> None:
    if not condition:
        _fail(message)


def _exact(value: object, fields: set[str], label: str) -> Mapping[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        _fail(f"{label} fields are not exact")
    return value


def _json_exact_equal(left: object, right: object) -> bool:
    """Compare JSON values without Python's bool/int/float coercions."""

    if type(left) is not type(right):
        return False
    if isinstance(left, dict):
        return (set(left) == set(right)
                and all(_json_exact_equal(left[key], right[key]) for key in left))
    if isinstance(left, list):
        return (len(left) == len(right)
                and all(_json_exact_equal(a, b) for a, b in zip(left, right)))
    return left == right


def _integer(value: object, label: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        _fail(f"{label} must be an integer >= {minimum}")
    return value


def _artifact_sha256(value: object, label: str, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase artifact SHA-256 digest")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must be nonzero")
    return value


def _hex_32(value: object, label: str) -> str:
    if (not isinstance(value, str) or HEX_32_RE.fullmatch(value) is None
            or value == "0" * 64):
        _fail(f"{label} must be one nonzero lowercase 32-byte hex value")
    return value


def _commit(value: object, label: str) -> str:
    if (not isinstance(value, str) or COMMIT_RE.fullmatch(value) is None
            or value == "0" * 40):
        _fail(f"{label} must be one nonzero lowercase 40-hex commit")
    return value


def _identity_text(value: object, label: str) -> str:
    if not isinstance(value, str) or IDENTITY_RE.fullmatch(value) is None:
        _fail(f"{label} is absent or noncanonical")
    return value


def _public_key(value: object, label: str) -> Mapping[str, object]:
    key = _exact(value, PUBLIC_KEY_FIELDS, label)
    _require(key["algorithm"] == "secp256k1",
             f"{label} algorithm is not the deployed receipt-key algorithm")
    payload = key["payload_hex"]
    _require(isinstance(payload, str) and SECP256K1_KEY_RE.fullmatch(payload) is not None
             and payload[:2] in {"02", "03"}
             and payload[2:] != "0" * 64, f"{label} payload is noncanonical")
    x = int(payload[2:], 16)
    _require(x < SECP256K1_FIELD_MODULUS,
             f"{label} x-coordinate is outside secp256k1")
    y_squared = (pow(x, 3, SECP256K1_FIELD_MODULUS) + 7) % SECP256K1_FIELD_MODULUS
    y = pow(y_squared, (SECP256K1_FIELD_MODULUS + 1) // 4,
            SECP256K1_FIELD_MODULUS)
    _require(pow(y, 2, SECP256K1_FIELD_MODULUS) == y_squared,
             f"{label} is not a secp256k1 curve point")
    return key


def _receipt_node_id(key: Mapping[str, object]) -> str:
    """Derive the canonical deployed node ID from one validated receipt key."""

    payload = key["payload_hex"]
    assert isinstance(payload, str)
    canonical_key = RECEIPT_PUBLIC_KEY_PREFIX + payload.upper()
    return RECEIPT_NODE_ID_PREFIX + hashlib.sha256(
        RECEIPT_NODE_ID_DOMAIN + canonical_key.encode("ascii")
    ).hexdigest()


def _runtime_binding_sha256(
    binary_sha256: str,
    binary_stat_seal: Sequence[int],
    config_sha256: str,
    restart_generation: str,
) -> str:
    """Derive the supervisor's exact binary/config/restart binding."""

    payload = {
        "binary_sha256": binary_sha256,
        "binary_stat_seal": list(binary_stat_seal),
        "config_sha256": config_sha256,
        "restart_generation": restart_generation,
        "schema": TERMINAL_UNHEALTHY_SCHEMA,
    }
    return hashlib.sha256(
        json.dumps(
            payload,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("ascii")
    ).hexdigest()


def _lifecycle_binding_sha256(
    runtime_binding_sha256: str,
    restart_generation: str,
    validator_id: str,
    node_id: str,
) -> str:
    """Derive the supervisor's domain-separated lifecycle binding."""

    payload = {
        "node_id": node_id,
        "restart_generation": restart_generation,
        "runtime_binding_sha256": runtime_binding_sha256,
        "schema": LIFECYCLE_STATE_SCHEMA,
        "validator_id": validator_id,
    }
    return hashlib.sha256(
        LIFECYCLE_BINDING_DOMAIN + _canonical_json(payload)
    ).hexdigest()


def _iroha_hash(value: object, label: str, expected_type: str) -> str:
    """Validate a marked Iroha HashOf value, never an artifact SHA-256."""

    identity = _exact(value, IROHA_HASH_FIELDS, label)
    _require(identity["algorithm"] == IROHA_HASH_ALGORITHM,
             f"{label} algorithm is not {IROHA_HASH_ALGORITHM}")
    _require(identity["type"] == expected_type,
             f"{label} HashOf type is wrong")
    digest = _hex_32(identity["value"], f"{label} value")
    _require(int(digest[-2:], 16) & 1 == 1,
             f"{label} HashOf value is missing the Iroha marker bit")
    return digest


def _canonical_json(value: object) -> bytes:
    try:
        return (
            json.dumps(value, ensure_ascii=True, allow_nan=False, sort_keys=True,
                       separators=(",", ":")) + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise EvidenceError(f"receipt is not canonically encodable: {error}") from error


def _reject_constant(value: str) -> NoReturn:
    _fail(f"non-finite JSON number is forbidden: {value}")


def _pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    decoded: dict[str, object] = {}
    for key, value in pairs:
        if key in decoded:
            _fail(f"duplicate JSON field is forbidden: {key}")
        decoded[key] = value
    return decoded


def _decode_json(payload: bytes, label: str, *, canonical: bool) -> dict[str, object]:
    try:
        value = json.loads(payload, object_pairs_hook=_pairs,
                           parse_constant=_reject_constant)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"{label} is not strict JSON") from error
    if not isinstance(value, dict):
        _fail(f"{label} root must be an object")
    if canonical and _canonical_json(value) != payload:
        _fail(f"{label} is not canonical closed JSON")
    return value


def _identity(info: os.stat_result) -> tuple[int, ...]:
    return tuple(getattr(info, field) for field in STABLE_FIELDS)


def _read_stable(path: Path, maximum_bytes: int, label: str) -> Artifact:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
    except OSError as error:
        raise EvidenceError(f"cannot inspect {label}: {path}") from error
    if resolved != path:
        _fail(f"{label} path must not traverse symbolic links")
    if (not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode)
            or before.st_nlink != 1 or before.st_size <= 0
            or before.st_size > maximum_bytes
            or stat.S_IMODE(before.st_mode) & 0o022
            or before.st_uid not in {0, os.geteuid()}):
        _fail(f"{label} is not one bounded owner-controlled regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise EvidenceError(f"cannot safely open {label}: {path}") from error
    try:
        opened = os.fstat(descriptor)
        if _identity(opened) != _identity(before):
            _fail(f"{label} changed while opening")
        chunks: list[bytes] = []
        total = 0
        while total < before.st_size:
            chunk = os.read(descriptor, min(1024 * 1024, before.st_size - total))
            if not chunk:
                _fail(f"{label} was truncated while reading")
            chunks.append(chunk)
            total += len(chunk)
        if os.read(descriptor, 1):
            _fail(f"{label} grew while reading")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    try:
        current = path.lstat()
    except OSError as error:
        raise EvidenceError(f"{label} path vanished while reading") from error
    if _identity(after) != _identity(before) or _identity(current) != _identity(before):
        _fail(f"{label} changed while reading")
    payload = b"".join(chunks)
    return Artifact(path, payload, hashlib.sha256(payload).hexdigest(), len(payload),
                    before.st_dev, before.st_ino)


def _source_identity(value: object, label: str) -> dict[str, object]:
    source = _exact(value, SOURCE_FIELDS, label)
    return {
        "commit": _commit(source["commit"], f"{label}.commit"),
        "dpn_validator_release_commit": _commit(
            source["dpn_validator_release_commit"],
            f"{label}.dpn_validator_release_commit"),
        "cargo_lock_sha256": _artifact_sha256(
            source["cargo_lock_sha256"], f"{label}.cargo_lock_sha256"),
        "workspace_source_manifest_sha256": _artifact_sha256(
            source["workspace_source_manifest_sha256"],
            f"{label}.workspace_source_manifest_sha256"),
    }


def _domain_digest(domain: bytes, value: object) -> str:
    return hashlib.sha256(domain + _canonical_json(value)).hexdigest()


def _artifact_reference(value: object, label: str, kind: str, schema: str,
                        artifact: Artifact, source: Mapping[str, object]) -> str:
    reference = _exact(value, HANDOFF_REFERENCE_FIELDS, label)
    _require(reference["kind"] == kind, f"{label}.kind is wrong")
    _require(reference["schema"] == schema, f"{label}.schema is wrong")
    digest = _artifact_sha256(reference["sha256"], f"{label}.sha256")
    _require(digest == artifact.sha256, f"{label} digest does not match its file")
    _require(_integer(reference["size_bytes"], f"{label}.size_bytes", minimum=1)
             == artifact.size, f"{label} size does not match its file")
    _require(_source_identity(reference["source"], f"{label}.source") == source,
             f"{label} source tuple is spliced")
    return digest


def _validate_handoff(artifact: Artifact, reference: object, *, kind: str,
                      identity_fields: set[str], source: Mapping[str, object]
                      ) -> tuple[str, Mapping[str, object]]:
    digest = _artifact_reference(reference, f"{kind}_handoff", kind,
                                 HANDOFF_SCHEMA, artifact, source)
    document = _exact(_decode_json(artifact.payload, f"{kind} handoff", canonical=True),
                      HANDOFF_DOCUMENT_FIELDS, f"{kind} handoff")
    _require(document["schema"] == HANDOFF_SCHEMA
             and type(document["schema_version"]) is int
             and document["schema_version"] == 1
             and document["kind"] == kind, f"{kind} handoff identity is wrong")
    _require(_source_identity(document["source"], f"{kind} handoff source") == source,
             f"{kind} handoff source tuple is wrong")
    identity = _exact(document["identity"], identity_fields, f"{kind} handoff identity")
    return digest, identity


def _validate_prerequisites(
    top: Mapping[str, object], *, source: Mapping[str, object],
    expected_binary_sha256: str, candidate_artifact: Artifact,
    publication_artifact: Artifact, deploy_artifact: Artifact,
    network: Mapping[str, object], expected_native_binary_sha256: str,
    expected_native_source_sha256: str,
) -> tuple[dict[str, str], dict[str, object]]:
    candidate_digest, candidate = _validate_handoff(
        candidate_artifact, top["candidate_handoff"], kind="candidate",
        identity_fields=CANDIDATE_IDENTITY_FIELDS, source=source)
    publication_digest, publication = _validate_handoff(
        publication_artifact, top["publication_handoff"], kind="publication",
        identity_fields=PUBLICATION_IDENTITY_FIELDS, source=source)
    deploy_digest, deploy = _validate_handoff(
        deploy_artifact, top["deploy_handoff"], kind="deploy",
        identity_fields=DEPLOY_IDENTITY_FIELDS, source=source)

    qualification = _artifact_sha256(candidate["qualification_receipt_id"],
                                     "candidate qualification receipt ID")
    for label, value in (
        ("publication", publication["qualification_receipt_id"]),
        ("deploy", deploy["qualification_receipt_id"]),
    ):
        _require(_artifact_sha256(value, f"{label} qualification receipt ID")
                 == qualification, "Taira prerequisite qualification IDs differ")
    for field in CANDIDATE_IDENTITY_FIELDS - {"qualification_receipt_id"}:
        _artifact_sha256(candidate[field], f"candidate {field}")
    for field in PUBLICATION_IDENTITY_FIELDS - {"qualification_receipt_id"}:
        _artifact_sha256(publication[field], f"publication {field}")
    for field in (
        "admission_receipt_id", "admission_archive_sha256",
        "deploy_receipt_sha256", "deploy_handoff_manifest_sha256",
        "controller_sha256", "validator_binary_sha256",
        "signed_genesis_sha256", "topology_sha256", "config_set_sha256",
        "supervisor_sha256", "restart_generation", "candidate_handoff_sha256",
        "publication_handoff_sha256", "handoff_inventory_sha256",
        "publication_receipt_sha256", "published_primary_oci_manifest_sha256",
    ):
        _artifact_sha256(deploy[field], f"deploy {field}")
    _require(candidate["validator_binary_sha256"] == expected_binary_sha256
             and publication["validator_binary_sha256"] == expected_binary_sha256
             and deploy["validator_binary_sha256"] == expected_binary_sha256,
             "candidate/publication/deploy binary differs from the trusted iroha3d")
    _require(publication["candidate_handoff_sha256"] == candidate_digest
             and deploy["candidate_handoff_sha256"] == candidate_digest,
             "publication/deploy does not consume the exact candidate handoff")
    _require(deploy["publication_handoff_sha256"] == publication_digest,
             "deploy does not consume the exact publication handoff")
    _require(publication["handoff_inventory_sha256"]
             == candidate["handoff_inventory_sha256"]
             == deploy["handoff_inventory_sha256"],
             "candidate/publication/deploy inventory identity differs")
    _require(candidate["admission_archive_sha256"] == deploy["admission_archive_sha256"],
             "deploy did not consume the authenticated candidate archive")
    _require(publication["admission_archive_sha256"]
             == candidate["admission_archive_sha256"],
             "publication did not consume the authenticated candidate archive")
    _require(deploy["publication_receipt_sha256"]
             == publication["publication_receipt_sha256"]
             and deploy["published_primary_oci_manifest_sha256"]
             == publication["published_primary_oci_manifest_sha256"],
             "deploy did not consume the authenticated publication")
    _require(deploy["network_name"] == network["name"]
             and deploy["chain_id"] == network["chain_id"]
             and deploy["network_id"] == network["network_id"]
             and type(deploy["protocol_version"]) is int
             and deploy["protocol_version"] == network["protocol_version"],
             "deploy network identity differs from the completion receipt")
    deploy_genesis = _iroha_hash(deploy["genesis_block_hash"],
                                 "deploy genesis block hash", BLOCK_HASH_TYPE)
    network_genesis = _iroha_hash(network["genesis_block_hash"],
                                  "network genesis block hash", BLOCK_HASH_TYPE)
    _require(deploy_genesis == network_genesis,
             "deploy genesis block differs from the completion receipt")
    _identity_text(deploy["controller_host_id"], "deploy controller host ID")
    _identity_text(deploy["controller_installation_id"],
                   "deploy controller installation ID")
    start_height = _integer(deploy["start_height"], "deploy start height", minimum=1)
    end_height = _integer(deploy["end_height"], "deploy end height", minimum=1)
    _require(end_height > start_height, "deploy handoff did not prove advancement")
    end_hash = _iroha_hash(deploy["end_block_hash"], "deploy end block hash",
                           BLOCK_HASH_TYPE)
    deployed_ms = _integer(deploy["deployment_completed_at_unix_ms"],
                           "deployment completion time", minimum=1)
    raw_signers = _exact(deploy["receipt_signers"], set(VALIDATORS),
                         "deploy receipt signer map")
    receipt_signers: dict[str, dict[str, object]] = {}
    node_ids: set[str] = set()
    public_keys: set[tuple[object, ...]] = set()
    native_receipts: set[str] = set()
    for validator in VALIDATORS:
        signer = _exact(raw_signers[validator], RECEIPT_SIGNER_FIELDS,
                        f"deploy receipt signer {validator}")
        node_id = _identity_text(signer["node_id"],
                                 f"deploy node ID {validator}")
        key = dict(_public_key(signer["public_key"],
                               f"deploy receipt public key {validator}"))
        _require(node_id == _receipt_node_id(key),
                 f"deploy node ID {validator} is not derived from its receipt key")
        binary_stat_seal = signer["binary_stat_seal"]
        _require(isinstance(binary_stat_seal, list)
                 and len(binary_stat_seal) == 5,
                 f"deploy binary stat seal {validator} is not exact")
        normalized_stat_seal = tuple(
            _integer(value, f"deploy binary stat seal {validator}[{index}]",
                     minimum=1 if index in {0, 1, 2} else 0)
            for index, value in enumerate(binary_stat_seal)
        )
        config_sha256 = _artifact_sha256(
            signer["config_sha256"], f"deploy config {validator}"
        )
        runtime_binding = _artifact_sha256(
            signer["runtime_binding_sha256"],
            f"deploy runtime binding {validator}",
        )
        expected_runtime_binding = _runtime_binding_sha256(
            str(deploy["validator_binary_sha256"]),
            normalized_stat_seal,
            config_sha256,
            str(deploy["restart_generation"]),
        )
        _require(runtime_binding == expected_runtime_binding,
                 f"deploy runtime binding {validator} is not derived from the "
                 "exact binary, config, and restart generation")
        lifecycle_binding = _artifact_sha256(
            signer["lifecycle_binding_sha256"],
            f"deploy lifecycle binding {validator}",
        )
        _require(lifecycle_binding == _lifecycle_binding_sha256(
            runtime_binding,
            str(deploy["restart_generation"]),
            validator,
            node_id,
        ), f"deploy lifecycle binding {validator} is not derived from its exact "
           "runtime and receipt identity")
        _require(_artifact_sha256(
            signer["native_verifier_binary_sha256"],
            f"deploy native verifier binary {validator}")
                 == expected_native_binary_sha256
                 and _artifact_sha256(
                     signer["native_verifier_source_sha256"],
                     f"deploy native verifier source {validator}")
                 == expected_native_source_sha256,
                 "deploy receipt signer native verifier is not pinned")
        native_receipt = _artifact_sha256(
            signer["native_verifier_receipt_sha256"],
            f"deploy native verifier receipt {validator}")
        _integer(signer["native_verifier_receipt_size_bytes"],
                 f"deploy native verifier receipt size {validator}", minimum=1)
        _require(signer["verification_result"] == "verified",
                 "deploy receipt signer native verification did not pass")
        _require(node_id not in node_ids, "deploy validator node IDs are aliased")
        key_tuple = tuple(sorted(key.items()))
        _require(key_tuple not in public_keys,
                 "deploy receipt signer public keys are aliased")
        _require(native_receipt not in native_receipts,
                 "deploy native verifier receipt was reused")
        node_ids.add(node_id)
        public_keys.add(key_tuple)
        native_receipts.add(native_receipt)
        receipt_signers[validator] = {
            "binary_stat_seal": list(normalized_stat_seal),
            "config_sha256": config_sha256,
            "lifecycle_binding_sha256": lifecycle_binding,
            "node_id": node_id,
            "public_key": key,
            "runtime_binding_sha256": runtime_binding,
        }
    return {
        "candidate": candidate_digest,
        "publication": publication_digest,
        "deploy": deploy_digest,
    }, {
        "qualification_receipt_id": qualification,
        "admission_receipt_id": str(deploy["admission_receipt_id"]),
        "controller_host_id": str(deploy["controller_host_id"]),
        "controller_installation_id": str(deploy["controller_installation_id"]),
        "controller_sha256": str(deploy["controller_sha256"]),
        "deployment_completed_at_unix_ms": deployed_ms,
        "restart_generation": str(deploy["restart_generation"]),
        "config_set_sha256": str(deploy["config_set_sha256"]),
        "topology_sha256": str(deploy["topology_sha256"]),
        "signed_genesis_sha256": str(deploy["signed_genesis_sha256"]),
        "supervisor_sha256": str(deploy["supervisor_sha256"]),
        "genesis_block_hash": deploy_genesis,
        "network_name": str(deploy["network_name"]),
        "chain_id": str(deploy["chain_id"]),
        "network_id": str(deploy["network_id"]),
        "protocol_version": int(deploy["protocol_version"]),
        "end_height": end_height,
        "end_hash": end_hash,
        "receipt_signers": receipt_signers,
        "native_verifier_receipts": native_receipts,
    }


def _validate_native_verifier(value: object, *, expected_binary_sha256: str,
                              expected_source_sha256: str) -> tuple[dict[str, object], str]:
    verifier = dict(_exact(value, NATIVE_VERIFIER_FIELDS, "native_verifier"))
    _require(verifier["protocol"] == NATIVE_VERIFIER_PROTOCOL,
             "native verifier protocol is wrong")
    _require(_artifact_sha256(verifier["binary_sha256"],
                              "native verifier binary digest")
             == _artifact_sha256(expected_binary_sha256,
                                  "trusted native verifier binary digest"),
             "native verifier binary differs from the trusted invocation")
    _require(_artifact_sha256(verifier["source_sha256"],
                              "native verifier source digest")
             == _artifact_sha256(expected_source_sha256,
                                  "trusted native verifier source digest"),
             "native verifier source differs from the trusted invocation")
    return verifier, _domain_digest(
        b"iroha.taira.public-v2-24h.native-verifier.v1\0", verifier)


def _inventory_prelude(artifact: Artifact, reference_value: object, *, kind: str,
                       schema: str, required_count: int | None = None
                       ) -> tuple[io.BytesIO, Mapping[str, object], int, object]:
    reference = _exact(reference_value, INVENTORY_REFERENCE_FIELDS,
                       f"{kind} inventory")
    _require(reference["kind"] == kind and reference["schema"] == schema,
             f"{kind} inventory kind or schema is wrong")
    _require(_artifact_sha256(reference["sha256"], f"{kind} inventory digest")
             == artifact.sha256, f"{kind} inventory digest mismatch")
    _require(_integer(reference["size_bytes"], f"{kind} inventory size", minimum=1)
             == artifact.size, f"{kind} inventory size mismatch")
    count = _integer(reference["record_count"], f"{kind} record count", minimum=1)
    if required_count is not None:
        _require(count == required_count, f"{kind} record count is not exact")
    stream = io.BytesIO(artifact.payload)
    header = _exact(_decode_json(stream.readline(), f"{kind} inventory header",
                                 canonical=True), INVENTORY_HEADER_FIELDS,
                    f"{kind} inventory header")
    _require(header["schema"] == schema
             and type(header["schema_version"]) is int
             and header["schema_version"] == 1
             and _integer(header["record_count"], f"{kind} header count", minimum=1)
             == count, f"{kind} inventory header is wrong")
    hasher = hashlib.sha256()
    hasher.update(f"iroha.taira.public-v2-24h.{kind}-records.v1\0".encode("ascii"))
    return stream, reference, count, hasher


def _finish_inventory(stream: io.BytesIO, reference: Mapping[str, object], count: int,
                      actual: int, hasher: object, kind: str) -> str:
    _require(actual == count, f"{kind} inventory is incomplete")
    _require(not stream.readline(), f"{kind} inventory has surplus records")
    records_sha256 = hasher.hexdigest()  # type: ignore[attr-defined]
    _require(_artifact_sha256(reference["records_sha256"],
                              f"{kind} records digest") == records_sha256,
             f"{kind} record-set digest is wrong")
    return records_sha256


def _validate_block_inventory(artifact: Artifact, reference: object,
                              deploy: Mapping[str, object],
                              used_native_receipts: set[str],
                              ) -> tuple[list[BlockEvidence], str]:
    stream, ref, count, hasher = _inventory_prelude(
        artifact, reference, kind="blocks", schema=BLOCK_SCHEMA)
    _require(2 <= count <= MAX_BLOCK_COUNT,
             "block inventory cannot prove deploy-to-soak advancement")
    blocks: list[BlockEvidence] = []
    seen_hashes: set[str] = set()
    seen_artifacts: set[str] = set()
    previous: BlockEvidence | None = None
    for index in range(count):
        line = stream.readline()
        if not line:
            break
        record = _exact(_decode_json(line, f"block record {index}", canonical=True),
                        BLOCK_RECORD_FIELDS, f"block record {index}")
        _require(_integer(record["index"], "block evidence index") == index,
                 "block evidence indexes are not exact and contiguous")
        height = _integer(record["height"], "block height", minimum=1)
        block_hash = _iroha_hash(record["block_hash"], "block hash", BLOCK_HASH_TYPE)
        parent_hash = _iroha_hash(record["parent_block_hash"], "parent block hash",
                                  BLOCK_HASH_TYPE)
        signed_wire = _artifact_sha256(record["signed_block_wire_sha256"],
                                       "SignedBlockWire artifact digest")
        _integer(record["signed_block_wire_size_bytes"],
                 "SignedBlockWire artifact size", minimum=1)
        finality = _artifact_sha256(record["finality_proof_sha256"],
                                    "finality artifact digest")
        _integer(record["finality_proof_size_bytes"],
                 "finality artifact size", minimum=1)
        native = _artifact_sha256(record["native_verifier_receipt_sha256"],
                                  "native verifier receipt digest")
        _integer(record["native_verifier_receipt_size_bytes"],
                 "native verifier receipt size", minimum=1)
        validators = record["finality_validators"]
        _require(isinstance(validators, list)
                 and all(isinstance(item, str) for item in validators)
                 and validators == sorted(set(validators))
                 and len(validators) == QUORUM
                 and set(validators) <= set(VALIDATORS),
                 "block finality validator set is not exactly one canonical quorum")
        _require(record["verification_result"] == "verified",
                 "block/finality native verification did not pass")
        _require(native not in used_native_receipts,
                 "native verifier receipt was reused")
        used_native_receipts.add(native)
        _require(block_hash not in seen_hashes,
                 "block evidence hash is duplicated")
        for digest in (signed_wire, finality, native):
            _require(digest not in seen_artifacts,
                     "block/finality/native evidence artifact is duplicated")
            seen_artifacts.add(digest)
        current = BlockEvidence(index, height, block_hash, parent_hash)
        if previous is None:
            _require(height == deploy["end_height"]
                     and block_hash == deploy["end_hash"],
                     "block inventory does not start at the deploy end tip")
        else:
            _require(height == previous.height + 1
                     and parent_hash == previous.block_hash,
                     "block inventory is not one contiguous deploy-descendant chain")
        seen_hashes.add(block_hash)
        blocks.append(current)
        previous = current
        hasher.update(line)  # type: ignore[attr-defined]
    return blocks, _finish_inventory(stream, ref, count, len(blocks), hasher, "blocks")


def _validate_anchor(value: object, *, deploy: Mapping[str, object],
                     blocks: Sequence[BlockEvidence], started_ms: int,
                     used_challenges: set[str], used_native_receipts: set[str]
                     ) -> tuple[str, int]:
    anchor = _exact(value, ANCHOR_FIELDS, "soak_anchor")
    _require(anchor["schema"] == "iroha.taira.public-v2-24h-soak-anchor.v1",
             "soak anchor schema is wrong")
    observed_start = _integer(anchor["observation_started_at_unix_ms"],
                              "anchor observation start", minimum=1)
    observed_end = _integer(anchor["observation_completed_at_unix_ms"],
                            "anchor observation end", minimum=1)
    _require(observed_start <= observed_end <= started_ms,
             "soak anchor was not closed before the workload started")
    _require(deploy["deployment_completed_at_unix_ms"] <= observed_start,
             "soak anchor predates deployment completion")
    _require(observed_end - observed_start <= MAX_OBSERVATION_WINDOW_MS,
             "soak anchor observation window exceeds its bound")
    anchor_gap = started_ms - observed_end
    _require(anchor_gap <= MAX_ANCHOR_TO_WORKLOAD_GAP_MS,
             "soak anchor is stale at workload start")
    for field in ("controller_host_id", "controller_installation_id"):
        _require(_identity_text(anchor[field], f"anchor {field}") == deploy[field],
                 "soak anchor controller identity differs from deployment")
    _require(_artifact_sha256(anchor["controller_sha256"],
                              "anchor controller digest")
             == deploy["controller_sha256"],
             "soak anchor controller binary differs from deployment")
    for field in ("controller_signing_key_id", "controller_receipt_sha256",
                  "controller_signature_sha256"):
        _artifact_sha256(anchor[field], f"anchor {field}")
    _require(_integer(anchor["deploy_end_height"], "anchor deploy end height", minimum=1)
             == deploy["end_height"]
             and _iroha_hash(anchor["deploy_end_block_hash"],
                             "anchor deploy end block hash", BLOCK_HASH_TYPE)
             == deploy["end_hash"], "soak anchor deploy tip is spliced")
    common_index = _integer(anchor["common_start_block_evidence_index"],
                            "anchor common-start block index", minimum=1)
    _require(common_index < len(blocks), "anchor common-start block is absent")
    common = blocks[common_index]
    _require(common.height > deploy["end_height"],
             "soak anchor did not prove common-chain advancement")
    validators = anchor["validators"]
    _require(isinstance(validators, list) and len(validators) == VALIDATOR_COUNT,
             "soak anchor must contain four validator attestations")
    seen_nodes: set[str] = set()
    seen_artifacts: set[str] = set()
    for index, validator in enumerate(VALIDATORS):
        record = _exact(validators[index], ANCHOR_VALIDATOR_FIELDS,
                        f"soak_anchor.validators[{index}]")
        _require(record["validator_id"] == validator,
                 "anchor validators are not in canonical order")
        signer = deploy["receipt_signers"][validator]
        node_id = _identity_text(record["node_id"], f"anchor node ID {validator}")
        _require(node_id == signer["node_id"],
                 "anchor node ID differs from the deploy handoff")
        _require(node_id not in seen_nodes, "anchor node IDs are aliased")
        seen_nodes.add(node_id)
        challenge = _hex_32(record["challenge_hex"], "anchor challenge")
        _require(challenge not in used_challenges, "anchor challenge was reused")
        used_challenges.add(challenge)
        attested = _integer(record["attested_at_unix_ms"],
                            "anchor attestation time", minimum=1)
        _require(observed_start <= attested <= observed_end,
                 "anchor attestation is outside its controller window")
        tip_index = _integer(record["tip_block_evidence_index"],
                             "anchor validator tip index", minimum=common_index)
        _require(tip_index < len(blocks), "anchor validator tip is absent")
        for digest_field, size_field in (
            ("attestation_sha256", "attestation_size_bytes"),
            ("ancestry_proof_sha256", "ancestry_proof_size_bytes"),
            ("native_verifier_receipt_sha256", "native_verifier_receipt_size_bytes"),
        ):
            digest = _artifact_sha256(record[digest_field],
                                      f"anchor {digest_field}")
            _integer(record[size_field], f"anchor {size_field}", minimum=1)
            _require(digest not in seen_artifacts,
                     "anchor evidence artifact identities are aliased")
            seen_artifacts.add(digest)
            if digest_field == "native_verifier_receipt_sha256":
                _require(digest not in used_native_receipts,
                         "native verifier receipt was reused")
                used_native_receipts.add(digest)
        _require(record["verification_result"] == "verified",
                 "anchor ancestry native verification did not pass")
    return (_domain_digest(b"iroha.taira.public-v2-24h.anchor.v1\0", anchor),
            anchor_gap)


def _lifecycle_window_digest(document: Mapping[str, object]) -> str:
    window = {
        field: document[field]
        for field in LIFECYCLE_FIELDS
        if field != "native_journal_verifier_receipt"
    }
    return _domain_digest(
        b"iroha.taira.public-v2-24h.lifecycle-window.v1\0", window)


def _validate_lifecycle(
    artifact: Artifact, reference_value: object, *,
    journal_artifact: Artifact, native_receipt_artifact: Artifact,
    deploy: Mapping[str, object], anchor_start_ms: int, started_ms: int,
    completed_ms: int, expected_native_binary_sha256: str,
    expected_native_source_sha256: str, used_native_receipts: set[str],
) -> tuple[dict[str, Mapping[str, object]], str, dict[str, object]]:
    reference = _exact(reference_value, LIFECYCLE_REFERENCE_FIELDS, "lifecycle")
    _require(reference["kind"] == "lifecycle-evidence"
             and reference["schema"] == LIFECYCLE_SCHEMA,
             "lifecycle kind or schema is wrong")
    _require(_artifact_sha256(reference["sha256"], "lifecycle artifact digest")
             == artifact.sha256, "lifecycle artifact digest mismatch")
    _require(_integer(reference["size_bytes"], "lifecycle artifact size", minimum=1)
             == artifact.size, "lifecycle artifact size mismatch")
    document = _exact(_decode_json(artifact.payload, "lifecycle evidence",
                                   canonical=True), LIFECYCLE_FIELDS,
                      "lifecycle evidence")
    _require(document["schema"] == LIFECYCLE_SCHEMA
             and type(document["schema_version"]) is int
             and document["schema_version"] == 1,
             "lifecycle evidence schema is wrong")
    _require(_integer(document["deployment_completed_at_unix_ms"],
                      "lifecycle deployment completion", minimum=1)
             == deploy["deployment_completed_at_unix_ms"],
             "lifecycle deployment generation is spliced")
    for field in ("restart_generation", "config_set_sha256", "topology_sha256",
                  "signed_genesis_sha256", "supervisor_sha256"):
        _require(_artifact_sha256(document[field], f"lifecycle {field}")
                 == deploy[field], f"lifecycle {field} differs from deployment")
    _require(_iroha_hash(document["genesis_block_hash"],
                         "lifecycle genesis block hash", BLOCK_HASH_TYPE)
             == deploy["genesis_block_hash"],
             "lifecycle genesis block differs from deployment")
    raw_windows = document["raw_windows"]
    _require(isinstance(raw_windows, list)
             and len(raw_windows) == VALIDATOR_COUNT,
             "lifecycle raw windows do not cover exactly four validators")
    raw_artifacts: set[str] = set()
    raw_record_sets: set[str] = set()
    for index, validator in enumerate(VALIDATORS):
        raw = _exact(
            raw_windows[index],
            LIFECYCLE_RAW_WINDOW_FIELDS,
            f"lifecycle raw window {index}",
        )
        _require(raw["validator_id"] == validator,
                 "lifecycle raw windows are not in canonical validator order")
        _require(_identity_text(raw["node_id"], "lifecycle raw-window node ID")
                 == deploy["receipt_signers"][validator]["node_id"],
                 "lifecycle raw-window node differs from deployment")
        _require(_artifact_sha256(
            raw["binding_sha256"], "lifecycle raw-window binding"
        ) == deploy["receipt_signers"][validator]["lifecycle_binding_sha256"],
                 "lifecycle raw-window binding differs from deployment")
        artifact_sha256 = _artifact_sha256(
            raw["artifact_sha256"], "lifecycle raw-window artifact"
        )
        records_sha256 = _artifact_sha256(
            raw["records_sha256"], "lifecycle raw-window records"
        )
        _integer(raw["artifact_size_bytes"],
                 "lifecycle raw-window artifact size", minimum=1)
        record_count = _integer(
            raw["record_count"], "lifecycle raw-window record count", minimum=2
        )
        baseline_sequence = _integer(
            raw["baseline_sequence"], "lifecycle raw-window baseline sequence"
        )
        terminal_sequence = _integer(
            raw["terminal_sequence"],
            "lifecycle raw-window terminal sequence",
            minimum=baseline_sequence + 2,
        )
        _require(record_count == terminal_sequence - baseline_sequence,
                 "lifecycle raw-window sequence interval is not exact")
        _require(artifact_sha256 not in raw_artifacts,
                 "lifecycle raw-window artifacts are aliased")
        _require(records_sha256 not in raw_record_sets,
                 "lifecycle raw-window record sets are aliased")
        raw_artifacts.add(artifact_sha256)
        raw_record_sets.add(records_sha256)
    checkpoints: dict[str, Mapping[str, object]] = {}
    validator_sets: dict[str, dict[str, Mapping[str, object]]] = {}
    for checkpoint_name in ("baseline", "terminal"):
        checkpoint = _exact(document[checkpoint_name], LIFECYCLE_CHECKPOINT_FIELDS,
                            f"lifecycle {checkpoint_name}")
        _integer(checkpoint["journal_sequence"],
                 f"lifecycle {checkpoint_name} journal sequence")
        _artifact_sha256(checkpoint["journal_chain_sha256"],
                         f"lifecycle {checkpoint_name} chain digest")
        validators = checkpoint["validators"]
        _require(isinstance(validators, list) and len(validators) == VALIDATOR_COUNT,
                 f"lifecycle {checkpoint_name} does not cover four validators")
        by_validator: dict[str, Mapping[str, object]] = {}
        for index, validator in enumerate(VALIDATORS):
            row = _exact(validators[index], LIFECYCLE_VALIDATOR_FIELDS,
                         f"lifecycle {checkpoint_name} validator {index}")
            _require(row["validator_id"] == validator,
                     "lifecycle validators are not in canonical order")
            _require(_identity_text(row["node_id"], "lifecycle node ID")
                     == deploy["receipt_signers"][validator]["node_id"],
                     "lifecycle node ID differs from the deploy handoff")
            for field in ("restart_count", "unexpected_exit_total"):
                _integer(row[field], f"lifecycle {checkpoint_name} {field}")
            for field in ("supervisor_generation", "process_generation"):
                _integer(row[field], f"lifecycle {checkpoint_name} {field}", minimum=1)
            by_validator[validator] = row
        checkpoints[checkpoint_name] = checkpoint
        validator_sets[checkpoint_name] = by_validator
    baseline = checkpoints["baseline"]
    terminal = checkpoints["terminal"]
    baseline_ms = _integer(baseline["captured_at_unix_ms"],
                           "lifecycle baseline time", minimum=1)
    terminal_ms = _integer(terminal["captured_at_unix_ms"],
                           "lifecycle terminal time", minimum=1)
    _require(deploy["deployment_completed_at_unix_ms"] <= baseline_ms
             <= anchor_start_ms <= started_ms,
             "lifecycle baseline does not precede the controller anchor")
    _require(terminal_ms == completed_ms,
             "lifecycle terminal does not close the evidence window")
    _require(int(terminal["journal_sequence"]) > int(baseline["journal_sequence"])
             and terminal["journal_chain_sha256"] != baseline["journal_chain_sha256"],
             "lifecycle journal did not advance from baseline to terminal")
    for validator in VALIDATORS:
        before = validator_sets["baseline"][validator]
        after = validator_sets["terminal"][validator]
        for field in ("restart_count", "supervisor_generation",
                      "process_generation", "unexpected_exit_total"):
            _require(after[field] == before[field],
                     "zero-unexpected-exit lifecycle generations changed")

    journal_reference = document["journal_inventory"]
    stream, journal_ref, journal_count, journal_hasher = _inventory_prelude(
        journal_artifact, journal_reference, kind="lifecycle-journal",
        schema=LIFECYCLE_JOURNAL_SCHEMA)
    _require(journal_count >= VALIDATOR_COUNT * 2,
             "lifecycle journal does not cover every validator twice")
    baseline_sequence = int(baseline["journal_sequence"])
    terminal_sequence = int(terminal["journal_sequence"])
    prior_sequence = baseline_sequence
    validator_rows: dict[str, list[Mapping[str, object]]] = {
        validator: [] for validator in VALIDATORS
    }
    derived_restarts = 0
    derived_exits = 0
    actual_journal_count = 0
    for index in range(journal_count):
        line = stream.readline()
        if not line:
            break
        row = _exact(
            _decode_json(line, f"lifecycle journal record {index}", canonical=True),
            LIFECYCLE_JOURNAL_RECORD_FIELDS,
            f"lifecycle journal record {index}",
        )
        _require(_integer(row["index"], "lifecycle journal index") == index,
                 "lifecycle journal indexes are not exact and contiguous")
        sequence = _integer(row["journal_sequence"],
                            "lifecycle journal sequence")
        _require(prior_sequence < sequence <= terminal_sequence,
                 "lifecycle journal sequence is not exact and monotonic")
        prior_sequence = sequence
        observed_ms = _integer(row["observed_at_unix_ms"],
                               "lifecycle journal observation", minimum=1)
        _require(baseline_ms <= observed_ms <= terminal_ms,
                 "lifecycle journal observation escapes the lifecycle window")
        validator = row["validator_id"]
        _require(isinstance(validator, str) and validator in VALIDATORS,
                 "lifecycle journal validator is unknown")
        _require(_identity_text(row["node_id"], "lifecycle journal node ID")
                 == deploy["receipt_signers"][validator]["node_id"],
                 "lifecycle journal node differs from deployment")
        event = row["event"]
        _require(type(event) is str
                 and event in {"healthy", "restart", "unexpected_exit"},
                 "lifecycle journal event is not exact")
        derived_restarts += int(event == "restart")
        derived_exits += int(event == "unexpected_exit")
        baseline_row = validator_sets["baseline"][validator]
        for field in ("restart_count", "unexpected_exit_total"):
            _integer(row[field], f"lifecycle journal {field}")
            _require(row[field] == baseline_row[field],
                     "lifecycle journal proves lifecycle counter drift")
        for field in ("supervisor_generation", "process_generation"):
            _integer(row[field], f"lifecycle journal {field}", minimum=1)
            _require(row[field] == baseline_row[field],
                     "lifecycle journal proves lifecycle generation drift")
        validator_rows[validator].append(row)
        journal_hasher.update(line)  # type: ignore[attr-defined]
        actual_journal_count += 1
    journal_records_sha256 = _finish_inventory(
        stream, journal_ref, journal_count, actual_journal_count,
        journal_hasher, "lifecycle-journal")
    _require(prior_sequence == terminal_sequence,
             "lifecycle journal does not reach the terminal checkpoint")
    for validator in VALIDATORS:
        _require(len(validator_rows[validator]) >= 2,
                 "lifecycle journal does not cover every validator twice")
        for edge, checkpoint_name in ((validator_rows[validator][0], "baseline"),
                                      (validator_rows[validator][-1], "terminal")):
            checkpoint_row = validator_sets[checkpoint_name][validator]
            for field in ("restart_count", "supervisor_generation",
                          "process_generation", "unexpected_exit_total"):
                _require(edge[field] == checkpoint_row[field],
                         "lifecycle journal edge differs from checkpoint")
    declared_restarts = _integer(document["restart_events"],
                                 "lifecycle restart events")
    declared_exits = _integer(document["unexpected_exit_events"],
                              "lifecycle unexpected exits")
    _require(declared_restarts == derived_restarts == 0,
             "lifecycle journal derives a restart")
    _require(declared_exits == derived_exits == 0,
             "lifecycle journal derives an unexpected exit")

    receipt_reference = _exact(
        document["native_journal_verifier_receipt"], ARTIFACT_IDENTITY_FIELDS,
        "lifecycle native-verifier receipt reference")
    receipt_digest = _artifact_sha256(
        receipt_reference["sha256"], "lifecycle native-verifier receipt digest")
    _require(receipt_digest == native_receipt_artifact.sha256,
             "lifecycle native-verifier receipt digest mismatch")
    _require(_integer(receipt_reference["size_bytes"],
                      "lifecycle native-verifier receipt size", minimum=1)
             == native_receipt_artifact.size,
             "lifecycle native-verifier receipt size mismatch")
    native_receipt = _exact(
        _decode_json(native_receipt_artifact.payload,
                     "lifecycle native-verifier receipt", canonical=True),
        LIFECYCLE_JOURNAL_RECEIPT_FIELDS,
        "lifecycle native-verifier receipt")
    _require(native_receipt["schema"] == LIFECYCLE_JOURNAL_RECEIPT_SCHEMA
             and type(native_receipt["schema_version"]) is int
             and native_receipt["schema_version"] == 1
             and native_receipt["protocol"] == NATIVE_JOURNAL_VERIFIER_PROTOCOL,
             "lifecycle native-verifier receipt identity is wrong")
    _require(_artifact_sha256(native_receipt["verifier_binary_sha256"],
                              "lifecycle native-verifier binary")
             == expected_native_binary_sha256
             and _artifact_sha256(native_receipt["verifier_source_sha256"],
                                  "lifecycle native-verifier source")
             == expected_native_source_sha256,
             "lifecycle native verifier is not pinned")
    _require(_artifact_sha256(native_receipt["journal_artifact_sha256"],
                              "verified lifecycle journal artifact")
             == journal_artifact.sha256
             and _integer(native_receipt["journal_artifact_size_bytes"],
                          "verified lifecycle journal size", minimum=1)
             == journal_artifact.size
             and _artifact_sha256(native_receipt["journal_records_sha256"],
                                  "verified lifecycle journal records")
             == journal_records_sha256
             and _integer(native_receipt["journal_record_count"],
                          "verified lifecycle journal count", minimum=1)
             == journal_count,
             "lifecycle native-verifier receipt does not bind the journal")
    window_sha256 = _lifecycle_window_digest(document)
    _require(_artifact_sha256(native_receipt["lifecycle_window_sha256"],
                              "verified lifecycle window") == window_sha256,
             "lifecycle native-verifier receipt does not bind the window")
    _require(native_receipt["verification_result"] == "verified",
             "lifecycle native verification did not pass")
    _require(receipt_digest not in used_native_receipts,
             "native verifier receipt was reused")
    used_native_receipts.add(receipt_digest)
    identity_sha256 = _domain_digest(
        b"iroha.taira.public-v2-24h.lifecycle.v1\0", document)
    _require(_artifact_sha256(reference["identity_sha256"],
                              "lifecycle identity digest") == identity_sha256,
             "lifecycle identity digest mismatch")
    return validator_sets["baseline"], identity_sha256, {
        "journal_artifact_sha256": journal_artifact.sha256,
        "journal_records_sha256": journal_records_sha256,
        "journal_record_count": journal_count,
        "native_verifier_receipt_sha256": receipt_digest,
        "window_sha256": window_sha256,
    }


def _validate_samples(value: object, *, started_ms: int, completed_ms: int,
                      blocks: Sequence[BlockEvidence], deploy: Mapping[str, object],
                      lifecycle_baseline: Mapping[str, Mapping[str, object]],
                      used_challenges: set[str], used_native_receipts: set[str]
                      ) -> tuple[list[Mapping[str, object]], dict[str, int], str]:
    _require(isinstance(value, list), "samples must be an array")
    _require(DURATION_MS // SAMPLE_INTERVAL_MS <= len(value) <= MAX_SAMPLE_COUNT,
             "sample inventory cannot cover the workload and bounded drain")
    samples: list[Mapping[str, object]] = []
    prior_observation_end = started_ms
    prior_observation_start = started_ms
    prior_applied = 0
    prior_common_index = 0
    maximum_window = 0
    maximum_gap = 0
    for sample_index, raw_sample in enumerate(value):
        sample = _exact(raw_sample, SAMPLE_FIELDS, f"samples[{sample_index}]")
        scheduled = _integer(sample["scheduled_elapsed_ms"],
                             "sample scheduled elapsed ms", minimum=SAMPLE_INTERVAL_MS)
        _require(scheduled == (sample_index + 1) * SAMPLE_INTERVAL_MS,
                 "sample schedule is not exact and monotonic")
        observed_start = _integer(sample["observation_started_at_unix_ms"],
                                  "sample observation start", minimum=1)
        observed_end = _integer(sample["observation_completed_at_unix_ms"],
                                "sample observation end", minimum=1)
        target_wall = started_ms + scheduled
        _require(target_wall <= observed_start
                 <= target_wall + MAX_OBSERVATION_START_LATENESS_MS,
                 "sample observation did not start within its lateness bound")
        _require(prior_observation_end <= observed_start <= observed_end <= completed_ms,
                 "sample observation windows overlap or escape completion")
        gap = observed_start - prior_observation_start
        _require(0 < gap <= MAXIMUM_SAMPLE_GAP_MS,
                 "actual sample observation gap exceeds its bound")
        maximum_gap = max(maximum_gap, gap)
        window = observed_end - observed_start
        _require(window <= MAX_OBSERVATION_WINDOW_MS,
                 "sample observation window exceeds its bound")
        maximum_window = max(maximum_window, window)
        applied = _integer(sample["applied_transfer_count"],
                           "sample Applied transfer count")
        _require(prior_applied <= applied <= REQUIRED_TRANSFER_COUNT,
                 "sample Applied counter regressed or overflowed")
        _require(_integer(sample["failed_transfer_count"],
                          "sample failed transfer count") == 0,
                 "valid signed-transfer workload records a failed transfer")
        common_index = _integer(sample["common_block_evidence_index"],
                                "sample common block index", minimum=1)
        _require(prior_common_index <= common_index < len(blocks),
                 "sample common-chain block regressed or is absent")
        statuses = sample["validators"]
        _require(isinstance(statuses, list) and len(statuses) == VALIDATOR_COUNT,
                 "sample must contain exactly four validator attestations")
        for validator_index, validator in enumerate(VALIDATORS):
            status_record = _exact(statuses[validator_index], VALIDATOR_SAMPLE_FIELDS,
                                   f"samples[{sample_index}].validators[{validator_index}]")
            _require(status_record["validator_id"] == validator,
                     "sample validators are not in canonical order")
            _require(_identity_text(status_record["node_id"], "sample node ID")
                     == deploy["receipt_signers"][validator]["node_id"],
                     "sample node ID differs from the deploy handoff")
            challenge = _hex_32(status_record["challenge_hex"], "sample challenge")
            _require(challenge not in used_challenges,
                     "sample attestation challenge was reused")
            used_challenges.add(challenge)
            attested = _integer(status_record["attested_at_unix_ms"],
                                "sample attestation time", minimum=1)
            _require(observed_start <= attested <= observed_end,
                     "sample attestation is outside its observation window")
            tip_index = _integer(status_record["tip_block_evidence_index"],
                                 "sample validator tip index", minimum=common_index)
            _require(tip_index < len(blocks), "sample validator tip is absent")
            capacity = _integer(status_record["queue_capacity"],
                                "validator queue capacity", minimum=1)
            depth = _integer(status_record["queue_depth"], "validator queue depth")
            dropped = _integer(status_record["queue_dropped_total"],
                               "validator queue drops")
            _require(depth < capacity and status_record["queue_saturated"] is False
                     and dropped == 0, "validator queue saturated or dropped work")
            baseline = lifecycle_baseline[validator]
            for field in ("restart_count", "supervisor_generation",
                          "process_generation", "unexpected_exit_total"):
                _integer(status_record[field], f"sample {field}")
                _require(status_record[field] == baseline[field],
                         "sample lifecycle counters differ from the baseline")
            _require(status_record["restart_required"] is False
                     and status_record["last_restart_successful"] is True
                     and status_record["healthy"] is True,
                     "validator health or restart state is not clean")
            _artifact_sha256(status_record["attestation_sha256"],
                             "sample attestation artifact digest")
            _integer(status_record["attestation_size_bytes"],
                     "sample attestation artifact size", minimum=1)
            native = _artifact_sha256(status_record["native_verifier_receipt_sha256"],
                                      "sample native verifier receipt digest")
            _integer(status_record["native_verifier_receipt_size_bytes"],
                     "sample native verifier receipt size", minimum=1)
            _require(native not in used_native_receipts,
                     "native verifier receipt was reused")
            used_native_receipts.add(native)
            _require(status_record["verification_result"] == "verified",
                     "sample attestation native verification did not pass")
        samples.append(sample)
        prior_observation_end = observed_end
        prior_observation_start = observed_start
        prior_applied = applied
        prior_common_index = common_index
    _require(int(samples[-1]["scheduled_elapsed_ms"]) >= DURATION_MS,
             "last sample does not reach the workload boundary")
    _require(int(samples[-1]["observation_completed_at_unix_ms"]) == completed_ms,
             "last observation does not close the evidence window")
    _require(prior_applied == REQUIRED_TRANSFER_COUNT,
             "terminal sample is not exactly 432,000 Applied transfers")
    sample_digest = _domain_digest(
        b"iroha.taira.public-v2-24h.sample-set.v1\0", samples)
    return samples, {
        "sample_count": len(samples),
        "maximum_gap": maximum_gap,
        "maximum_window": maximum_window,
        "confirmed_count": prior_applied,
    }, sample_digest


def _validate_submission_inventory(
    artifact: Artifact, reference: object, *, started_ms: int, completed_ms: int,
    deploy: Mapping[str, object], used_native_receipts: set[str],
) -> tuple[list[SubmissionEvidence], str]:
    stream, ref, count, hasher = _inventory_prelude(
        artifact, reference, kind="submissions", schema=SUBMISSION_SCHEMA,
        required_count=REQUIRED_TRANSFER_COUNT)
    submissions: list[SubmissionEvidence] = []
    signed_hashes: set[str] = set()
    receipt_digests: set[str] = set()
    for index in range(count):
        line = stream.readline()
        if not line:
            break
        record = _exact(
            _decode_json(line, f"submission record {index}", canonical=True),
            SUBMISSION_RECORD_FIELDS, f"submission record {index}")
        _require(_integer(record["index"], "submission receipt index") == index,
                 "submission receipt indexes are not exact and contiguous")
        signed_hash = _iroha_hash(record["signed_transaction_hash"],
                                  "submission signed transaction hash",
                                  SIGNED_TRANSACTION_HASH_TYPE)
        entrypoint_hash = _iroha_hash(record["entrypoint_hash"],
                                      "submission entrypoint hash",
                                      ENTRYPOINT_HASH_TYPE)
        _require(signed_hash == entrypoint_hash,
                 "external signed-transaction and entrypoint hashes differ")
        _require(signed_hash not in signed_hashes,
                 "submission signed transaction hash is duplicated")
        signed_hashes.add(signed_hash)
        receipt = _artifact_sha256(record["receipt_sha256"],
                                   "submission receipt artifact digest")
        _integer(record["receipt_size_bytes"],
                 "submission receipt artifact size", minimum=1)
        _require(receipt not in receipt_digests,
                 "submission receipt artifact is duplicated")
        receipt_digests.add(receipt)
        submitted_ms = _integer(record["submitted_at_unix_ms"],
                                "submission receipt time", minimum=1)
        _require(started_ms - MAX_WALL_CLOCK_SKEW_MS <= submitted_ms <= completed_ms,
                 "submission receipt time is outside the evidence window")
        _require(deploy["deployment_completed_at_unix_ms"] <= submitted_ms,
                 "submission receipt predates deployment completion")
        submitted_height = _integer(record["submitted_at_height"],
                                    "submission receipt height", minimum=1)
        validator = record["receipt_signer_validator_id"]
        _require(validator in VALIDATORS, "submission receipt signer is not a validator")
        signer = deploy["receipt_signers"][validator]
        _require(record["receipt_signer_node_id"] == signer["node_id"],
                 "submission receipt signer node differs from deployment")
        _require(dict(_public_key(record["receipt_signer_public_key"],
                                  "submission receipt signer public key"))
                 == signer["public_key"],
                 "submission receipt signer key differs from deployment")
        native = _artifact_sha256(record["native_verifier_receipt_sha256"],
                                  "submission native verifier receipt digest")
        _integer(record["native_verifier_receipt_size_bytes"],
                 "submission native verifier receipt size", minimum=1)
        _require(native not in used_native_receipts,
                 "native verifier receipt was reused")
        used_native_receipts.add(native)
        _require(record["verification_result"] == "verified",
                 "submission receipt native verification did not pass")
        submissions.append(SubmissionEvidence(
            index, signed_hash, entrypoint_hash, submitted_ms, submitted_height,
            str(validator)))
        hasher.update(line)  # type: ignore[attr-defined]
    records_sha256 = _finish_inventory(
        stream, ref, count, len(submissions), hasher, "submissions")
    return submissions, records_sha256


def _validate_status_inventory(
    artifact: Artifact, reference: object, *, started_ms: int, completed_ms: int,
    samples: Sequence[Mapping[str, object]], blocks: Sequence[BlockEvidence],
    used_native_receipts: set[str],
) -> tuple[list[StatusEvidence], str]:
    stream, ref, count, hasher = _inventory_prelude(
        artifact, reference, kind="statuses", schema=STATUS_SCHEMA,
        required_count=REQUIRED_TRANSFER_COUNT)
    statuses: list[StatusEvidence] = []
    signed_hashes: set[str] = set()
    response_digests: set[str] = set()
    previous_order: tuple[int, str] | None = None
    for index in range(count):
        line = stream.readline()
        if not line:
            break
        record = _exact(
            _decode_json(line, f"Applied status record {index}", canonical=True),
            STATUS_RECORD_FIELDS, f"Applied status record {index}")
        _require(_integer(record["index"], "Applied status index") == index,
                 "Applied status indexes are not exact and contiguous")
        signed_hash = _iroha_hash(record["signed_transaction_hash"],
                                  "Applied signed transaction hash",
                                  SIGNED_TRANSACTION_HASH_TYPE)
        entrypoint_hash = _iroha_hash(record["entrypoint_hash"],
                                      "Applied entrypoint hash",
                                      ENTRYPOINT_HASH_TYPE)
        _require(signed_hash == entrypoint_hash,
                 "Applied signed-transaction and entrypoint hashes differ")
        _require(signed_hash not in signed_hashes,
                 "Applied signed transaction hash is duplicated")
        signed_hashes.add(signed_hash)
        _require(record["result"] == "Applied",
                 "global transaction status is not Applied")
        observed_ms = _integer(record["observed_at_unix_ms"],
                               "Applied status observation time", minimum=1)
        _require(started_ms <= observed_ms <= completed_ms,
                 "Applied status observation is outside the evidence window")
        order = (observed_ms, signed_hash)
        _require(previous_order is None or order > previous_order,
                 "Applied status inventory is not in canonical observation order")
        previous_order = order
        observation_index = _integer(record["observation_index"],
                                     "Applied sample index")
        _require(observation_index < len(samples),
                 "Applied status sample index is outside the sample inventory")
        lower = (started_ms if observation_index == 0 else
                 int(samples[observation_index - 1]["observation_completed_at_unix_ms"]))
        upper = int(samples[observation_index]["observation_completed_at_unix_ms"])
        _require(lower < observed_ms <= upper,
                 "Applied status is not assigned to its exact observation interval")
        block_index = _integer(record["block_evidence_index"],
                               "Applied status block index", minimum=1)
        _require(block_index < len(blocks),
                 "Applied status block evidence is absent")
        sample_common_index = _integer(
            samples[observation_index]["common_block_evidence_index"],
            "Applied sample common block index", minimum=1)
        _require(block_index <= sample_common_index,
                 "Applied status block is newer than its observation sample")
        response = _artifact_sha256(record["response_sha256"],
                                    "Applied status response artifact digest")
        _integer(record["response_size_bytes"],
                 "Applied status response artifact size", minimum=1)
        _require(response not in response_digests,
                 "Applied status response artifact is duplicated")
        response_digests.add(response)
        native = _artifact_sha256(record["native_verifier_receipt_sha256"],
                                  "Applied status native verifier receipt digest")
        _integer(record["native_verifier_receipt_size_bytes"],
                 "Applied status native verifier receipt size", minimum=1)
        _require(native not in used_native_receipts,
                 "native verifier receipt was reused")
        used_native_receipts.add(native)
        _require(record["verification_result"] == "verified",
                 "Applied status native verification did not pass")
        statuses.append(StatusEvidence(
            index, signed_hash, entrypoint_hash, observed_ms,
            observation_index, block_index))
        hasher.update(line)  # type: ignore[attr-defined]
    records_sha256 = _finish_inventory(
        stream, ref, count, len(statuses), hasher, "statuses")
    return statuses, records_sha256


def _validate_workload_inventory(
    artifact: Artifact, reference_value: object, *, started_ms: int,
    completed_ms: int, submissions: Sequence[SubmissionEvidence],
    statuses: Sequence[StatusEvidence], blocks: Sequence[BlockEvidence],
) -> tuple[str, str, dict[str, int]]:
    reference = _exact(reference_value, WORKLOAD_REFERENCE_FIELDS,
                       "workload_inventory")
    base_reference = {field: reference[field] for field in INVENTORY_REFERENCE_FIELDS}
    stream, checked, count, hasher = _inventory_prelude(
        artifact, base_reference, kind="workload", schema=WORKLOAD_SCHEMA,
        required_count=REQUIRED_TRANSFER_COUNT)
    _require(len(submissions) == count and len(statuses) == count,
             "workload, submission, and Applied inventories have different counts")
    seen_signed_hashes: set[str] = set()
    seen_versioned_bytes: set[str] = set()
    seen_submission_indexes: set[int] = set()
    seen_status_indexes: set[int] = set()
    first_hash = ""
    last_hash = ""
    prior_request_start = -1
    maximum_lateness = 0
    maximum_request_window = 0
    for sequence in range(count):
        line = stream.readline()
        if not line:
            break
        record = _exact(_decode_json(line, f"workload record {sequence}", canonical=True),
                        WORKLOAD_RECORD_FIELDS, f"workload record {sequence}")
        _require(_integer(record["sequence"], "workload sequence") == sequence,
                 "workload sequence is not exact and contiguous")
        _require(record["operation"] == "transfer",
                 "workload record is not a signed transfer")
        scheduled = _integer(record["scheduled_elapsed_ms"],
                             "workload scheduled elapsed ms")
        _require(scheduled == sequence * SLOT_INTERVAL_MS,
                 "workload does not enumerate the exact 200ms slot schedule")
        request_start = _integer(record["request_started_elapsed_ms"],
                                 "submission request start elapsed ms")
        request_end = _integer(record["request_completed_elapsed_ms"],
                               "submission request completion elapsed ms")
        lateness = request_start - scheduled
        _require(0 <= lateness <= MAX_SUBMISSION_START_LATENESS_MS
                 and request_start < DURATION_MS,
                 "submission request missed its bounded scheduled start")
        _require(request_start >= prior_request_start,
                 "submission request starts are not monotonic")
        request_window = request_end - request_start
        _require(0 <= request_window <= MAX_SUBMISSION_REQUEST_WINDOW_MS
                 and request_end <= completed_ms - started_ms,
                 "submission request window exceeds its bound")
        prior_request_start = request_start
        maximum_lateness = max(maximum_lateness, lateness)
        maximum_request_window = max(maximum_request_window, request_window)
        signed_hash = _iroha_hash(record["signed_transaction_hash"],
                                  "workload signed transaction hash",
                                  SIGNED_TRANSACTION_HASH_TYPE)
        entrypoint_hash = _iroha_hash(record["entrypoint_hash"],
                                      "workload entrypoint hash",
                                      ENTRYPOINT_HASH_TYPE)
        _require(signed_hash == entrypoint_hash,
                 "external signed-transaction and entrypoint hashes differ")
        _require(signed_hash not in seen_signed_hashes,
                 "workload signed transaction hash is duplicated")
        seen_signed_hashes.add(signed_hash)
        versioned_sha256 = _artifact_sha256(
            record["versioned_signed_transaction_sha256"],
            "versioned SignedTransaction bytes digest")
        _integer(record["versioned_signed_transaction_size_bytes"],
                 "versioned SignedTransaction bytes size", minimum=1)
        _require(versioned_sha256 not in seen_versioned_bytes,
                 "versioned SignedTransaction artifact is duplicated")
        seen_versioned_bytes.add(versioned_sha256)
        submission_index = _integer(record["submission_receipt_index"],
                                    "submission receipt index")
        status_index = _integer(record["applied_status_index"],
                                "Applied status index")
        block_index = _integer(record["block_evidence_index"],
                               "workload block evidence index", minimum=1)
        _require(submission_index < count and status_index < count
                 and block_index < len(blocks),
                 "workload evidence index is outside its inventory")
        _require(submission_index not in seen_submission_indexes
                 and status_index not in seen_status_indexes,
                 "workload receipt/status evidence index is reused")
        seen_submission_indexes.add(submission_index)
        seen_status_indexes.add(status_index)
        submission = submissions[submission_index]
        status = statuses[status_index]
        _require((submission.signed_hash, submission.entrypoint_hash)
                 == (signed_hash, entrypoint_hash),
                 "submission receipt does not bind the workload transaction")
        _require((status.signed_hash, status.entrypoint_hash, status.block_index)
                 == (signed_hash, entrypoint_hash, block_index),
                 "Applied status/block evidence does not bind the workload transaction")
        _require(blocks[0].height <= submission.submitted_height
                 <= blocks[block_index].height,
                 "submission height is outside deploy-to-inclusion bounds")
        _require(status.observed_ms >= submission.submitted_ms,
                 "Applied observation predates the submission receipt")
        expected_wall_start = started_ms + request_start
        expected_wall_end = started_ms + request_end
        _require(expected_wall_start - MAX_WALL_CLOCK_SKEW_MS
                 <= submission.submitted_ms
                 <= expected_wall_end + MAX_WALL_CLOCK_SKEW_MS,
                 "submission receipt wall time is outside its monotonic request window")
        _require(status.observed_ms <= completed_ms,
                 "Applied status is after evidence completion")
        if sequence == 0:
            first_hash = signed_hash
        last_hash = signed_hash
        hasher.update(line)  # type: ignore[attr-defined]
    records_sha256 = _finish_inventory(
        stream, checked, count, len(seen_signed_hashes), hasher, "workload")
    _require(len(seen_submission_indexes) == count
             and len(seen_status_indexes) == count,
             "workload does not cover every receipt and status exactly once")
    _require((count - 1) * SLOT_INTERVAL_MS == DURATION_MS - SLOT_INTERVAL_MS,
             "workload slots do not span exactly 86,400,000ms")
    _require(_iroha_hash(reference["first_signed_transaction_hash"],
                         "first workload signed transaction hash",
                         SIGNED_TRANSACTION_HASH_TYPE) == first_hash,
             "first workload signed transaction hash is wrong")
    _require(_iroha_hash(reference["last_signed_transaction_hash"],
                         "last workload signed transaction hash",
                         SIGNED_TRANSACTION_HASH_TYPE) == last_hash,
             "last workload signed transaction hash is wrong")
    return artifact.sha256, records_sha256, {
        "maximum_lateness": maximum_lateness,
        "maximum_request_window": maximum_request_window,
    }


def _cross_validate_sample_counts(
    samples: Sequence[Mapping[str, object]], statuses: Sequence[StatusEvidence],
) -> None:
    counts = [0] * len(samples)
    for status in statuses:
        counts[status.observation_index] += 1
    cumulative = 0
    for index, sample in enumerate(samples):
        cumulative += counts[index]
        _require(sample["applied_transfer_count"] == cumulative,
                 "sample Applied counter is not derived from exact status evidence")
    _require(cumulative == REQUIRED_TRANSFER_COUNT,
             "Applied status inventory does not contain exactly 432,000 transfers")


def _validate_structural_evidence(
    receipt: object, *, receipt_artifact: Artifact,
    expected_source: Mapping[str, object], expected_binary_sha256: str,
    expected_native_verifier_binary_sha256: str,
    expected_native_verifier_source_sha256: str,
    authority_envelope: Artifact, durable_admission_receipt: Artifact,
    candidate_handoff: Artifact, publication_handoff: Artifact,
    deploy_handoff: Artifact, workload_inventory: Artifact,
    submission_receipt_inventory: Artifact, applied_status_inventory: Artifact,
    block_evidence_inventory: Artifact, lifecycle_evidence: Artifact,
    lifecycle_journal: Artifact,
    lifecycle_native_verifier_receipt: Artifact,
) -> StructuralResult:
    decoded_receipt = _decode_json(receipt_artifact.payload, "receipt artifact",
                                   canonical=True)
    _require(_json_exact_equal(decoded_receipt, receipt),
             "receipt object does not match the exact receipt artifact bytes")
    top = _exact(receipt, TOP_LEVEL_FIELDS, "receipt")
    _require(top["schema"] == SCHEMA
             and type(top["schema_version"]) is int
             and top["schema_version"] == SCHEMA_VERSION
             and top["result"] == RESULT,
             "receipt schema or terminal result is wrong")
    source = _source_identity(top["source"], "source")
    trusted_source = _source_identity(expected_source, "trusted invocation source")
    _require(source == trusted_source,
             "receipt source differs from the trusted invocation")
    iroha3d_sha256 = _artifact_sha256(expected_binary_sha256,
                                      "trusted iroha3d digest")

    network = _exact(top["network"], NETWORK_FIELDS, "network")
    network_genesis = _iroha_hash(
        network["genesis_block_hash"], "network genesis block hash", BLOCK_HASH_TYPE
    )
    expected_network_id = taira_constants.network_id_from_genesis_hash(
        str(network_genesis["value"])
    )
    _require(_json_exact_equal(
        {field: network[field] for field in NETWORK_FIELDS
         if field != "genesis_block_hash"},
        {
            "name": taira_constants.NETWORK_NAME,
            "deployment": "public",
            "chain_id": taira_constants.CHAIN_ID,
            "network_id": expected_network_id,
            "protocol_version": PROTOCOL_VERSION,
        }),
             "network identity is not exact public Taira v2")
    profile = _exact(top["profile"], PROFILE_FIELDS, "profile")
    _require(_json_exact_equal(profile, {
        "duration_ms": DURATION_MS,
        "validator_count": VALIDATOR_COUNT,
        "quorum": QUORUM,
        "target_tps": TARGET_TPS,
        "slot_interval_ms": SLOT_INTERVAL_MS,
        "required_transfer_slots": REQUIRED_TRANSFER_COUNT,
        "sample_interval_ms": SAMPLE_INTERVAL_MS,
        "maximum_sample_gap_ms": MAXIMUM_SAMPLE_GAP_MS,
        "maximum_observation_start_lateness_ms": MAX_OBSERVATION_START_LATENESS_MS,
        "maximum_observation_window_ms": MAX_OBSERVATION_WINDOW_MS,
        "maximum_anchor_to_workload_gap_ms": MAX_ANCHOR_TO_WORKLOAD_GAP_MS,
        "maximum_submission_start_lateness_ms": MAX_SUBMISSION_START_LATENESS_MS,
        "maximum_submission_request_window_ms": MAX_SUBMISSION_REQUEST_WINDOW_MS,
        "maximum_confirmation_drain_ms": MAX_CONFIRMATION_DRAIN_MS,
        "maximum_wall_clock_skew_ms": MAX_WALL_CLOCK_SKEW_MS,
        "workload": WORKLOAD,
        "fault_injection": FAULT_INJECTION,
    }), "receipt does not declare the fixed public soak profile")

    trusted_native_binary = _artifact_sha256(
        expected_native_verifier_binary_sha256,
        "trusted native verifier binary digest")
    trusted_native_source = _artifact_sha256(
        expected_native_verifier_source_sha256,
        "trusted native verifier source digest")

    handoff_digests, deploy = _validate_prerequisites(
        top, source=source, expected_binary_sha256=iroha3d_sha256,
        candidate_artifact=candidate_handoff,
        publication_artifact=publication_handoff,
        deploy_artifact=deploy_handoff, network=network,
        expected_native_binary_sha256=trusted_native_binary,
        expected_native_source_sha256=trusted_native_source)
    native_verifier, native_verifier_identity_sha256 = _validate_native_verifier(
        top["native_verifier"],
        expected_binary_sha256=expected_native_verifier_binary_sha256,
        expected_source_sha256=expected_native_verifier_source_sha256)

    completion = _exact(top["completion"], COMPLETION_FIELDS, "completion")
    started_ms = _integer(completion["workload_started_at_unix_ms"],
                          "workload start", minimum=1)
    workload_ended_ms = _integer(completion["workload_ended_at_unix_ms"],
                                 "workload end", minimum=1)
    completed_ms = _integer(completion["evidence_completed_at_unix_ms"],
                            "evidence completion", minimum=1)
    _require(workload_ended_ms == started_ms + DURATION_MS,
             "workload wall window is not exactly 86,400,000ms")
    _require(deploy["deployment_completed_at_unix_ms"] <= started_ms,
             "workload starts before deployment completion")
    drain_ms = completed_ms - workload_ended_ms
    _require(0 <= drain_ms <= MAX_CONFIRMATION_DRAIN_MS,
             "post-workload confirmation drain exceeds its bound")
    _require(completed_ms <= MAX_TIMESTAMP_MS,
             "completion timestamp exceeds the fixed bound")

    used_challenges: set[str] = set()
    used_native_receipts = set(deploy["native_verifier_receipts"])
    blocks, block_records_sha256 = _validate_block_inventory(
        block_evidence_inventory, top["block_evidence_inventory"], deploy,
        used_native_receipts)
    anchor_value = _exact(top["soak_anchor"], ANCHOR_FIELDS, "soak_anchor")
    anchor_start_ms = _integer(anchor_value["observation_started_at_unix_ms"],
                               "anchor observation start", minimum=1)
    anchor_sha256, anchor_gap_ms = _validate_anchor(
        anchor_value, deploy=deploy, blocks=blocks, started_ms=started_ms,
        used_challenges=used_challenges,
        used_native_receipts=used_native_receipts)
    lifecycle_baseline, lifecycle_identity_sha256, lifecycle_metrics = (
        _validate_lifecycle(
        lifecycle_evidence, top["lifecycle"], journal_artifact=lifecycle_journal,
        native_receipt_artifact=lifecycle_native_verifier_receipt, deploy=deploy,
        anchor_start_ms=anchor_start_ms, started_ms=started_ms,
        completed_ms=completed_ms,
        expected_native_binary_sha256=trusted_native_binary,
        expected_native_source_sha256=trusted_native_source,
        used_native_receipts=used_native_receipts))
    samples, sample_metrics, sample_set_sha256 = _validate_samples(
        top["samples"], started_ms=started_ms, completed_ms=completed_ms,
        blocks=blocks, deploy=deploy, lifecycle_baseline=lifecycle_baseline,
        used_challenges=used_challenges,
        used_native_receipts=used_native_receipts)
    submissions, submission_records_sha256 = _validate_submission_inventory(
        submission_receipt_inventory, top["submission_receipt_inventory"],
        started_ms=started_ms, completed_ms=completed_ms, deploy=deploy,
        used_native_receipts=used_native_receipts)
    statuses, status_records_sha256 = _validate_status_inventory(
        applied_status_inventory, top["applied_status_inventory"],
        started_ms=started_ms, completed_ms=completed_ms, samples=samples,
        blocks=blocks, used_native_receipts=used_native_receipts)
    workload_sha256, workload_records_sha256, workload_metrics = (
        _validate_workload_inventory(
            workload_inventory, top["workload_inventory"],
            started_ms=started_ms, completed_ms=completed_ms,
            submissions=submissions, statuses=statuses, blocks=blocks))
    _cross_validate_sample_counts(samples, statuses)

    source_tuple_sha256 = _domain_digest(
        b"iroha.taira.public-v2-24h.source-tuple.v1\0", source)
    exact_completion = {
        "state": "completed",
        "publication": "atomic-rename",
        "natural_completion": True,
        "workload_started_at_unix_ms": started_ms,
        "workload_ended_at_unix_ms": workload_ended_ms,
        "evidence_completed_at_unix_ms": completed_ms,
        "workload_duration_ms": DURATION_MS,
        "confirmation_drain_ms": drain_ms,
        "transfer_slot_count": REQUIRED_TRANSFER_COUNT,
        "sample_count": sample_metrics["sample_count"],
        "anchor_to_workload_gap_ms": anchor_gap_ms,
        "maximum_observed_sample_gap_ms": sample_metrics["maximum_gap"],
        "maximum_observation_window_ms": sample_metrics["maximum_window"],
        "maximum_submission_start_lateness_ms": workload_metrics["maximum_lateness"],
        "maximum_submission_request_window_ms": workload_metrics["maximum_request_window"],
        "applied_transfer_count": REQUIRED_TRANSFER_COUNT,
        "failed_transfer_count": 0,
        "queue_drop_events": 0,
        "unhealthy_samples": 0,
        "restart_events": 0,
        "unexpected_exit_events": 0,
        "source_tuple_sha256": source_tuple_sha256,
        "candidate_handoff_sha256": handoff_digests["candidate"],
        "publication_handoff_sha256": handoff_digests["publication"],
        "deploy_handoff_sha256": handoff_digests["deploy"],
        "native_verifier_identity_sha256": native_verifier_identity_sha256,
        "anchor_sha256": anchor_sha256,
        "sample_set_sha256": sample_set_sha256,
        "workload_inventory_sha256": workload_sha256,
        "workload_records_sha256": workload_records_sha256,
        "submission_inventory_sha256": submission_receipt_inventory.sha256,
        "submission_records_sha256": submission_records_sha256,
        "status_inventory_sha256": applied_status_inventory.sha256,
        "status_records_sha256": status_records_sha256,
        "block_inventory_sha256": block_evidence_inventory.sha256,
        "block_records_sha256": block_records_sha256,
        "lifecycle_artifact_sha256": lifecycle_evidence.sha256,
        "lifecycle_identity_sha256": lifecycle_identity_sha256,
        "lifecycle_journal_artifact_sha256": (
            lifecycle_metrics["journal_artifact_sha256"]),
        "lifecycle_journal_records_sha256": (
            lifecycle_metrics["journal_records_sha256"]),
        "lifecycle_journal_record_count": (
            lifecycle_metrics["journal_record_count"]),
        "lifecycle_native_verifier_receipt_sha256": (
            lifecycle_metrics["native_verifier_receipt_sha256"]),
        "lifecycle_window_sha256": lifecycle_metrics["window_sha256"],
    }
    _require(_json_exact_equal(dict(completion), exact_completion),
             "atomic completion counters, windows, or links are wrong")

    def inventory_subject(artifact: Artifact, records: str, count: int) -> dict[str, object]:
        return {"artifact_sha256": artifact.sha256,
                "records_sha256": records, "record_count": count}

    subject_core = {
        "schema": soak_authority.SUBJECT_SCHEMA,
        "receipt": {"sha256": receipt_artifact.sha256,
                    "size_bytes": receipt_artifact.size},
        "source": {"tuple_sha256": source_tuple_sha256},
        "prerequisites": {
            "candidate_handoff_sha256": handoff_digests["candidate"],
            "publication_handoff_sha256": handoff_digests["publication"],
            "deploy_handoff_sha256": handoff_digests["deploy"],
        },
        "anchor": {"sha256": anchor_sha256,
                   "validator_count": VALIDATOR_COUNT},
        "samples": {"sha256": sample_set_sha256,
                    "count": sample_metrics["sample_count"]},
        "workload": inventory_subject(
            workload_inventory, workload_records_sha256,
            REQUIRED_TRANSFER_COUNT),
        "submission_receipts": inventory_subject(
            submission_receipt_inventory, submission_records_sha256,
            REQUIRED_TRANSFER_COUNT),
        "applied_statuses": inventory_subject(
            applied_status_inventory, status_records_sha256,
            REQUIRED_TRANSFER_COUNT),
        "blocks": inventory_subject(
            block_evidence_inventory, block_records_sha256, len(blocks)),
        "lifecycle": {
            "artifact_sha256": lifecycle_evidence.sha256,
            "identity_sha256": lifecycle_identity_sha256,
            "journal_artifact_sha256": (
                lifecycle_metrics["journal_artifact_sha256"]),
            "journal_records_sha256": (
                lifecycle_metrics["journal_records_sha256"]),
            "journal_record_count": lifecycle_metrics["journal_record_count"],
            "native_verifier_receipt_sha256": (
                lifecycle_metrics["native_verifier_receipt_sha256"]),
            "window_sha256": lifecycle_metrics["window_sha256"],
        },
        "native_verifier": {
            "binary_sha256": native_verifier["binary_sha256"],
            "source_sha256": native_verifier["source_sha256"],
        },
    }
    try:
        soak_authority.validate_durable_admission_receipt_claims(
            durable_admission_receipt.payload,
            authority_envelope=authority_envelope.payload,
            subject_core=subject_core,
            completed_at_unix_ms=completed_ms)
    except soak_authority.PublicSoakAuthorityError as error:
        raise EvidenceError(str(error)) from error
    return StructuralResult(subject_core, completed_ms)


def _require_observation_authority() -> None:
    try:
        soak_authority.require_public_soak_authority_provisioned()
    except soak_authority.PublicSoakAuthorityError as error:
        raise EvidenceError(str(error)) from error


def validate_evidence(*args: object, **kwargs: object) -> None:
    """Validate admitted evidence, refusing before inspection while unprovisioned."""

    _require_observation_authority()
    artifact_arguments = (
        "receipt_artifact", "authority_envelope", "durable_admission_receipt",
        "candidate_handoff", "publication_handoff", "deploy_handoff",
        "workload_inventory", "submission_receipt_inventory",
        "applied_status_inventory", "block_evidence_inventory",
        "lifecycle_evidence", "lifecycle_journal",
        "lifecycle_native_verifier_receipt",
    )
    captures: list[Artifact] = []
    for name in artifact_arguments:
        capture = kwargs.get(name)
        _require(isinstance(capture, Artifact),
                 f"{name} is not one captured artifact")
        _validate_artifact_capture(capture, name)
        captures.append(capture)
    _distinct_artifacts(captures)
    result = _validate_structural_evidence(*args, **kwargs)  # type: ignore[arg-type]
    authority_envelope = kwargs["authority_envelope"]
    durable_receipt = kwargs["durable_admission_receipt"]
    _require(isinstance(authority_envelope, Artifact)
             and isinstance(durable_receipt, Artifact),
             "authority inputs are not captured artifacts")
    try:
        soak_authority.verify_authenticated_public_soak_authority_envelope(
            authority_envelope.payload,
            durable_admission_receipt=durable_receipt.payload,
            subject_core=result.authority_subject_core,
            completed_at_unix_ms=result.completed_at_unix_ms)
    except soak_authority.PublicSoakAuthorityError as error:
        raise EvidenceError(str(error)) from error


def build_parser() -> argparse.ArgumentParser:
    """Build the closed offline-verifier command line."""

    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("receipt", type=Path)
    parser.add_argument("--observation-authority-envelope", type=Path, required=True)
    parser.add_argument("--durable-admission-receipt", type=Path, required=True)
    parser.add_argument("--candidate-handoff", type=Path, required=True)
    parser.add_argument("--publication-handoff", type=Path, required=True)
    parser.add_argument("--deploy-handoff", type=Path, required=True)
    parser.add_argument("--workload-inventory", type=Path, required=True)
    parser.add_argument("--submission-receipt-inventory", type=Path, required=True)
    parser.add_argument("--applied-status-inventory", type=Path, required=True)
    parser.add_argument("--block-evidence-inventory", type=Path, required=True)
    parser.add_argument("--lifecycle-evidence", type=Path, required=True)
    parser.add_argument("--lifecycle-journal", type=Path, required=True)
    parser.add_argument("--lifecycle-native-verifier-receipt", type=Path,
                        required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--dpn-validator-release-commit", required=True)
    parser.add_argument("--cargo-lock-sha256", required=True)
    parser.add_argument("--workspace-source-manifest-sha256", required=True)
    parser.add_argument("--iroha3d-sha256", required=True)
    parser.add_argument("--native-verifier-binary-sha256", required=True)
    parser.add_argument("--native-verifier-source-sha256", required=True)
    return parser


def _distinct_artifacts(artifacts: Sequence[Artifact]) -> None:
    identities = [(artifact.device, artifact.inode) for artifact in artifacts]
    _require(len(identities) == len(set(identities)),
             "evidence inputs contain a file alias")


def _validate_artifact_capture(artifact: Artifact, label: str) -> None:
    _require(type(artifact.payload) is bytes,
             f"{label} captured payload is not immutable bytes")
    _require(type(artifact.size) is int and artifact.size > 0
             and artifact.size == len(artifact.payload),
             f"{label} captured size is not derived from its bytes")
    _require(type(artifact.sha256) is str
             and hashlib.sha256(artifact.payload).hexdigest() == artifact.sha256,
             f"{label} captured digest is not derived from its bytes")
    _require(type(artifact.device) is int and artifact.device >= 0
             and type(artifact.inode) is int and artifact.inode > 0,
             f"{label} captured file identity is invalid")


def main(argv: Sequence[str] | None = None) -> int:
    """Validate one durable admitted receipt without network access or mutation."""

    args = build_parser().parse_args(argv)
    try:
        # This barrier must remain before every caller-controlled path operation.
        _require_observation_authority()
        _require(args.receipt.name == COMPLETION_FILENAME,
                 f"receipt must use the terminal name {COMPLETION_FILENAME}")
        if os.path.lexists(args.receipt.with_name(PARTIAL_FILENAME)):
            _fail("a partial public-soak receipt is still present")
        inputs = {
            "receipt_artifact": _read_stable(args.receipt, MAX_RECEIPT_BYTES, "receipt"),
            "authority_envelope": _read_stable(
                args.observation_authority_envelope, MAX_AUTHORITY_BYTES,
                "observation authority envelope"),
            "durable_admission_receipt": _read_stable(
                args.durable_admission_receipt, MAX_ADMISSION_RECEIPT_BYTES,
                "durable admission receipt"),
            "candidate_handoff": _read_stable(
                args.candidate_handoff, MAX_HANDOFF_BYTES, "candidate handoff"),
            "publication_handoff": _read_stable(
                args.publication_handoff, MAX_HANDOFF_BYTES, "publication handoff"),
            "deploy_handoff": _read_stable(
                args.deploy_handoff, MAX_HANDOFF_BYTES, "deploy handoff"),
            "workload_inventory": _read_stable(
                args.workload_inventory, MAX_WORKLOAD_BYTES, "workload inventory"),
            "submission_receipt_inventory": _read_stable(
                args.submission_receipt_inventory, MAX_SUBMISSION_BYTES,
                "submission receipt inventory"),
            "applied_status_inventory": _read_stable(
                args.applied_status_inventory, MAX_STATUS_BYTES,
                "Applied status inventory"),
            "block_evidence_inventory": _read_stable(
                args.block_evidence_inventory, MAX_BLOCK_BYTES,
                "block evidence inventory"),
            "lifecycle_evidence": _read_stable(
                args.lifecycle_evidence, MAX_LIFECYCLE_BYTES,
                "lifecycle evidence"),
            "lifecycle_journal": _read_stable(
                args.lifecycle_journal, MAX_LIFECYCLE_JOURNAL_BYTES,
                "lifecycle journal"),
            "lifecycle_native_verifier_receipt": _read_stable(
                args.lifecycle_native_verifier_receipt,
                MAX_LIFECYCLE_JOURNAL_RECEIPT_BYTES,
                "lifecycle native-verifier receipt"),
        }
        artifacts = list(inputs.values())
        _distinct_artifacts(artifacts)
        receipt_artifact = inputs["receipt_artifact"]
        receipt = _decode_json(receipt_artifact.payload, "receipt", canonical=True)
        expected_source = {
            "commit": args.source_commit,
            "dpn_validator_release_commit": args.dpn_validator_release_commit,
            "cargo_lock_sha256": args.cargo_lock_sha256,
            "workspace_source_manifest_sha256": args.workspace_source_manifest_sha256,
        }
        result = _validate_structural_evidence(
            receipt,
            expected_source=expected_source,
            expected_binary_sha256=args.iroha3d_sha256,
            expected_native_verifier_binary_sha256=(
                args.native_verifier_binary_sha256),
            expected_native_verifier_source_sha256=(
                args.native_verifier_source_sha256),
            **inputs,
        )
        soak_authority.verify_authenticated_public_soak_authority_envelope(
            inputs["authority_envelope"].payload,
            durable_admission_receipt=inputs["durable_admission_receipt"].payload,
            subject_core=result.authority_subject_core,
            completed_at_unix_ms=result.completed_at_unix_ms)
    except (EvidenceError, soak_authority.PublicSoakAuthorityError,
            OSError, ValueError) as error:
        print(f"invalid deployed-public-Taira 24h evidence: {error}", file=sys.stderr)
        return 1
    print("deployed-public-Taira 24h evidence verified: "
          f"sha256={receipt_artifact.sha256}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
