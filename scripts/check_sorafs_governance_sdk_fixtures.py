#!/usr/bin/env python3
"""Verify the signed SoraFS Governance DAG cross-SDK fixture inventory.

The checker is deliberately offline and read-only. It validates the
schema-closed inventory, exact path set and order, canonical outcome JSON,
per-file SHA-256/length bindings, and the detached Ed25519 fixture signature.
No environment variables are required.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sccp_release_common import verify_ed25519  # noqa: E402


SCHEMA = "sorafs.reference_sdk.governance_fixture_inventory.v1"
SCOPE = "governance_sdk_subset"
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
    / "governance"
    / "sdk_validation_inventory_v1.json"
)
MAX_INVENTORY_BYTES = 1 << 20
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
    "kind",
    "encoding",
    "signature_expectation",
    "byte_length",
    "sha256",
)
PAYLOAD_FIELDS = set(PAYLOAD_FIELD_ORDER)
OUTCOME_FIELD_ORDER = (
    "path",
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
EXPECTED_PAYLOADS = {
    "dag_block_0_v1.json": ("governance_dag_block", "json", "valid"),
    "dag_block_0_v1.to": ("governance_dag_block", "norito", "valid"),
    "dag_block_1_bad_predecessor_v1.json": (
        "governance_dag_block",
        "json",
        "valid",
    ),
    "dag_block_1_bad_predecessor_v1.to": (
        "governance_dag_block",
        "norito",
        "valid",
    ),
    "dag_block_1_v1.json": ("governance_dag_block", "json", "valid"),
    "dag_block_1_v1.to": ("governance_dag_block", "norito", "valid"),
    "dag_block_bad_signature_v1.json": (
        "governance_dag_block",
        "json",
        "invalid_signature",
    ),
    "dag_block_bad_signature_v1.to": (
        "governance_dag_block",
        "norito",
        "invalid_signature",
    ),
    "dag_block_trailing_bytes_v1.to": (
        "governance_dag_block",
        "norito",
        "noncanonical_trailing_bytes",
    ),
    "dag_head_bad_predecessor_v1.json": (
        "governance_dag_head",
        "json",
        "valid",
    ),
    "dag_head_bad_predecessor_v1.to": (
        "governance_dag_head",
        "norito",
        "valid",
    ),
    "dag_head_bad_signature_v1.json": (
        "governance_dag_head",
        "json",
        "invalid_signature",
    ),
    "dag_head_bad_signature_v1.to": (
        "governance_dag_head",
        "norito",
        "invalid_signature",
    ),
    "dag_head_v1.json": ("governance_dag_head", "json", "valid"),
    "dag_head_v1.to": ("governance_dag_head", "norito", "valid"),
    "node_v1.json": ("governance_log_node", "json", "valid"),
    "node_v1.to": ("governance_log_node", "norito", "valid"),
}
EXPECTED_OUTCOMES = {
    "dag_block_bad_signature_validation_outcome_v1.json": (
        "block_bad_signature",
        "Error",
        "SFS-SIG-006",
    ),
    "dag_block_cid_mismatch_validation_outcome_v1.json": (
        "block_expected_cid_mismatch",
        "Error",
        "SFS-GOV-004",
    ),
    "dag_block_trailing_bytes_validation_outcome_v1.json": (
        "block_noncanonical_trailing_bytes",
        "Error",
        "SFS-NORITO-001",
    ),
    "dag_block_validation_outcome_v1.json": (
        "block_valid",
        "Ok",
        "SFS-OK-000",
    ),
    "dag_head_bad_predecessor_validation_outcome_v1.json": (
        "head_bad_predecessor",
        "Error",
        "SFS-GOV-006",
    ),
    "dag_head_bad_signature_validation_outcome_v1.json": (
        "head_bad_signature",
        "Error",
        "SFS-SIG-007",
    ),
    "dag_head_reordered_validation_outcome_v1.json": (
        "head_reordered_blocks",
        "Error",
        "SFS-GOV-006",
    ),
    "dag_head_validation_outcome_v1.json": (
        "head_valid",
        "Ok",
        "SFS-OK-000",
    ),
}


class DuplicateKeyError(ValueError):
    """Raised when an input JSON object repeats a key."""


class NonFiniteNumberError(ValueError):
    """Raised when input JSON uses NaN or an infinity literal."""


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateKeyError(f"duplicate JSON key `{key}`")
        result[key] = value
    return result


def _reject_nonfinite_number(value: str) -> None:
    raise NonFiniteNumberError(f"non-finite JSON number `{value}` is forbidden")


def _open_directory(path: Path, *, label: str) -> tuple[int, os.stat_result]:
    """Open one real directory and bind its identity for the validation scan."""

    try:
        before = path.lstat()
    except OSError as error:
        raise ValueError(f"{label} cannot be inspected: {error}") from error
    if stat.S_ISLNK(before.st_mode):
        raise ValueError(f"{label} must not be a symlink")
    if not stat.S_ISDIR(before.st_mode):
        raise ValueError(f"{label} must be a directory")

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
    if not stat.S_ISDIR(opened.st_mode):
        os.close(descriptor)
        raise ValueError(f"{label} must remain a directory")
    if (before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino):
        os.close(descriptor)
        raise ValueError(f"{label} changed while it was opened")
    return descriptor, opened


def _check_directory_identity(
    path: Path,
    opened: os.stat_result,
    *,
    label: str,
) -> None:
    """Reject replacement or symlink substitution of an opened directory."""

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


def _read_regular_file(
    path: str,
    *,
    directory_fd: int,
    label: str,
    max_bytes: int,
) -> bytes:
    """Read one bounded, singly-linked regular file from an opened directory."""

    try:
        before = os.stat(path, dir_fd=directory_fd, follow_symlinks=False)
    except OSError as error:
        raise ValueError(f"{label} cannot be inspected: {error}") from error
    if stat.S_ISLNK(before.st_mode):
        raise ValueError(f"{label} must not be a symlink")
    if not stat.S_ISREG(before.st_mode):
        raise ValueError(f"{label} must be a regular file")
    if before.st_nlink != 1:
        raise ValueError(f"{label} must have exactly one hard link")
    if before.st_size > max_bytes:
        raise ValueError(f"{label} exceeds {max_bytes} bytes")

    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, dir_fd=directory_fd)
    except OSError as error:
        raise ValueError(f"{label} cannot be opened safely: {error}") from error
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode):
            raise ValueError(f"{label} must remain a regular file")
        if opened.st_nlink != 1:
            raise ValueError(f"{label} must have exactly one hard link")
        if (before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino):
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
        if len(data) > max_bytes:
            raise ValueError(f"{label} exceeds {max_bytes} bytes")
        after = os.fstat(descriptor)
        if (
            opened.st_size != after.st_size
            or opened.st_mtime_ns != after.st_mtime_ns
            or after.st_nlink != 1
            or len(data) != after.st_size
        ):
            raise ValueError(f"{label} changed while it was read")
        return data
    finally:
        os.close(descriptor)


def _decode_json(data: bytes, *, label: str) -> Any:
    try:
        text = data.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise ValueError(f"{label} must be canonical UTF-8 JSON") from error
    try:
        return json.loads(
            text,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=_reject_nonfinite_number,
        )
    except (
        json.JSONDecodeError,
        DuplicateKeyError,
        NonFiniteNumberError,
    ) as error:
        raise ValueError(f"{label} is invalid: {error}") from error


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
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        errors.append(
            f"{label} fields must match the V1 schema "
            f"(missing={missing}, extra={extra})"
        )
        return None
    return value


def _is_canonical_basename(value: Any) -> bool:
    return (
        type(value) is str
        and value
        and value.isascii()
        and value == value.strip()
        and "/" not in value
        and "\\" not in value
        and value not in {".", ".."}
        and Path(value).name == value
    )


def _canonical_signing_payload(inventory: dict[str, Any]) -> bytes:
    unsigned = {key: inventory[key] for key in inventory if key != "signature"}
    return SIGNING_DOMAIN + json.dumps(
        unsigned,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
    ).encode("ascii")


def _canonical_inventory_bytes(inventory: dict[str, Any]) -> bytes | None:
    """Render the one accepted field order and pretty JSON layout."""

    payloads = inventory.get("payloads")
    outcomes = inventory.get("outcomes")
    signature = inventory.get("signature")
    if (
        set(inventory) != TOP_LEVEL_FIELDS
        or type(payloads) is not list
        or type(outcomes) is not list
        or type(signature) is not dict
        or set(signature) != SIGNATURE_FIELDS
        or any(type(entry) is not dict or set(entry) != PAYLOAD_FIELDS for entry in payloads)
        or any(type(entry) is not dict or set(entry) != OUTCOME_FIELDS for entry in outcomes)
    ):
        return None

    canonical = {key: inventory[key] for key in TOP_LEVEL_FIELD_ORDER}
    canonical["payloads"] = [
        {key: entry[key] for key in PAYLOAD_FIELD_ORDER} for entry in payloads
    ]
    canonical["outcomes"] = [
        {key: entry[key] for key in OUTCOME_FIELD_ORDER} for entry in outcomes
    ]
    canonical["signature"] = {
        key: signature[key] for key in SIGNATURE_FIELD_ORDER
    }
    return (json.dumps(canonical, indent=2, ensure_ascii=True) + "\n").encode("utf-8")


def _canonical_outcome_bytes(outcome: dict[str, Any]) -> bytes | None:
    """Render the canonical ValidationOutcomeV1 field and nested-object order."""

    if set(outcome) != VALIDATION_OUTCOME_FIELDS:
        return None
    context = outcome.get("context")
    inputs = outcome.get("inputs")
    if (
        type(context) is not list
        or type(inputs) is not list
        or any(type(entry) is not dict or set(entry) != {"key", "value"} for entry in context)
        or any(type(entry) is not dict or set(entry) != {"kind", "path"} for entry in inputs)
    ):
        return None
    canonical = {key: outcome[key] for key in VALIDATION_OUTCOME_FIELD_ORDER}
    canonical["context"] = [
        {"key": entry["key"], "value": entry["value"]} for entry in context
    ]
    canonical["inputs"] = [
        {"kind": entry["kind"], "path": entry["path"]} for entry in inputs
    ]
    return (json.dumps(canonical, indent=2, ensure_ascii=True) + "\n").encode("utf-8")


def _validate_entries(
    inventory: dict[str, Any],
    fixture_root_fd: int,
    errors: list[str],
) -> None:
    payloads = inventory["payloads"]
    outcomes = inventory["outcomes"]
    if type(payloads) is not list:
        errors.append("inventory.payloads must be an array")
        payloads = []
    if type(outcomes) is not list:
        errors.append("inventory.outcomes must be an array")
        outcomes = []

    payload_paths: list[str] = []
    for index, raw_entry in enumerate(payloads):
        label = f"inventory.payloads[{index}]"
        entry = _require_exact_fields(
            raw_entry,
            PAYLOAD_FIELDS,
            label=label,
            errors=errors,
        )
        if entry is None:
            continue
        path = entry["path"]
        if not _is_canonical_basename(path):
            errors.append(f"{label}.path must be an exact canonical basename")
            continue
        payload_paths.append(path)
        expected = EXPECTED_PAYLOADS.get(path)
        if expected is None:
            errors.append(f"{label}.path is not in the closed payload inventory")
        elif (
            entry["kind"],
            entry["encoding"],
            entry["signature_expectation"],
        ) != expected:
            errors.append(
                f"{label} kind/encoding/signature expectation does not match {path}"
            )
        data = _validate_file_binding(entry, fixture_root_fd, label, errors)
        if data is not None and entry["encoding"] == "json":
            _validate_payload_json(data, label, errors)

    outcome_paths: list[str] = []
    for index, raw_entry in enumerate(outcomes):
        label = f"inventory.outcomes[{index}]"
        entry = _require_exact_fields(
            raw_entry,
            OUTCOME_FIELDS,
            label=label,
            errors=errors,
        )
        if entry is None:
            continue
        path = entry["path"]
        if not _is_canonical_basename(path):
            errors.append(f"{label}.path must be an exact canonical basename")
            continue
        outcome_paths.append(path)
        expected = EXPECTED_OUTCOMES.get(path)
        if expected is None:
            errors.append(f"{label}.path is not in the closed outcome inventory")
        elif (entry["scenario"], entry["status"], entry["code"]) != expected:
            errors.append(f"{label} scenario/status/code does not match {path}")
        data = _validate_file_binding(entry, fixture_root_fd, label, errors)
        if data is not None:
            _validate_outcome_json(data, entry, label, errors)

    expected_payload_paths = sorted(EXPECTED_PAYLOADS)
    expected_outcome_paths = sorted(EXPECTED_OUTCOMES)
    if payload_paths != expected_payload_paths:
        errors.append(
            "inventory.payloads paths must be unique, sorted, and exactly match "
            f"{expected_payload_paths}"
        )
    if outcome_paths != expected_outcome_paths:
        errors.append(
            "inventory.outcomes paths must be unique, sorted, and exactly match "
            f"{expected_outcome_paths}"
        )

    try:
        names = os.listdir(fixture_root_fd)
    except OSError as error:
        errors.append(f"governance SDK fixture directory cannot be scanned: {error}")
        names = []
    disk_paths = sorted(
        name
        for name in names
        if name.endswith(".to") or name.endswith(".json")
    )
    expected_disk_paths = sorted(
        set(EXPECTED_PAYLOADS)
        | set(EXPECTED_OUTCOMES)
        | {DEFAULT_INVENTORY.name}
    )
    if disk_paths != expected_disk_paths:
        errors.append(
            "governance SDK fixture directory must contain the exact signed artifact "
            f"inventory (expected={expected_disk_paths}, actual={disk_paths})"
        )


def _validate_file_binding(
    entry: dict[str, Any],
    fixture_root_fd: int,
    label: str,
    errors: list[str],
) -> bytes | None:
    byte_length = entry["byte_length"]
    digest = entry["sha256"]
    if type(byte_length) is not int or byte_length <= 0:
        errors.append(f"{label}.byte_length must be a positive integer")
    if type(digest) is not str or HEX_32_RE.fullmatch(digest) is None:
        errors.append(f"{label}.sha256 must be canonical lowercase SHA-256 hex")
    try:
        data = _read_regular_file(
            entry["path"],
            directory_fd=fixture_root_fd,
            label=f"{label} fixture",
            max_bytes=MAX_FIXTURE_BYTES,
        )
    except ValueError as error:
        errors.append(str(error))
        return None
    if type(byte_length) is int and len(data) != byte_length:
        errors.append(f"{label}.byte_length does not match fixture bytes")
    actual_digest = hashlib.sha256(data).hexdigest()
    if type(digest) is str and actual_digest != digest:
        errors.append(f"{label}.sha256 does not match fixture bytes")
    return data


def _validate_outcome_json(
    data: bytes,
    entry: dict[str, Any],
    label: str,
    errors: list[str],
) -> None:
    try:
        outcome = _decode_json(data, label=f"{label} fixture")
    except ValueError as error:
        errors.append(str(error))
        return
    outcome = _require_exact_fields(
        outcome,
        VALIDATION_OUTCOME_FIELDS,
        label=f"{label} ValidationOutcomeV1",
        errors=errors,
    )
    if outcome is None:
        return
    if outcome["status"] != entry["status"] or outcome["code"] != entry["code"]:
        errors.append(f"{label} outcome status/code does not match its inventory row")
    if outcome["version"] != 1:
        errors.append(f"{label} outcome version must be 1")
    if outcome["generated_at"] != 123:
        errors.append(f"{label} outcome generated_at must be the canonical value 123")
    canonical = _canonical_outcome_bytes(outcome)
    if canonical is None or data != canonical:
        errors.append(f"{label} outcome JSON must use the canonical checked-in layout")


def _validate_payload_json(
    data: bytes,
    label: str,
    errors: list[str],
) -> None:
    try:
        payload = _decode_json(data, label=f"{label} fixture")
    except ValueError as error:
        errors.append(str(error))
        return
    if type(payload) is not dict:
        errors.append(f"{label} JSON sidecar must be an object")
        return
    canonical = json.dumps(
        payload,
        indent=2,
        ensure_ascii=True,
        sort_keys=True,
    ).encode("utf-8")
    if data != canonical:
        errors.append(
            f"{label} JSON sidecar must use canonical sorted pretty bytes "
            "without a trailing newline"
        )


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
    if signature["key_usage"] != "test_only_governance_fixture":
        errors.append(
            "inventory.signature.key_usage must be `test_only_governance_fixture`"
        )
    public_key_hex = signature["public_key_hex"]
    fingerprint = signature["public_key_fingerprint_sha256"]
    signature_hex = signature["signature_hex"]
    if public_key_hex != TEST_FIXTURE_PUBLIC_KEY_HEX:
        errors.append("inventory signature public key must match the Governance DAG fixture key")
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
    """Return every validation error for one Governance DAG SDK inventory."""

    errors: list[str] = []
    try:
        fixture_root_fd, fixture_root_identity = _open_directory(
            inventory_path.parent,
            label="governance SDK fixture directory",
        )
    except ValueError as error:
        return [str(error)]
    try:
        try:
            data = _read_regular_file(
                inventory_path.name,
                directory_fd=fixture_root_fd,
                label="governance SDK fixture inventory",
                max_bytes=MAX_INVENTORY_BYTES,
            )
            decoded = _decode_json(data, label="governance SDK fixture inventory")
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
                if canonical is None or data != canonical:
                    errors.append(
                        "governance SDK fixture inventory JSON must use the canonical "
                        "checked-in layout"
                    )
                if inventory["schema"] != SCHEMA:
                    errors.append(f"inventory.schema must be `{SCHEMA}`")
                if inventory["scope"] != SCOPE:
                    errors.append(f"inventory.scope must be `{SCOPE}`")
                if inventory["signing_domain"] != SIGNING_DOMAIN_TEXT:
                    errors.append(
                        f"inventory.signing_domain must be `{SIGNING_DOMAIN_TEXT}`"
                    )
                if (
                    set(key for key in inventory if key != "signature")
                    != UNSIGNED_FIELDS
                ):
                    errors.append("inventory signed fields do not match the V1 contract")
                _validate_entries(inventory, fixture_root_fd, errors)
                _validate_signature(inventory, errors)
    finally:
        try:
            _check_directory_identity(
                inventory_path.parent,
                fixture_root_identity,
                label="governance SDK fixture directory",
            )
        except ValueError as error:
            errors.append(str(error))
        os.close(fixture_root_fd)
    return errors


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Verify the signed, digest-bound SoraFS Governance DAG cross-SDK "
            "fixture inventory without network access."
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
    """Run the fixture verifier."""

    args = _parser().parse_args(argv)
    errors = validate_inventory(args.inventory)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(
        "SoraFS Governance DAG SDK fixtures verified: "
        f"{len(EXPECTED_PAYLOADS)} payload artifacts "
        f"({sum(spec[1] == 'norito' for spec in EXPECTED_PAYLOADS.values())} Norito, "
        f"{sum(spec[1] == 'json' for spec in EXPECTED_PAYLOADS.values())} JSON), "
        f"{len(EXPECTED_OUTCOMES)} outcomes, "
        f"Ed25519 key {TEST_FIXTURE_PUBLIC_KEY_FINGERPRINT}."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
