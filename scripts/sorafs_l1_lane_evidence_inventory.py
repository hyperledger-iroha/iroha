#!/usr/bin/env python3
"""Build and verify the payload-free signed SoraFS L1 lane inventory.

The tool never accepts a private key.  An operator prepares domain-separated
bytes, sends those bytes to an authenticated external software Ed25519 signer,
then finalizes and verifies the inventory against the original 17 summaries.
All timestamps and trust values are explicit so replay is deterministic.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import sys
import unicodedata
from pathlib import Path
from typing import Any, Mapping, Sequence

from sccp_release_common import verify_ed25519
import taira_constants


INVENTORY_SCHEMA = "sorafs.l1.lane_evidence_inventory.v1"
VERIFICATION_SCHEMA = "sorafs.l1.lane_evidence_inventory.verification.v1"
SIGNING_DOMAIN = b"sorafs:l1:lane-evidence-inventory:v1\x00"
SIGNER_ROLE = "l1-lane-evidence-inventory"
SIGNER_KIND = "authenticated-external-signer"
TAIRA_NETWORK = taira_constants.NETWORK_NAME
TAIRA_CHAIN_ID = taira_constants.CHAIN_ID
TAIRA_CHAIN_DISCRIMINANT = taira_constants.CHAIN_DISCRIMINANT
MAX_SUMMARY_BYTES = 4 * 1024 * 1024
MAX_INVENTORY_BYTES = 128 * 1024
MAX_SUMMARY_AGE_SECS = 24 * 60 * 60
MAX_INTEGER = (1 << 63) - 1
PRODUCTION_ENVIRONMENTS = frozenset({"prod", "production"})
OPEN_DIR_FD_SUPPORTED = os.open in os.supports_dir_fd
STAT_DIR_FD_SUPPORTED = os.stat in os.supports_dir_fd
STAT_NOFOLLOW_SUPPORTED = os.stat in os.supports_follow_symlinks

LANES = (
    ("ai_prescreen", "sorafs.moderation.ai_prescreen.rollout_evidence_gate.v1"),
    ("appeal_finance", "sorafs.appeal_finance.rollout_evidence_gate.v1"),
    ("gateway_compliance", "sorafs.gateway_compliance.rollout_evidence_gate.v1"),
    ("gateway_load", "sorafs.gateway_load.rollout_evidence_gate.v1"),
    ("governance_dag", "sorafs.governance_dag.rollout_evidence_gate.v1"),
    ("hedging_billing", "sorafs.hedging_billing.rollout_evidence_gate.v1"),
    ("moderation_panel", "sorafs.moderation_panel.rollout_evidence_gate.v1"),
    ("orderbook", "sorafs.orderbook.rollout_evidence_gate.v1"),
    ("pdp", "sorafs.pdp.rollout_evidence_gate.v1"),
    ("pop_credentials", "sorafs.pop_credentials.rollout_evidence_gate.v1"),
    ("por", "sorafs.por.rollout_evidence_gate.v1"),
    ("potr", "sorafs.potr.rollout_evidence_gate.v1"),
    (
        "reference_sdk_release",
        "sorafs.reference_sdk.release_evidence_gate.v1",
    ),
    ("repair", "sorafs.repair.rollout_evidence_gate.v1"),
    ("reputation", "sorafs.reputation.rollout_evidence_gate.v1"),
    ("reserve_rent", "sorafs.reserve_rent.rollout_evidence_gate.v1"),
    ("transparency", "sorafs.transparency.rollout_evidence_gate.v1"),
)

DEPLOYMENT_FIELDS = frozenset(
    {"deployment_id", "environment", "network", "chain_id", "chain_discriminant"}
)
ANCHOR_FIELDS = frozenset(
    {
        "topology_qualification_summary_sha256",
        "topology_manifest_sha256",
        "topology_canonical_manifest_sha256",
        "validator_ids_sha256",
        "oldest_evidence_generated_at_unix",
        "newest_evidence_generated_at_unix",
    }
)
ROW_FIELDS = frozenset(
    {
        "lane",
        "schema",
        "summary_sha256",
        "recognized_artifact_count",
        "oldest_generated_at_unix",
        "newest_generated_at_unix",
    }
)
SIGNER_BASE_FIELDS = frozenset(
    {
        "role",
        "service_kind",
        "algorithm",
        "backend",
        "service_id",
        "administrator_id",
        "key_revision",
        "policy_revision",
        "policy_digest_sha256",
        "public_key_fingerprint_sha256",
    }
)
SIGNER_FIELDS = SIGNER_BASE_FIELDS | {"signature_hex"}
INVENTORY_FIELDS = frozenset(
    {
        "schema",
        "status",
        "signer_qualification",
        "generated_at_unix",
        "max_summary_age_secs",
        "summary_file_count",
        "recognized_summary_count",
        "deployment",
        "anchors",
        "summaries",
        "signer",
    }
)
TOPOLOGY_FIELDS = frozenset(
    {
        "qualification_summary_sha256",
        "manifest_sha256",
        "canonical_manifest_sha256",
        "deployment_id",
        "environment",
        "network",
        "chain_id",
        "chain_discriminant",
        "validator_ids_sha256",
    }
)
NON_PRODUCTION_MARKERS = (
    "dummy",
    "example",
    "fake",
    "fixture",
    "local",
    "minamoto",
    "mock",
    "placeholder",
    "staging",
    "test",
)
SECRET_ARGUMENT_PREFIXES = (
    "--private",
    "--seed",
    "--signing-key",
    "--secret",
)


class InventoryError(ValueError):
    """Raised when lane inventory input fails closed."""


def canonical_json_bytes(value: Any) -> bytes:
    """Return canonical compact JSON used inside the signing transcript."""

    try:
        return json.dumps(
            value,
            ensure_ascii=True,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("ascii")
    except (RecursionError, TypeError, UnicodeEncodeError, ValueError) as error:
        raise InventoryError("value is not canonical JSON") from error


def canonical_file_bytes(value: Any) -> bytes:
    """Return the canonical checked-file representation."""

    try:
        rendered = json.dumps(
            value,
            ensure_ascii=True,
            allow_nan=False,
            indent=2,
            sort_keys=True,
        )
    except (RecursionError, TypeError, UnicodeEncodeError, ValueError) as error:
        raise InventoryError("value is not canonical JSON") from error
    return (rendered + "\n").encode("ascii")


def _reject_constant(_value: str) -> None:
    raise InventoryError("non-standard JSON constants are not accepted")


def _object_without_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise InventoryError("JSON contains a duplicate object key")
        result[key] = value
    return result


def _decode_canonical_file(data: bytes, *, label: str) -> dict[str, Any]:
    try:
        value = json.loads(
            data.decode("utf-8", "strict"),
            object_pairs_hook=_object_without_duplicates,
            parse_constant=_reject_constant,
        )
    except InventoryError:
        raise
    except (RecursionError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise InventoryError(f"{label} is not valid JSON") from error
    if not isinstance(value, dict):
        raise InventoryError(f"{label} must be a JSON object")
    if data != canonical_file_bytes(value):
        raise InventoryError(
            f"{label} must be canonical sorted JSON with exactly one trailing LF"
        )
    return value


def _open_anchored_parent(path: Path, *, label: str) -> tuple[int, str]:
    """Open every ancestor by dirfd and return the anchored parent and leaf."""

    nofollow = getattr(os, "O_NOFOLLOW", 0)
    directory = getattr(os, "O_DIRECTORY", 0)
    if (
        not nofollow
        or not directory
        or not OPEN_DIR_FD_SUPPORTED
        or not STAT_DIR_FD_SUPPORTED
        or not STAT_NOFOLLOW_SUPPORTED
    ):
        raise InventoryError(
            f"{label} cannot guarantee anchored no-follow path traversal"
        )
    candidate = Path(path)
    parts = candidate.parts
    if candidate.is_absolute():
        if not parts or parts[0] != os.sep:
            raise InventoryError(f"{label} path root is not canonical")
        anchor = os.sep
        relative_parts = parts[1:]
    else:
        anchor = "."
        relative_parts = parts
    if (
        not relative_parts
        or any(part in {"", ".", ".."} for part in relative_parts)
    ):
        raise InventoryError(f"{label} path must name a direct file")
    flags = os.O_RDONLY | directory | nofollow | getattr(os, "O_CLOEXEC", 0)
    try:
        current = os.open(anchor, flags)
    except OSError as error:
        raise InventoryError(f"{label} path anchor is not accessible") from error
    try:
        for part in relative_parts[:-1]:
            try:
                child = os.open(part, flags, dir_fd=current)
            except OSError as error:
                raise InventoryError(
                    f"{label} parent chain must contain direct directories"
                ) from error
            metadata = os.fstat(child)
            if not stat.S_ISDIR(metadata.st_mode):
                os.close(child)
                raise InventoryError(
                    f"{label} parent chain must contain direct directories"
                )
            os.close(current)
            current = child
        return current, relative_parts[-1]
    except BaseException:
        os.close(current)
        raise


def read_direct_file(
    path: Path,
    *,
    label: str,
    maximum: int,
) -> tuple[bytes, tuple[int, int]]:
    """Read one bounded, stable, single-link regular file without following links."""

    parent, leaf = _open_anchored_parent(path, label=label)
    try:
        try:
            before = os.stat(leaf, dir_fd=parent, follow_symlinks=False)
        except OSError as error:
            raise InventoryError(f"{label} is not accessible") from error
        if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
            raise InventoryError(f"{label} must be a direct regular file")
        if before.st_nlink != 1:
            raise InventoryError(f"{label} must not be hard-linked")
        if before.st_size <= 0 or before.st_size > maximum:
            raise InventoryError(f"{label} exceeds its byte bound")
        flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | os.O_NOFOLLOW
            | getattr(os, "O_NONBLOCK", 0)
        )
        try:
            descriptor = os.open(leaf, flags, dir_fd=parent)
        except OSError as error:
            raise InventoryError(f"{label} could not be opened safely") from error
        try:
            opened = os.fstat(descriptor)
            expected = (
                before.st_dev,
                before.st_ino,
                before.st_size,
                before.st_mtime_ns,
                before.st_ctime_ns,
            )
            observed = (
                opened.st_dev,
                opened.st_ino,
                opened.st_size,
                opened.st_mtime_ns,
                opened.st_ctime_ns,
            )
            if (
                not stat.S_ISREG(opened.st_mode)
                or opened.st_nlink != 1
                or observed != expected
            ):
                raise InventoryError(f"{label} changed while opening")
            chunks: list[bytes] = []
            remaining = maximum + 1
            while remaining:
                chunk = os.read(descriptor, min(1024 * 1024, remaining))
                if not chunk:
                    break
                chunks.append(chunk)
                remaining -= len(chunk)
            after_open = os.fstat(descriptor)
        finally:
            os.close(descriptor)
        try:
            after = os.stat(leaf, dir_fd=parent, follow_symlinks=False)
        except OSError as error:
            raise InventoryError(f"{label} disappeared while reading") from error
        expected = (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
        )
        final = (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        )
        open_final = (
            after_open.st_dev,
            after_open.st_ino,
            after_open.st_size,
            after_open.st_mtime_ns,
            after_open.st_ctime_ns,
        )
        data = b"".join(chunks)
        if final != expected or open_final != expected or len(data) != before.st_size:
            raise InventoryError(f"{label} changed while reading")
        if len(data) > maximum:
            raise InventoryError(f"{label} exceeds its byte bound")
        return data, (before.st_dev, before.st_ino)
    finally:
        os.close(parent)


def write_new_file(path: Path, data: bytes, *, label: str) -> None:
    """Create one owner-only output without replacing an existing path."""

    parent, leaf = _open_anchored_parent(path, label=label)
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | os.O_NOFOLLOW
    )
    try:
        try:
            descriptor = os.open(leaf, flags, 0o600, dir_fd=parent)
        except OSError as error:
            raise InventoryError(f"{label} could not be created safely") from error
        try:
            metadata = os.fstat(descriptor)
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise InventoryError(f"{label} is not a direct regular file")
            view = memoryview(data)
            while view:
                written = os.write(descriptor, view)
                if written <= 0:
                    raise InventoryError(f"{label} could not be written completely")
                view = view[written:]
            os.fsync(descriptor)
            after_open = os.fstat(descriptor)
        finally:
            os.close(descriptor)
        try:
            after = os.stat(leaf, dir_fd=parent, follow_symlinks=False)
        except OSError as error:
            raise InventoryError(f"{label} disappeared while writing") from error
        if (
            not stat.S_ISREG(after.st_mode)
            or after.st_nlink != 1
            or (after.st_dev, after.st_ino, after.st_size, after.st_ctime_ns)
            != (
                after_open.st_dev,
                after_open.st_ino,
                after_open.st_size,
                after_open.st_ctime_ns,
            )
        ):
            raise InventoryError(f"{label} changed while writing")
        os.fsync(parent)
    finally:
        os.close(parent)


def _canonical_identity(value: Any, *, label: str) -> str:
    if (
        not isinstance(value, str)
        or not 1 <= len(value) <= 160
        or value != value.strip()
        or value != unicodedata.normalize("NFC", value)
        or any(unicodedata.category(character).startswith("C") for character in value)
    ):
        raise InventoryError(f"{label} must be a bounded canonical identity")
    lowered = value.casefold()
    tokens = "".join(
        character if character.isalnum() else " " for character in lowered
    ).split()
    if any(
        token in NON_PRODUCTION_MARKERS
        or token == "localhost"
        or any(
            token.startswith(marker) and token[len(marker) :].isdigit()
            for marker in NON_PRODUCTION_MARKERS
        )
        for token in tokens
    ):
        raise InventoryError(f"{label} must identify production Taira administration")
    return value


def _positive_integer(value: Any, *, label: str) -> int:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value <= 0
        or value > MAX_INTEGER
    ):
        raise InventoryError(f"{label} must be an integer in 1..2^63-1")
    return value


def _sha256(value: Any, *, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or value != value.lower()
    ):
        raise InventoryError(f"{label} must be a canonical non-zero SHA-256")
    try:
        decoded = bytes.fromhex(value)
    except ValueError as error:
        raise InventoryError(f"{label} must be a canonical non-zero SHA-256") from error
    if not any(decoded):
        raise InventoryError(f"{label} must be a canonical non-zero SHA-256")
    return value


def _public_key(value: Any) -> bytes:
    if not isinstance(value, str) or len(value) != 64 or value != value.lower():
        raise InventoryError("verification public key must be canonical Ed25519 hex")
    try:
        decoded = bytes.fromhex(value)
    except ValueError as error:
        raise InventoryError("verification public key must be canonical Ed25519 hex") from error
    if not any(decoded):
        raise InventoryError("verification public key must be non-zero")
    return decoded


def _signature(value: Any) -> bytes:
    if not isinstance(value, str) or len(value) != 128 or value != value.lower():
        raise InventoryError("signature must be canonical Ed25519 hex")
    try:
        decoded = bytes.fromhex(value)
    except ValueError as error:
        raise InventoryError("signature must be canonical Ed25519 hex") from error
    if not any(decoded):
        raise InventoryError("signature must be non-zero")
    return decoded


def _deployment(deployment_id: Any, environment: Any) -> dict[str, Any]:
    environment_value = _canonical_identity(environment, label="environment")
    if environment_value not in PRODUCTION_ENVIRONMENTS:
        raise InventoryError("environment must be exactly prod or production")
    return {
        "deployment_id": _canonical_identity(deployment_id, label="deployment_id"),
        "environment": environment_value,
        "network": TAIRA_NETWORK,
        "chain_id": TAIRA_CHAIN_ID,
        "chain_discriminant": TAIRA_CHAIN_DISCRIMINANT,
    }


def _signer(
    public_key: bytes,
    *,
    service_id: Any,
    administrator_id: Any,
    key_revision: Any,
    policy_revision: Any,
    policy_digest_sha256: Any,
) -> dict[str, Any]:
    service = _canonical_identity(service_id, label="service_id")
    administrator = _canonical_identity(administrator_id, label="administrator_id")
    if service.casefold() == administrator.casefold():
        raise InventoryError("service_id and administrator_id must be distinct")
    return {
        "role": SIGNER_ROLE,
        "service_kind": SIGNER_KIND,
        "algorithm": "ed25519",
        "backend": "software",
        "service_id": service,
        "administrator_id": administrator,
        "key_revision": _positive_integer(key_revision, label="key_revision"),
        "policy_revision": _positive_integer(
            policy_revision,
            label="policy_revision",
        ),
        "policy_digest_sha256": _sha256(
            policy_digest_sha256,
            label="policy_digest_sha256",
        ),
        "public_key_fingerprint_sha256": hashlib.sha256(public_key).hexdigest(),
    }


def trusted_signer_binding(
    verification_public_key_hex: Any,
    *,
    service_id: Any,
    administrator_id: Any,
    key_revision: Any,
    policy_revision: Any,
    policy_digest_sha256: Any,
) -> dict[str, Any]:
    """Return the exact public software-signer binding expected by callers."""

    return _signer(
        _public_key(verification_public_key_hex),
        service_id=service_id,
        administrator_id=administrator_id,
        key_revision=key_revision,
        policy_revision=policy_revision,
        policy_digest_sha256=policy_digest_sha256,
    )


def parse_summary_specs(values: Sequence[str]) -> tuple[tuple[str, Path], ...]:
    """Parse the exact, ordered 17-lane ``LANE=PATH`` input set."""

    if len(values) != len(LANES):
        raise InventoryError("exactly 17 ordered --summary inputs are required")
    result: list[tuple[str, Path]] = []
    for index, (value, expected) in enumerate(zip(values, LANES)):
        if not isinstance(value, str):
            raise InventoryError("--summary inputs must be strings")
        lane, separator, raw_path = value.partition("=")
        if separator != "=" or not raw_path or lane != expected[0]:
            raise InventoryError(
                f"--summary[{index}] must use the canonical lane order and spelling"
            )
        result.append((lane, Path(raw_path)))
    return tuple(result)


def _walk(value: Any):
    if isinstance(value, Mapping):
        yield value
        for nested in value.values():
            yield from _walk(nested)
    elif isinstance(value, list):
        for nested in value:
            yield from _walk(nested)


def _validate_contexts(
    payload: Mapping[str, Any],
    deployment: Mapping[str, Any],
) -> None:
    observed = {field: 0 for field in DEPLOYMENT_FIELDS}
    for mapping in _walk(payload):
        for field in DEPLOYMENT_FIELDS:
            if field not in mapping:
                continue
            observed[field] += 1
            if mapping[field] != deployment[field]:
                raise InventoryError(f"lane summary {field} does not match Taira")
        for value in mapping.values():
            if isinstance(value, str) and "minamoto" in value.casefold():
                raise InventoryError("Minamoto evidence is not accepted for L1")
    if observed["deployment_id"] == 0 or observed["environment"] == 0:
        raise InventoryError("lane summary does not bind its deployment context")


def _validate_topology(
    value: Any,
    deployment: Mapping[str, Any],
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != TOPOLOGY_FIELDS:
        raise InventoryError("lane topology qualification has the wrong exact schema")
    for field in DEPLOYMENT_FIELDS:
        if value.get(field) != deployment[field]:
            raise InventoryError("lane topology qualification does not match Taira")
    for field in (
        "qualification_summary_sha256",
        "manifest_sha256",
        "canonical_manifest_sha256",
        "validator_ids_sha256",
    ):
        _sha256(value.get(field), label=f"topology_qualification.{field}")
    return value


def _summary_row(
    lane: str,
    expected_schema: str,
    path: Path,
    deployment: Mapping[str, Any],
    evaluation_now: int,
) -> tuple[dict[str, Any], tuple[str, str, str, str], tuple[int, int]]:
    data, identity = read_direct_file(
        path,
        label=f"{lane} summary",
        maximum=MAX_SUMMARY_BYTES,
    )
    payload = _decode_canonical_file(data, label=f"{lane} summary")
    if payload.get("schema") != expected_schema:
        raise InventoryError(f"{lane} summary schema does not match its lane")
    if payload.get("status") != "ready" or payload.get("errors") != []:
        raise InventoryError(f"{lane} summary must have status=ready and no errors")
    topology = _validate_topology(payload.get("topology_qualification"), deployment)
    _validate_contexts(payload, deployment)
    artifacts = payload.get("recognized_artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        raise InventoryError(f"{lane} summary must contain recognized artifacts")
    count = payload.get("recognized_artifact_count")
    if not isinstance(count, int) or isinstance(count, bool) or count != len(artifacts):
        raise InventoryError(f"{lane} recognized artifact count is inconsistent")
    timestamps: list[int] = []
    for artifact in artifacts:
        if (
            not isinstance(artifact, dict)
            or not isinstance(artifact.get("status"), str)
            or artifact["status"] not in {"passed", "verified"}
            or artifact.get("valid") is not True
            or artifact.get("errors") != []
        ):
            raise InventoryError(f"{lane} contains an invalid recognized artifact")
        fingerprint = artifact.get("fingerprint")
        if not isinstance(fingerprint, dict):
            raise InventoryError(f"{lane} artifact fingerprint is missing")
        if (
            fingerprint.get("deployment_id") != deployment["deployment_id"]
            or fingerprint.get("environment") != deployment["environment"]
            or fingerprint.get("deployment_context_reviewed") is not True
        ):
            raise InventoryError(f"{lane} artifact deployment is not reviewed")
        generated = _positive_integer(
            fingerprint.get("generated_at_unix"),
            label=f"{lane} generated_at_unix",
        )
        if generated > evaluation_now:
            raise InventoryError(f"{lane} evidence timestamp is in the future")
        if evaluation_now - generated > MAX_SUMMARY_AGE_SECS:
            raise InventoryError(f"{lane} evidence is stale")
        timestamps.append(generated)
    anchor = (
        topology["qualification_summary_sha256"],
        topology["manifest_sha256"],
        topology["canonical_manifest_sha256"],
        topology["validator_ids_sha256"],
    )
    row = {
        "lane": lane,
        "schema": expected_schema,
        "summary_sha256": hashlib.sha256(data).hexdigest(),
        "recognized_artifact_count": count,
        "oldest_generated_at_unix": min(timestamps),
        "newest_generated_at_unix": max(timestamps),
    }
    return row, anchor, identity


def build_unsigned_inventory(
    summary_specs: Sequence[tuple[str, Path]],
    *,
    deployment_id: Any,
    environment: Any,
    generated_at_unix: Any,
    evaluation_now: Any,
    verification_public_key_hex: Any,
    service_id: Any,
    administrator_id: Any,
    key_revision: Any,
    policy_revision: Any,
    policy_digest_sha256: Any,
    expected_topology_qualification_summary_sha256: Any,
    expected_topology_manifest_sha256: Any,
    expected_topology_canonical_manifest_sha256: Any,
    expected_validator_ids_sha256: Any,
) -> dict[str, Any]:
    """Replay the summaries and build one schema-closed unsigned inventory."""

    generated = _positive_integer(generated_at_unix, label="generated_at_unix")
    now = _positive_integer(evaluation_now, label="evaluation_now")
    if generated > now or now - generated > MAX_SUMMARY_AGE_SECS:
        raise InventoryError("inventory generation time is future or stale")
    if tuple(lane for lane, _path in summary_specs) != tuple(lane for lane, _ in LANES):
        raise InventoryError("summary inputs do not use the canonical 17-lane order")
    deployment = _deployment(deployment_id, environment)
    public_key = _public_key(verification_public_key_hex)
    signer = _signer(
        public_key,
        service_id=service_id,
        administrator_id=administrator_id,
        key_revision=key_revision,
        policy_revision=policy_revision,
        policy_digest_sha256=policy_digest_sha256,
    )
    expected_anchor = (
        _sha256(
            expected_topology_qualification_summary_sha256,
            label="expected topology qualification summary SHA-256",
        ),
        _sha256(
            expected_topology_manifest_sha256,
            label="expected topology manifest SHA-256",
        ),
        _sha256(
            expected_topology_canonical_manifest_sha256,
            label="expected topology canonical manifest SHA-256",
        ),
        _sha256(
            expected_validator_ids_sha256,
            label="expected validator IDs SHA-256",
        ),
    )
    rows: list[dict[str, Any]] = []
    anchors: set[tuple[str, str, str, str]] = set()
    identities: set[tuple[int, int]] = set()
    digests: set[str] = set()
    for (lane, path), expected in zip(summary_specs, LANES):
        if lane != expected[0]:
            raise InventoryError("summary inputs do not use the canonical lane order")
        row, anchor, identity = _summary_row(
            lane,
            expected[1],
            path,
            deployment,
            now,
        )
        if identity in identities or row["summary_sha256"] in digests:
            raise InventoryError("lane summaries must be distinct files and bytes")
        identities.add(identity)
        digests.add(row["summary_sha256"])
        anchors.add(anchor)
        rows.append(row)
    if len(rows) != 17 or anchors != {expected_anchor}:
        raise InventoryError(
            "all 17 lane summaries must match the operator-trusted fresh topology anchor"
        )
    anchor = next(iter(anchors))
    return {
        "schema": INVENTORY_SCHEMA,
        "status": "ready",
        "signer_qualification": "software-key-qualified",
        "generated_at_unix": generated,
        "max_summary_age_secs": MAX_SUMMARY_AGE_SECS,
        "summary_file_count": 17,
        "recognized_summary_count": 17,
        "deployment": deployment,
        "anchors": {
            "topology_qualification_summary_sha256": anchor[0],
            "topology_manifest_sha256": anchor[1],
            "topology_canonical_manifest_sha256": anchor[2],
            "validator_ids_sha256": anchor[3],
            "oldest_evidence_generated_at_unix": min(
                row["oldest_generated_at_unix"] for row in rows
            ),
            "newest_evidence_generated_at_unix": max(
                row["newest_generated_at_unix"] for row in rows
            ),
        },
        "summaries": rows,
        "signer": signer,
    }


def signing_bytes(unsigned_inventory: Mapping[str, Any]) -> bytes:
    """Return exact domain-separated bytes for the external software signer."""

    _validate_inventory_shape(unsigned_inventory, signed=False)
    return SIGNING_DOMAIN + canonical_json_bytes(unsigned_inventory)


def _validate_inventory_shape(value: Mapping[str, Any], *, signed: bool) -> None:
    if not isinstance(value, Mapping) or set(value) != INVENTORY_FIELDS:
        raise InventoryError("inventory has the wrong exact schema")
    signer = value.get("signer")
    expected_signer_fields = SIGNER_FIELDS if signed else SIGNER_BASE_FIELDS
    if not isinstance(signer, Mapping) or set(signer) != expected_signer_fields:
        raise InventoryError("inventory signer has the wrong exact schema")
    deployment = value.get("deployment")
    anchors = value.get("anchors")
    rows = value.get("summaries")
    if not isinstance(deployment, Mapping) or set(deployment) != DEPLOYMENT_FIELDS:
        raise InventoryError("inventory deployment has the wrong exact schema")
    if not isinstance(anchors, Mapping) or set(anchors) != ANCHOR_FIELDS:
        raise InventoryError("inventory anchors have the wrong exact schema")
    if not isinstance(rows, list) or any(
        not isinstance(row, Mapping) or set(row) != ROW_FIELDS for row in rows
    ):
        raise InventoryError("inventory summaries have the wrong exact schema")


def load_canonical_inventory_file(
    path: Path,
    *,
    signed: bool = True,
) -> tuple[dict[str, Any], bytes]:
    """Load one bounded canonical inventory without following any path link."""

    raw, _identity = read_direct_file(
        path,
        label="L1 lane evidence inventory",
        maximum=MAX_INVENTORY_BYTES,
    )
    value = _decode_canonical_file(raw, label="L1 lane evidence inventory")
    _validate_inventory_shape(value, signed=signed)
    return value, raw


def replay_unsigned_inventory(
    value: Mapping[str, Any],
    summary_specs: Sequence[tuple[str, Path]],
    *,
    deployment_id: Any,
    environment: Any,
    evaluation_now: Any,
    verification_public_key_hex: Any,
    service_id: Any,
    administrator_id: Any,
    key_revision: Any,
    policy_revision: Any,
    policy_digest_sha256: Any,
    expected_topology_qualification_summary_sha256: Any,
    expected_topology_manifest_sha256: Any,
    expected_topology_canonical_manifest_sha256: Any,
    expected_validator_ids_sha256: Any,
) -> dict[str, Any]:
    """Rebuild and compare every unsigned inventory field."""

    _validate_inventory_shape(value, signed=False)
    expected = build_unsigned_inventory(
        summary_specs,
        deployment_id=deployment_id,
        environment=environment,
        generated_at_unix=value.get("generated_at_unix"),
        evaluation_now=evaluation_now,
        verification_public_key_hex=verification_public_key_hex,
        service_id=service_id,
        administrator_id=administrator_id,
        key_revision=key_revision,
        policy_revision=policy_revision,
        policy_digest_sha256=policy_digest_sha256,
        expected_topology_qualification_summary_sha256=(
            expected_topology_qualification_summary_sha256
        ),
        expected_topology_manifest_sha256=expected_topology_manifest_sha256,
        expected_topology_canonical_manifest_sha256=(
            expected_topology_canonical_manifest_sha256
        ),
        expected_validator_ids_sha256=expected_validator_ids_sha256,
    )
    if value != expected:
        raise InventoryError("inventory does not match deterministic summary replay")
    return expected


def finalize_inventory(
    prepared: Mapping[str, Any],
    signature_hex: Any,
    summary_specs: Sequence[tuple[str, Path]],
    **trust: Any,
) -> dict[str, Any]:
    """Replay, authenticate, and attach one detached Ed25519 signature."""

    unsigned = replay_unsigned_inventory(prepared, summary_specs, **trust)
    signature = _signature(signature_hex)
    public_key = _public_key(trust["verification_public_key_hex"])
    if not verify_ed25519(public_key, signature, signing_bytes(unsigned)):
        raise InventoryError("detached Ed25519 inventory signature is invalid")
    result = dict(unsigned)
    result["signer"] = dict(unsigned["signer"])
    result["signer"]["signature_hex"] = signature.hex()
    _validate_inventory_shape(result, signed=True)
    return result


def verify_inventory(
    inventory: Mapping[str, Any],
    summary_specs: Sequence[tuple[str, Path]],
    **trust: Any,
) -> dict[str, Any]:
    """Verify a signed inventory and return its deterministic payload-free result."""

    _validate_inventory_shape(inventory, signed=True)
    unsigned = dict(inventory)
    signer = dict(inventory["signer"])
    signature = _signature(signer.pop("signature_hex"))
    unsigned["signer"] = signer
    replay_unsigned_inventory(unsigned, summary_specs, **trust)
    public_key = _public_key(trust["verification_public_key_hex"])
    if not verify_ed25519(public_key, signature, signing_bytes(unsigned)):
        raise InventoryError("signed inventory Ed25519 signature is invalid")
    return {
        "schema": VERIFICATION_SCHEMA,
        "status": "ready",
        "signer_qualification": "software-key-qualified",
        "inventory_sha256": hashlib.sha256(canonical_file_bytes(inventory)).hexdigest(),
        "summary_file_count": 17,
        "recognized_summary_count": 17,
        "deployment": dict(inventory["deployment"]),
        "anchors": dict(inventory["anchors"]),
        "signer": {
            key: signer[key]
            for key in (
                "role",
                "service_kind",
                "backend",
                "algorithm",
                "service_id",
                "administrator_id",
                "key_revision",
                "policy_revision",
                "policy_digest_sha256",
                "public_key_fingerprint_sha256",
            )
        },
    }


def _positive_arg(value: str) -> int:
    try:
        parsed = int(value, 10)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be an integer") from error
    if parsed <= 0 or parsed > MAX_INTEGER:
        raise argparse.ArgumentTypeError("must be in 1..2^63-1")
    return parsed


def _add_summary_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--summary",
        action="append",
        default=[],
        metavar="LANE=PATH",
        help="One canonical lane summary; repeat exactly 17 times in canonical order.",
    )
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--now-unix", required=True, type=_positive_arg)


def _add_trust_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--verification-public-key-hex", required=True)
    parser.add_argument("--service-id", required=True)
    parser.add_argument("--administrator-id", required=True)
    parser.add_argument("--key-revision", required=True, type=_positive_arg)
    parser.add_argument("--policy-revision", required=True, type=_positive_arg)
    parser.add_argument("--policy-digest-sha256", required=True)
    parser.add_argument(
        "--expected-topology-qualification-summary-sha256",
        required=True,
    )
    parser.add_argument("--expected-topology-manifest-sha256", required=True)
    parser.add_argument(
        "--expected-topology-canonical-manifest-sha256",
        required=True,
    )
    parser.add_argument("--expected-validator-ids-sha256", required=True)


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse the public no-private-key CLI."""

    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("prepare", help="Validate lanes and emit signing bytes.")
    _add_summary_arguments(prepare)
    _add_trust_arguments(prepare)
    prepare.add_argument("--generated-at-unix", required=True, type=_positive_arg)
    prepare.add_argument("--prepared-out", required=True, type=Path)
    prepare.add_argument("--signing-payload-out", required=True, type=Path)
    finalize = commands.add_parser("finalize", help="Attach a detached signature.")
    _add_summary_arguments(finalize)
    _add_trust_arguments(finalize)
    finalize.add_argument("--prepared", required=True, type=Path)
    finalize.add_argument("--signature-hex", required=True)
    finalize.add_argument("--inventory-out", required=True, type=Path)
    verify = commands.add_parser("verify", help="Replay lanes and verify the inventory.")
    _add_summary_arguments(verify)
    _add_trust_arguments(verify)
    verify.add_argument("--inventory", required=True, type=Path)
    verify.add_argument("--verification-out", type=Path)
    return parser.parse_args(argv)


def _trust(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "evaluation_now": args.now_unix,
        "verification_public_key_hex": args.verification_public_key_hex,
        "service_id": args.service_id,
        "administrator_id": args.administrator_id,
        "key_revision": args.key_revision,
        "policy_revision": args.policy_revision,
        "policy_digest_sha256": args.policy_digest_sha256,
        "expected_topology_qualification_summary_sha256": (
            args.expected_topology_qualification_summary_sha256
        ),
        "expected_topology_manifest_sha256": args.expected_topology_manifest_sha256,
        "expected_topology_canonical_manifest_sha256": (
            args.expected_topology_canonical_manifest_sha256
        ),
        "expected_validator_ids_sha256": args.expected_validator_ids_sha256,
    }


def main(argv: Sequence[str] | None = None) -> int:
    """Run prepare, finalize, or deterministic verification."""

    raw_argv = list(sys.argv[1:] if argv is None else argv)
    if any(
        argument.casefold().startswith(SECRET_ARGUMENT_PREFIXES)
        for argument in raw_argv
    ):
        print("error: secret signing inputs are not accepted", file=sys.stderr)
        return 2
    try:
        args = parse_args(raw_argv)
        specs = parse_summary_specs(args.summary)
        if args.command == "prepare":
            unsigned = build_unsigned_inventory(
                specs,
                generated_at_unix=args.generated_at_unix,
                **_trust(args),
            )
            write_new_file(
                args.prepared_out,
                canonical_file_bytes(unsigned),
                label="prepared inventory output",
            )
            write_new_file(
                args.signing_payload_out,
                signing_bytes(unsigned),
                label="signing payload output",
            )
            return 0
        source_path = args.prepared if args.command == "finalize" else args.inventory
        raw, _identity = read_direct_file(
            source_path,
            label=f"{args.command} inventory",
            maximum=MAX_INVENTORY_BYTES,
        )
        value = _decode_canonical_file(raw, label=f"{args.command} inventory")
        if args.command == "finalize":
            finalized = finalize_inventory(value, args.signature_hex, specs, **_trust(args))
            write_new_file(
                args.inventory_out,
                canonical_file_bytes(finalized),
                label="signed inventory output",
            )
            return 0
        verified = verify_inventory(value, specs, **_trust(args))
        rendered = canonical_file_bytes(verified)
        if args.verification_out is None:
            sys.stdout.buffer.write(rendered)
        else:
            write_new_file(
                args.verification_out,
                rendered,
                label="verification output",
            )
        return 0
    except InventoryError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
