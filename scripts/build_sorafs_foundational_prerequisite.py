#!/usr/bin/env python3
"""Prepare and finalize the signed SoraFS foundational prerequisite envelope.

The tool never accepts private signing material. ``prepare`` writes the exact
domain-separated binary payload for an external Ed25519 HSM, while ``finalize``
verifies the raw detached signature against an operator-trusted public key and
writes the schema-closed envelope consumed by the aggregate readiness gate.
Prerequisite anchors are derived only from validated, exact evidence-package
files; callers cannot supply digests or evidence timestamps directly.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import secrets
import stat
import sys
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_production_readiness import (  # noqa: E402
    DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    DEFAULT_REQUIRED_GATES,
    FOUNDATIONAL_PREREQUISITE_FIELDS,
    FOUNDATIONAL_PREREQUISITE_IDS,
    FOUNDATIONAL_PREREQUISITE_ROW_FIELDS,
    FOUNDATIONAL_PREREQUISITE_SCHEMA,
    FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN,
    FOUNDATIONAL_PREREQUISITE_SIGNATURE_FIELDS,
    MAX_FOUNDATIONAL_RELEASE_SEQUENCE,
    MAX_SUMMARY_BYTES,
    GATE_BY_NAME,
    SENSITIVE_KEYS,
    ValidationOptions,
    canonical_lower_hex,
    canonical_string,
    foundational_signing_payload,
    is_archive_portable_artifact_path,
    load_resilience_qualification_binding,
    parse_foundational_signer_public_key,
    validate_gate_summary,
    validate_foundational_exact_fields,
    validate_foundational_prerequisite_summary,
)
from sccp_release_common import verify_ed25519  # noqa: E402
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_lines,
    emit_checker_exception,
    emit_checker_notice,
    validate_checker_output_parent,
    write_all_checker_summary_bytes,
)
from sorafs_path_identity import (  # noqa: E402
    error_diagnostic_label,
    path_diagnostic_label,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)
from sorafs_runner_preflight import plan_rendered_path_is_safe  # noqa: E402
from sorafs_topology_qualification import (  # noqa: E402
    add_topology_qualification_argument,
    load_topology_qualification_binding,
    validate_topology_binding_object,
)


MAX_FOUNDATIONAL_ARTIFACT_BYTES = 64 * 1024
MAX_PREREQUISITE_PACKAGE_BYTES = MAX_SUMMARY_BYTES
MAX_LANE_SUMMARY_BYTES = MAX_SUMMARY_BYTES
MAX_DEPLOYMENT_ID_BYTES = 128
MAX_ENVIRONMENT_BYTES = 16
MAX_PATH_TEXT_BYTES = 4096
MAX_TIMESTAMP = (1 << 63) - 1
RAW_ED25519_PUBLIC_KEY_BYTES = 32
RAW_ED25519_SIGNATURE_BYTES = 64
FOUNDATIONAL_PREREQUISITE_EVIDENCE_PACKAGE_SCHEMA = (
    "sorafs.production_readiness.foundational_prerequisite_evidence_package.v1"
)
FOUNDATIONAL_PREREQUISITE_EVIDENCE_PACKAGE_FIELDS = frozenset(
    {
        "schema",
        "prerequisite_id",
        "status",
        "deployment",
        "evidence_generated_at_unix",
        "topology_qualification",
        "readiness_summary",
        "errors",
    }
)
FOUNDATIONAL_PREREQUISITE_EVIDENCE_DEPLOYMENT_FIELDS = frozenset(
    {"deployment_id", "environment"}
)
FOUNDATIONAL_PREREQUISITE_EVIDENCE_SUMMARY_FIELDS = frozenset(
    {"gate", "path", "sha256"}
)
UNSIGNED_SIGNATURE_FIELDS = (
    FOUNDATIONAL_PREREQUISITE_SIGNATURE_FIELDS - {"signature_hex"}
)
ZERO_SIGNATURE_DIAGNOSTIC = (
    "foundational prerequisite signature must be a non-zero canonical "
    "Ed25519 signature"
)


def canonical_json_bytes(value: Any) -> bytes:
    """Return the exact canonical JSON bytes used by the signing contract."""

    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")


def render_envelope(payload: dict[str, Any]) -> bytes:
    """Render one deterministic final envelope artifact."""

    return (
        json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    ).encode("utf-8")


def _bounded_positive_integer(
    value: Any,
    *,
    label: str,
    maximum: int,
    errors: list[str],
) -> int | None:
    """Return a bounded positive integer or record one stable diagnostic."""

    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value <= 0
        or value > maximum
    ):
        errors.append(f"{label} must be an integer in 1..{maximum}")
        return None
    return value


def _validate_clock_inputs(
    *,
    now_unix: Any,
    max_evidence_age_secs: Any,
    errors: list[str],
) -> None:
    """Bound the reviewed clock and freshness window."""

    _bounded_positive_integer(
        now_unix,
        label="--now-unix",
        maximum=MAX_TIMESTAMP,
        errors=errors,
    )
    if (
        not isinstance(max_evidence_age_secs, int)
        or isinstance(max_evidence_age_secs, bool)
        or max_evidence_age_secs < 0
        or max_evidence_age_secs > MAX_TIMESTAMP
    ):
        errors.append(
            f"--max-evidence-age-secs must be an integer in 0..{MAX_TIMESTAMP}"
        )


def _validate_bounded_text(
    value: Any,
    *,
    label: str,
    maximum_bytes: int,
    errors: list[str],
) -> None:
    """Require canonical bounded UTF-8 text without echoing its contents."""

    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        errors.append(f"{label} must be a non-empty canonical string")
        return
    try:
        encoded = value.encode("utf-8")
    except UnicodeEncodeError:
        errors.append(f"{label} must be valid UTF-8")
        return
    if len(encoded) > maximum_bytes:
        errors.append(f"{label} must be at most {maximum_bytes} UTF-8 bytes")


def parse_prerequisite_specs(
    values: list[str],
    errors: list[str],
    *,
    deployment_id: str,
    environment: str,
    generated_at_unix: int,
    now_unix: int,
    max_evidence_age_secs: int,
    topology_qualification: dict[str, str],
) -> list[dict[str, Any]]:
    """Validate and hash the exact ordered nine prerequisite evidence packages."""

    rows: list[dict[str, Any]] = []
    validated_summary_cache: dict[tuple[str, str], dict[str, Any]] = {}
    if len(values) != len(FOUNDATIONAL_PREREQUISITE_IDS):
        errors.append(
            "exactly nine --prerequisite values are required in canonical order"
        )
    for index, value in enumerate(values):
        if not isinstance(value, str):
            errors.append(f"--prerequisite[{index}] must be a string")
            continue
        try:
            encoded = value.encode("utf-8")
        except UnicodeEncodeError:
            errors.append(f"--prerequisite[{index}] must be valid UTF-8")
            continue
        if len(encoded) > MAX_PATH_TEXT_BYTES + 128:
            errors.append(f"--prerequisite[{index}] is too long")
            continue
        prerequisite_id, separator, path_text = value.partition("=")
        if separator != "=" or not prerequisite_id or not path_text:
            errors.append(f"--prerequisite[{index}] must use ID=PATH")
            continue
        expected_id = (
            FOUNDATIONAL_PREREQUISITE_IDS[index]
            if index < len(FOUNDATIONAL_PREREQUISITE_IDS)
            else None
        )
        if prerequisite_id != expected_id:
            errors.append(
                "foundational prerequisites must match the exact required set and "
                "canonical order"
            )
        package_path = Path(path_text)
        package_raw, package_read_errors = read_bounded_regular_file(
            package_path,
            label=f"--prerequisite[{index}]",
            maximum_bytes=MAX_PREREQUISITE_PACKAGE_BYTES,
        )
        errors.extend(package_read_errors)
        if package_raw is None:
            continue

        package_errors: list[str] = []
        package = _strict_lane_summary_object(
            package_raw,
            label=f"--prerequisite[{index}]",
            errors=package_errors,
        )
        if package is None:
            errors.extend(package_errors)
            continue
        validate_foundational_exact_fields(
            package,
            FOUNDATIONAL_PREREQUISITE_EVIDENCE_PACKAGE_FIELDS,
            f"--prerequisite[{index}] evidence package",
            package_errors,
        )
        if package.get("schema") != FOUNDATIONAL_PREREQUISITE_EVIDENCE_PACKAGE_SCHEMA:
            package_errors.append(
                f"--prerequisite[{index}] evidence package schema must match "
                "the foundational prerequisite evidence contract"
            )
        package_id = canonical_string(package.get("prerequisite_id"))
        if package_id != prerequisite_id:
            package_errors.append(
                f"--prerequisite[{index}] evidence package prerequisite_id must "
                "match its ordered command-line id"
            )
        if package.get("status") != "verified":
            package_errors.append(
                f"--prerequisite[{index}] evidence package status must be `verified`"
            )
        package_deployment = validate_foundational_exact_fields(
            package.get("deployment"),
            FOUNDATIONAL_PREREQUISITE_EVIDENCE_DEPLOYMENT_FIELDS,
            f"--prerequisite[{index}] evidence package deployment",
            package_errors,
        )
        if package_deployment is not None:
            if package_deployment.get("deployment_id") != deployment_id:
                package_errors.append(
                    f"--prerequisite[{index}] evidence package deployment_id must "
                    "match --deployment-id"
                )
            if package_deployment.get("environment") != environment:
                package_errors.append(
                    f"--prerequisite[{index}] evidence package environment must "
                    "match --environment"
                )
        package_errors.extend(
            validate_topology_binding_object(
                package.get("topology_qualification"),
                expected=topology_qualification,
                path=(
                    f"--prerequisite[{index}] evidence package "
                    "topology_qualification"
                ),
            )
        )
        if package.get("errors") != []:
            package_errors.append(
                f"--prerequisite[{index}] evidence package errors must be empty"
            )
        evidence_generated_at_unix = _bounded_positive_integer(
            package.get("evidence_generated_at_unix"),
            label=f"--prerequisite[{index}] evidence_generated_at_unix",
            maximum=MAX_TIMESTAMP,
            errors=package_errors,
        )
        if evidence_generated_at_unix is not None:
            if evidence_generated_at_unix > now_unix:
                package_errors.append(
                    f"--prerequisite[{index}] evidence_generated_at_unix must "
                    "not be future"
                )
            elif now_unix - evidence_generated_at_unix > max_evidence_age_secs:
                package_errors.append(
                    f"--prerequisite[{index}] evidence_generated_at_unix exceeds "
                    "max summary artifact age"
                )
            if evidence_generated_at_unix > generated_at_unix:
                package_errors.append(
                    f"--prerequisite[{index}] evidence_generated_at_unix must not "
                    "be later than the signed envelope"
                )

        summary_reference = validate_foundational_exact_fields(
            package.get("readiness_summary"),
            FOUNDATIONAL_PREREQUISITE_EVIDENCE_SUMMARY_FIELDS,
            f"--prerequisite[{index}] evidence package readiness_summary",
            package_errors,
        )
        summary_row: dict[str, Any] | None = None
        if summary_reference is not None:
            gate_name = canonical_string(summary_reference.get("gate"))
            gate = GATE_BY_NAME.get(gate_name) if gate_name is not None else None
            if gate is None:
                package_errors.append(
                    f"--prerequisite[{index}] evidence package readiness_summary.gate "
                    "must name an authoritative bundled readiness checker"
                )
            relative_path = canonical_string(summary_reference.get("path"))
            if (
                relative_path is None
                or not is_archive_portable_artifact_path(relative_path)
            ):
                package_errors.append(
                    f"--prerequisite[{index}] evidence package "
                    "readiness_summary.path must be archive-relative and portable"
                )
                summary_path = None
            else:
                summary_path = package_path.parent.joinpath(*relative_path.split("/"))
            expected_digest = canonical_lower_hex(
                summary_reference.get("sha256"),
                64,
            )
            if expected_digest is None or not any(bytes.fromhex(expected_digest)):
                package_errors.append(
                    f"--prerequisite[{index}] evidence package "
                    "readiness_summary.sha256 must be a non-zero canonical "
                    "lowercase SHA-256"
                )
            if summary_path is not None:
                summary_raw, summary_read_errors = read_bounded_regular_file(
                    summary_path,
                    label=(
                        f"--prerequisite[{index}] evidence package "
                        "readiness_summary"
                    ),
                    maximum_bytes=MAX_SUMMARY_BYTES,
                )
                package_errors.extend(summary_read_errors)
            else:
                summary_raw = None
            if summary_raw is not None:
                observed_digest = hashlib.sha256(summary_raw).hexdigest()
                if expected_digest != observed_digest:
                    package_errors.append(
                        f"--prerequisite[{index}] evidence package readiness_summary "
                        "digest does not match the exact file"
                    )
                cache_key = (
                    (gate.name, observed_digest) if gate is not None else None
                )
                if cache_key is not None:
                    summary_row = validated_summary_cache.get(cache_key)
                if summary_row is None:
                    summary_payload = _strict_lane_summary_object(
                        summary_raw,
                        label=(
                            f"--prerequisite[{index}] evidence package "
                            "readiness_summary"
                        ),
                        errors=package_errors,
                    )
                    if gate is not None and summary_payload is not None:
                        if summary_payload.get("schema") != gate.schema:
                            package_errors.append(
                                f"--prerequisite[{index}] evidence package "
                                "readiness_summary schema must match its gate"
                            )
                        summary_row, summary_errors = validate_gate_summary(
                            gate,
                            summary_payload,
                            ValidationOptions(
                                now_unix=now_unix,
                                max_summary_artifact_age_secs=max_evidence_age_secs,
                                deployment_id=deployment_id,
                                environment=environment,
                                topology_qualification=topology_qualification,
                            ),
                        )
                        package_errors.extend(
                            f"--prerequisite[{index}] evidence package "
                            f"readiness_summary: {error}"
                            for error in summary_errors
                        )
                        if (
                            cache_key is not None
                            and expected_digest == observed_digest
                            and not summary_errors
                            and summary_payload.get("schema") == gate.schema
                        ):
                            validated_summary_cache[cache_key] = summary_row
        if (
            summary_row is not None
            and evidence_generated_at_unix is not None
            and summary_row.get("newest_generated_at_unix")
            != evidence_generated_at_unix
        ):
            package_errors.append(
                f"--prerequisite[{index}] evidence_generated_at_unix must match "
                "the authoritative readiness summary"
            )
        errors.extend(package_errors)
        if (
            package_errors
            or prerequisite_id != expected_id
            or evidence_generated_at_unix is None
        ):
            continue
        rows.append(
            {
                "id": prerequisite_id,
                "status": "verified",
                "evidence_anchor_sha256": hashlib.sha256(package_raw).hexdigest(),
                "evidence_generated_at_unix": evidence_generated_at_unix,
            }
        )

    observed_ids = [row["id"] for row in rows]
    expected_ids = list(FOUNDATIONAL_PREREQUISITE_IDS)
    if observed_ids != expected_ids:
        errors.append(
            "foundational prerequisites must match the exact required set and "
            "canonical order"
        )
        if len(observed_ids) != len(set(observed_ids)):
            errors.append("foundational prerequisites must not contain duplicate ids")
        if set(expected_ids) - set(observed_ids):
            errors.append("foundational prerequisites are missing required ids")
        if set(observed_ids) - set(expected_ids):
            errors.append("foundational prerequisites contain unknown ids")
    anchors = [row["evidence_anchor_sha256"] for row in rows]
    if len(anchors) != len(set(anchors)):
        errors.append("foundational prerequisites must use unique evidence anchors")
    return rows


def _strict_lane_summary_object(
    raw: bytes,
    *,
    label: str,
    errors: list[str],
) -> dict[str, Any] | None:
    """Decode one bounded lane summary without accepting duplicate JSON keys."""

    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        errors.append(f"{label} must be valid UTF-8 JSON")
        return None

    def reject_duplicate_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate object member")
            result[key] = value
        return result

    try:
        value = json.loads(
            text,
            parse_constant=lambda _value: (_ for _ in ()).throw(
                ValueError("non-finite number")
            ),
            object_pairs_hook=reject_duplicate_pairs,
        )
    except (RecursionError, TypeError, ValueError, json.JSONDecodeError):
        errors.append(f"{label} must be strict duplicate-free JSON")
        return None
    if not isinstance(value, dict):
        errors.append(f"{label} must contain a JSON object")
        return None
    return value


def parse_lane_summary_specs(
    values: list[str],
    errors: list[str],
    *,
    topology_qualification: dict[str, str],
) -> list[dict[str, str]]:
    """Read and hash the exact ordered 17 lane summaries approved for signing."""

    rows: list[dict[str, str]] = []
    if len(values) != len(DEFAULT_REQUIRED_GATES):
        errors.append(
            "exactly 17 --lane-summary values are required in canonical lane order"
        )
    for index, value in enumerate(values):
        label = f"--lane-summary[{index}]"
        if not isinstance(value, str):
            errors.append(f"{label} must be a string")
            continue
        try:
            encoded = value.encode("utf-8")
        except UnicodeEncodeError:
            errors.append(f"{label} must be valid UTF-8")
            continue
        if len(encoded) > MAX_PATH_TEXT_BYTES + 128:
            errors.append(f"{label} is too long")
            continue
        gate_name, separator, path_text = value.partition("=")
        if separator != "=" or not gate_name or not path_text:
            errors.append(f"{label} must use GATE=PATH")
            continue
        expected_gate = (
            DEFAULT_REQUIRED_GATES[index]
            if index < len(DEFAULT_REQUIRED_GATES)
            else None
        )
        if gate_name != expected_gate:
            errors.append(
                "--lane-summary values must match all 17 readiness lanes in "
                "canonical order"
            )
        path = Path(path_text)
        raw, read_errors = read_bounded_regular_file(
            path,
            label=label,
            maximum_bytes=MAX_LANE_SUMMARY_BYTES,
        )
        errors.extend(read_errors)
        if raw is None:
            continue
        payload = _strict_lane_summary_object(raw, label=label, errors=errors)
        gate = GATE_BY_NAME.get(gate_name)
        if gate is None:
            errors.append(f"{label} gate is not part of the readiness contract")
            continue
        if payload is None:
            continue
        if payload.get("schema") != gate.schema:
            errors.append(f"{label} schema must match the {gate_name} gate")
        if payload.get("status") != "ready":
            errors.append(f"{label} status must be `ready`")
        errors.extend(
            validate_topology_binding_object(
                payload.get("topology_qualification"),
                expected=topology_qualification,
                path=f"{label} topology_qualification",
            )
        )
        rows.append(
            {
                "gate": gate_name,
                "sha256": hashlib.sha256(raw).hexdigest(),
            }
        )

    observed_gates = [row["gate"] for row in rows]
    if observed_gates != list(DEFAULT_REQUIRED_GATES):
        errors.append(
            "foundational lane_summaries must match all 17 readiness lanes in "
            "canonical order"
        )
    digests = [row["sha256"] for row in rows]
    if len(digests) != len(set(digests)):
        errors.append("foundational lane_summaries must use unique summary digests")
    return rows


def parse_trusted_public_key(value: Any, errors: list[str]) -> bytes | None:
    """Parse the operator-trusted key through the aggregate gate contract."""

    return parse_foundational_signer_public_key(
        value,
        errors,
        path="--trusted-public-key-hex",
    )


def validate_output_path(path: Path, *, label: str, errors: list[str]) -> None:
    """Reject ambiguous, secret-looking, symlinked, or clobbering outputs."""

    if (
        not isinstance(path, Path)
        or len(str(path).encode("utf-8", errors="ignore")) > MAX_PATH_TEXT_BYTES
        or not plan_rendered_path_is_safe(path)
    ):
        errors.append(f"{label} path is not a canonical safe artifact path")
        return
    try:
        if path.is_symlink():
            errors.append(f"{label} must not be a symlink")
            return
        if path.exists():
            errors.append(f"{label} must not already exist")
            return
    except (OSError, RuntimeError):
        errors.append(f"{label} cannot be inspected")
        return
    validate_checker_output_parent(path, errors, label=label)


def validate_input_path(path: Path, *, label: str, errors: list[str]) -> None:
    """Reject unsafe input paths before any artifact bytes are opened."""

    if (
        not isinstance(path, Path)
        or len(str(path).encode("utf-8", errors="ignore")) > MAX_PATH_TEXT_BYTES
        or not plan_rendered_path_is_safe(path)
    ):
        errors.append(f"{label} path is not a canonical safe artifact path")
        return
    validate_checker_output_parent(path, errors, label=label)
    if errors:
        return
    try:
        if path.is_symlink():
            errors.append(f"{label} must not be a symlink")
        elif not path.is_file():
            errors.append(f"{label} must be an existing regular file")
    except (OSError, RuntimeError):
        errors.append(f"{label} cannot be inspected")


def _anchored_path_components(path: Path) -> tuple[str, ...]:
    """Return canonical absolute components without resolving symbolic links."""

    absolute = path if path.is_absolute() else Path.cwd() / path
    components = absolute.parts[1:]
    if not components or any(
        component in {"", ".", ".."} for component in components
    ):
        raise OSError("path is not canonical")
    return components


def _directory_open_flags() -> int:
    """Return flags for fail-closed directory-FD traversal."""

    return (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def _open_anchored_parent(
    path: Path,
    *,
    create: bool,
) -> tuple[int, str, list[int]]:
    """Pin every parent component and return its directory FD plus leaf name."""

    components = _anchored_path_components(path)
    directory_fds: list[int] = []
    try:
        current_fd = os.open("/", _directory_open_flags())
        directory_fds.append(current_fd)
        for component in components[:-1]:
            try:
                next_fd = os.open(
                    component,
                    _directory_open_flags(),
                    dir_fd=current_fd,
                )
            except FileNotFoundError:
                if not create:
                    raise
                os.mkdir(component, mode=0o700, dir_fd=current_fd)
                next_fd = os.open(
                    component,
                    _directory_open_flags(),
                    dir_fd=current_fd,
                )
            current_fd = next_fd
            directory_fds.append(current_fd)
        return current_fd, components[-1], directory_fds
    except BaseException:
        for directory_fd in reversed(directory_fds):
            os.close(directory_fd)
        raise


def _anchored_path_identity_matches(
    path: Path,
    *,
    expected_parent: os.stat_result,
    expected_leaf: os.stat_result,
) -> bool:
    """Reopen a path and confirm that its pinned parent and leaf are unchanged."""

    directory_fds: list[int] = []
    try:
        parent_fd, leaf, directory_fds = _open_anchored_parent(
            path,
            create=False,
        )
        observed_parent = os.fstat(parent_fd)
        observed_leaf = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        return (
            observed_parent.st_dev,
            observed_parent.st_ino,
            observed_leaf.st_dev,
            observed_leaf.st_ino,
        ) == (
            expected_parent.st_dev,
            expected_parent.st_ino,
            expected_leaf.st_dev,
            expected_leaf.st_ino,
        )
    except (OSError, RuntimeError):
        return False
    finally:
        for directory_fd in reversed(directory_fds):
            os.close(directory_fd)


def read_bounded_regular_file(
    path: Path,
    *,
    label: str,
    maximum_bytes: int,
) -> tuple[bytes | None, list[str]]:
    """Read a stable regular artifact that is not group/world writable or linked."""

    errors: list[str] = []
    validate_input_path(path, label=label, errors=errors)
    if errors:
        return None, errors

    fd = -1
    directory_fds: list[int] = []
    try:
        flags = (
            os.O_RDONLY
            | getattr(os, "O_BINARY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        parent_fd, leaf, directory_fds = _open_anchored_parent(
            path,
            create=False,
        )
        parent_identity = os.fstat(parent_fd)
        before_path = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        fd = os.open(leaf, flags, dir_fd=parent_fd)
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode):
            errors.append(f"{label} must be a regular file")
            return None, errors
        if (before.st_dev, before.st_ino) != (
            before_path.st_dev,
            before_path.st_ino,
        ):
            errors.append(f"{label} changed before it could be pinned")
            return None, errors
        if before.st_nlink != 1:
            errors.append(f"{label} must not be hardlinked")
            return None, errors
        if before.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
            errors.append(f"{label} must not be group- or world-writable")
            return None, errors
        if before.st_size > maximum_bytes:
            errors.append(f"{label} exceeds {maximum_bytes} bytes")
            return None, errors
        chunks: list[bytes] = []
        size = 0
        while True:
            chunk = os.read(fd, min(8192, maximum_bytes + 1 - size))
            if not chunk:
                break
            size += len(chunk)
            if size > maximum_bytes:
                errors.append(f"{label} exceeds {maximum_bytes} bytes")
                return None, errors
            chunks.append(chunk)
        after = os.fstat(fd)
        stable_fields = (
            "st_dev",
            "st_ino",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if any(
            getattr(before, field) != getattr(after, field)
            for field in stable_fields
        ):
            errors.append(f"{label} changed while it was read")
            return None, errors
        if not _anchored_path_identity_matches(
            path,
            expected_parent=parent_identity,
            expected_leaf=after,
        ):
            errors.append(f"{label} path changed while it was read")
            return None, errors
        return b"".join(chunks), errors
    except (OSError, RuntimeError):
        errors.append(f"{label} cannot be read")
        return None, errors
    finally:
        if fd >= 0:
            os.close(fd)
        for directory_fd in reversed(directory_fds):
            os.close(directory_fd)


def write_new_artifact_atomic(
    path: Path,
    payload: bytes,
    *,
    label: str,
) -> list[str]:
    """Atomically publish a new file without following or replacing links."""

    errors: list[str] = []
    validate_output_path(path, label=label, errors=errors)
    if errors:
        return errors
    temporary = f".{path.name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    fd = -1
    linked = False
    parent_fd = -1
    leaf = ""
    directory_fds: list[int] = []
    try:
        parent_fd, leaf, directory_fds = _open_anchored_parent(
            path,
            create=True,
        )
        parent_identity = os.fstat(parent_fd)
        flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_BINARY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        fd = os.open(temporary, flags, 0o600, dir_fd=parent_fd)
        write_all_checker_summary_bytes(fd, payload)
        os.fsync(fd)
        temporary_stat = os.fstat(fd)
        os.close(fd)
        fd = -1
        os.link(
            temporary,
            leaf,
            src_dir_fd=parent_fd,
            dst_dir_fd=parent_fd,
            follow_symlinks=False,
        )
        linked = True
        published_stat = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        if (
            not stat.S_ISREG(published_stat.st_mode)
            or published_stat.st_dev != temporary_stat.st_dev
            or published_stat.st_ino != temporary_stat.st_ino
        ):
            return [f"{label} changed during atomic publication"]
        os.unlink(temporary, dir_fd=parent_fd)
        if not _anchored_path_identity_matches(
            path,
            expected_parent=parent_identity,
            expected_leaf=published_stat,
        ):
            os.unlink(leaf, dir_fd=parent_fd)
            os.fsync(parent_fd)
            linked = False
            return [f"{label} path changed during atomic publication"]
        os.fsync(parent_fd)
        linked = False
        return []
    except (OSError, RuntimeError) as error:
        path_label = path_diagnostic_label(path)
        return [
            f"{label} cannot be written: "
            f"{error_diagnostic_label(error, path_label=path_label)}"
        ]
    finally:
        if fd >= 0:
            os.close(fd)
        try:
            if parent_fd >= 0:
                os.unlink(temporary, dir_fd=parent_fd)
        except FileNotFoundError:
            pass
        except (OSError, RuntimeError):
            pass
        if linked and parent_fd >= 0 and leaf:
            try:
                os.unlink(leaf, dir_fd=parent_fd)
                os.fsync(parent_fd)
            except (OSError, RuntimeError):
                pass
        for directory_fd in reversed(directory_fds):
            os.close(directory_fd)


def validation_options(
    *,
    now_unix: int,
    max_evidence_age_secs: int,
    deployment_id: str,
    environment: str,
    public_key: bytes,
    release_sequence: int,
    previous_envelope_sha256: str,
    topology_qualification: dict[str, str] | None,
    resilience_qualification: dict[str, Any] | None = None,
) -> ValidationOptions:
    """Build exact aggregate-gate options for one reviewed envelope."""

    return ValidationOptions(
        now_unix=now_unix,
        max_summary_artifact_age_secs=max_evidence_age_secs,
        deployment_id=deployment_id,
        environment=environment,
        foundational_signer_public_key=public_key,
        foundational_release_sequence=release_sequence,
        foundational_previous_envelope_sha256=previous_envelope_sha256,
        topology_qualification=topology_qualification,
        resilience_qualification=resilience_qualification,
    )


def validate_unsigned_envelope(
    payload: Any,
    options: ValidationOptions,
) -> list[str]:
    """Validate an unsigned body through the signed checker contract."""

    errors: list[str] = []
    body = validate_foundational_exact_fields(
        payload,
        FOUNDATIONAL_PREREQUISITE_FIELDS,
        "foundational prerequisite signing body",
        errors,
    )
    if body is None:
        return errors
    signature = validate_foundational_exact_fields(
        body.get("signature"),
        UNSIGNED_SIGNATURE_FIELDS,
        "foundational prerequisite unsigned signature",
        errors,
    )
    if signature is None or errors:
        return errors

    candidate = copy.deepcopy(body)
    candidate_signature = dict(signature)
    candidate_signature["signature_hex"] = "00" * RAW_ED25519_SIGNATURE_BYTES
    candidate["signature"] = candidate_signature
    _summary, checker_errors, _context = validate_foundational_prerequisite_summary(
        candidate,
        options,
    )
    if checker_errors.count(ZERO_SIGNATURE_DIAGNOSTIC) != 1:
        errors.append(
            "foundational unsigned body did not reach the expected signature "
            "boundary"
        )
    checker_errors = [
        error for error in checker_errors if error != ZERO_SIGNATURE_DIAGNOSTIC
    ]
    errors.extend(checker_errors)
    return errors


def build_unsigned_envelope(
    args: argparse.Namespace,
    public_key: bytes,
    prerequisites: list[dict[str, Any]],
    lane_summaries: list[dict[str, str]],
    topology_qualification: dict[str, str],
    resilience_qualification: dict[str, Any],
) -> dict[str, Any]:
    """Build the schema-closed body carried by the binary signing payload."""

    return {
        "schema": FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "status": "verified",
        "deployment": {
            "deployment_id": args.deployment_id,
            "environment": args.environment,
        },
        "generated_at_unix": args.generated_at_unix,
        "release_sequence": args.release_sequence,
        "previous_envelope_sha256": args.previous_envelope_sha256,
        "topology_qualification": topology_qualification,
        "resilience_qualification": resilience_qualification,
        "prerequisites": prerequisites,
        "lane_summaries": lane_summaries,
        "signature": {
            "algorithm": "ed25519",
            "public_key_fingerprint_sha256": hashlib.sha256(public_key).hexdigest(),
        },
    }


def validate_prepare_inputs(
    args: argparse.Namespace,
) -> tuple[
    bytes | None,
    list[dict[str, Any]],
    list[dict[str, str]],
    dict[str, str] | None,
    dict[str, Any] | None,
    list[str],
]:
    """Validate all reviewed prepare-phase inputs."""

    errors: list[str] = []
    _validate_clock_inputs(
        now_unix=args.now_unix,
        max_evidence_age_secs=args.max_evidence_age_secs,
        errors=errors,
    )
    _validate_bounded_text(
        args.deployment_id,
        label="--deployment-id",
        maximum_bytes=MAX_DEPLOYMENT_ID_BYTES,
        errors=errors,
    )
    _validate_bounded_text(
        args.environment,
        label="--environment",
        maximum_bytes=MAX_ENVIRONMENT_BYTES,
        errors=errors,
    )
    _bounded_positive_integer(
        args.generated_at_unix,
        label="--generated-at-unix",
        maximum=MAX_TIMESTAMP,
        errors=errors,
    )
    _bounded_positive_integer(
        args.release_sequence,
        label="--release-sequence",
        maximum=MAX_FOUNDATIONAL_RELEASE_SEQUENCE,
        errors=errors,
    )
    predecessor = canonical_lower_hex(args.previous_envelope_sha256, 64)
    if predecessor is None:
        errors.append(
            "--previous-envelope-sha256 must be canonical lowercase SHA-256"
        )
    elif args.release_sequence == 1 and any(bytes.fromhex(predecessor)):
        errors.append("--release-sequence 1 requires the zero predecessor")
    elif args.release_sequence > 1 and not any(bytes.fromhex(predecessor)):
        errors.append("--release-sequence after 1 requires a non-zero predecessor")
    public_key = parse_trusted_public_key(args.trusted_public_key_hex, errors)
    resilience_signer_public_key = parse_foundational_signer_public_key(
        args.resilience_qualification_signer_public_key_hex,
        errors,
        path="--resilience-qualification-signer-public-key-hex",
    )
    topology_qualification, topology_errors = load_topology_qualification_binding(
        args.topology_qualification_summary,
        expected_deployment_id=args.deployment_id,
        expected_environment=args.environment,
    )
    errors.extend(topology_errors)
    resilience_qualification = None
    if topology_qualification is not None and resilience_signer_public_key is not None:
        resilience_qualification, resilience_errors = (
            load_resilience_qualification_binding(
                args.resilience_qualification_summary,
                expected_deployment_id=args.deployment_id,
                expected_environment=args.environment,
                expected_topology_qualification=topology_qualification,
                now_unix=args.now_unix,
                max_age_secs=args.max_evidence_age_secs,
                trusted_public_key=resilience_signer_public_key,
            )
        )
        errors.extend(resilience_errors)
    prerequisites = (
        parse_prerequisite_specs(
            args.prerequisite,
            errors,
            deployment_id=args.deployment_id,
            environment=args.environment,
            generated_at_unix=args.generated_at_unix,
            now_unix=args.now_unix,
            max_evidence_age_secs=args.max_evidence_age_secs,
            topology_qualification=topology_qualification,
        )
        if topology_qualification is not None
        else []
    )
    lane_summaries = (
        parse_lane_summary_specs(
            args.lane_summary,
            errors,
            topology_qualification=topology_qualification,
        )
        if topology_qualification is not None
        else []
    )
    if public_key is not None:
        errors.extend(
            validate_previous_envelope(
                path=args.previous_envelope,
                expected_sha256=args.previous_envelope_sha256,
                current_release_sequence=args.release_sequence,
                current_generated_at_unix=args.generated_at_unix,
                now_unix=args.now_unix,
                deployment_id=args.deployment_id,
                environment=args.environment,
                public_key=public_key,
            )
        )
    validate_output_path(
        args.signing_payload_out,
        label="--signing-payload-out",
        errors=errors,
    )
    return (
        public_key,
        prerequisites,
        lane_summaries,
        topology_qualification,
        resilience_qualification,
        errors,
    )


def prepare(args: argparse.Namespace) -> int:
    """Write the exact external-HSM signing payload."""

    (
        public_key,
        prerequisites,
        lane_summaries,
        topology_qualification,
        resilience_qualification,
        errors,
    ) = validate_prepare_inputs(args)
    if (
        errors
        or public_key is None
        or topology_qualification is None
        or resilience_qualification is None
    ):
        emit_checker_error_lines(errors)
        return 2
    unsigned = build_unsigned_envelope(
        args,
        public_key,
        prerequisites,
        lane_summaries,
        topology_qualification,
        resilience_qualification,
    )
    options = validation_options(
        now_unix=args.now_unix,
        max_evidence_age_secs=args.max_evidence_age_secs,
        deployment_id=args.deployment_id,
        environment=args.environment,
        public_key=public_key,
        release_sequence=args.release_sequence,
        previous_envelope_sha256=args.previous_envelope_sha256,
        topology_qualification=topology_qualification,
        resilience_qualification=resilience_qualification,
    )
    errors = validate_unsigned_envelope(unsigned, options)
    if errors:
        emit_checker_error_lines(errors)
        return 2
    signing_payload = foundational_signing_payload(unsigned)
    if signing_payload != (
        FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN + canonical_json_bytes(unsigned)
    ):
        emit_checker_error_lines(
            ["foundational signing payload contract drift was detected"]
        )
        return 2
    errors = write_new_artifact_atomic(
        args.signing_payload_out,
        signing_payload,
        label="--signing-payload-out",
    )
    if errors:
        emit_checker_error_lines(errors)
        return 2
    emit_checker_notice(
        "Prepared SoraFS foundational signing payload with SHA-256 "
        f"{hashlib.sha256(signing_payload).hexdigest()}."
    )
    return 0


def _strict_json_object(raw: bytes, errors: list[str]) -> dict[str, Any] | None:
    """Decode a canonical JSON object while rejecting duplicate member names."""

    try:
        text = raw.decode("ascii")
    except UnicodeDecodeError:
        errors.append("signing payload JSON must be ASCII")
        return None

    def reject_duplicate_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate object member")
            result[key] = value
        return result

    try:
        value = json.loads(
            text,
            parse_constant=lambda _value: (_ for _ in ()).throw(
                ValueError("non-finite number")
            ),
            object_pairs_hook=reject_duplicate_pairs,
        )
    except (RecursionError, TypeError, ValueError, json.JSONDecodeError):
        errors.append("signing payload JSON must be strict and duplicate-free")
        return None
    if not isinstance(value, dict):
        errors.append("signing payload JSON must be an object")
        return None
    try:
        canonical = canonical_json_bytes(value)
    except (TypeError, ValueError):
        errors.append("signing payload JSON must use canonical JSON values")
        return None
    if raw != canonical:
        errors.append("signing payload JSON must use the exact canonical encoding")
        return None
    return value


def _strict_rendered_envelope_object(
    raw: bytes,
    errors: list[str],
) -> dict[str, Any] | None:
    """Decode the exact deterministic JSON representation of a final envelope."""

    try:
        text = raw.decode("ascii")
    except UnicodeDecodeError:
        errors.append("previous foundational envelope must be ASCII JSON")
        return None

    def reject_duplicate_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate object member")
            result[key] = value
        return result

    try:
        value = json.loads(
            text,
            parse_constant=lambda _value: (_ for _ in ()).throw(
                ValueError("non-finite number")
            ),
            object_pairs_hook=reject_duplicate_pairs,
        )
    except (RecursionError, TypeError, ValueError, json.JSONDecodeError):
        errors.append(
            "previous foundational envelope must be strict and duplicate-free"
        )
        return None
    if not isinstance(value, dict):
        errors.append("previous foundational envelope must be a JSON object")
        return None
    try:
        canonical = render_envelope(value)
    except (TypeError, ValueError):
        errors.append(
            "previous foundational envelope must use deterministic JSON values"
        )
        return None
    if raw != canonical:
        errors.append(
            "previous foundational envelope must use the exact deterministic encoding"
        )
        return None
    return value


def validate_previous_envelope(
    *,
    path: Path | None,
    expected_sha256: str,
    current_release_sequence: int,
    current_generated_at_unix: int,
    now_unix: int,
    deployment_id: str,
    environment: str,
    public_key: bytes,
) -> list[str]:
    """Validate the immediately preceding signed envelope for sequence continuity."""

    errors: list[str] = []
    if current_release_sequence == 1:
        if path is not None:
            errors.append(
                "--previous-envelope must be omitted for release sequence 1"
            )
        return errors
    if current_release_sequence <= 0:
        return errors
    if path is None:
        errors.append(
            "--previous-envelope is required for release sequences after 1"
        )
        return errors

    raw, read_errors = read_bounded_regular_file(
        path,
        label="--previous-envelope",
        maximum_bytes=MAX_FOUNDATIONAL_ARTIFACT_BYTES,
    )
    errors.extend(read_errors)
    if raw is None:
        return errors
    observed_sha256 = hashlib.sha256(raw).hexdigest()
    if not secrets.compare_digest(observed_sha256, expected_sha256):
        errors.append(
            "--previous-envelope SHA-256 does not match the reviewed predecessor"
        )
        return errors

    previous = _strict_rendered_envelope_object(raw, errors)
    if previous is None:
        return errors
    previous_sequence = current_release_sequence - 1
    previous_predecessor = previous.get("previous_envelope_sha256")
    if canonical_lower_hex(previous_predecessor, 64) is None:
        errors.append(
            "previous foundational envelope has an invalid predecessor digest"
        )
        return errors
    previous_generated_at_unix = previous.get("generated_at_unix")
    if (
        not isinstance(previous_generated_at_unix, int)
        or isinstance(previous_generated_at_unix, bool)
        or previous_generated_at_unix <= 0
        or previous_generated_at_unix >= current_generated_at_unix
    ):
        errors.append(
            "previous foundational envelope timestamp must precede the current envelope"
        )
        return errors

    options = validation_options(
        now_unix=now_unix,
        max_evidence_age_secs=MAX_TIMESTAMP,
        deployment_id=deployment_id,
        environment=environment,
        public_key=public_key,
        release_sequence=previous_sequence,
        previous_envelope_sha256=previous_predecessor,
        topology_qualification=None,
    )
    _summary, checker_errors, _context = validate_foundational_prerequisite_summary(
        previous,
        options,
    )
    errors.extend(
        f"previous foundational envelope: {error}" for error in checker_errors
    )
    return errors


def load_unsigned_signing_payload(
    path: Path,
) -> tuple[bytes | None, dict[str, Any] | None, list[str]]:
    """Load and parse one exact domain-separated binary signing request."""

    signing_payload, errors = read_bounded_regular_file(
        path,
        label="--signing-payload",
        maximum_bytes=MAX_FOUNDATIONAL_ARTIFACT_BYTES,
    )
    if signing_payload is None:
        return None, None, errors
    if not signing_payload.startswith(FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN):
        errors.append("signing payload uses the wrong signature domain")
        return None, None, errors
    raw_json = signing_payload[len(FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN) :]
    unsigned = _strict_json_object(raw_json, errors)
    if unsigned is None:
        return None, None, errors
    if foundational_signing_payload(unsigned) != signing_payload:
        errors.append("signing payload does not match the foundational contract")
        return None, None, errors
    return signing_payload, unsigned, errors


def validate_finalize_inputs(
    args: argparse.Namespace,
) -> tuple[
    bytes | None,
    bytes | None,
    dict[str, Any] | None,
    bytes | None,
    dict[str, str] | None,
    dict[str, Any] | None,
    list[str],
]:
    """Validate reviewed finalization inputs and load bounded public artifacts."""

    errors: list[str] = []
    _validate_clock_inputs(
        now_unix=args.now_unix,
        max_evidence_age_secs=args.max_evidence_age_secs,
        errors=errors,
    )
    _validate_bounded_text(
        args.expected_deployment_id,
        label="--expected-deployment-id",
        maximum_bytes=MAX_DEPLOYMENT_ID_BYTES,
        errors=errors,
    )
    _validate_bounded_text(
        args.expected_environment,
        label="--expected-environment",
        maximum_bytes=MAX_ENVIRONMENT_BYTES,
        errors=errors,
    )
    _bounded_positive_integer(
        args.expected_release_sequence,
        label="--expected-release-sequence",
        maximum=MAX_FOUNDATIONAL_RELEASE_SEQUENCE,
        errors=errors,
    )
    predecessor = canonical_lower_hex(
        args.expected_previous_envelope_sha256,
        64,
    )
    if predecessor is None:
        errors.append(
            "--expected-previous-envelope-sha256 must be canonical lowercase "
            "SHA-256"
        )
    elif args.expected_release_sequence == 1 and any(bytes.fromhex(predecessor)):
        errors.append("--expected-release-sequence 1 requires the zero predecessor")
    elif args.expected_release_sequence > 1 and not any(bytes.fromhex(predecessor)):
        errors.append(
            "--expected-release-sequence after 1 requires a non-zero predecessor"
        )
    public_key = parse_trusted_public_key(args.trusted_public_key_hex, errors)
    resilience_signer_public_key = parse_foundational_signer_public_key(
        args.resilience_qualification_signer_public_key_hex,
        errors,
        path="--resilience-qualification-signer-public-key-hex",
    )
    topology_qualification, topology_errors = load_topology_qualification_binding(
        args.topology_qualification_summary,
        expected_deployment_id=args.expected_deployment_id,
        expected_environment=args.expected_environment,
    )
    errors.extend(topology_errors)
    resilience_qualification = None
    if topology_qualification is not None and resilience_signer_public_key is not None:
        resilience_qualification, resilience_errors = (
            load_resilience_qualification_binding(
                args.resilience_qualification_summary,
                expected_deployment_id=args.expected_deployment_id,
                expected_environment=args.expected_environment,
                expected_topology_qualification=topology_qualification,
                now_unix=args.now_unix,
                max_age_secs=args.max_evidence_age_secs,
                trusted_public_key=resilience_signer_public_key,
            )
        )
        errors.extend(resilience_errors)
    validate_output_path(
        args.envelope_out,
        label="--envelope-out",
        errors=errors,
    )
    if errors:
        return (
            None,
            None,
            None,
            public_key,
            topology_qualification,
            resilience_qualification,
            errors,
        )

    signing_payload, unsigned, payload_errors = load_unsigned_signing_payload(
        args.signing_payload
    )
    errors.extend(payload_errors)
    if public_key is not None and unsigned is not None:
        current_generated_at_unix = unsigned.get("generated_at_unix")
        if (
            isinstance(current_generated_at_unix, int)
            and not isinstance(current_generated_at_unix, bool)
            and current_generated_at_unix > 0
        ):
            errors.extend(
                validate_previous_envelope(
                    path=args.previous_envelope,
                    expected_sha256=args.expected_previous_envelope_sha256,
                    current_release_sequence=args.expected_release_sequence,
                    current_generated_at_unix=current_generated_at_unix,
                    now_unix=args.now_unix,
                    deployment_id=args.expected_deployment_id,
                    environment=args.expected_environment,
                    public_key=public_key,
                )
            )
    signature, signature_errors = read_bounded_regular_file(
        args.signature_file,
        label="--signature-file",
        maximum_bytes=RAW_ED25519_SIGNATURE_BYTES,
    )
    errors.extend(signature_errors)
    if signature is not None:
        if len(signature) != RAW_ED25519_SIGNATURE_BYTES:
            errors.append("--signature-file must contain exactly 64 raw bytes")
        elif not any(signature):
            errors.append("--signature-file must not contain an all-zero signature")
    return (
        signing_payload,
        signature,
        unsigned,
        public_key,
        topology_qualification,
        resilience_qualification,
        errors,
    )


def finalize(args: argparse.Namespace) -> int:
    """Verify the detached signature and publish the final signed envelope."""

    (
        signing_payload,
        signature,
        unsigned,
        public_key,
        topology_qualification,
        resilience_qualification,
        errors,
    ) = validate_finalize_inputs(args)
    if (
        errors
        or signing_payload is None
        or signature is None
        or unsigned is None
        or public_key is None
        or topology_qualification is None
        or resilience_qualification is None
    ):
        emit_checker_error_lines(errors)
        return 2

    options = validation_options(
        now_unix=args.now_unix,
        max_evidence_age_secs=args.max_evidence_age_secs,
        deployment_id=args.expected_deployment_id,
        environment=args.expected_environment,
        public_key=public_key,
        release_sequence=args.expected_release_sequence,
        previous_envelope_sha256=args.expected_previous_envelope_sha256,
        topology_qualification=topology_qualification,
        resilience_qualification=resilience_qualification,
    )
    errors = validate_unsigned_envelope(unsigned, options)
    if errors:
        emit_checker_error_lines(errors)
        return 2
    if not verify_ed25519(public_key, signature, signing_payload):
        emit_checker_error_lines(
            ["foundational prerequisite signature verification failed"]
        )
        return 1

    final_envelope = copy.deepcopy(unsigned)
    final_envelope["signature"] = {
        **final_envelope["signature"],
        "signature_hex": signature.hex(),
    }
    if foundational_signing_payload(final_envelope) != signing_payload:
        emit_checker_error_lines(
            ["final envelope does not preserve the reviewed signing payload"]
        )
        return 2
    _summary, checker_errors, _context = validate_foundational_prerequisite_summary(
        final_envelope,
        options,
    )
    if checker_errors:
        emit_checker_error_lines(checker_errors)
        return 1

    envelope_bytes = render_envelope(final_envelope)
    if len(envelope_bytes) > MAX_FOUNDATIONAL_ARTIFACT_BYTES:
        emit_checker_error_lines(
            [
                "final foundational prerequisite envelope exceeds "
                f"{MAX_FOUNDATIONAL_ARTIFACT_BYTES} bytes"
            ]
        )
        return 2
    errors = write_new_artifact_atomic(
        args.envelope_out,
        envelope_bytes,
        label="--envelope-out",
    )
    if errors:
        emit_checker_error_lines(errors)
        return 2
    emit_checker_notice(
        "Finalized SoraFS foundational prerequisite envelope with SHA-256 "
        f"{hashlib.sha256(envelope_bytes).hexdigest()}."
    )
    return 0


def build_parser() -> EvidenceArgumentParser:
    """Build the two-phase external-HSM command line."""

    parser = EvidenceArgumentParser(
        description=(
            "Prepare and finalize the payload-free SoraFS foundational "
            "prerequisite envelope without accepting private keys."
        ),
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    prepare_parser = subparsers.add_parser(
        "prepare",
        help="Write the exact binary payload for an external Ed25519 signer.",
    )
    add_topology_qualification_argument(prepare_parser)
    prepare_parser.add_argument(
        "--resilience-qualification-summary",
        required=True,
        type=Path,
        help=(
            "Exact evidence-qualified resilience/DR summary to bind beside the "
            "existing nine prerequisite anchors."
        ),
    )
    prepare_parser.add_argument(
        "--resilience-qualification-signer-public-key-hex",
        required=True,
        help=(
            "Operator-trusted raw Ed25519 key authenticating the resilience "
            "receipt, in lowercase hex."
        ),
    )
    prepare_parser.add_argument("--deployment-id", required=True)
    prepare_parser.add_argument("--environment", required=True)
    prepare_parser.add_argument(
        "--generated-at-unix",
        required=True,
        type=positive_int_arg,
    )
    prepare_parser.add_argument("--now-unix", required=True, type=positive_int_arg)
    prepare_parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    )
    prepare_parser.add_argument(
        "--release-sequence",
        required=True,
        type=positive_int_arg,
    )
    prepare_parser.add_argument("--previous-envelope-sha256", required=True)
    prepare_parser.add_argument(
        "--previous-envelope",
        type=Path,
        help=(
            "Exact preceding finalized envelope; required after release sequence "
            "1 and forbidden for sequence 1."
        ),
    )
    prepare_parser.add_argument(
        "--trusted-public-key-hex",
        required=True,
        help="Operator-trusted raw 32-byte Ed25519 public key in lowercase hex.",
    )
    prepare_parser.add_argument(
        "--prerequisite",
        action="append",
        default=[],
        metavar="ID=PATH",
        help=(
            "One exact prerequisite evidence-package manifest. Repeat exactly "
            "nine times in canonical SFM-1, SF-1, SF-2, SF-2c, SF-3, SF-4, "
            "SF-5b, SF-6, SF-8a order. Each manifest and its digest-bound "
            "readiness summary are opened and validated before signing."
        ),
    )
    prepare_parser.add_argument(
        "--lane-summary",
        action="append",
        default=[],
        metavar="GATE=PATH",
        help=(
            "Exact ready lane summary approved for this release. Repeat once "
            "for every readiness lane in canonical aggregate order; the stable "
            "file bytes are hashed into the HSM-signed envelope."
        ),
    )
    prepare_parser.add_argument(
        "--signing-payload-out",
        required=True,
        type=Path,
        help="New output path for the exact binary HSM signing payload.",
    )

    finalize_parser = subparsers.add_parser(
        "finalize",
        help="Verify a raw detached signature and write the final envelope.",
    )
    add_topology_qualification_argument(finalize_parser)
    finalize_parser.add_argument(
        "--resilience-qualification-summary",
        required=True,
        type=Path,
        help=(
            "Exact evidence-qualified resilience/DR summary reviewed during "
            "prepare."
        ),
    )
    finalize_parser.add_argument(
        "--resilience-qualification-signer-public-key-hex",
        required=True,
        help=(
            "Operator-trusted raw Ed25519 key authenticating the resilience "
            "receipt, in lowercase hex."
        ),
    )
    finalize_parser.add_argument(
        "--signing-payload",
        required=True,
        type=Path,
        help="Exact binary payload emitted by prepare.",
    )
    finalize_parser.add_argument(
        "--signature-file",
        required=True,
        type=Path,
        help="Regular file containing exactly 64 raw Ed25519 signature bytes.",
    )
    finalize_parser.add_argument(
        "--trusted-public-key-hex",
        required=True,
        help="Operator-trusted raw 32-byte Ed25519 public key in lowercase hex.",
    )
    finalize_parser.add_argument("--expected-deployment-id", required=True)
    finalize_parser.add_argument("--expected-environment", required=True)
    finalize_parser.add_argument(
        "--expected-release-sequence",
        required=True,
        type=positive_int_arg,
    )
    finalize_parser.add_argument(
        "--expected-previous-envelope-sha256",
        required=True,
    )
    finalize_parser.add_argument(
        "--previous-envelope",
        type=Path,
        help=(
            "Exact preceding finalized envelope; required after release sequence "
            "1 and forbidden for sequence 1."
        ),
    )
    finalize_parser.add_argument("--now-unix", required=True, type=positive_int_arg)
    finalize_parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    )
    finalize_parser.add_argument(
        "--envelope-out",
        required=True,
        type=Path,
        help="New output path for the verified signed JSON envelope.",
    )
    return parser


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse direct or reviewed response-file arguments."""

    parser = build_parser()
    raw_args = sys.argv[1:] if argv is None else argv
    expanded = expand_response_args(raw_args, parser)
    return parser.parse_args(expanded)


def main(argv: list[str] | None = None) -> int:
    """Run one foundational prerequisite signing phase."""

    try:
        args = parse_args(argv)
    except (SystemExit, ValueError) as error:
        if isinstance(error, ValueError):
            emit_checker_exception(error)
            return 2
        return error.code if isinstance(error.code, int) else 2
    if args.command == "prepare":
        return prepare(args)
    if args.command == "finalize":
        return finalize(args)
    emit_checker_error_lines(["unknown foundational prerequisite command"])
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
