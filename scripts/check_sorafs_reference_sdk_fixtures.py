#!/usr/bin/env python3
"""Verify the signed release-wide SoraFS V1 reference-SDK fixture inventory.

The checker is offline and read-only. It rejects schema extensions, path
substitution, duplicate JSON keys, non-finite numbers, digest/length drift,
untrusted fixture keys, and non-canonical inventory or ValidationOutcomeV1
JSON. The closed path list is intentionally independent of the generator.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path, PurePosixPath
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sccp_release_common import verify_ed25519  # noqa: E402


SCHEMA = "sorafs.reference_sdk.validation_fixture_inventory.v1"
SCOPE = "sorafs_v1_release"
SIGNING_DOMAIN_TEXT = SCHEMA
SIGNING_DOMAIN = SCHEMA.encode("ascii") + b"\x00"
TEST_FIXTURE_PUBLIC_KEY_HEX = (
    "d5af25e204ad03d0a26e236996404f1be51a60948bcc026cd084a83690b756d3"
)
TEST_FIXTURE_PUBLIC_KEY_FINGERPRINT = (
    "1a09a6a1b85cec77787ba6ce26f18500a2434865cee04d79c69a481888f52fff"
)
DEFAULT_INVENTORY = (
    SCRIPT_DIR.parent
    / "fixtures"
    / "sorafs_manifest"
    / "reference_sdk_validation_inventory_v1.json"
)
MAX_INVENTORY_BYTES = 2 << 20
MAX_FIXTURE_BYTES = 64 << 20
HEX_32_RE = re.compile(r"^[0-9a-f]{64}$")
HEX_64_RE = re.compile(r"^[0-9a-f]{128}$")
TOP_LEVEL_FIELD_ORDER = (
    "schema",
    "scope",
    "signing_domain",
    "payloads",
    "outcomes",
    "signature",
)
TOP_LEVEL_FIELDS = set(TOP_LEVEL_FIELD_ORDER)
UNSIGNED_FIELDS = {
    "schema",
    "scope",
    "signing_domain",
    "payloads",
    "outcomes",
}
PAYLOAD_FIELD_ORDER = (
    "path",
    "domain",
    "kind",
    "encoding",
    "expectation",
    "byte_length",
    "sha256",
)
PAYLOAD_FIELDS = set(PAYLOAD_FIELD_ORDER)
OUTCOME_FIELD_ORDER = (
    "path",
    "domain",
    "scenario",
    "status",
    "code",
    "byte_length",
    "sha256",
)
OUTCOME_FIELDS = set(OUTCOME_FIELD_ORDER)
SIGNATURE_FIELD_ORDER = (
    "algorithm",
    "key_usage",
    "public_key_hex",
    "public_key_fingerprint_sha256",
    "signature_hex",
)
SIGNATURE_FIELDS = set(SIGNATURE_FIELD_ORDER)
VALIDATION_OUTCOME_FIELD_ORDER = (
    "status",
    "code",
    "category",
    "message",
    "action",
    "docs_url",
    "telemetry_tags",
    "context",
    "inputs",
    "version",
    "generated_at",
)
VALIDATION_OUTCOME_FIELDS = set(VALIDATION_OUTCOME_FIELD_ORDER)


def _payload(
    domain: str,
    kind: str,
    expectation: str = "valid",
) -> tuple[str, str, str, str]:
    return domain, kind, "norito", expectation


def _pair(
    rows: dict[str, tuple[str, str, str, str]],
    base: str,
    domain: str,
    kind: str,
    expectation: str = "valid",
) -> None:
    rows[f"{base}.json"] = (domain, kind, "json", expectation)
    rows[f"{base}.to"] = (domain, kind, "norito", expectation)


EXPECTED_PAYLOADS: dict[str, tuple[str, str, str, str]] = {}
_pair(
    EXPECTED_PAYLOADS,
    "governance/dag_block_0_v1",
    "governance_dag",
    "governance_dag_block",
)
_pair(
    EXPECTED_PAYLOADS,
    "governance/dag_block_1_bad_predecessor_v1",
    "governance_dag",
    "governance_dag_block",
    "chain_invalid_predecessor",
)
_pair(
    EXPECTED_PAYLOADS,
    "governance/dag_block_1_v1",
    "governance_dag",
    "governance_dag_block",
)
_pair(
    EXPECTED_PAYLOADS,
    "governance/dag_block_bad_signature_v1",
    "governance_dag",
    "governance_dag_block",
    "invalid_signature",
)
EXPECTED_PAYLOADS["governance/dag_block_trailing_bytes_v1.to"] = _payload(
    "governance_dag",
    "governance_dag_block",
    "noncanonical_trailing_bytes",
)
_pair(
    EXPECTED_PAYLOADS,
    "governance/dag_head_bad_predecessor_v1",
    "governance_dag",
    "governance_dag_head",
    "chain_invalid_predecessor",
)
_pair(
    EXPECTED_PAYLOADS,
    "governance/dag_head_bad_signature_v1",
    "governance_dag",
    "governance_dag_head",
    "invalid_signature",
)
_pair(
    EXPECTED_PAYLOADS,
    "governance/dag_head_v1",
    "governance_dag",
    "governance_dag_head",
)
_pair(
    EXPECTED_PAYLOADS,
    "governance/node_v1",
    "governance_dag",
    "governance_log_node",
)
_pair(
    EXPECTED_PAYLOADS,
    "moderation/governance_node_v1",
    "moderation",
    "moderation_ballot_governance_node",
)
_pair(
    EXPECTED_PAYLOADS,
    "orderbook/negative/order_request_bad_signature_v1",
    "orderbook",
    "orderbook_order_request",
    "invalid_signature",
)
EXPECTED_PAYLOADS[
    "orderbook/negative/order_request_trailing_bytes_v1.to"
] = _payload(
    "orderbook",
    "orderbook_order_request",
    "noncanonical_trailing_bytes",
)
for _base, _kind in (
    ("order_cancel_v1", "orderbook_order_cancel"),
    ("order_request_v1", "orderbook_order_request"),
    ("settlement_channel_v1", "orderbook_settlement_channel"),
    ("settlement_receipt_v1", "orderbook_settlement_receipt"),
    ("trade_event_v1", "orderbook_trade_event"),
):
    _pair(EXPECTED_PAYLOADS, f"orderbook/{_base}", "orderbook", _kind)
_pair(EXPECTED_PAYLOADS, "pdp/challenge_v1", "pdp", "pdp_challenge")
_pair(EXPECTED_PAYLOADS, "pdp/commitment_v1", "pdp", "pdp_commitment")
for _base, _kind, _expectation in (
    (
        "duplicate_hot_leaf_challenge_v1",
        "pdp_challenge",
        "invalid_duplicate_hot_leaf",
    ),
    ("late_proof_v1", "pdp_proof", "invalid_late_proof"),
    (
        "missing_hot_leaf_path_proof_v1",
        "pdp_proof",
        "invalid_missing_hot_leaf_path",
    ),
    (
        "missing_segment_path_proof_v1",
        "pdp_proof",
        "invalid_missing_segment_path",
    ),
    ("missing_signature_proof_v1", "pdp_proof", "invalid_missing_signature"),
    ("wrong_manifest_proof_v1", "pdp_proof", "invalid_manifest_binding"),
    ("wrong_path_proof_v1", "pdp_proof", "invalid_merkle_path"),
    ("wrong_provider_proof_v1", "pdp_proof", "invalid_provider_binding"),
):
    _pair(
        EXPECTED_PAYLOADS,
        f"pdp/negative/{_base}",
        "pdp",
        _kind,
        _expectation,
    )
_pair(EXPECTED_PAYLOADS, "pdp/proof_v1", "pdp", "pdp_proof")
_pair(EXPECTED_PAYLOADS, "por/challenge_v1", "por", "por_challenge")
_pair(EXPECTED_PAYLOADS, "por/proof_v1", "por", "por_proof")
_pair(EXPECTED_PAYLOADS, "por/verdict_v1", "por", "por_audit_verdict")
_pair(EXPECTED_PAYLOADS, "potr/receipt_v1", "potr", "potr_receipt")
_pair(
    EXPECTED_PAYLOADS,
    "provider_admission/advert_v1",
    "routing",
    "provider_advert",
)
_pair(
    EXPECTED_PAYLOADS,
    "provider_admission/envelope_v1",
    "routing",
    "provider_admission_envelope",
)
_pair(
    EXPECTED_PAYLOADS,
    "repair/negative/task_manifest_mismatch_v1",
    "repair",
    "repair_task_record",
    "invalid_manifest_binding",
)
_pair(
    EXPECTED_PAYLOADS,
    "repair/negative/task_provider_unassigned_v1",
    "repair",
    "repair_task_record",
    "invalid_provider_assignment",
)
_pair(EXPECTED_PAYLOADS, "repair/task_v1", "repair", "repair_task_record")
_pair(
    EXPECTED_PAYLOADS,
    "replication_order/order_v1",
    "routing",
    "replication_order",
)


# path -> (domain, scenario, status, code, generated_at)
EXPECTED_OUTCOMES: dict[str, tuple[str, str, str, str, int]] = {
    "governance/dag_block_bad_signature_validation_outcome_v1.json": (
        "governance_dag",
        "block_bad_signature",
        "Error",
        "SFS-SIG-006",
        123,
    ),
    "governance/dag_block_cid_mismatch_validation_outcome_v1.json": (
        "governance_dag",
        "block_expected_cid_mismatch",
        "Error",
        "SFS-GOV-004",
        123,
    ),
    "governance/dag_block_trailing_bytes_validation_outcome_v1.json": (
        "governance_dag",
        "block_noncanonical_trailing_bytes",
        "Error",
        "SFS-NORITO-001",
        123,
    ),
    "governance/dag_block_validation_outcome_v1.json": (
        "governance_dag",
        "block_valid",
        "Ok",
        "SFS-OK-000",
        123,
    ),
    "governance/dag_head_bad_predecessor_validation_outcome_v1.json": (
        "governance_dag",
        "head_bad_predecessor",
        "Error",
        "SFS-GOV-006",
        123,
    ),
    "governance/dag_head_bad_signature_validation_outcome_v1.json": (
        "governance_dag",
        "head_bad_signature",
        "Error",
        "SFS-SIG-007",
        123,
    ),
    "governance/dag_head_reordered_validation_outcome_v1.json": (
        "governance_dag",
        "head_reordered_blocks",
        "Error",
        "SFS-GOV-006",
        123,
    ),
    "governance/dag_head_validation_outcome_v1.json": (
        "governance_dag",
        "head_valid",
        "Ok",
        "SFS-OK-000",
        123,
    ),
    "moderation/governance_node_validation_outcome_v1.json": (
        "moderation",
        "moderation_ballot_governance_node_valid",
        "Ok",
        "SFS-OK-000",
        1_700_001_234,
    ),
    "orderbook/negative/order_request_bad_signature_validation_outcome_v1.json": (
        "orderbook",
        "order_request_bad_signature",
        "Error",
        "SFS-SIG-007",
        123,
    ),
    "orderbook/negative/order_request_trailing_bytes_validation_outcome_v1.json": (
        "orderbook",
        "order_request_noncanonical_trailing_bytes",
        "Error",
        "SFS-NORITO-001",
        123,
    ),
    "orderbook/order_request_validation_outcome_v1.json": (
        "orderbook",
        "order_request_valid",
        "Ok",
        "SFS-OK-000",
        123,
    ),
    "pdp/bundle_validation_outcome_v1.json": (
        "pdp",
        "pdp_bundle_valid",
        "Ok",
        "SFS-PDP-DIAG-000",
        123,
    ),
    "pdp/negative/duplicate_hot_leaf_challenge_validation_outcome_v1.json": (
        "pdp",
        "duplicate_hot_leaf_challenge",
        "Error",
        "SFS-PDP-001",
        123,
    ),
    "pdp/negative/late_proof_validation_outcome_v1.json": (
        "pdp",
        "late_proof",
        "Error",
        "SFS-POL-002",
        123,
    ),
    "pdp/negative/missing_hot_leaf_path_proof_validation_outcome_v1.json": (
        "pdp",
        "missing_hot_leaf_path",
        "Error",
        "SFS-PDP-001",
        123,
    ),
    "pdp/negative/missing_segment_path_proof_validation_outcome_v1.json": (
        "pdp",
        "missing_segment_path",
        "Error",
        "SFS-PDP-001",
        123,
    ),
    "pdp/negative/missing_signature_proof_validation_outcome_v1.json": (
        "pdp",
        "missing_proof_signature",
        "Error",
        "SFS-SIG-008",
        123,
    ),
    "pdp/negative/wrong_manifest_proof_validation_outcome_v1.json": (
        "pdp",
        "wrong_manifest",
        "Error",
        "SFS-PDP-003",
        123,
    ),
    "pdp/negative/wrong_path_proof_validation_outcome_v1.json": (
        "pdp",
        "wrong_merkle_path",
        "Error",
        "SFS-PDP-003",
        123,
    ),
    "pdp/negative/wrong_provider_proof_validation_outcome_v1.json": (
        "pdp",
        "wrong_provider",
        "Error",
        "SFS-PDP-003",
        123,
    ),
    "reference_sdk/bundle_heterogeneous_positive_validation_outcome_v1.json": (
        "reference_sdk",
        "bundle_heterogeneous_positive",
        "Ok",
        "SFS-PDP-DIAG-000",
        1_700_001_234,
    ),
    "reference_sdk/bundle_orderbook_bad_signature_negative_validation_outcome_v1.json": (
        "reference_sdk",
        "bundle_orderbook_bad_signature_negative",
        "Error",
        "SFS-BND-001",
        1_700_001_234,
    ),
    "reference_sdk/bundle_orderbook_trailing_bytes_negative_validation_outcome_v1.json": (
        "reference_sdk",
        "bundle_orderbook_trailing_bytes_negative",
        "Error",
        "SFS-BND-001",
        1_700_001_234,
    ),
    "reference_sdk/bundle_pdp_duplicate_hot_leaf_negative_validation_outcome_v1.json": (
        "reference_sdk",
        "bundle_pdp_duplicate_hot_leaf_negative",
        "Error",
        "SFS-BND-001",
        1_700_001_234,
    ),
    "reference_sdk/bundle_pdp_missing_signature_negative_validation_outcome_v1.json": (
        "reference_sdk",
        "bundle_pdp_missing_signature_negative",
        "Error",
        "SFS-BND-001",
        1_700_001_234,
    ),
    "reference_sdk/bundle_pdp_wrong_provider_negative_validation_outcome_v1.json": (
        "reference_sdk",
        "bundle_pdp_wrong_provider_negative",
        "Error",
        "SFS-BND-001",
        1_700_001_234,
    ),
    "reference_sdk/bundle_repair_manifest_mismatch_negative_validation_outcome_v1.json": (
        "reference_sdk",
        "bundle_repair_manifest_mismatch_negative",
        "Error",
        "SFS-BND-002",
        1_700_001_234,
    ),
    "reference_sdk/bundle_repair_provider_unassigned_negative_validation_outcome_v1.json": (
        "reference_sdk",
        "bundle_repair_provider_unassigned_negative",
        "Error",
        "SFS-BND-003",
        1_700_001_234,
    ),
    "reference_sdk/bundle_routing_admission_positive_validation_outcome_v1.json": (
        "reference_sdk",
        "bundle_routing_admission_positive",
        "Ok",
        "SFS-OK-000",
        1_700_001_234,
    ),
}
_HETEROGENEOUS_INPUTS = [
    ("replication_order", "replication_order/order_v1.to"),
    ("pdp_commitment", "pdp/commitment_v1.to"),
    ("pdp_challenge", "pdp/challenge_v1.to"),
    ("pdp_proof", "pdp/proof_v1.to"),
    ("por_challenge", "por/challenge_v1.to"),
    ("por_proof", "por/proof_v1.to"),
    ("potr_receipt", "potr/receipt_v1.to"),
    ("repair_task_record", "repair/task_v1.to"),
    ("orderbook_order_request", "orderbook/order_request_v1.to"),
    ("orderbook_order_cancel", "orderbook/order_cancel_v1.to"),
    ("orderbook_trade_event", "orderbook/trade_event_v1.to"),
    ("settlement_channel", "orderbook/settlement_channel_v1.to"),
    ("settlement_receipt", "orderbook/settlement_receipt_v1.to"),
]
_ORDERBOOK_NEGATIVE_PREFIX = [
    ("replication_order", "replication_order/order_v1.to"),
    ("por_challenge", "por/challenge_v1.to"),
    ("por_proof", "por/proof_v1.to"),
]
_PDP_NEGATIVE_PREFIX = [
    ("replication_order", "replication_order/order_v1.to"),
    ("pdp_commitment", "pdp/commitment_v1.to"),
]
EXPECTED_OUTCOME_INPUTS: dict[str, list[tuple[str, str]]] = {
    "moderation/governance_node_validation_outcome_v1.json": [
        ("governance_log_node", "moderation/governance_node_v1.to"),
    ],
    "reference_sdk/bundle_heterogeneous_positive_validation_outcome_v1.json": (
        _HETEROGENEOUS_INPUTS
    ),
    "reference_sdk/bundle_orderbook_bad_signature_negative_validation_outcome_v1.json": (
        _ORDERBOOK_NEGATIVE_PREFIX
        + [
            (
                "orderbook_order_request",
                "orderbook/negative/order_request_bad_signature_v1.to",
            )
        ]
    ),
    "reference_sdk/bundle_orderbook_trailing_bytes_negative_validation_outcome_v1.json": (
        _ORDERBOOK_NEGATIVE_PREFIX
        + [
            (
                "orderbook_order_request",
                "orderbook/negative/order_request_trailing_bytes_v1.to",
            )
        ]
    ),
    "reference_sdk/bundle_pdp_duplicate_hot_leaf_negative_validation_outcome_v1.json": (
        _PDP_NEGATIVE_PREFIX
        + [
            (
                "pdp_challenge",
                "pdp/negative/duplicate_hot_leaf_challenge_v1.to",
            )
        ]
    ),
    "reference_sdk/bundle_pdp_missing_signature_negative_validation_outcome_v1.json": (
        _PDP_NEGATIVE_PREFIX
        + [
            ("pdp_challenge", "pdp/challenge_v1.to"),
            (
                "pdp_proof",
                "pdp/negative/missing_signature_proof_v1.to",
            ),
        ]
    ),
    "reference_sdk/bundle_pdp_wrong_provider_negative_validation_outcome_v1.json": (
        _PDP_NEGATIVE_PREFIX
        + [
            ("pdp_challenge", "pdp/challenge_v1.to"),
            ("pdp_proof", "pdp/negative/wrong_provider_proof_v1.to"),
        ]
    ),
    "reference_sdk/bundle_repair_manifest_mismatch_negative_validation_outcome_v1.json": [
        (
            "repair_task_record",
            "repair/negative/task_manifest_mismatch_v1.to",
        ),
    ],
    "reference_sdk/bundle_repair_provider_unassigned_negative_validation_outcome_v1.json": [
        ("replication_order", "replication_order/order_v1.to"),
        (
            "repair_task_record",
            "repair/negative/task_provider_unassigned_v1.to",
        ),
    ],
    "reference_sdk/bundle_routing_admission_positive_validation_outcome_v1.json": [
        ("provider_advert", "provider_admission/advert_v1.to"),
        (
            "provider_admission_envelope",
            "provider_admission/envelope_v1.to",
        ),
    ],
}
EXPECTED_BUNDLE_PAYLOAD_CODES = {
    "reference_sdk/bundle_orderbook_bad_signature_negative_validation_outcome_v1.json": (
        "SFS-SIG-007"
    ),
    "reference_sdk/bundle_orderbook_trailing_bytes_negative_validation_outcome_v1.json": (
        "SFS-NORITO-001"
    ),
    "reference_sdk/bundle_pdp_duplicate_hot_leaf_negative_validation_outcome_v1.json": (
        "SFS-PDP-001"
    ),
    "reference_sdk/bundle_pdp_missing_signature_negative_validation_outcome_v1.json": (
        "SFS-SIG-008"
    ),
    "reference_sdk/bundle_pdp_wrong_provider_negative_validation_outcome_v1.json": (
        "SFS-PDP-003"
    ),
}
REQUIRED_DOMAINS = {
    "governance_dag",
    "moderation",
    "orderbook",
    "pdp",
    "por",
    "potr",
    "reference_sdk",
    "repair",
    "routing",
}
REQUIRED_OUTCOME_DOMAINS = {
    "governance_dag",
    "moderation",
    "orderbook",
    "pdp",
    "reference_sdk",
}


class DuplicateKeyError(ValueError):
    """Raised when a JSON object repeats a key."""


class NonFiniteNumberError(ValueError):
    """Raised when JSON uses NaN or an infinity literal."""


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateKeyError(f"duplicate JSON key `{key}`")
        result[key] = value
    return result


def _reject_nonfinite_number(value: str) -> None:
    raise NonFiniteNumberError(f"non-finite JSON number `{value}` is forbidden")


def _decode_json(data: bytes, *, label: str) -> Any:
    try:
        text = data.decode("utf-8", errors="strict")
        return json.loads(
            text,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=_reject_nonfinite_number,
        )
    except (
        UnicodeDecodeError,
        json.JSONDecodeError,
        DuplicateKeyError,
        NonFiniteNumberError,
    ) as error:
        raise ValueError(f"{label} is invalid canonical UTF-8 JSON: {error}") from error


def _open_directory(path: Path, *, label: str) -> tuple[int, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as error:
        raise ValueError(f"{label} cannot be inspected: {error}") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISDIR(before.st_mode):
        raise ValueError(f"{label} must be a real directory")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ValueError(f"{label} cannot be opened safely: {error}") from error
    opened = os.fstat(descriptor)
    if (
        not stat.S_ISDIR(opened.st_mode)
        or (before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino)
    ):
        os.close(descriptor)
        raise ValueError(f"{label} changed while it was opened")
    return descriptor, opened


def _check_directory_identity(
    path: Path,
    opened: os.stat_result,
    *,
    label: str,
) -> None:
    try:
        after = path.lstat()
    except OSError as error:
        raise ValueError(f"{label} changed during validation: {error}") from error
    if (
        stat.S_ISLNK(after.st_mode)
        or not stat.S_ISDIR(after.st_mode)
        or (after.st_dev, after.st_ino) != (opened.st_dev, opened.st_ino)
    ):
        raise ValueError(f"{label} identity changed during validation")


def _canonical_relative_path(value: Any) -> tuple[str, ...] | None:
    if type(value) is not str or not value or not value.isascii():
        return None
    if value != value.strip() or "\\" in value or value.startswith("/"):
        return None
    pure = PurePosixPath(value)
    if str(pure) != value or any(part in {"", ".", ".."} for part in pure.parts):
        return None
    if len(pure.parts) < 2:
        return None
    return pure.parts


def _read_regular_file_at(
    path: str,
    *,
    root_fd: int,
    label: str,
    max_bytes: int,
) -> bytes:
    parts = _canonical_relative_path(path)
    if parts is None:
        raise ValueError(f"{label} path must be canonical repository-relative ASCII")
    current_fd = os.dup(root_fd)
    try:
        for index, component in enumerate(parts[:-1]):
            flags = os.O_RDONLY
            if hasattr(os, "O_NOFOLLOW"):
                flags |= os.O_NOFOLLOW
            if hasattr(os, "O_DIRECTORY"):
                flags |= os.O_DIRECTORY
            try:
                child_fd = os.open(component, flags, dir_fd=current_fd)
            except OSError as error:
                raise ValueError(
                    f"{label} parent component {index} cannot be opened safely: {error}"
                ) from error
            opened = os.fstat(child_fd)
            if not stat.S_ISDIR(opened.st_mode):
                os.close(child_fd)
                raise ValueError(f"{label} parent component {index} must be a directory")
            os.close(current_fd)
            current_fd = child_fd

        name = parts[-1]
        try:
            before = os.stat(name, dir_fd=current_fd, follow_symlinks=False)
        except OSError as error:
            raise ValueError(f"{label} cannot be inspected: {error}") from error
        if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
            raise ValueError(f"{label} must be a regular non-symlink file")
        if before.st_nlink != 1:
            raise ValueError(f"{label} must have exactly one hard link")
        if before.st_size > max_bytes:
            raise ValueError(f"{label} exceeds {max_bytes} bytes")
        flags = os.O_RDONLY
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        try:
            descriptor = os.open(name, flags, dir_fd=current_fd)
        except OSError as error:
            raise ValueError(f"{label} cannot be opened safely: {error}") from error
        try:
            opened = os.fstat(descriptor)
            if (
                not stat.S_ISREG(opened.st_mode)
                or opened.st_nlink != 1
                or (before.st_dev, before.st_ino)
                != (opened.st_dev, opened.st_ino)
            ):
                raise ValueError(f"{label} changed while it was opened")
            chunks: list[bytes] = []
            remaining = max_bytes + 1
            while remaining:
                chunk = os.read(descriptor, min(1 << 20, remaining))
                if not chunk:
                    break
                chunks.append(chunk)
                remaining -= len(chunk)
            data = b"".join(chunks)
            after = os.fstat(descriptor)
            if (
                len(data) > max_bytes
                or len(data) != after.st_size
                or opened.st_size != after.st_size
                or opened.st_mtime_ns != after.st_mtime_ns
                or after.st_nlink != 1
            ):
                raise ValueError(f"{label} changed while it was read")
            return data
        finally:
            os.close(descriptor)
    finally:
        os.close(current_fd)


def _read_root_regular_file(
    name: str,
    *,
    root_fd: int,
    label: str,
    max_bytes: int,
) -> bytes:
    if (
        type(name) is not str
        or not name
        or not name.isascii()
        or Path(name).name != name
        or name in {".", ".."}
    ):
        raise ValueError(f"{label} name must be a canonical ASCII basename")
    try:
        before = os.stat(name, dir_fd=root_fd, follow_symlinks=False)
    except OSError as error:
        raise ValueError(f"{label} cannot be inspected: {error}") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise ValueError(f"{label} must be a regular non-symlink file")
    if before.st_nlink != 1:
        raise ValueError(f"{label} must have exactly one hard link")
    if before.st_size > max_bytes:
        raise ValueError(f"{label} exceeds {max_bytes} bytes")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(name, flags, dir_fd=root_fd)
    except OSError as error:
        raise ValueError(f"{label} cannot be opened safely: {error}") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or (before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino)
        ):
            raise ValueError(f"{label} changed while it was opened")
        chunks: list[bytes] = []
        remaining = max_bytes + 1
        while remaining:
            chunk = os.read(descriptor, min(1 << 20, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        after = os.fstat(descriptor)
        if (
            len(data) > max_bytes
            or len(data) != after.st_size
            or opened.st_size != after.st_size
            or opened.st_mtime_ns != after.st_mtime_ns
            or after.st_nlink != 1
        ):
            raise ValueError(f"{label} changed while it was read")
        return data
    finally:
        os.close(descriptor)


def _require_exact_fields(
    value: Any,
    expected: set[str],
    *,
    label: str,
    errors: list[str],
) -> dict[str, Any] | None:
    if type(value) is not dict:
        errors.append(f"{label} must be an object")
        return None
    actual = set(value)
    if actual != expected:
        errors.append(
            f"{label} fields must match V1 "
            f"(missing={sorted(expected - actual)}, extra={sorted(actual - expected)})"
        )
        return None
    return value


def _canonical_signing_payload(inventory: dict[str, Any]) -> bytes:
    unsigned = {key: inventory[key] for key in inventory if key != "signature"}
    return SIGNING_DOMAIN + json.dumps(
        unsigned,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
    ).encode("ascii")


def _canonical_inventory_bytes(inventory: dict[str, Any]) -> bytes | None:
    payloads = inventory.get("payloads")
    outcomes = inventory.get("outcomes")
    signature = inventory.get("signature")
    if (
        set(inventory) != TOP_LEVEL_FIELDS
        or type(payloads) is not list
        or type(outcomes) is not list
        or type(signature) is not dict
        or set(signature) != SIGNATURE_FIELDS
        or any(type(row) is not dict or set(row) != PAYLOAD_FIELDS for row in payloads)
        or any(type(row) is not dict or set(row) != OUTCOME_FIELDS for row in outcomes)
    ):
        return None
    canonical = {key: inventory[key] for key in TOP_LEVEL_FIELD_ORDER}
    canonical["payloads"] = [
        {key: row[key] for key in PAYLOAD_FIELD_ORDER} for row in payloads
    ]
    canonical["outcomes"] = [
        {key: row[key] for key in OUTCOME_FIELD_ORDER} for row in outcomes
    ]
    canonical["signature"] = {
        key: signature[key] for key in SIGNATURE_FIELD_ORDER
    }
    return (json.dumps(canonical, indent=2, ensure_ascii=True) + "\n").encode()


def _canonical_outcome_bytes(outcome: dict[str, Any]) -> bytes | None:
    if set(outcome) != VALIDATION_OUTCOME_FIELDS:
        return None
    context = outcome.get("context")
    inputs = outcome.get("inputs")
    if (
        type(context) is not list
        or type(inputs) is not list
        or any(type(row) is not dict or set(row) != {"key", "value"} for row in context)
        or any(type(row) is not dict or set(row) != {"kind", "path"} for row in inputs)
    ):
        return None
    canonical = {key: outcome[key] for key in VALIDATION_OUTCOME_FIELD_ORDER}
    canonical["context"] = [
        {"key": row["key"], "value": row["value"]} for row in context
    ]
    canonical["inputs"] = [
        {"kind": row["kind"], "path": row["path"]} for row in inputs
    ]
    return (
        json.dumps(canonical, indent=2, ensure_ascii=False) + "\n"
    ).encode("utf-8")


def _validate_file_binding(
    row: dict[str, Any],
    root_fd: int,
    label: str,
    errors: list[str],
) -> bytes | None:
    byte_length = row["byte_length"]
    digest = row["sha256"]
    if type(byte_length) is not int or byte_length <= 0:
        errors.append(f"{label}.byte_length must be a positive integer")
    if type(digest) is not str or HEX_32_RE.fullmatch(digest) is None:
        errors.append(f"{label}.sha256 must be canonical lowercase SHA-256 hex")
    try:
        data = _read_regular_file_at(
            row["path"],
            root_fd=root_fd,
            label=f"{label} fixture",
            max_bytes=MAX_FIXTURE_BYTES,
        )
    except ValueError as error:
        errors.append(str(error))
        return None
    if type(byte_length) is int and len(data) != byte_length:
        errors.append(f"{label}.byte_length does not match fixture bytes")
    if type(digest) is str and hashlib.sha256(data).hexdigest() != digest:
        errors.append(f"{label}.sha256 does not match fixture bytes")
    return data


def _validate_payloads(
    inventory: dict[str, Any],
    root_fd: int,
    errors: list[str],
) -> None:
    rows = inventory["payloads"]
    if type(rows) is not list:
        errors.append("inventory.payloads must be an array")
        return
    paths: list[str] = []
    domains: set[str] = set()
    for index, raw in enumerate(rows):
        label = f"inventory.payloads[{index}]"
        row = _require_exact_fields(raw, PAYLOAD_FIELDS, label=label, errors=errors)
        if row is None:
            continue
        path = row["path"]
        if _canonical_relative_path(path) is None:
            errors.append(f"{label}.path must be canonical repository-relative ASCII")
            continue
        paths.append(path)
        expected = EXPECTED_PAYLOADS.get(path)
        if expected is None:
            errors.append(f"{label}.path is not in the closed V1 payload inventory")
        elif (
            row["domain"],
            row["kind"],
            row["encoding"],
            row["expectation"],
        ) != expected:
            errors.append(f"{label} metadata does not match the closed V1 row")
        if type(row["domain"]) is str:
            domains.add(row["domain"])
        data = _validate_file_binding(row, root_fd, label, errors)
        if data is not None:
            if row["encoding"] == "norito" and not data.startswith(b"NRT0"):
                errors.append(f"{label} Norito payload must use the canonical NRT0 envelope")
            if row["encoding"] == "json":
                try:
                    decoded = _decode_json(data, label=f"{label} JSON sidecar")
                    if type(decoded) is not dict:
                        errors.append(f"{label} JSON sidecar must be an object")
                except ValueError as error:
                    errors.append(str(error))
    expected_paths = sorted(EXPECTED_PAYLOADS)
    if paths != expected_paths:
        errors.append(
            "inventory.payloads paths must be unique, sorted, and exactly match "
            "the closed V1 payload set"
        )
    if domains != REQUIRED_DOMAINS - {"reference_sdk"}:
        errors.append(
            "inventory.payloads must cover the exact routing/orderbook/PDP/PoR/"
            "PoTR/repair/Governance DAG/moderation domain set"
        )


def _validate_outcomes(
    inventory: dict[str, Any],
    root_fd: int,
    errors: list[str],
) -> None:
    rows = inventory["outcomes"]
    if type(rows) is not list:
        errors.append("inventory.outcomes must be an array")
        return
    paths: list[str] = []
    domains: set[str] = set()
    for index, raw in enumerate(rows):
        label = f"inventory.outcomes[{index}]"
        row = _require_exact_fields(raw, OUTCOME_FIELDS, label=label, errors=errors)
        if row is None:
            continue
        path = row["path"]
        if _canonical_relative_path(path) is None:
            errors.append(f"{label}.path must be canonical repository-relative ASCII")
            continue
        paths.append(path)
        expected = EXPECTED_OUTCOMES.get(path)
        if expected is None:
            errors.append(f"{label}.path is not in the closed V1 outcome inventory")
            expected_generated_at = None
        else:
            expected_generated_at = expected[4]
            if (
                row["domain"],
                row["scenario"],
                row["status"],
                row["code"],
            ) != expected[:4]:
                errors.append(f"{label} metadata does not match the closed V1 row")
        if type(row["domain"]) is str:
            domains.add(row["domain"])
        data = _validate_file_binding(row, root_fd, label, errors)
        if data is None:
            continue
        try:
            outcome = _decode_json(data, label=f"{label} ValidationOutcomeV1")
        except ValueError as error:
            errors.append(str(error))
            continue
        outcome = _require_exact_fields(
            outcome,
            VALIDATION_OUTCOME_FIELDS,
            label=f"{label} ValidationOutcomeV1",
            errors=errors,
        )
        if outcome is None:
            continue
        if outcome["status"] != row["status"] or outcome["code"] != row["code"]:
            errors.append(f"{label} status/code does not match its inventory row")
        if outcome["version"] != 1:
            errors.append(f"{label} ValidationOutcomeV1.version must be 1")
        if (
            expected_generated_at is not None
            and outcome["generated_at"] != expected_generated_at
        ):
            errors.append(
                f"{label} generated_at must be the closed value "
                f"{expected_generated_at}"
            )
        expected_inputs = EXPECTED_OUTCOME_INPUTS.get(path)
        if expected_inputs is not None:
            actual_inputs = [
                (input_row.get("kind"), input_row.get("path"))
                for input_row in outcome["inputs"]
                if type(input_row) is dict
            ]
            if actual_inputs != expected_inputs:
                errors.append(
                    f"{label} inputs must match the exact ordered SDK golden labels"
                )
        expected_payload_code = EXPECTED_BUNDLE_PAYLOAD_CODES.get(path)
        if expected_payload_code is not None:
            context = {
                context_row.get("key"): context_row.get("value")
                for context_row in outcome["context"]
                if type(context_row) is dict
            }
            if context.get("payload_code") != expected_payload_code:
                errors.append(
                    f"{label} payload_code must be `{expected_payload_code}`"
                )
        canonical = _canonical_outcome_bytes(outcome)
        if canonical is None or data != canonical:
            errors.append(
                f"{label} ValidationOutcomeV1 JSON must use canonical checked-in bytes"
            )
    if paths != sorted(EXPECTED_OUTCOMES):
        errors.append(
            "inventory.outcomes paths must be unique, sorted, and exactly match "
            "the closed V1 outcome set"
        )
    if domains != REQUIRED_OUTCOME_DOMAINS:
        errors.append("inventory.outcomes must cover the exact closed V1 domain set")


def _validate_owned_directories(root_fd: int, errors: list[str]) -> None:
    expected_by_directory = {
        "moderation": sorted(
            PurePosixPath(path).name
            for path in set(EXPECTED_PAYLOADS) | set(EXPECTED_OUTCOMES)
                if path.startswith("moderation/")
        ),
        "repair/negative": sorted(
            PurePosixPath(path).name
            for path in EXPECTED_PAYLOADS
            if path.startswith("repair/negative/")
        ),
        "reference_sdk": sorted(
            PurePosixPath(path).name
            for path in EXPECTED_OUTCOMES
            if path.startswith("reference_sdk/")
        ),
    }
    for directory, expected in expected_by_directory.items():
        flags = os.O_RDONLY
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        if hasattr(os, "O_DIRECTORY"):
            flags |= os.O_DIRECTORY
        try:
            descriptor = os.open(directory, flags, dir_fd=root_fd)
        except OSError as error:
            errors.append(f"{directory} fixture directory cannot be opened: {error}")
            continue
        try:
            actual = sorted(
                name
                for name in os.listdir(descriptor)
                if name.endswith(".json") or name.endswith(".to")
            )
        except OSError as error:
            errors.append(f"{directory} fixture directory cannot be scanned: {error}")
        else:
            if actual != expected:
                errors.append(
                    f"{directory} fixture directory must be path-closed "
                    f"(expected={expected}, actual={actual})"
                )
        finally:
            os.close(descriptor)


def _validate_signature(inventory: dict[str, Any], errors: list[str]) -> None:
    signature = _require_exact_fields(
        inventory["signature"],
        SIGNATURE_FIELDS,
        label="inventory.signature",
        errors=errors,
    )
    if signature is None:
        return
    if signature["algorithm"] != "ed25519":
        errors.append("inventory.signature.algorithm must be `ed25519`")
    if signature["key_usage"] != "test_only_reference_sdk_fixture":
        errors.append(
            "inventory.signature.key_usage must be `test_only_reference_sdk_fixture`"
        )
    public_key_hex = signature["public_key_hex"]
    fingerprint = signature["public_key_fingerprint_sha256"]
    signature_hex = signature["signature_hex"]
    if public_key_hex != TEST_FIXTURE_PUBLIC_KEY_HEX:
        errors.append("inventory signature public key is not the trusted fixture key")
    if fingerprint != TEST_FIXTURE_PUBLIC_KEY_FINGERPRINT:
        errors.append("inventory signature public-key fingerprint is not trusted")
    if type(public_key_hex) is not str or HEX_32_RE.fullmatch(public_key_hex) is None:
        errors.append("inventory signature public key must be canonical 32-byte hex")
        return
    if type(fingerprint) is not str or HEX_32_RE.fullmatch(fingerprint) is None:
        errors.append("inventory signature fingerprint must be canonical SHA-256 hex")
        return
    if type(signature_hex) is not str or HEX_64_RE.fullmatch(signature_hex) is None:
        errors.append("inventory signature must be canonical 64-byte hex")
        return
    public_key = bytes.fromhex(public_key_hex)
    if hashlib.sha256(public_key).hexdigest() != fingerprint:
        errors.append("inventory signature fingerprint does not bind the public key")
    if not verify_ed25519(
        public_key,
        bytes.fromhex(signature_hex),
        _canonical_signing_payload(inventory),
    ):
        errors.append("inventory Ed25519 signature is invalid")


def validate_inventory(inventory_path: Path) -> list[str]:
    """Return every validation error for one release-wide inventory."""

    errors: list[str] = []
    try:
        root_fd, root_identity = _open_directory(
            inventory_path.parent,
            label="SoraFS fixture root",
        )
    except ValueError as error:
        return [str(error)]
    try:
        try:
            data = _read_root_regular_file(
                inventory_path.name,
                root_fd=root_fd,
                label="reference SDK fixture inventory",
                max_bytes=MAX_INVENTORY_BYTES,
            )
        except ValueError as error:
            errors.append(str(error))
            data = None
        if data is not None:
            try:
                decoded = _decode_json(
                    data,
                    label="reference SDK fixture inventory",
                )
            except ValueError as error:
                errors.append(str(error))
                decoded = None
            if decoded is not None:
                inventory = _require_exact_fields(
                    decoded,
                    TOP_LEVEL_FIELDS,
                    label="inventory",
                    errors=errors,
                )
                if inventory is not None:
                    canonical = _canonical_inventory_bytes(inventory)
                    if canonical is None or canonical != data:
                        errors.append(
                            "reference SDK fixture inventory must use canonical "
                            "checked-in JSON bytes"
                        )
                    if inventory["schema"] != SCHEMA:
                        errors.append(f"inventory.schema must be `{SCHEMA}`")
                    if inventory["scope"] != SCOPE:
                        errors.append(f"inventory.scope must be `{SCOPE}`")
                    if inventory["signing_domain"] != SIGNING_DOMAIN_TEXT:
                        errors.append(
                            f"inventory.signing_domain must be `{SIGNING_DOMAIN_TEXT}`"
                        )
                    if {
                        key for key in inventory if key != "signature"
                    } != UNSIGNED_FIELDS:
                        errors.append("inventory signed fields do not match V1")
                    _validate_payloads(inventory, root_fd, errors)
                    _validate_outcomes(inventory, root_fd, errors)
                    _validate_owned_directories(root_fd, errors)
                    _validate_signature(inventory, errors)
    finally:
        try:
            _check_directory_identity(
                inventory_path.parent,
                root_identity,
                label="SoraFS fixture root",
            )
        except ValueError as error:
            errors.append(str(error))
        os.close(root_fd)
    return errors


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Verify the signed, path-closed SoraFS V1 reference-SDK fixture "
            "inventory without network access."
        )
    )
    parser.add_argument(
        "--inventory",
        type=Path,
        default=DEFAULT_INVENTORY,
        help=f"Inventory JSON path (default: {DEFAULT_INVENTORY}).",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Run the offline verifier."""

    args = _parser().parse_args(argv)
    errors = validate_inventory(args.inventory)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    negative_count = sum(
        expectation != "valid"
        for _, _, _, expectation in EXPECTED_PAYLOADS.values()
    )
    print(
        "SoraFS V1 reference-SDK fixtures verified: "
        f"{len(EXPECTED_PAYLOADS)} payload artifacts, "
        f"{len(EXPECTED_OUTCOMES)} ValidationOutcomeV1 goldens, "
        f"{negative_count} negative payload vectors, "
        f"Ed25519 key {TEST_FIXTURE_PUBLIC_KEY_FINGERPRINT}."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
