#!/usr/bin/env python3
"""Validate a DOI-backed AtomicPrivateSettlementV1 release evidence bundle.

This tool intentionally validates evidence that already exists. It never
creates placeholder qualification results and it does not turn local unit-test
coverage into real-network, independent-audit, or publication evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import ntpath
import os
import posixpath
import re
import stat
import struct
import sys
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

MANIFEST_VERSION = 1
PROTOCOL = "AtomicPrivateSettlementV1"
REQUIRED_PARTICIPANTS = (2, 3, 4, 8, 16)
REQUIRED_SEEDS_PER_PARTICIPANT = 10
REQUIRED_LOSS_PERCENTAGES = (5, 10, 20)
REQUIRED_LOSS_PHASES = ("restricted_da", "prepare", "commit")
REQUIRED_PHASE_CUTS = (
    "da_before_availability_qc",
    "prepare_before_complete_barrier",
    "commit_before_complete_barrier",
    "carrier_before_global_finality",
)
REQUIRED_CRASH_BOUNDARIES = (
    "sidecar_fsync",
    "staged_delta_fsync",
    "prepare_qc",
    "commit_qc",
    "kura_append",
    "wsv_application",
    "receipt_publication",
)
FAULT_TRANSCRIPT_ARTIFACT_KINDS = frozenset({"operator_log"})
FAULT_CAPTURE_ARTIFACT_KINDS = frozenset({"sanitized_capture"})
REQUIRED_AUDIT_SCOPES = (
    "air",
    "dummy_slot_selectors",
    "asset_capsule_bindings",
    "sponsor_reimbursement",
    "hybrid_cryptography",
    "auditor_qc_domains",
    "cross_dataspace_state_machine",
)
REQUIRED_ARTIFACT_KINDS = (
    "audit_attestation",
    "audit_report",
    "auditor_key_custody_report",
    "benchmark_raw",
    "benchmark_report",
    "block_wire_capture",
    "canary_manifest",
    "clippy_report",
    "configuration",
    "configuration_manifest",
    "differential_pair_manifest",
    "event_capture",
    "format_report",
    "formal_model_report",
    "hardware_description",
    "kura_artifact",
    "leakage_capture_provenance",
    "leakage_report",
    "limitations",
    "merge_artifact",
    "traffic_count_manifest",
    "operator_log",
    "plot",
    "privacy_release_report",
    "protocol_argument",
    "public_p2p_capture",
    "query_capture",
    "randomized_seed_report",
    "real_network_fault_raw",
    "real_network_fault_report",
    "release_binary",
    "release_inventory_report",
    "reproducible_build_report",
    "restricted_audit_source",
    "restricted_packet_source",
    "restricted_p2p_capture",
    "sanitized_capture",
    "sbom",
    "sdk_test_report",
    "snapshot_artifact",
    "soak_report",
    "source_archive",
    "source_commit",
    "source_lockfile",
    "source_manifest",
    "source_path_list",
    "telemetry_capture",
    "test_report",
    "threat_model",
    "torii_capture",
    "workspace_test_report",
)
REQUIRED_LEAKAGE_CANARY_NAMES = (
    "account_id",
    "amount",
    "asset_alias",
    "asset_id",
    "capsule",
    "memo",
)
REQUIRED_TRAFFIC_COUNT_CHANNELS = (
    "torii_request_packets",
    "torii_response_packets",
    "public_p2p_packets",
    "restricted_p2p_packets",
    "block_messages",
    "query_responses",
    "event_records",
    "log_records",
    "telemetry_records",
)
REQUIRED_LEAKAGE_ARTIFACT_KINDS = (
    "block_wire_capture",
    "event_capture",
    "kura_artifact",
    "leakage_capture_provenance",
    "merge_artifact",
    "traffic_count_manifest",
    "operator_log",
    "public_p2p_capture",
    "query_capture",
    "restricted_audit_source",
    "restricted_p2p_capture",
    "restricted_packet_source",
    "sanitized_capture",
    "snapshot_artifact",
    "telemetry_capture",
    "torii_capture",
)
REQUIRED_DIFFERENTIAL_ARTIFACT_KINDS = (
    "block_wire_capture",
    "event_capture",
    "kura_artifact",
    "merge_artifact",
    "operator_log",
    "public_p2p_capture",
    "query_capture",
    "restricted_audit_source",
    "restricted_p2p_capture",
    "restricted_packet_source",
    "sanitized_capture",
    "snapshot_artifact",
    "telemetry_capture",
    "torii_capture",
)
DIFFERENTIAL_SURFACE_FILES = {
    "block_wire_capture": "block-wire.bin",
    "event_capture": "events.json",
    "kura_artifact": "kura.bin",
    "merge_artifact": "merge.bin",
    "operator_log": "operator.json",
    "public_p2p_capture": "public-p2p.pcapng",
    "query_capture": "queries.json",
    "restricted_audit_source": "restricted-audit-sources.bin",
    "restricted_p2p_capture": "restricted-p2p.pcapng",
    "restricted_packet_source": "raw-loopback.pcap",
    "sanitized_capture": "sanitized-capture.pcapng",
    "snapshot_artifact": "snapshot.bin",
    "telemetry_capture": "telemetry.json",
    "torii_capture": "torii.pcapng",
}
REQUIRED_DIFFERENTIAL_STATE_CHANGES = frozenset(
    {"block_wire_capture", "kura_artifact", "snapshot_artifact"}
)
_HEX_64 = re.compile(r"[0-9a-f]{64}")
_GIT_COMMIT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_DOI = re.compile(r"10\.\d{4,9}/[-._;()/:a-z0-9]+", re.IGNORECASE)
_UTC_TIMESTAMP = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z")
_GIT_INVENTORY_MODES = frozenset({"100644", "100755", "120000", "160000"})
_MAX_SOURCE_INVENTORY_ENTRIES = 1_000_000
_MAX_SOURCE_INVENTORY_PATH_BYTES = 4096
_SOURCE_SEAL_DOMAIN = b"iroha-workspace-source-seal-v1\0"
_SOURCE_PATH_LIST_DOMAIN = b"iroha-workspace-source-path-list-v1\0"
_WORKSPACE_SOURCE_MANIFEST_DOMAIN = b"iroha-workspace-source-manifest-v2\0"
_MAX_SOURCE_ARCHIVE_BYTES = 16 * 1024 * 1024 * 1024
_MAX_SOURCE_SEAL_MEMBER_BYTES = 8 * 1024 * 1024 * 1024
_MAX_SOURCE_SYMLINK_TARGET_BYTES = 1024 * 1024
_MAX_SOURCE_LOCKFILE_BYTES = 64 * 1024 * 1024
_MAX_SOURCE_MANIFEST_BYTES = 16 * 1024 * 1024
_MAX_RELEASE_MANIFEST_BYTES = 64 * 1024 * 1024
_MAX_PASS_REPORT_BYTES = 512 * 1024 * 1024
_MAX_FORMAL_INPUT_BYTES = 64 * 1024 * 1024
_MAX_FORMAL_PACKAGE_BYTES = 256 * 1024 * 1024
_MAX_FORMAL_TRANSCRIPT_BYTES = 512 * 1024 * 1024
_MAX_FORMAL_JAVA_VERSION_OUTPUT_BYTES = 64 * 1024
_EXACT_ONE_SOURCE_ARTIFACT_KINDS = (
    "release_inventory_report",
    "source_archive",
    "source_commit",
    "source_lockfile",
    "source_manifest",
    "source_path_list",
)
PASS_REPORT_GATES = {
    "clippy_report": "strict_clippy",
    "format_report": "format_verification",
    "privacy_release_report": "serial_privacy_release",
    "release_inventory_report": "release_inventory",
    "sdk_test_report": "sdk_matrix",
    "test_report": "focused_tests",
    "workspace_test_report": "workspace_tests",
}
REQUIRED_FORMAL_CONFIGURATION_MODELS = (
    ("AtomicPrivateSettlementV1_3.cfg", "pass", "AtomicPrivateSettlementV1.tla"),
    ("AtomicPrivateSettlementV1_255.cfg", "pass", "AtomicPrivateSettlementV1.tla"),
    ("AtomicPrivateSettlementV1_expiry.cfg", "pass", "AtomicPrivateSettlementV1.tla"),
    (
        "AtomicPrivateSettlementV1CommitteeFaults_2_validator_focused.cfg",
        "pass",
        "AtomicPrivateSettlementV1CommitteeFaults.tla",
    ),
    (
        "AtomicPrivateSettlementV1CommitteeFaults_2.cfg",
        "pass",
        "AtomicPrivateSettlementV1CommitteeFaults.tla",
    ),
    (
        "AtomicPrivateSettlementV1CommitteeFaults_3.cfg",
        "pass",
        "AtomicPrivateSettlementV1CommitteeFaults.tla",
    ),
    (
        "AtomicPrivateSettlementV1CommitteeFaults_4_clean.cfg",
        "pass",
        "AtomicPrivateSettlementV1CommitteeFaults.tla",
    ),
    (
        "AtomicPrivateSettlementV1CommitteeFaults_expiry.cfg",
        "pass",
        "AtomicPrivateSettlementV1CommitteeFaults.tla",
    ),
    (
        "AtomicPrivateSettlementV1_partial_apply_bug.cfg",
        "safety_violation",
        "AtomicPrivateSettlementV1.tla",
    ),
    (
        "AtomicPrivateSettlementV1_commit_before_prepare_bug.cfg",
        "safety_violation",
        "AtomicPrivateSettlementV1.tla",
    ),
    (
        "AtomicPrivateSettlementV1_drop_stage_on_crash_bug.cfg",
        "safety_violation",
        "AtomicPrivateSettlementV1.tla",
    ),
)
REQUIRED_FORMAL_CONFIGURATIONS = tuple(
    (name, outcome)
    for name, outcome, _model in REQUIRED_FORMAL_CONFIGURATION_MODELS
)
_FORMAL_MODEL_FILES = (
    "AtomicPrivateSettlementV1.tla",
    "AtomicPrivateSettlementV1CommitteeFaults.tla",
)
_FORMAL_INPUT_FILES = _FORMAL_MODEL_FILES + tuple(
    name for name, _ in REQUIRED_FORMAL_CONFIGURATIONS
)
_FORMAL_SOURCE_PREFIX = "formal/private_settlement/"
_FORMAL_SOURCE_PATHS = tuple(
    f"{_FORMAL_SOURCE_PREFIX}{name}" for name in _FORMAL_INPUT_FILES
)
_FORMAL_SOURCE_PATH_SET = frozenset(_FORMAL_SOURCE_PATHS)
_FORMAL_EVIDENCE_CODE_SOURCE_PATHS = (
    "scripts/formal/private_settlement_tlc_report.py",
    "scripts/formal/run_atomic_private_settlement_tlc.sh",
    "scripts/formal/sumeragi_v2_tlc_result_contract.sh",
    "scripts/formal/resolve_java.sh",
)
_FORMAL_EVIDENCE_CODE_SOURCE_PATH_SET = frozenset(
    _FORMAL_EVIDENCE_CODE_SOURCE_PATHS
)
_FORMAL_REQUIRED_SOURCE_PATH_SET = (
    _FORMAL_SOURCE_PATH_SET | _FORMAL_EVIDENCE_CODE_SOURCE_PATH_SET
)
_FORMAL_EVIDENCE_CODE_DOMAIN = b"iroha-aps-formal-evidence-code-v1\0"
_PINNED_FORMAL_TOOL_VERSION = "TLC 2.19 / TLA+ tools 1.7.4"
_PINNED_FORMAL_TLC_VERSION = "2.19"
_PINNED_FORMAL_TOOL_SHA256 = (
    "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
)
_BENCHMARK_PROFILES = ("private", "transparent_control")
_BENCHMARK_PRIVATE_STAGES = (
    "proof_generation",
    "restricted_upload_availability",
    "auditor_response",
    "committee_verification",
    "prepare",
    "commit",
    "global_finality",
    "end_to_end",
)
_BENCHMARK_RESOURCE_FIELDS = (
    "throughput_bundles_per_second",
    "cpu_seconds",
    "peak_rss_bytes",
    "network_bytes",
    "proof_bytes",
    "receipt_bytes",
    "storage_growth_bytes",
)


class EvidenceError(ValueError):
    """Raised when release evidence is incomplete, unsafe, or inconsistent."""


@dataclass(frozen=True)
class Artifact:
    """One validated artifact declaration."""

    kind: str
    path: PurePosixPath
    sha256: str
    bytes: int


@dataclass(frozen=True)
class FormalSourceBindings:
    """Source-sealed identities for models and their evidence-producing code."""

    model_sha256: str
    evidence_code_sha256: str


def _exact_fields(value: Any, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceError(f"{label} must be an object")
    actual = set(value)
    if actual != expected:
        raise EvidenceError(
            f"{label} fields mismatch; missing={sorted(expected - actual)} "
            f"unknown={sorted(actual - expected)}"
        )
    return value


def _nonempty_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise EvidenceError(f"{label} must be a non-empty string")
    return value


def _git_object_digest(payload: bytes, kind: bytes, oid_hex_chars: int) -> str:
    framed = kind + b" " + str(len(payload)).encode("ascii") + b"\0" + payload
    if oid_hex_chars == 40:
        return hashlib.sha1(framed).hexdigest()
    if oid_hex_chars == 64:
        return hashlib.sha256(framed).hexdigest()
    raise EvidenceError("source inventory uses an unsupported Git object format")


def _validated_git_inventory(
    entries: Any,
    *,
    label: str,
    oid_hex_chars: int,
) -> dict[str, tuple[str, str]]:
    if (
        not isinstance(entries, list)
        or not entries
        or len(entries) > _MAX_SOURCE_INVENTORY_ENTRIES
    ):
        raise EvidenceError(f"{label} must be a non-empty bounded list")
    inventory: dict[str, tuple[str, str]] = {}
    canonical_paths: list[bytes] = []
    for index, candidate in enumerate(entries):
        entry = _exact_fields(
            candidate,
            {"path", "mode", "object_type", "object_id"},
            f"{label}[{index}]",
        )
        path = entry["path"]
        mode = entry["mode"]
        object_type = entry["object_type"]
        object_id = entry["object_id"]
        if not isinstance(path, str):
            raise EvidenceError(f"{label}[{index}].path must be a string")
        try:
            encoded_path = path.encode("utf-8")
        except UnicodeEncodeError as error:
            raise EvidenceError(
                f"{label}[{index}].path must be canonical UTF-8"
            ) from error
        components = path.split("/")
        if (
            not encoded_path
            or len(encoded_path) > _MAX_SOURCE_INVENTORY_PATH_BYTES
            or path.startswith("/")
            or "\0" in path
            or any(component in ("", ".", "..") for component in components)
            or components[0] == ".git"
        ):
            raise EvidenceError(f"{label}[{index}].path is unsafe")
        if mode not in _GIT_INVENTORY_MODES:
            raise EvidenceError(f"{label}[{index}].mode is not a tracked Git mode")
        expected_object_type = "commit" if mode == "160000" else "blob"
        if object_type != expected_object_type:
            raise EvidenceError(
                f"{label}[{index}].object_type must be {expected_object_type!r}"
            )
        if (
            not isinstance(object_id, str)
            or len(object_id) != oid_hex_chars
            or re.fullmatch(r"[0-9a-f]+", object_id) is None
        ):
            raise EvidenceError(f"{label}[{index}].object_id is not a Git object ID")
        if path in inventory:
            raise EvidenceError(f"{label} contains duplicate path {path!r}")
        inventory[path] = (mode, object_id)
        canonical_paths.append(encoded_path)
    if canonical_paths != sorted(canonical_paths):
        raise EvidenceError(f"{label} paths must be raw-byte sorted")
    return inventory


def _git_inventory_tree_oid_v1(
    inventory: Mapping[str, tuple[str, str]], oid_hex_chars: int
) -> FormalSourceBindings:
    root: dict[bytes, Any] = {}
    for path, leaf in inventory.items():
        components = [component.encode("utf-8") for component in path.split("/")]
        node = root
        for component in components[:-1]:
            existing = node.get(component)
            if existing is None:
                child: dict[bytes, Any] = {}
                node[component] = child
                node = child
            elif isinstance(existing, dict):
                node = existing
            else:
                raise EvidenceError(
                    f"source inventory path conflicts with file {path!r}"
                )
        basename = components[-1]
        if basename in node:
            raise EvidenceError(f"source inventory path conflicts at {path!r}")
        node[basename] = leaf

    tree_digests: dict[int, str] = {}
    pending: list[tuple[dict[bytes, Any], bool]] = [(root, False)]
    while pending:
        node, children_visited = pending.pop()
        ordered = sorted(
            node.items(),
            key=lambda item: item[0] + (b"/" if isinstance(item[1], dict) else b""),
        )
        if not children_visited:
            pending.append((node, True))
            pending.extend(
                (value, False)
                for _, value in reversed(ordered)
                if isinstance(value, dict)
            )
            continue

        body = bytearray()
        for name, value in ordered:
            if isinstance(value, dict):
                mode = "40000"
                object_id = tree_digests[id(value)]
            else:
                mode, object_id = value
            body.extend(mode.encode("ascii"))
            body.extend(b" ")
            body.extend(name)
            body.extend(b"\0")
            body.extend(bytes.fromhex(object_id))
        tree_digests[id(node)] = _git_object_digest(
            bytes(body), b"tree", oid_hex_chars
        )

    return tree_digests[id(root)]


def _stable_metadata(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _read_stable_bounded_artifact(
    path: Path,
    *,
    maximum_bytes: int,
    expected_sha256: str,
    expected_bytes: int,
    label: str,
) -> bytes:
    try:
        before = path.lstat()
        if (
            not stat.S_ISREG(before.st_mode)
            or stat.S_ISLNK(before.st_mode)
            or before.st_nlink != 1
            or before.st_size != expected_bytes
            or before.st_size > maximum_bytes
        ):
            raise EvidenceError(f"{label} must be one bounded regular artifact")
        with path.open("rb") as stream:
            opened = os.fstat(stream.fileno())
            if _stable_metadata(opened) != _stable_metadata(before):
                raise EvidenceError(f"{label} changed before it was opened")
            payload = stream.read(maximum_bytes + 1)
            after = os.fstat(stream.fileno())
        path_after = path.lstat()
    except OSError as error:
        raise EvidenceError(f"cannot read {label}: {error}") from error
    if (
        not payload
        or len(payload) != expected_bytes
        or len(payload) > maximum_bytes
        or _stable_metadata(after) != _stable_metadata(opened)
        or _stable_metadata(path_after) != _stable_metadata(before)
        or hashlib.sha256(payload).hexdigest() != expected_sha256
    ):
        raise EvidenceError(f"{label} changed or differs from its artifact binding")
    return payload


def _decode_strict_json(payload: bytes, *, label: str) -> Any:
    """Decode UTF-8 JSON while rejecting duplicate keys and non-finite numbers."""

    def exact_object(pairs: Sequence[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise EvidenceError(f"{label} contains duplicate JSON key {key!r}")
            result[key] = value
        return result

    def reject_nonfinite(raw: str) -> Any:
        raise EvidenceError(f"{label} contains non-finite JSON number {raw!r}")

    try:
        return json.loads(
            payload.decode("utf-8"),
            object_pairs_hook=exact_object,
            parse_constant=reject_nonfinite,
        )
    except EvidenceError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise EvidenceError(
            f"cannot read {label} as UTF-8 JSON: {error}"
        ) from error


def _read_strict_json_file(
    path: Path, *, maximum_bytes: int, label: str
) -> Any:
    """Read one stable, bounded, regular JSON file without following links."""

    try:
        before = path.lstat()
        if (
            not stat.S_ISREG(before.st_mode)
            or stat.S_ISLNK(before.st_mode)
            or before.st_nlink != 1
            or before.st_size <= 0
            or before.st_size > maximum_bytes
        ):
            raise EvidenceError(f"{label} must be one bounded regular file")
        with path.open("rb") as stream:
            opened = os.fstat(stream.fileno())
            if _stable_metadata(opened) != _stable_metadata(before):
                raise EvidenceError(f"{label} changed before it was opened")
            payload = stream.read(maximum_bytes + 1)
            after = os.fstat(stream.fileno())
        path_after = path.lstat()
    except OSError as error:
        raise EvidenceError(f"cannot read {label}: {error}") from error
    if (
        len(payload) != before.st_size
        or len(payload) > maximum_bytes
        or _stable_metadata(after) != _stable_metadata(opened)
        or _stable_metadata(path_after) != _stable_metadata(before)
    ):
        raise EvidenceError(f"{label} changed while it was read")
    return _decode_strict_json(payload, label=label)


def _read_bound_json_artifact(
    path: Path,
    *,
    maximum_bytes: int,
    expected_sha256: str,
    expected_bytes: int,
    label: str,
) -> Any:
    payload = _read_stable_bounded_artifact(
        path,
        maximum_bytes=maximum_bytes,
        expected_sha256=expected_sha256,
        expected_bytes=expected_bytes,
        label=label,
    )
    return _decode_strict_json(payload, label=label)


def _validate_source_commit(
    path: Path,
    commit: str,
    *,
    expected_sha256: str,
    expected_bytes: int,
) -> str:
    payload = _read_stable_bounded_artifact(
        path,
        maximum_bytes=16 * 1024 * 1024,
        expected_sha256=expected_sha256,
        expected_bytes=expected_bytes,
        label="source_commit",
    )
    if _git_object_digest(payload, b"commit", len(commit)) != commit:
        raise EvidenceError("source_commit does not hash to the release commit")
    header, separator, _ = payload.partition(b"\n\n")
    if not separator:
        raise EvidenceError("source_commit is missing its Git header boundary")
    first_line = header.splitlines()[0] if header else b""
    prefix = b"tree "
    if not first_line.startswith(prefix):
        raise EvidenceError("source_commit is missing its leading tree header")
    try:
        tree = first_line[len(prefix) :].decode("ascii")
    except UnicodeDecodeError as error:
        raise EvidenceError("source_commit tree is not ASCII") from error
    if len(tree) != len(commit) or re.fullmatch(r"[0-9a-f]+", tree) is None:
        raise EvidenceError("source_commit tree is not a Git object ID")
    return tree


def _source_seal_take(stream: Any, size: int, label: str) -> bytes:
    payload = stream.read(size)
    if len(payload) != size:
        raise EvidenceError(f"source archive is truncated while reading {label}")
    return payload


def _source_seal_u64(stream: Any, label: str) -> int:
    return int.from_bytes(_source_seal_take(stream, 8, label), "big")


class _DigestingSourceSealReader:
    def __init__(self, stream: Any) -> None:
        self._stream = stream
        self.digest = hashlib.sha256()
        self.bytes_read = 0

    def read(self, size: int = -1) -> bytes:
        payload = self._stream.read(size)
        self.digest.update(payload)
        self.bytes_read += len(payload)
        return payload


def _manifest_frame(digest: Any, payload: bytes) -> None:
    digest.update(struct.pack(">Q", len(payload)))
    digest.update(payload)


def _formal_package_sha256_from_source_payloads(
    payloads: Mapping[str, bytes],
) -> str:
    """Hash the exact release-model package using the TLC report framing."""

    if set(payloads) != _FORMAL_SOURCE_PATH_SET:
        missing = sorted(_FORMAL_SOURCE_PATH_SET - set(payloads))
        unexpected = sorted(set(payloads) - _FORMAL_SOURCE_PATH_SET)
        raise EvidenceError(
            "formal source package is incomplete; "
            f"missing={missing!r} unexpected={unexpected!r}"
        )
    total_bytes = sum(len(payload) for payload in payloads.values())
    if total_bytes > _MAX_FORMAL_PACKAGE_BYTES:
        raise EvidenceError("formal source package exceeds its aggregate bound")
    digest = hashlib.sha256()
    for name, source_path in zip(_FORMAL_INPUT_FILES, _FORMAL_SOURCE_PATHS, strict=True):
        payload = payloads[source_path]
        encoded_name = name.encode("utf-8")
        _manifest_frame(digest, encoded_name)
        _manifest_frame(digest, payload)
    return digest.hexdigest()


def _formal_evidence_code_sha256_from_source_payloads(
    payloads: Mapping[str, bytes],
) -> str:
    """Hash the exact source-sealed producer, runner, and helper scripts."""

    if set(payloads) != _FORMAL_EVIDENCE_CODE_SOURCE_PATH_SET:
        missing = sorted(_FORMAL_EVIDENCE_CODE_SOURCE_PATH_SET - set(payloads))
        unexpected = sorted(set(payloads) - _FORMAL_EVIDENCE_CODE_SOURCE_PATH_SET)
        raise EvidenceError(
            "formal evidence code package is incomplete; "
            f"missing={missing!r} unexpected={unexpected!r}"
        )
    digest = hashlib.sha256(_FORMAL_EVIDENCE_CODE_DOMAIN)
    for source_path in _FORMAL_EVIDENCE_CODE_SOURCE_PATHS:
        _manifest_frame(digest, source_path.encode("utf-8"))
        _manifest_frame(digest, payloads[source_path])
    return digest.hexdigest()


def _validate_source_symlink_target(member: bytes, target: bytes) -> None:
    if (
        not target
        or len(target) > _MAX_SOURCE_SYMLINK_TARGET_BYTES
        or b"\0" in target
        or target.startswith(b"/")
        or b"\\" in target
        or bool(ntpath.splitdrive(target)[0])
    ):
        raise EvidenceError("source archive contains an unsafe symlink target")
    parent = member.rpartition(b"/")[0]
    resolved = posixpath.normpath(posixpath.join(parent, target))
    if (
        resolved == b".."
        or resolved.startswith(b"../")
        or resolved == b".git"
        or resolved.startswith(b".git/")
        or resolved.startswith(b"/")
    ):
        raise EvidenceError("source archive symlink escapes the source root")


def _validate_source_symlink_graph(symlinks: Mapping[bytes, bytes]) -> None:
    """Reject source-seal link chains that escape the logical root."""

    for member, target in symlinks.items():
        _validate_source_symlink_target(member, target)
        current = member.split(b"/")[:-1]
        pending = list(reversed(target.split(b"/")))
        followed: set[bytes] = set()
        while pending:
            component = pending.pop()
            if component in (b"", b"."):
                continue
            if component == b"..":
                if not current:
                    raise EvidenceError(
                        "source archive contains a chained symlink escape"
                    )
                current.pop()
                continue
            current.append(component)
            candidate = b"/".join(current)
            if candidate == b".git" or candidate.startswith(b".git/"):
                raise EvidenceError("source archive symlink resolves into .git")
            replacement = symlinks.get(candidate)
            if replacement is None:
                continue
            if candidate in followed:
                raise EvidenceError("source archive contains a cyclic symlink chain")
            followed.add(candidate)
            current = candidate.split(b"/")[:-1]
            pending.extend(reversed(replacement.split(b"/")))


def _validate_source_path_list(
    path: Path,
    inventory: Mapping[str, tuple[str, str]],
    *,
    expected_sha256: str,
    expected_bytes: int,
) -> None:
    payload = _read_stable_bounded_artifact(
        path,
        maximum_bytes=64 * 1024 * 1024,
        expected_sha256=expected_sha256,
        expected_bytes=expected_bytes,
        label="source_path_list",
    )
    if not payload.startswith(_SOURCE_PATH_LIST_DOMAIN):
        raise EvidenceError("source_path_list has the wrong domain")
    offset = len(_SOURCE_PATH_LIST_DOMAIN)

    def take_u64(label: str) -> int:
        nonlocal offset
        end = offset + 8
        if end > len(payload):
            raise EvidenceError(f"source_path_list is truncated at {label}")
        value = int.from_bytes(payload[offset:end], "big")
        offset = end
        return value

    count = take_u64("count")
    if count == 0 or count > _MAX_SOURCE_INVENTORY_ENTRIES:
        raise EvidenceError("source_path_list count exceeds its bound")
    paths: list[str] = []
    encoded_paths: list[bytes] = []
    for index in range(count):
        size = take_u64(f"path {index} size")
        if size == 0 or size > _MAX_SOURCE_INVENTORY_PATH_BYTES:
            raise EvidenceError("source_path_list path exceeds its bound")
        end = offset + size
        if end > len(payload):
            raise EvidenceError("source_path_list is truncated in a path")
        encoded = payload[offset:end]
        offset = end
        try:
            decoded = encoded.decode("utf-8")
        except UnicodeDecodeError as error:
            raise EvidenceError("source_path_list path is not canonical UTF-8") from error
        encoded_paths.append(encoded)
        paths.append(decoded)
    if offset != len(payload) or encoded_paths != sorted(set(encoded_paths)):
        raise EvidenceError("source_path_list is not one exact sorted path list")
    missing = sorted(set(inventory) - set(paths))
    unexpected = sorted(set(paths) - set(inventory))
    if missing or unexpected:
        raise EvidenceError(
            "source_path_list differs from the release inventory; "
            f"missing={missing!r} unexpected={unexpected!r}"
        )


def _validate_source_seal_inventory(
    path: Path,
    inventory: Mapping[str, tuple[str, str]],
    oid_hex_chars: int,
    *,
    expected_sha256: str,
    expected_bytes: int,
    expected_workspace_manifest_sha256: str,
    expected_lockfile_sha256: str,
    expected_lockfile_bytes: int,
    require_formal_package: bool = False,
) -> FormalSourceBindings | None:
    actual: dict[str, tuple[str, str]] = {}
    formal_payloads: dict[str, bytes] = {}
    evidence_code_payloads: dict[str, bytes] = {}
    formal_payload_bytes = 0
    prior_path: bytes | None = None
    workspace_manifest = hashlib.sha256(_WORKSPACE_SOURCE_MANIFEST_DOMAIN)
    lockfile_binding: tuple[str, int] | None = None
    symlinks: dict[bytes, bytes] = {}
    try:
        before = path.lstat()
        if (
            not stat.S_ISREG(before.st_mode)
            or stat.S_ISLNK(before.st_mode)
            or before.st_nlink != 1
            or before.st_size != expected_bytes
            or before.st_size > _MAX_SOURCE_ARCHIVE_BYTES
        ):
            raise EvidenceError("source_archive must be one exact regular artifact")
        raw_stream = path.open("rb")
    except OSError as error:
        raise EvidenceError(f"cannot read source_archive: {error}") from error
    with raw_stream:
        opened = os.fstat(raw_stream.fileno())
        if _stable_metadata(opened) != _stable_metadata(before):
            raise EvidenceError("source_archive changed before it was opened")
        stream = _DigestingSourceSealReader(raw_stream)
        if _source_seal_take(
            stream, len(_SOURCE_SEAL_DOMAIN), "domain"
        ) != _SOURCE_SEAL_DOMAIN:
            raise EvidenceError("source_archive is not a workspace source seal")
        count = _source_seal_u64(stream, "member count")
        if count == 0 or count > _MAX_SOURCE_INVENTORY_ENTRIES:
            raise EvidenceError("source archive member count exceeds its bound")
        for index in range(count):
            path_size = _source_seal_u64(stream, f"member {index} path size")
            if path_size == 0 or path_size > _MAX_SOURCE_INVENTORY_PATH_BYTES:
                raise EvidenceError("source archive member path exceeds its bound")
            encoded_path = _source_seal_take(
                stream, path_size, f"member {index} path"
            )
            try:
                actual_path = encoded_path.decode("utf-8")
            except UnicodeDecodeError as error:
                raise EvidenceError("source archive path is not canonical UTF-8") from error
            if prior_path is not None and encoded_path <= prior_path:
                raise EvidenceError("source archive paths are duplicated or out of order")
            prior_path = encoded_path
            components = actual_path.split("/")
            if (
                actual_path.startswith("/")
                or "\0" in actual_path
                or any(component in ("", ".", "..") for component in components)
                or components[0] == ".git"
            ):
                raise EvidenceError("source archive contains an unsafe path")
            _manifest_frame(workspace_manifest, encoded_path)
            kind = _source_seal_take(stream, 1, f"member {index} kind")
            mode = int.from_bytes(
                _source_seal_take(stream, 4, f"member {index} mode"), "big"
            )
            payload_size = _source_seal_u64(stream, f"member {index} payload size")
            if mode & ~0o7777 or payload_size > _MAX_SOURCE_SEAL_MEMBER_BYTES:
                raise EvidenceError("source archive member metadata exceeds its bound")
            if kind not in (b"F", b"G", b"L"):
                raise EvidenceError(
                    f"source archive contains unsupported member {actual_path!r}"
                )
            workspace_manifest.update(struct.pack(">I", mode))
            workspace_manifest.update(kind)
            if kind == b"F":
                if (
                    require_formal_package
                    and actual_path in _FORMAL_REQUIRED_SOURCE_PATH_SET
                    and payload_size > _MAX_FORMAL_INPUT_BYTES
                ):
                    raise EvidenceError(
                        f"formal input {actual_path!r} exceeds its per-file bound"
                    )
                if (
                    require_formal_package
                    and actual_path in _FORMAL_REQUIRED_SOURCE_PATH_SET
                    and formal_payload_bytes + payload_size
                    > _MAX_FORMAL_PACKAGE_BYTES
                ):
                    raise EvidenceError("formal source package exceeds its aggregate bound")
                git_mode = "100755" if mode & 0o111 else "100644"
                algorithm = hashlib.sha1 if oid_hex_chars == 40 else hashlib.sha256
                digest = algorithm()
                formal_payload = (
                    bytearray()
                    if require_formal_package
                    and actual_path in _FORMAL_REQUIRED_SOURCE_PATH_SET
                    else None
                )
                lockfile_digest = (
                    hashlib.sha256() if actual_path == "Cargo.lock" else None
                )
                digest.update(b"blob " + str(payload_size).encode("ascii") + b"\0")
                workspace_manifest.update(struct.pack(">Q", payload_size))
                remaining = payload_size
                while remaining:
                    chunk = _source_seal_take(
                        stream,
                        min(1024 * 1024, remaining),
                        f"member {index} contents",
                    )
                    digest.update(chunk)
                    workspace_manifest.update(chunk)
                    if formal_payload is not None:
                        formal_payload.extend(chunk)
                    if lockfile_digest is not None:
                        lockfile_digest.update(chunk)
                    remaining -= len(chunk)
                actual_object = digest.hexdigest()
                actual[actual_path] = (git_mode, actual_object)
                if formal_payload is not None:
                    captured_payload = bytes(formal_payload)
                    if actual_path in _FORMAL_SOURCE_PATH_SET:
                        formal_payloads[actual_path] = captured_payload
                    else:
                        evidence_code_payloads[actual_path] = captured_payload
                    formal_payload_bytes += payload_size
                if lockfile_digest is not None:
                    lockfile_binding = (lockfile_digest.hexdigest(), payload_size)
            elif kind == b"L":
                if (
                    require_formal_package
                    and actual_path in _FORMAL_REQUIRED_SOURCE_PATH_SET
                ):
                    raise EvidenceError(
                        f"formal input {actual_path!r} must be a regular file"
                    )
                if payload_size > _MAX_SOURCE_SYMLINK_TARGET_BYTES:
                    raise EvidenceError("source archive symlink target exceeds its bound")
                target = _source_seal_take(
                    stream, payload_size, f"member {index} symlink target"
                )
                _validate_source_symlink_target(encoded_path, target)
                symlinks[encoded_path] = target
                _manifest_frame(workspace_manifest, target)
                actual[actual_path] = (
                    "120000",
                    _git_object_digest(target, b"blob", oid_hex_chars),
                )
            elif kind == b"G":
                if (
                    require_formal_package
                    and actual_path in _FORMAL_REQUIRED_SOURCE_PATH_SET
                ):
                    raise EvidenceError(
                        f"formal input {actual_path!r} must be a regular file"
                    )
                if payload_size != 0:
                    raise EvidenceError("source archive gitlink has a payload")
                expected_gitlink = inventory.get(actual_path)
                actual[actual_path] = (
                    "160000",
                    expected_gitlink[1]
                    if expected_gitlink is not None and expected_gitlink[0] == "160000"
                    else "0" * oid_hex_chars,
                )
        _validate_source_symlink_graph(symlinks)
        if stream.read(1) != b"":
            raise EvidenceError("source archive has trailing bytes")
        after = os.fstat(raw_stream.fileno())
    try:
        path_after = path.lstat()
    except OSError as error:
        raise EvidenceError(f"cannot restat source_archive: {error}") from error
    if (
        stream.bytes_read != expected_bytes
        or stream.digest.hexdigest() != expected_sha256
        or _stable_metadata(after) != _stable_metadata(opened)
        or _stable_metadata(path_after) != _stable_metadata(before)
    ):
        raise EvidenceError("source_archive changed or differs from its artifact binding")
    missing = sorted(set(inventory) - set(actual))
    unexpected = sorted(set(actual) - set(inventory))
    incorrect = sorted(
        source_path
        for source_path in set(inventory) & set(actual)
        if inventory[source_path] != actual[source_path]
    )
    if missing or unexpected or incorrect:
        raise EvidenceError(
            "release inventory differs from the source archive; "
            f"missing={missing!r} unexpected={unexpected!r} incorrect={incorrect!r}"
        )
    if workspace_manifest.hexdigest() != expected_workspace_manifest_sha256:
        raise EvidenceError("source archive workspace manifest differs from source manifest")
    if lockfile_binding != (expected_lockfile_sha256, expected_lockfile_bytes):
        raise EvidenceError("source archive Cargo.lock differs from source_lockfile")
    if require_formal_package:
        return FormalSourceBindings(
            model_sha256=_formal_package_sha256_from_source_payloads(formal_payloads),
            evidence_code_sha256=(
                _formal_evidence_code_sha256_from_source_payloads(
                    evidence_code_payloads
                )
            ),
        )
    return None


def _validate_release_inventory_details(
    details: Any,
    *,
    source_tree: str,
    source_tracked_file_count: int,
    source_archive_path: Path,
    source_archive_sha256: str,
    source_archive_bytes: int,
    workspace_manifest_sha256: str,
    source_lockfile_sha256: str,
    source_lockfile_bytes: int,
    source_path_list_path: Path,
    source_path_list_sha256: str,
    source_path_list_bytes: int,
) -> str:
    inventory = _exact_fields(
        details,
        {"tree", "entries"},
        "release_inventory_report.details",
    )
    tree = inventory["tree"]
    if tree != source_tree:
        raise EvidenceError("release inventory tree differs from source manifest")
    oid_hex_chars = len(source_tree)
    expected = _validated_git_inventory(
        inventory["entries"],
        label="release_inventory_report.details.entries",
        oid_hex_chars=oid_hex_chars,
    )
    if len(expected) != source_tracked_file_count:
        raise EvidenceError(
            "release inventory count differs from the clean source manifest"
        )
    reconstructed_tree = _git_inventory_tree_oid_v1(expected, oid_hex_chars)
    if reconstructed_tree != source_tree:
        raise EvidenceError(
            "release inventory entries do not reconstruct the clean source tree"
        )
    _validate_source_path_list(
        source_path_list_path,
        expected,
        expected_sha256=source_path_list_sha256,
        expected_bytes=source_path_list_bytes,
    )
    formal_source_bindings = _validate_source_seal_inventory(
        source_archive_path,
        expected,
        oid_hex_chars,
        expected_sha256=source_archive_sha256,
        expected_bytes=source_archive_bytes,
        expected_workspace_manifest_sha256=workspace_manifest_sha256,
        expected_lockfile_sha256=source_lockfile_sha256,
        expected_lockfile_bytes=source_lockfile_bytes,
        require_formal_package=True,
    )
    if formal_source_bindings is None:
        raise EvidenceError("source archive did not yield a formal source package")
    return formal_source_bindings


def _exact_integer(value: Any, expected: int, label: str) -> None:
    if isinstance(value, bool) or not isinstance(value, int) or value != expected:
        raise EvidenceError(f"{label} must be exactly {expected}")


def _exact_list(value: Any, expected: Sequence[Any], label: str) -> None:
    if not isinstance(value, list) or value != list(expected):
        raise EvidenceError(f"{label} must be exactly {list(expected)}")


def _parse_artifact(value: Any, index: int) -> Artifact:
    label = f"artifacts[{index}]"
    record = _exact_fields(value, {"kind", "path", "sha256", "bytes"}, label)
    kind = _nonempty_string(record["kind"], f"{label}.kind")
    if kind not in REQUIRED_ARTIFACT_KINDS:
        raise EvidenceError(f"{label}.kind is not a recognized release artifact kind")
    raw_path = _nonempty_string(record["path"], f"{label}.path")
    path = PurePosixPath(raw_path)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in ("", ".", "..") for part in path.parts)
    ):
        raise EvidenceError(f"{label}.path must be a normalized relative POSIX path")
    digest = record["sha256"]
    if not isinstance(digest, str) or _HEX_64.fullmatch(digest) is None:
        raise EvidenceError(
            f"{label}.sha256 must be 64 lowercase hexadecimal characters"
        )
    byte_count = record["bytes"]
    if (
        isinstance(byte_count, bool)
        or not isinstance(byte_count, int)
        or byte_count < 0
    ):
        raise EvidenceError(f"{label}.bytes must be a non-negative integer")
    return Artifact(kind=kind, path=path, sha256=digest, bytes=byte_count)


def parse_manifest(document: Any) -> tuple[dict[str, Any], list[Artifact]]:
    """Parse the strict V1 release manifest and enforce policy-level gates."""

    manifest = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "worktree_clean",
            "doi",
            "qualification",
            "independent_audit",
            "artifacts",
        },
        "manifest",
    )
    _exact_integer(manifest["version"], MANIFEST_VERSION, "manifest.version")
    if manifest["protocol"] != PROTOCOL:
        raise EvidenceError(f"manifest.protocol must be {PROTOCOL!r}")
    commit = manifest["commit"]
    if not isinstance(commit, str) or _GIT_COMMIT.fullmatch(commit) is None:
        raise EvidenceError("manifest.commit must be a full lowercase Git object id")
    if manifest["worktree_clean"] is not True:
        raise EvidenceError("manifest.worktree_clean must be true")
    doi = _nonempty_string(manifest["doi"], "manifest.doi")
    normalized_doi = doi.removeprefix("https://doi.org/").removeprefix("doi:")
    if _DOI.fullmatch(normalized_doi) is None:
        raise EvidenceError("manifest.doi must contain a canonical DOI")

    qualification = _exact_fields(
        manifest["qualification"],
        {
            "real_network_participants",
            "validators_per_dataspace",
            "quorum",
            "mandatory_signed_rs16_da_rbc",
            "max_unavailable_per_committee",
            "loss_percentages",
            "crash_boundaries",
            "randomized_seeds",
            "soak_seconds",
            "minimum_warmups",
            "minimum_measured_bundles",
        },
        "manifest.qualification",
    )
    _exact_list(
        qualification["real_network_participants"],
        REQUIRED_PARTICIPANTS,
        "manifest.qualification.real_network_participants",
    )
    _exact_integer(
        qualification["validators_per_dataspace"],
        4,
        "manifest.qualification.validators_per_dataspace",
    )
    if qualification["quorum"] != "3-of-4":
        raise EvidenceError("manifest.qualification.quorum must be '3-of-4'")
    if qualification["mandatory_signed_rs16_da_rbc"] is not True:
        raise EvidenceError(
            "manifest.qualification.mandatory_signed_rs16_da_rbc must be true"
        )
    _exact_integer(
        qualification["max_unavailable_per_committee"],
        1,
        "manifest.qualification.max_unavailable_per_committee",
    )
    _exact_list(
        qualification["loss_percentages"],
        REQUIRED_LOSS_PERCENTAGES,
        "manifest.qualification.loss_percentages",
    )
    _exact_list(
        qualification["crash_boundaries"],
        REQUIRED_CRASH_BOUNDARIES,
        "manifest.qualification.crash_boundaries",
    )
    seeds = qualification["randomized_seeds"]
    if (
        isinstance(seeds, bool)
        or not isinstance(seeds, int)
        or seeds < REQUIRED_SEEDS_PER_PARTICIPANT
    ):
        raise EvidenceError(
            "manifest.qualification.randomized_seeds must be at least 10"
        )
    soak = qualification["soak_seconds"]
    if isinstance(soak, bool) or not isinstance(soak, int) or soak < 7200:
        raise EvidenceError("manifest.qualification.soak_seconds must be at least 7200")
    warmups = qualification["minimum_warmups"]
    if isinstance(warmups, bool) or not isinstance(warmups, int) or warmups < 5:
        raise EvidenceError("manifest.qualification.minimum_warmups must be at least 5")
    measured = qualification["minimum_measured_bundles"]
    if isinstance(measured, bool) or not isinstance(measured, int) or measured < 30:
        raise EvidenceError(
            "manifest.qualification.minimum_measured_bundles must be at least 30"
        )

    audit = _exact_fields(
        manifest["independent_audit"],
        {"independent", "organization", "conclusion", "scopes", "report_path"},
        "manifest.independent_audit",
    )
    if audit["independent"] is not True:
        raise EvidenceError("manifest.independent_audit.independent must be true")
    _nonempty_string(audit["organization"], "manifest.independent_audit.organization")
    if audit["conclusion"] != "passed":
        raise EvidenceError("manifest.independent_audit.conclusion must be 'passed'")
    _exact_list(
        audit["scopes"], REQUIRED_AUDIT_SCOPES, "manifest.independent_audit.scopes"
    )
    audit_report_path = PurePosixPath(
        _nonempty_string(audit["report_path"], "manifest.independent_audit.report_path")
    )

    raw_artifacts = manifest["artifacts"]
    if not isinstance(raw_artifacts, list) or not raw_artifacts:
        raise EvidenceError("manifest.artifacts must be a non-empty list")
    artifacts = [
        _parse_artifact(value, index) for index, value in enumerate(raw_artifacts)
    ]
    paths = [artifact.path for artifact in artifacts]
    if len(paths) != len(set(paths)):
        raise EvidenceError("manifest.artifacts paths must be unique")
    if paths != sorted(paths, key=str):
        raise EvidenceError("manifest.artifacts must be sorted by path")
    present_kinds = {artifact.kind for artifact in artifacts}
    missing_kinds = set(REQUIRED_ARTIFACT_KINDS) - present_kinds
    if missing_kinds:
        raise EvidenceError(
            f"manifest.artifacts is missing kinds: {sorted(missing_kinds)}"
        )
    audit_artifact = next(
        (artifact for artifact in artifacts if artifact.path == audit_report_path), None
    )
    if audit_artifact is None or audit_artifact.kind != "audit_report":
        raise EvidenceError(
            "manifest.independent_audit.report_path must name an audit_report artifact"
        )
    return manifest, artifacts


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _relative_path(value: Any, label: str) -> PurePosixPath:
    raw = _nonempty_string(value, label)
    path = PurePosixPath(raw)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in ("", ".", "..") for part in path.parts)
    ):
        raise EvidenceError(f"{label} must be a normalized relative POSIX path")
    return path


def _validate_artifact_reference(
    value: Any,
    *,
    label: str,
    expected_kind: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    reference = _exact_fields(value, {"path", "sha256", "bytes"}, label)
    artifact_path = _relative_path(reference["path"], f"{label}.path")
    artifact = artifacts_by_path.get(artifact_path)
    if artifact is None or artifact.kind != expected_kind:
        raise EvidenceError(f"{label} must name a {expected_kind} artifact")
    binding = _parse_file_binding(
        {"sha256": reference["sha256"], "bytes": reference["bytes"]}, label
    )
    if binding != (artifact.sha256, artifact.bytes):
        raise EvidenceError(f"{label} binding does not match archive")
    if artifact.bytes == 0:
        raise EvidenceError(f"{label} must not be empty")
    return artifact_path


def _validate_transcript_binding(
    value: Any,
    *,
    label: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    return _validate_artifact_reference(
        value,
        label=label,
        expected_kind="operator_log",
        artifacts_by_path=artifacts_by_path,
    )


def _validate_pass_report(
    path: Path,
    *,
    artifact_kind: str,
    commit: str,
    expected_sha256: str,
    expected_bytes: int,
    artifacts_by_path: dict[PurePosixPath, Artifact],
    source_tree: str | None = None,
    source_tracked_file_count: int | None = None,
    source_archive_path: Path | None = None,
    source_archive_sha256: str | None = None,
    source_archive_bytes: int | None = None,
    workspace_manifest_sha256: str | None = None,
    source_lockfile_sha256: str | None = None,
    source_lockfile_bytes: int | None = None,
    source_path_list_path: Path | None = None,
    source_path_list_sha256: str | None = None,
    source_path_list_bytes: int | None = None,
) -> tuple[PurePosixPath, FormalSourceBindings | None]:
    """Validate one successful command gate and its separately bound transcript."""

    document = _read_bound_json_artifact(
        path,
        maximum_bytes=_MAX_PASS_REPORT_BYTES,
        expected_sha256=expected_sha256,
        expected_bytes=expected_bytes,
        label=artifact_kind,
    )
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "gate",
            "command",
            "exit_code",
            "passed",
            "started_at_utc",
            "duration_seconds",
            "details",
            "transcript",
        },
        artifact_kind,
    )
    if report["version"] != MANIFEST_VERSION or report["protocol"] != PROTOCOL:
        raise EvidenceError(f"{artifact_kind} must be a V1 {PROTOCOL} report")
    if report["commit"] != commit:
        raise EvidenceError(f"{artifact_kind} commit differs from release manifest")
    expected_gate = PASS_REPORT_GATES[artifact_kind]
    if report["gate"] != expected_gate:
        raise EvidenceError(f"{artifact_kind}.gate must be {expected_gate!r}")
    _nonempty_string(report["command"], f"{artifact_kind}.command")
    if report["exit_code"] != 0 or report["passed"] is not True:
        raise EvidenceError(f"{artifact_kind} must record a passing zero-exit command")
    started_at = report["started_at_utc"]
    if not isinstance(started_at, str) or _UTC_TIMESTAMP.fullmatch(started_at) is None:
        raise EvidenceError(f"{artifact_kind}.started_at_utc must be canonical UTC")
    duration = report["duration_seconds"]
    if (
        isinstance(duration, bool)
        or not isinstance(duration, (int, float))
        or not math.isfinite(float(duration))
        or duration <= 0
    ):
        raise EvidenceError(f"{artifact_kind}.duration_seconds must be positive")
    details = report["details"]
    formal_source_bindings: FormalSourceBindings | None = None
    if artifact_kind == "release_inventory_report":
        if (
            source_tree is None
            or source_tracked_file_count is None
            or source_archive_path is None
            or source_archive_sha256 is None
            or source_archive_bytes is None
            or workspace_manifest_sha256 is None
            or source_lockfile_sha256 is None
            or source_lockfile_bytes is None
            or source_path_list_path is None
            or source_path_list_sha256 is None
            or source_path_list_bytes is None
        ):
            raise EvidenceError(
                "release_inventory_report requires the validated source manifest"
            )
        formal_source_bindings = _validate_release_inventory_details(
            details,
            source_tree=source_tree,
            source_tracked_file_count=source_tracked_file_count,
            source_archive_path=source_archive_path,
            source_archive_sha256=source_archive_sha256,
            source_archive_bytes=source_archive_bytes,
            workspace_manifest_sha256=workspace_manifest_sha256,
            source_lockfile_sha256=source_lockfile_sha256,
            source_lockfile_bytes=source_lockfile_bytes,
            source_path_list_path=source_path_list_path,
            source_path_list_sha256=source_path_list_sha256,
            source_path_list_bytes=source_path_list_bytes,
        )
    elif artifact_kind == "sdk_test_report":
        sdk_details = _exact_fields(details, {"sdks"}, f"{artifact_kind}.details")
        sdks = sdk_details["sdks"]
        required_sdks = {
            "rust",
            "cli",
            "kotlin",
            "java",
            "swift",
            "python",
            "javascript",
        }
        if not isinstance(sdks, dict) or set(sdks) != required_sdks:
            raise EvidenceError("sdk_test_report must cover every supported SDK")
        for sdk in sorted(required_sdks):
            result = _exact_fields(
                sdks[sdk],
                {"tests", "failures", "skipped", "package_smoke", "passed"},
                f"sdk_test_report.details.sdks.{sdk}",
            )
            if (
                isinstance(result["tests"], bool)
                or not isinstance(result["tests"], int)
                or result["tests"] <= 0
                or result["failures"] != 0
                or result["skipped"] != 0
                or result["package_smoke"] is not True
                or result["passed"] is not True
            ):
                raise EvidenceError(f"sdk_test_report SDK {sdk!r} is not qualified")
    else:
        gate_details = _exact_fields(
            details,
            {"checks", "failures", "skipped"},
            f"{artifact_kind}.details",
        )
        checks = gate_details["checks"]
        skipped = gate_details["skipped"]
        if (
            isinstance(checks, bool)
            or not isinstance(checks, int)
            or checks <= 0
            or gate_details["failures"] != 0
            or isinstance(skipped, bool)
            or not isinstance(skipped, int)
            or skipped < 0
        ):
            raise EvidenceError(f"{artifact_kind} gate details are not passing")
    return (
        _validate_transcript_binding(
            report["transcript"],
            label=f"{artifact_kind}.transcript",
            artifacts_by_path=artifacts_by_path,
        ),
        formal_source_bindings,
    )


def _validate_randomized_seed_report(
    path: Path,
    *,
    commit: str,
    minimum_seeds: int,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read randomized_seed_report: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "seeds",
            "runs_per_seed",
            "failures",
            "passed",
            "transcript",
        },
        "randomized_seed_report",
    )
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
        or report["failures"] != []
    ):
        raise EvidenceError("randomized seed report is not a passing release report")
    seeds = report["seeds"]
    if (
        not isinstance(seeds, list)
        or any(
            isinstance(seed, bool) or not isinstance(seed, int) or seed < 0
            for seed in seeds
        )
        or seeds != sorted(set(seeds))
        or len(seeds) < minimum_seeds
    ):
        raise EvidenceError(
            "randomized seed report lacks the declared unique seed count"
        )
    runs_per_seed = report["runs_per_seed"]
    if (
        isinstance(runs_per_seed, bool)
        or not isinstance(runs_per_seed, int)
        or runs_per_seed <= 0
    ):
        raise EvidenceError("randomized seed report runs_per_seed must be positive")
    return _validate_transcript_binding(
        report["transcript"],
        label="randomized_seed_report.transcript",
        artifacts_by_path=artifacts_by_path,
    )


def _validate_soak_report(
    path: Path,
    *,
    commit: str,
    minimum_seconds: int,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read soak_report: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "duration_seconds",
            "iterations",
            "seeds",
            "validators_per_dataspace",
            "quorum",
            "mandatory_signed_rs16_da_rbc",
            "max_unavailable_per_committee",
            "partial_visibility_observations",
            "partial_spendable_observations",
            "failures",
            "passed",
            "transcript",
        },
        "soak_report",
    )
    duration = report["duration_seconds"]
    iterations = report["iterations"]
    seeds = report["seeds"]
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
        or report["failures"] != []
        or isinstance(duration, bool)
        or not isinstance(duration, (int, float))
        or not math.isfinite(float(duration))
        or duration < minimum_seconds
        or isinstance(iterations, bool)
        or not isinstance(iterations, int)
        or iterations <= 0
        or not isinstance(seeds, list)
        or not seeds
        or any(
            isinstance(seed, bool) or not isinstance(seed, int) or seed < 0
            for seed in seeds
        )
        or seeds != sorted(set(seeds))
        or report["validators_per_dataspace"] != 4
        or report["quorum"] != "3-of-4"
        or report["mandatory_signed_rs16_da_rbc"] is not True
        or report["max_unavailable_per_committee"] != 1
        or report["partial_visibility_observations"] != 0
        or report["partial_spendable_observations"] != 0
    ):
        raise EvidenceError(
            "soak report does not prove the required atomic two-hour run"
        )
    return _validate_transcript_binding(
        report["transcript"],
        label="soak_report.transcript",
        artifacts_by_path=artifacts_by_path,
    )


def _load_formal_tlc_report_validator() -> Any:
    """Load the strict TLC result parser shared with the evidence producer."""

    validator_path = (
        Path(__file__).with_name("formal") / "private_settlement_tlc_report.py"
    )
    validator_digest = hashlib.sha256(validator_path.read_bytes()).hexdigest()
    module_name = f"_private_settlement_tlc_report_for_release_{validator_digest}"
    sys.modules.pop(module_name, None)
    spec = importlib.util.spec_from_file_location(module_name, validator_path)
    if spec is None or spec.loader is None:
        raise EvidenceError("cannot load the strict formal TLC report validator")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    except Exception:
        del sys.modules[module_name]
        raise
    return module


def _formal_transcript_sections(
    transcript: str, headers: Sequence[str], first_offset: int
) -> list[str]:
    """Split an exact producer transcript into its ordered payload sections."""

    markers = [f"{header}\n" for header in headers]
    observed = list(re.finditer(r"(?m)^===== .* =====\n", transcript[first_offset:]))
    if len(observed) != len(markers):
        raise EvidenceError("formal TLC transcript contains an unexpected section header")
    offsets = [first_offset + match.start() for match in observed]
    if offsets[0] != first_offset:
        raise EvidenceError("formal TLC transcript contains data before its first section")
    for marker, offset in zip(markers, offsets, strict=True):
        if not transcript.startswith(marker, offset):
            raise EvidenceError("formal TLC transcript sections are missing or reordered")
    sections: list[str] = []
    for index, (marker, offset) in enumerate(zip(markers, offsets, strict=True)):
        payload_offset = offset + len(marker)
        payload_end = offsets[index + 1] if index + 1 < len(offsets) else len(transcript)
        sections.append(transcript[payload_offset:payload_end])
    return sections


def _validate_formal_tlc_transcript(
    payload: bytes,
    *,
    commit: str,
    model_sha256: str,
    evidence_code_sha256: str,
    java_runtime: Mapping[str, Any],
    configurations: Sequence[Mapping[str, Any]],
) -> None:
    """Replay the report rows from one exact, bound TLC/SANY transcript."""

    try:
        transcript = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise EvidenceError("formal TLC transcript is not UTF-8") from error
    validator = _load_formal_tlc_report_validator()
    producer_matrix = tuple(validator.CONFIGURATIONS)
    if (
        producer_matrix != REQUIRED_FORMAL_CONFIGURATION_MODELS
        or (validator.COUNT_MODEL, validator.INDEXED_MODEL) != _FORMAL_MODEL_FILES
    ):
        raise EvidenceError("formal TLC producer and release verifier matrices differ")

    first_header = f"===== SANY {_FORMAL_MODEL_FILES[0]} stdout (status 0) ====="
    first_offset = transcript.find(f"{first_header}\n")
    if first_offset < 0:
        raise EvidenceError("formal TLC transcript lacks the first SANY section")
    metadata = transcript[:first_offset]
    metadata_match = re.fullmatch(
        re.escape(
            "===== AtomicPrivateSettlementV1 TLC release run =====\n"
            f"commit={commit}\n"
            f"tool_version={_PINNED_FORMAL_TOOL_VERSION}\n"
            f"tool_sha256={_PINNED_FORMAL_TOOL_SHA256}\n"
            f"model_sha256={model_sha256}\n"
            f"evidence_code_sha256={evidence_code_sha256}\n"
            f"java_binary_sha256={java_runtime['binary_sha256']}\n"
            f"java_binary_bytes={java_runtime['binary_bytes']}\n"
            f"java_version_output_sha256={java_runtime['version_output_sha256']}\n"
            f"java_version_output_bytes={java_runtime['version_output_bytes']}\n"
        )
        + r"seed=(?P<seed>0|[1-9][0-9]*)\n"
        + r"fingerprint_index=(?P<fingerprint_index>0|[1-9][0-9]*)\n"
        + r"workers=(?P<workers>[1-9][0-9]*)\n",
        metadata,
    )
    if metadata_match is None:
        raise EvidenceError("formal TLC transcript metadata differs from the report")
    controls = {
        field: int(metadata_match.group(field))
        for field in ("seed", "fingerprint_index", "workers")
    }
    if (
        controls["fingerprint_index"] > 63
        or controls["workers"] < 1
    ):
        raise EvidenceError("formal TLC transcript run controls are out of range")

    headers: list[str] = []
    for model in _FORMAL_MODEL_FILES:
        headers.extend(
            (
                f"===== SANY {model} stdout (status 0) =====",
                f"===== SANY {model} stderr =====",
            )
        )
    for name, outcome, model in REQUIRED_FORMAL_CONFIGURATION_MODELS:
        status = 0 if outcome == "pass" else 12
        headers.extend(
            (
                f"===== {name} model {model} stdout (status {status}) =====",
                f"===== {name} model {model} stderr =====",
            )
        )
    sections = _formal_transcript_sections(transcript, headers, first_offset)

    section_index = 0
    for model in _FORMAL_MODEL_FILES:
        stdout = sections[section_index]
        stderr = sections[section_index + 1]
        section_index += 2
        try:
            validator.validate_sany(
                model=model,
                stdout=stdout,
                stderr=stderr,
                status=0,
            )
        except Exception as error:
            raise EvidenceError(
                f"formal TLC transcript has no clean SANY result for {model}: {error}"
            ) from error

    for row, (name, expected_outcome, model) in zip(
        configurations, REQUIRED_FORMAL_CONFIGURATION_MODELS, strict=True
    ):
        stdout = sections[section_index]
        stderr = sections[section_index + 1]
        section_index += 2
        status = 0 if expected_outcome == "pass" else 12
        try:
            summary = validator.parse_run(
                name=name,
                model=model,
                expected_outcome=expected_outcome,
                stdout=stdout,
                stderr=stderr,
                status=status,
                seed=controls["seed"],
                fingerprint_index=controls["fingerprint_index"],
                workers=str(controls["workers"]),
                tlc_version=_PINNED_FORMAL_TLC_VERSION,
            )
        except Exception as error:
            raise EvidenceError(
                f"formal TLC transcript result for {name} is invalid: {error}"
            ) from error
        if summary.as_json() != dict(row):
            raise EvidenceError(
                f"formal_model_report row for {name} differs from its TLC transcript"
            )


def _validate_formal_model_report(
    path: Path,
    *,
    root: Path,
    commit: str,
    expected_sha256: str,
    expected_bytes: int,
    formal_source_bindings: FormalSourceBindings,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    document = _read_bound_json_artifact(
        path,
        maximum_bytes=_MAX_PASS_REPORT_BYTES,
        expected_sha256=expected_sha256,
        expected_bytes=expected_bytes,
        label="formal_model_report",
    )
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "tool",
            "tool_version",
            "tool_sha256",
            "model_sha256",
            "evidence_code_sha256",
            "java_runtime",
            "configurations",
            "passed",
            "transcript",
        },
        "formal_model_report",
    )
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["tool"] != "TLC"
        or report["passed"] is not True
    ):
        raise EvidenceError("formal model report is not a passing TLC V1 report")
    if report["tool_version"] != _PINNED_FORMAL_TOOL_VERSION:
        raise EvidenceError("formal_model_report.tool_version is not the pinned toolchain")
    if report["tool_sha256"] != _PINNED_FORMAL_TOOL_SHA256:
        raise EvidenceError("formal_model_report.tool_sha256 is not the pinned TLA+ tools JAR")
    if report["model_sha256"] != formal_source_bindings.model_sha256:
        raise EvidenceError(
            "formal_model_report.model_sha256 differs from the validated source package"
        )
    if (
        report["evidence_code_sha256"]
        != formal_source_bindings.evidence_code_sha256
    ):
        raise EvidenceError(
            "formal_model_report.evidence_code_sha256 differs from the validated producer code"
        )
    java_runtime = _exact_fields(
        report["java_runtime"],
        {
            "binary_sha256",
            "binary_bytes",
            "version_output",
            "version_output_sha256",
            "version_output_bytes",
        },
        "formal_model_report.java_runtime",
    )
    version_output = java_runtime["version_output"]
    if not isinstance(version_output, str):
        raise EvidenceError("formal_model_report Java version output must be text")
    version_payload = version_output.encode("utf-8")
    if (
        not isinstance(java_runtime["binary_sha256"], str)
        or _HEX_64.fullmatch(java_runtime["binary_sha256"]) is None
        or isinstance(java_runtime["binary_bytes"], bool)
        or not isinstance(java_runtime["binary_bytes"], int)
        or java_runtime["binary_bytes"] <= 0
        or not version_payload
        or len(version_payload) > _MAX_FORMAL_JAVA_VERSION_OUTPUT_BYTES
        or "\0" in version_output
        or re.search(
            r'(?m)^(?:openjdk|java) version "[0-9][^"]*"', version_output
        )
        is None
        or java_runtime["version_output_sha256"]
        != hashlib.sha256(version_payload).hexdigest()
        or java_runtime["version_output_bytes"] != len(version_payload)
    ):
        raise EvidenceError(
            "formal_model_report Java runtime provenance is incomplete or inconsistent"
        )
    configurations = report["configurations"]
    if not isinstance(configurations, list) or len(configurations) != len(
        REQUIRED_FORMAL_CONFIGURATIONS
    ):
        raise EvidenceError("formal model report configuration matrix is incomplete")
    observed: list[tuple[str, str, str]] = []
    for index, value in enumerate(configurations):
        row = _exact_fields(
            value,
            {
                "name",
                "model",
                "expected_outcome",
                "observed_outcome",
                "generated_states",
                "distinct_states",
                "depth",
            },
            f"formal_model_report.configurations[{index}]",
        )
        name = row["name"]
        model = row["model"]
        expected = row["expected_outcome"]
        outcome = row["observed_outcome"]
        if (
            not isinstance(name, str)
            or not isinstance(model, str)
            or not isinstance(expected, str)
            or outcome != expected
        ):
            raise EvidenceError("formal model report outcome differs from expectation")
        observed.append((name, expected, model))
        for field in ("generated_states", "distinct_states", "depth"):
            count = row[field]
            if isinstance(count, bool) or not isinstance(count, int) or count <= 0:
                raise EvidenceError(
                    f"formal_model_report.configurations[{index}].{field} must be positive"
                )
    if observed != list(REQUIRED_FORMAL_CONFIGURATION_MODELS):
        raise EvidenceError(
            "formal model report lacks an exact positive/negative matrix"
        )
    transcript_reference = _validate_transcript_binding(
        report["transcript"],
        label="formal_model_report.transcript",
        artifacts_by_path=artifacts_by_path,
    )
    transcript_artifact = artifacts_by_path[transcript_reference]
    transcript_payload = _read_stable_bounded_artifact(
        root.joinpath(*transcript_reference.parts),
        maximum_bytes=_MAX_FORMAL_TRANSCRIPT_BYTES,
        expected_sha256=transcript_artifact.sha256,
        expected_bytes=transcript_artifact.bytes,
        label="formal_model_report.transcript",
    )
    _validate_formal_tlc_transcript(
        transcript_payload,
        commit=commit,
        model_sha256=formal_source_bindings.model_sha256,
        evidence_code_sha256=formal_source_bindings.evidence_code_sha256,
        java_runtime=java_runtime,
        configurations=configurations,
    )
    return transcript_reference


def _validate_auditor_key_custody_report(
    path: Path,
    *,
    commit: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(
            f"cannot read auditor_key_custody_report: {error}"
        ) from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "provider",
            "isolated_key_custody",
            "signing_encryption_keys_separate",
            "signing_consensus_keys_separate",
            "encryption_consensus_keys_separate",
            "rotation_tested",
            "retired_key_retention_tested",
            "capsule_rewrap_tested",
            "recovery_tested",
            "retention_period_days",
            "findings",
            "passed",
            "transcript",
        },
        "auditor_key_custody_report",
    )
    retention_days = report["retention_period_days"]
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
        or report["findings"] != []
        or report["isolated_key_custody"] is not True
        or report["signing_encryption_keys_separate"] is not True
        or report["signing_consensus_keys_separate"] is not True
        or report["encryption_consensus_keys_separate"] is not True
        or report["rotation_tested"] is not True
        or report["recovery_tested"] is not True
        or (
            report["retired_key_retention_tested"] is not True
            and report["capsule_rewrap_tested"] is not True
        )
        or isinstance(retention_days, bool)
        or not isinstance(retention_days, int)
        or retention_days <= 0
    ):
        raise EvidenceError(
            "auditor key custody report does not prove separation, rotation, and retention"
        )
    _nonempty_string(report["provider"], "auditor_key_custody_report.provider")
    return _validate_transcript_binding(
        report["transcript"],
        label="auditor_key_custody_report.transcript",
        artifacts_by_path=artifacts_by_path,
    )


def _parse_reproducible_artifact(
    value: Any, *, label: str, archived: bool
) -> tuple[str, str, str, int, PurePosixPath | None]:
    expected = {"target", "name", "sha256", "bytes"}
    if archived:
        expected.add("path")
    row = _exact_fields(value, expected, label)
    target = _nonempty_string(row["target"], f"{label}.target")
    name = _nonempty_string(row["name"], f"{label}.name")
    digest, byte_count = _parse_file_binding(
        {"sha256": row["sha256"], "bytes": row["bytes"]}, label
    )
    if byte_count == 0:
        raise EvidenceError(f"{label}.bytes must be positive")
    artifact_path = _relative_path(row["path"], f"{label}.path") if archived else None
    return target, name, digest, byte_count, artifact_path


def _validate_reproducible_build_report(
    path: Path,
    *,
    commit: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> list[PurePosixPath]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(
            f"cannot read reproducible_build_report: {error}"
        ) from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "source_date_epoch",
            "targets",
            "archived_artifacts",
            "builds",
            "passed",
        },
        "reproducible_build_report",
    )
    epoch = report["source_date_epoch"]
    targets = report["targets"]
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
        or isinstance(epoch, bool)
        or not isinstance(epoch, int)
        or epoch <= 0
        or not isinstance(targets, list)
        or not targets
        or any(not isinstance(target, str) or not target for target in targets)
        or targets != sorted(set(targets))
    ):
        raise EvidenceError(
            "reproducible build report identity or target set is invalid"
        )
    raw_archived = report["archived_artifacts"]
    if not isinstance(raw_archived, list) or not raw_archived:
        raise EvidenceError("reproducible build report lacks archived artifacts")
    archived_rows = [
        _parse_reproducible_artifact(
            value,
            label=f"reproducible_build_report.archived_artifacts[{index}]",
            archived=True,
        )
        for index, value in enumerate(raw_archived)
    ]
    if archived_rows != sorted(
        archived_rows, key=lambda row: (row[0], row[1], str(row[4]))
    ):
        raise EvidenceError("reproducible build archived artifacts must be sorted")
    if sorted({row[0] for row in archived_rows}) != targets:
        raise EvidenceError("reproducible build targets differ from archived artifacts")
    archived_paths = [row[4] for row in archived_rows]
    if len(archived_paths) != len(set(archived_paths)):
        raise EvidenceError("reproducible build archived paths must be unique")
    declared_release_binaries = {
        artifact.path: artifact
        for artifact in artifacts_by_path.values()
        if artifact.kind == "release_binary"
    }
    if set(archived_paths) != set(declared_release_binaries):
        raise EvidenceError(
            "reproducible build report does not bind every release binary"
        )
    for _, _, digest, byte_count, artifact_path in archived_rows:
        if artifact_path is None:
            raise EvidenceError("reproducible build archived path is missing")
        artifact = declared_release_binaries[artifact_path]
        if (digest, byte_count) != (artifact.sha256, artifact.bytes):
            raise EvidenceError("reproducible build archived binary binding is invalid")
    expected_build_rows = [
        (target, name, digest, byte_count, None)
        for target, name, digest, byte_count, _ in archived_rows
    ]
    raw_builds = report["builds"]
    if not isinstance(raw_builds, list) or len(raw_builds) < 2:
        raise EvidenceError("reproducible build report requires two independent builds")
    builder_ids: set[str] = set()
    environments: set[str] = set()
    transcripts: list[PurePosixPath] = []
    for index, value in enumerate(raw_builds):
        build = _exact_fields(
            value,
            {"builder_id", "environment_sha256", "artifacts", "transcript"},
            f"reproducible_build_report.builds[{index}]",
        )
        builder_id = _nonempty_string(
            build["builder_id"], f"reproducible_build_report.builds[{index}].builder_id"
        )
        environment = build["environment_sha256"]
        if not isinstance(environment, str) or _HEX_64.fullmatch(environment) is None:
            raise EvidenceError("reproducible build environment digest is invalid")
        if builder_id in builder_ids or environment in environments:
            raise EvidenceError(
                "reproducible builds must use distinct builders and environments"
            )
        builder_ids.add(builder_id)
        environments.add(environment)
        rows = build["artifacts"]
        if not isinstance(rows, list):
            raise EvidenceError("reproducible build artifacts must be a list")
        parsed_rows = [
            _parse_reproducible_artifact(
                row,
                label=f"reproducible_build_report.builds[{index}].artifacts[{row_index}]",
                archived=False,
            )
            for row_index, row in enumerate(rows)
        ]
        if parsed_rows != expected_build_rows:
            raise EvidenceError(
                "independent builds did not produce byte-identical artifacts"
            )
        transcripts.append(
            _validate_transcript_binding(
                build["transcript"],
                label=f"reproducible_build_report.builds[{index}].transcript",
                artifacts_by_path=artifacts_by_path,
            )
        )
    if len(transcripts) != len(set(transcripts)):
        raise EvidenceError("independent builds must use distinct transcripts")
    return transcripts


def _validate_cyclonedx_sbom(
    path: Path, *, commit: str, release_binaries: Sequence[Artifact]
) -> None:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read SBOM: {error}") from error
    if not isinstance(document, dict):
        raise EvidenceError("SBOM must be a CycloneDX JSON object")
    if (
        document.get("bomFormat") != "CycloneDX"
        or document.get("specVersion") not in {"1.5", "1.6"}
        or isinstance(document.get("version"), bool)
        or not isinstance(document.get("version"), int)
        or document["version"] <= 0
        or not isinstance(document.get("serialNumber"), str)
        or not document["serialNumber"].startswith("urn:uuid:")
    ):
        raise EvidenceError("SBOM must be versioned CycloneDX 1.5 or 1.6 JSON")
    metadata = document.get("metadata")
    if not isinstance(metadata, dict) or not isinstance(
        metadata.get("component"), dict
    ):
        raise EvidenceError("SBOM metadata must identify the Iroha component")
    component = metadata["component"]
    if component.get("name") != "iroha" or not isinstance(
        component.get("version"), str
    ):
        raise EvidenceError("SBOM metadata component must be Iroha with a version")
    properties = metadata.get("properties")
    if not isinstance(properties, list) or not any(
        isinstance(item, dict)
        and item.get("name") == "iroha.git.commit"
        and item.get("value") == commit
        for item in properties
    ):
        raise EvidenceError("SBOM does not bind the exact release commit")
    components = document.get("components")
    if not isinstance(components, list) or not components:
        raise EvidenceError("SBOM must contain a non-empty component inventory")
    recorded_hashes: set[str] = set()
    for candidate in [component, *components]:
        if not isinstance(candidate, dict):
            raise EvidenceError("SBOM component entry must be an object")
        hashes = candidate.get("hashes", [])
        if not isinstance(hashes, list):
            raise EvidenceError("SBOM component hashes must be a list")
        for item in hashes:
            if (
                isinstance(item, dict)
                and item.get("alg") == "SHA-256"
                and isinstance(item.get("content"), str)
                and _HEX_64.fullmatch(item["content"]) is not None
            ):
                recorded_hashes.add(item["content"])
    missing_hashes = {
        artifact.sha256 for artifact in release_binaries
    } - recorded_hashes
    if missing_hashes:
        raise EvidenceError("SBOM does not hash every archived release binary")


def _validate_source_manifest(
    path: Path,
    *,
    commit: str,
    expected_sha256: str,
    expected_bytes: int,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> tuple[
    PurePosixPath,
    str,
    int,
    str,
    PurePosixPath,
    PurePosixPath,
    PurePosixPath,
    PurePosixPath,
]:
    document = _read_bound_json_artifact(
        path,
        maximum_bytes=_MAX_SOURCE_MANIFEST_BYTES,
        expected_sha256=expected_sha256,
        expected_bytes=expected_bytes,
        label="source_manifest",
    )
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "tree",
            "workspace_manifest_sha256",
            "worktree_clean",
            "tracked_file_count",
            "modified",
            "untracked",
            "source_archive",
            "source_commit",
            "source_lockfile",
            "source_path_list",
            "passed",
            "transcript",
        },
        "source_manifest",
    )
    tree = report["tree"]
    workspace_manifest_sha256 = report["workspace_manifest_sha256"]
    tracked = report["tracked_file_count"]
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or not isinstance(tree, str)
        or _GIT_COMMIT.fullmatch(tree) is None
        or not isinstance(workspace_manifest_sha256, str)
        or _HEX_64.fullmatch(workspace_manifest_sha256) is None
        or report["worktree_clean"] is not True
        or isinstance(tracked, bool)
        or not isinstance(tracked, int)
        or tracked <= 0
        or report["modified"] != []
        or report["untracked"] != []
        or report["passed"] is not True
    ):
        raise EvidenceError("source manifest does not prove one clean exact Git tree")
    source_archive = _validate_artifact_reference(
        report["source_archive"],
        label="source_manifest.source_archive",
        expected_kind="source_archive",
        artifacts_by_path=artifacts_by_path,
    )
    source_commit = _validate_artifact_reference(
        report["source_commit"],
        label="source_manifest.source_commit",
        expected_kind="source_commit",
        artifacts_by_path=artifacts_by_path,
    )
    source_lockfile = _validate_artifact_reference(
        report["source_lockfile"],
        label="source_manifest.source_lockfile",
        expected_kind="source_lockfile",
        artifacts_by_path=artifacts_by_path,
    )
    source_path_list = _validate_artifact_reference(
        report["source_path_list"],
        label="source_manifest.source_path_list",
        expected_kind="source_path_list",
        artifacts_by_path=artifacts_by_path,
    )
    transcript = _validate_transcript_binding(
        report["transcript"],
        label="source_manifest.transcript",
        artifacts_by_path=artifacts_by_path,
    )
    return (
        transcript,
        tree,
        tracked,
        workspace_manifest_sha256,
        source_archive,
        source_commit,
        source_lockfile,
        source_path_list,
    )


def _validate_audit_attestation(
    path: Path,
    *,
    commit: str,
    audit_manifest: dict[str, Any],
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> None:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read audit_attestation: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "independent",
            "organization",
            "conclusion",
            "scopes",
            "issued_at_utc",
            "report_identifier",
            "report",
            "open_critical_findings",
            "open_high_findings",
            "passed",
        },
        "audit_attestation",
    )
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["independent"] is not True
        or report["organization"] != audit_manifest["organization"]
        or report["conclusion"] != "passed"
        or report["conclusion"] != audit_manifest["conclusion"]
        or report["scopes"] != audit_manifest["scopes"]
        or report["open_critical_findings"] != 0
        or report["open_high_findings"] != 0
        or report["passed"] is not True
    ):
        raise EvidenceError(
            "audit attestation does not match the independent passing audit declaration"
        )
    issued_at = report["issued_at_utc"]
    if not isinstance(issued_at, str) or _UTC_TIMESTAMP.fullmatch(issued_at) is None:
        raise EvidenceError("audit_attestation.issued_at_utc must be canonical UTC")
    _nonempty_string(report["report_identifier"], "audit_attestation.report_identifier")
    report_path = _validate_artifact_reference(
        report["report"],
        label="audit_attestation.report",
        expected_kind="audit_report",
        artifacts_by_path=artifacts_by_path,
    )
    if report_path != PurePosixPath(audit_manifest["report_path"]):
        raise EvidenceError("audit attestation binds a different audit report")


_HARDWARE_PROFILE_FIELDS = (
    "version",
    "protocol",
    "host_id",
    "operating_system",
    "kernel",
    "architecture",
    "cpu_model",
    "physical_cores",
    "logical_cores",
    "memory_bytes",
    "storage_model",
    "network_description",
    "clock_policy",
    "power_profile",
    "virtualized",
)


def _hardware_profile_sha256(report: Mapping[str, Any]) -> str:
    """Hash stable benchmark-host properties, excluding release-specific metadata."""

    profile = {field: report[field] for field in _HARDWARE_PROFILE_FIELDS}
    canonical = json.dumps(
        profile, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def _validate_hardware_description(path: Path, *, commit: str) -> str:
    """Validate the exact artifact and return its stable benchmark profile digest."""
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read hardware_description: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "collected_at_utc",
            "host_id",
            "operating_system",
            "kernel",
            "architecture",
            "cpu_model",
            "physical_cores",
            "logical_cores",
            "memory_bytes",
            "storage_model",
            "network_description",
            "clock_policy",
            "power_profile",
            "virtualized",
            "passed",
        },
        "hardware_description",
    )
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
        or not isinstance(report["virtualized"], bool)
    ):
        raise EvidenceError("hardware description does not bind the release candidate")
    collected = report["collected_at_utc"]
    if not isinstance(collected, str) or _UTC_TIMESTAMP.fullmatch(collected) is None:
        raise EvidenceError(
            "hardware_description.collected_at_utc must be canonical UTC"
        )
    for field in (
        "host_id",
        "operating_system",
        "kernel",
        "architecture",
        "cpu_model",
        "storage_model",
        "network_description",
        "clock_policy",
        "power_profile",
    ):
        _nonempty_string(report[field], f"hardware_description.{field}")
    physical = report["physical_cores"]
    logical = report["logical_cores"]
    memory = report["memory_bytes"]
    if (
        isinstance(physical, bool)
        or not isinstance(physical, int)
        or physical <= 0
        or isinstance(logical, bool)
        or not isinstance(logical, int)
        or logical < physical
        or isinstance(memory, bool)
        or not isinstance(memory, int)
        or memory <= 0
    ):
        raise EvidenceError("hardware description resource counts are invalid")
    return _hardware_profile_sha256(report)


def _validate_configuration_manifest(
    path: Path,
    *,
    commit: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> dict[int, str]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read configuration_manifest: {error}") from error
    report = _exact_fields(
        document,
        {"version", "protocol", "commit", "configurations", "passed"},
        "configuration_manifest",
    )
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
    ):
        raise EvidenceError(
            "configuration manifest does not bind the release candidate"
        )
    rows = report["configurations"]
    if not isinstance(rows, list) or len(rows) != len(REQUIRED_PARTICIPANTS):
        raise EvidenceError(
            "configuration manifest must cover every real participant count"
        )
    participants_seen: list[int] = []
    configuration_digests: dict[int, str] = {}
    paths: list[PurePosixPath] = []
    for index, value in enumerate(rows):
        row = _exact_fields(
            value,
            {
                "participants",
                "validators_per_dataspace",
                "quorum",
                "mandatory_signed_rs16_da_rbc",
                "path",
                "sha256",
                "bytes",
            },
            f"configuration_manifest.configurations[{index}]",
        )
        participants = row["participants"]
        if (
            participants not in REQUIRED_PARTICIPANTS
            or row["validators_per_dataspace"] != 4
            or row["quorum"] != "3-of-4"
            or row["mandatory_signed_rs16_da_rbc"] is not True
        ):
            raise EvidenceError(
                "configuration manifest contains an invalid network profile"
            )
        reference = {
            "path": row["path"],
            "sha256": row["sha256"],
            "bytes": row["bytes"],
        }
        artifact_path = _validate_artifact_reference(
            reference,
            label=f"configuration_manifest.configurations[{index}]",
            expected_kind="configuration",
            artifacts_by_path=artifacts_by_path,
        )
        participants_seen.append(participants)
        paths.append(artifact_path)
        configuration_digests[participants] = row["sha256"]
    if participants_seen != list(REQUIRED_PARTICIPANTS) or len(paths) != len(
        set(paths)
    ):
        raise EvidenceError(
            "configuration manifest matrix must be canonical and unique"
        )
    declared_configurations = {
        artifact.path
        for artifact in artifacts_by_path.values()
        if artifact.kind == "configuration"
    }
    if set(paths) != declared_configurations:
        raise EvidenceError(
            "configuration manifest does not bind every archived configuration"
        )
    return configuration_digests


def _regenerate_fault_report(raw_paths: Sequence[Path]) -> dict[str, Any]:
    reporter_path = Path(__file__).with_name("private_settlement_fault_report.py")
    spec = importlib.util.spec_from_file_location(
        "_private_settlement_fault_report_for_release", reporter_path
    )
    if spec is None or spec.loader is None:
        raise EvidenceError("cannot load the strict fault-matrix reporter")
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
        return module.build_report(
            module.load_runs(raw_paths), module.input_bindings(raw_paths)
        )
    except Exception as error:
        raise EvidenceError(
            f"real-network fault raw evidence is invalid: {error}"
        ) from error


def _load_fault_evidence_validator() -> Any:
    """Load the strict nested command/state validator used by the runner."""

    module_name = "private_settlement_release_runner"
    existing = sys.modules.get(module_name)
    if existing is not None:
        return existing
    validator_path = Path(__file__).with_name("private_settlement_release_runner.py")
    spec = importlib.util.spec_from_file_location(module_name, validator_path)
    if spec is None or spec.loader is None:
        raise EvidenceError("cannot load the strict nested fault-evidence validator")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    except Exception:
        del sys.modules[module_name]
        raise
    return module


def _validate_fault_trial_evidence_bindings(
    raw_paths: Sequence[Path], artifacts: Sequence[Artifact], root: Path
) -> None:
    """Require every fault-trial transcript/capture digest to resolve in the archive."""

    transcript_artifacts: dict[str, list[Artifact]] = defaultdict(list)
    capture_artifacts: dict[str, list[Artifact]] = defaultdict(list)
    for artifact in artifacts:
        if artifact.kind in FAULT_TRANSCRIPT_ARTIFACT_KINDS:
            transcript_artifacts[artifact.sha256].append(artifact)
        if artifact.kind in FAULT_CAPTURE_ARTIFACT_KINDS:
            capture_artifacts[artifact.sha256].append(artifact)

    record_cache: dict[PurePosixPath, dict[str, dict[str, Any]]] = {}
    validated_files: set[tuple[PurePosixPath, str, int, int, int]] = set()
    validator = _load_fault_evidence_validator()

    def archived_records(
        digest: str, candidates: dict[str, list[Artifact]], label: str
    ) -> dict[str, dict[str, Any]]:
        matches = candidates.get(digest, [])
        if len(matches) != 1:
            raise EvidenceError(f"{label} does not resolve to one archived artifact")
        artifact = matches[0]
        cached = record_cache.get(artifact.path)
        if cached is not None:
            return cached
        path = root.joinpath(*artifact.path.parts)
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeError) as error:
            raise EvidenceError(f"cannot read {label}: {error}") from error
        records: dict[str, dict[str, Any]] = {}
        for line_number, line in enumerate(lines, 1):
            if not line.strip():
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError as error:
                raise EvidenceError(
                    f"{artifact.path}:{line_number} is not JSONL evidence: {error}"
                ) from error
            if not isinstance(entry, dict) or not isinstance(entry.get("record"), str):
                raise EvidenceError(
                    f"{artifact.path}:{line_number} lacks an evidence record identifier"
                )
            record = entry["record"]
            if record in records:
                raise EvidenceError(
                    f"{artifact.path}:{line_number} duplicates evidence record {record!r}"
                )
            records[record] = entry
        if not records:
            raise EvidenceError(f"{artifact.path} contains no evidence records")
        record_cache[artifact.path] = records
        return records

    transcript_digests = Counter(
        artifact.sha256
        for artifact in artifacts
        if artifact.kind in FAULT_TRANSCRIPT_ARTIFACT_KINDS
    )
    capture_digests = Counter(
        artifact.sha256
        for artifact in artifacts
        if artifact.kind in FAULT_CAPTURE_ARTIFACT_KINDS
    )
    for path in raw_paths:
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeError) as error:
            raise EvidenceError(f"cannot read fault trial evidence: {error}") from error
        for line_number, line in enumerate(lines, 1):
            if not line.strip():
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise EvidenceError(
                    f"invalid fault trial evidence at {path}:{line_number}: {error}"
                ) from error
            for collection in ("loss_trials", "phase_cut_partitions", "crash_recoveries"):
                for index, trial in enumerate(record[collection]):
                    transcript = trial["control_transcript_sha256"]
                    transcript_record = trial["control_transcript_record"]
                    capture = trial["observation_capture_sha256"]
                    capture_record = trial["observation_capture_record"]
                    if transcript_digests[transcript] != 1:
                        raise EvidenceError(
                            f"{path}:{line_number}.{collection}[{index}] control transcript "
                            "does not resolve to one archived operator log"
                        )
                    if capture_digests[capture] != 1:
                        raise EvidenceError(
                            f"{path}:{line_number}.{collection}[{index}] observation capture "
                            "does not resolve to one archived capture"
                        )
                    transcript_records = archived_records(
                        transcript, transcript_artifacts, "control transcript"
                    )
                    capture_records = archived_records(
                        capture, capture_artifacts, "observation capture"
                    )
                    transcript_entry = transcript_records.get(transcript_record)
                    if transcript_entry is None:
                        raise EvidenceError(
                            f"{path}:{line_number}.{collection}[{index}] control record "
                            "is absent from its archived transcript"
                        )
                    capture_entry = capture_records.get(capture_record)
                    if capture_entry is None:
                        raise EvidenceError(
                            f"{path}:{line_number}.{collection}[{index}] observation record "
                            "is absent from its archived capture"
                        )
                    common = {
                        "record": transcript_record,
                        "participants": record["participants"],
                        "seed": record["seed"],
                        "run": record["run"],
                        "collection": collection,
                        "trial_index": index,
                    }
                    if any(
                        transcript_entry.get(field) != value
                        for field, value in common.items()
                    ):
                        raise EvidenceError(
                            f"{path}:{line_number}.{collection}[{index}] control record "
                            "does not bind the raw fault-trial identity"
                        )
                    capture_common = {
                        **common,
                        "record": capture_record,
                    }
                    if any(
                        capture_entry.get(field) != value
                        for field, value in capture_common.items()
                    ):
                        raise EvidenceError(
                            f"{path}:{line_number}.{collection}[{index}] observation record "
                            "does not bind the raw fault-trial identity"
                        )
                    transcript_artifact = transcript_artifacts[transcript][0]
                    capture_artifact = capture_artifacts[capture][0]
                    binding = (
                        record["participants"],
                        record["seed"],
                        record["run"],
                    )
                    transcript_key = (transcript_artifact.path, "control", *binding)
                    capture_key = (capture_artifact.path, "capture", *binding)
                    try:
                        if transcript_key not in validated_files:
                            bound_transcript_records = [
                                entry
                                for entry in transcript_records.values()
                                if (
                                    entry.get("participants"),
                                    entry.get("seed"),
                                    entry.get("run"),
                                )
                                == binding
                            ]
                            validator.validate_fault_control_records(
                                bound_transcript_records,
                                participants=binding[0],
                                seed=binding[1],
                                run=binding[2],
                            )
                            validated_files.add(transcript_key)
                        if capture_key not in validated_files:
                            bound_capture_records = [
                                entry
                                for entry in capture_records.values()
                                if (
                                    entry.get("participants"),
                                    entry.get("seed"),
                                    entry.get("run"),
                                )
                                == binding
                            ]
                            validator.validate_fault_observation_records(
                                bound_capture_records,
                                participants=binding[0],
                                seed=binding[1],
                                run=binding[2],
                            )
                            validated_files.add(capture_key)
                        validator.validate_fault_trial_control_semantics(
                            transcript_entry,
                            collection=collection,
                            trial=trial,
                            label=f"{path}:{line_number}.{collection}[{index}]",
                        )
                    except Exception as error:
                        raise EvidenceError(
                            f"{path}:{line_number}.{collection}[{index}] "
                            f"nested fault evidence is invalid: {error}"
                        ) from error
                    if (
                        capture_entry["partial_visibility_observed"]
                        != trial["partial_visibility_observed"]
                        or capture_entry["partial_spendable_observations"]
                        != record["atomicity"]["partial_spendable_observations"]
                    ):
                        raise EvidenceError(
                            f"{path}:{line_number}.{collection}[{index}] observation record "
                            "contradicts the raw atomicity trial"
                        )


def _validate_fault_report(
    path: Path,
    *,
    raw_artifacts: Sequence[Artifact],
    artifacts: Sequence[Artifact],
    root: Path,
    commit: str,
    hardware_sha256: str,
    configuration_sha256_by_participants: dict[int, str],
) -> None:
    """Bind the release manifest to a passing strict fault-matrix summary."""

    try:
        report = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(
            f"cannot read real-network fault report: {error}"
        ) from error
    record = _exact_fields(
        report,
        {
            "version",
            "protocol",
            "commit",
            "raw_inputs",
            "environment",
            "requirements",
            "matrix",
            "passed",
        },
        "real_network_fault_report",
    )
    if (
        record["version"] != MANIFEST_VERSION
        or record["protocol"] != PROTOCOL
        or record["passed"] is not True
        or record["commit"] != commit
    ):
        raise EvidenceError("real-network fault report must be a passing V1 report")
    raw_inputs = record["raw_inputs"]
    if not isinstance(raw_inputs, list) or not raw_inputs:
        raise EvidenceError("real-network fault report must bind raw JSONL inputs")
    parsed_bindings = [
        _parse_file_binding(value, f"real_network_fault_report.raw_inputs[{index}]")
        for index, value in enumerate(raw_inputs)
    ]
    if parsed_bindings != sorted(parsed_bindings) or Counter(
        parsed_bindings
    ) != Counter((artifact.sha256, artifact.bytes) for artifact in raw_artifacts):
        raise EvidenceError(
            "real-network fault report raw bindings do not match archive"
        )
    environment = _exact_fields(
        record["environment"],
        {"hardware_sha256", "configuration_sha256_by_participants"},
        "real_network_fault_report.environment",
    )
    expected_configurations = {
        str(participants): configuration_sha256_by_participants[participants]
        for participants in REQUIRED_PARTICIPANTS
    }
    if (
        environment["hardware_sha256"] != hardware_sha256
        or environment["configuration_sha256_by_participants"]
        != expected_configurations
    ):
        raise EvidenceError(
            "real-network fault report used different hardware or configs"
        )
    requirements = _exact_fields(
        record["requirements"],
        {
            "participants",
            "minimum_seeds_per_participant",
            "validators_per_dataspace",
            "quorum",
            "loss_phases",
            "loss_percentages",
            "phase_cuts",
            "crash_boundaries",
        },
        "real_network_fault_report.requirements",
    )
    _exact_list(
        requirements["participants"],
        REQUIRED_PARTICIPANTS,
        "real_network_fault_report.requirements.participants",
    )
    _exact_integer(
        requirements["minimum_seeds_per_participant"],
        REQUIRED_SEEDS_PER_PARTICIPANT,
        "real_network_fault_report.requirements.minimum_seeds_per_participant",
    )
    _exact_integer(
        requirements["validators_per_dataspace"],
        4,
        "real_network_fault_report.requirements.validators_per_dataspace",
    )
    if requirements["quorum"] != "3-of-4":
        raise EvidenceError("real-network fault report quorum must be '3-of-4'")
    _exact_list(
        requirements["loss_phases"],
        ("restricted_da", "prepare", "commit"),
        "real_network_fault_report.requirements.loss_phases",
    )
    _exact_list(
        requirements["loss_percentages"],
        REQUIRED_LOSS_PERCENTAGES,
        "real_network_fault_report.requirements.loss_percentages",
    )
    _exact_list(
        requirements["phase_cuts"],
        (
            "da_before_availability_qc",
            "prepare_before_complete_barrier",
            "commit_before_complete_barrier",
            "carrier_before_global_finality",
        ),
        "real_network_fault_report.requirements.phase_cuts",
    )
    _exact_list(
        requirements["crash_boundaries"],
        REQUIRED_CRASH_BOUNDARIES,
        "real_network_fault_report.requirements.crash_boundaries",
    )
    matrix = record["matrix"]
    expected_keys = {str(participants) for participants in REQUIRED_PARTICIPANTS}
    if not isinstance(matrix, dict) or set(matrix) != expected_keys:
        raise EvidenceError("real-network fault report matrix is incomplete")
    for participants in REQUIRED_PARTICIPANTS:
        bucket = _exact_fields(
            matrix[str(participants)],
            {"runs", "seeds"},
            f"real_network_fault_report.matrix.{participants}",
        )
        runs = bucket["runs"]
        seeds = bucket["seeds"]
        if (
            isinstance(runs, bool)
            or not isinstance(runs, int)
            or runs < REQUIRED_SEEDS_PER_PARTICIPANT
            or not isinstance(seeds, list)
            or len(seeds) < REQUIRED_SEEDS_PER_PARTICIPANT
            or any(
                isinstance(seed, bool) or not isinstance(seed, int) for seed in seeds
            )
            or seeds != sorted(set(seeds))
        ):
            raise EvidenceError(
                f"real-network fault report N={participants} lacks ten unique seeds"
            )
    raw_paths = [root.joinpath(*artifact.path.parts) for artifact in raw_artifacts]
    if _regenerate_fault_report(raw_paths) != report:
        raise EvidenceError(
            "real-network fault report does not match archived raw runs"
        )
    _validate_fault_trial_evidence_bindings(raw_paths, artifacts, root)


def _parse_file_binding(value: Any, label: str) -> tuple[str, int]:
    record = _exact_fields(value, {"sha256", "bytes"}, label)
    digest = record["sha256"]
    byte_count = record["bytes"]
    if not isinstance(digest, str) or _HEX_64.fullmatch(digest) is None:
        raise EvidenceError(f"{label}.sha256 must be lowercase SHA-256")
    if (
        isinstance(byte_count, bool)
        or not isinstance(byte_count, int)
        or byte_count < 0
    ):
        raise EvidenceError(f"{label}.bytes must be a non-negative integer")
    return digest, byte_count


def _load_canary_names(path: Path) -> list[str]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read canary manifest: {error}") from error
    manifest = _exact_fields(document, {"version", "canaries"}, "canary_manifest")
    if manifest["version"] != MANIFEST_VERSION:
        raise EvidenceError("canary manifest version must be 1")
    entries = manifest["canaries"]
    if not isinstance(entries, list) or not entries:
        raise EvidenceError("canary manifest must contain canaries")
    names: list[str] = []
    for index, value in enumerate(entries):
        entry = _exact_fields(
            value, {"name", "kind", "value"}, f"canary_manifest.canaries[{index}]"
        )
        name = _nonempty_string(
            entry["name"], f"canary_manifest.canaries[{index}].name"
        )
        kind = entry["kind"]
        secret = entry["value"]
        if kind == "text":
            _nonempty_string(secret, f"canary_manifest.canaries[{index}].value")
        elif kind == "integer":
            if isinstance(secret, bool) or not isinstance(secret, int) or secret < 0:
                raise EvidenceError(
                    f"canary_manifest.canaries[{index}].value must be non-negative"
                )
        elif kind == "binary_base64":
            _nonempty_string(secret, f"canary_manifest.canaries[{index}].value")
        else:
            raise EvidenceError(
                f"canary_manifest.canaries[{index}].kind is unsupported"
            )
        names.append(name)
    if names != sorted(set(names)):
        raise EvidenceError("canary manifest names must be unique and sorted")
    if not set(REQUIRED_LEAKAGE_CANARY_NAMES).issubset(names):
        raise EvidenceError("canary manifest lacks a required secret class")
    return names


def _load_traffic_count_manifest(path: Path) -> dict[str, int]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read traffic-count manifest: {error}") from error
    manifest = _exact_fields(
        document, {"version", "channels"}, "traffic_count_manifest"
    )
    if manifest["version"] != MANIFEST_VERSION:
        raise EvidenceError("traffic-count manifest version must be 1")
    channels = manifest["channels"]
    if not isinstance(channels, dict) or set(channels) != set(
        REQUIRED_TRAFFIC_COUNT_CHANNELS
    ):
        raise EvidenceError("traffic-count manifest channels are incomplete")
    for channel, count in channels.items():
        if isinstance(count, bool) or not isinstance(count, int) or count < 0:
            raise EvidenceError(
                f"traffic-count manifest channel {channel!r} is invalid"
            )
    return {channel: channels[channel] for channel in REQUIRED_TRAFFIC_COUNT_CHANNELS}


def _verify_archived_canary_scan(
    canary_manifest: Path,
    artifacts: Sequence[Artifact],
    root: Path,
) -> None:
    scanner_path = Path(__file__).with_name("private_settlement_leakage_audit.py")
    module_name = "_private_settlement_leakage_audit_for_release"
    spec = importlib.util.spec_from_file_location(module_name, scanner_path)
    if spec is None or spec.loader is None:
        raise EvidenceError("cannot load the strict leakage scanner")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(module_name)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
        canaries = module.load_canaries(canary_manifest)
        for artifact in artifacts:
            if artifact.kind not in REQUIRED_LEAKAGE_ARTIFACT_KINDS:
                continue
            if module.scan_file(root.joinpath(*artifact.path.parts), canaries):
                raise EvidenceError(
                    "an archived privacy surface contains a planted secret canary"
                )
    except EvidenceError:
        raise
    except Exception as error:
        raise EvidenceError(f"archived canary scan is invalid: {error}") from error
    finally:
        if previous is None:
            del sys.modules[module_name]
        else:
            sys.modules[module_name] = previous


def _recompute_archived_differential(left: Path, right: Path) -> dict[str, Any]:
    """Run the strict differential comparator over the archived pair roots."""

    scanner_path = Path(__file__).with_name("private_settlement_leakage_audit.py")
    module_name = "_private_settlement_leakage_differential_for_release"
    spec = importlib.util.spec_from_file_location(module_name, scanner_path)
    if spec is None or spec.loader is None:
        raise EvidenceError("cannot load the strict differential scanner")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(module_name)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
        return module.compare_capture_roots(
            left, right, module.DEFAULT_MAX_FILE_BYTES
        )
    except EvidenceError:
        raise
    except Exception as error:
        raise EvidenceError(f"archived differential replay is invalid: {error}") from error
    finally:
        if previous is None:
            del sys.modules[module_name]
        else:
            sys.modules[module_name] = previous


def _load_release_runner_for_evidence_replay() -> Any:
    """Load the release runner so archived harness responses use one validator."""

    module_name = "_private_settlement_release_runner_for_evidence"
    existing = sys.modules.get(module_name)
    if existing is not None:
        return existing
    runner_path = Path(__file__).with_name("private_settlement_release_runner.py")
    spec = importlib.util.spec_from_file_location(module_name, runner_path)
    if spec is None or spec.loader is None:
        raise EvidenceError("cannot load the private-settlement release runner")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    except Exception:
        del sys.modules[module_name]
        raise
    return module


def _validate_archived_leakage_provenance(
    *,
    provenance_artifacts: Sequence[Artifact],
    artifacts_by_path: Mapping[PurePosixPath, Artifact],
    root: Path,
    commit: str,
    variant_roots: Mapping[str, PurePosixPath],
    archived_counts: Mapping[str, Mapping[str, int]],
) -> None:
    """Replay raw capture, source, and atomicity evidence from archived runs."""

    if len(provenance_artifacts) != 2:
        raise EvidenceError(
            "evidence bundle must contain exactly two leakage capture provenance records"
        )
    try:
        runner = _load_release_runner_for_evidence_replay()
    except Exception as error:
        raise EvidenceError(f"cannot load leakage provenance validator: {error}") from error
    seen: set[str] = set()
    for artifact in provenance_artifacts:
        try:
            raw = root.joinpath(*artifact.path.parts).read_text(encoding="utf-8")
            response = runner.strict_json_loads(raw, "archived leakage provenance")
            payload = runner.exact_fields(
                response["payload"],
                runner.LEAKAGE_PAYLOAD_FIELDS,
                "archived leakage provenance payload",
            )
            variant = payload["variant"]
            if variant not in {"left", "right"} or variant in seen:
                raise EvidenceError("leakage provenance variants are incomplete or duplicated")
            if artifact.path.name != f"capture-provenance-{variant}.json":
                raise EvidenceError("leakage provenance used a non-canonical filename")
            seen.add(variant)
            if response["commit"] != commit or response["kind"] != "leakage":
                raise EvidenceError("leakage provenance does not bind the release candidate")
            plan = {
                "commit": commit,
                "hardware": {
                    "sha256": response["hardware_sha256"],
                    "profile_sha256": response["hardware_profile_sha256"],
                },
            }
            job = {
                "request_id": response["request_id"],
                "invocation_nonce": response["invocation_nonce"],
                "kind": "leakage",
                "configuration_sha256": response["configuration_sha256"],
                "participants": response["participants"],
                "variant": variant,
                "canary_names": payload["canaries_injected"],
                "canary_commitments": payload["canary_commitments"],
            }
            evidence_dir = root.joinpath(*variant_roots[variant].parts)
            counts, surfaces = runner.validate_leakage_response(
                response,
                plan=plan,
                job=job,
                evidence_dir=evidence_dir,
            )
            if counts != dict(archived_counts[variant]):
                raise EvidenceError(
                    f"archived {variant} traffic counts differ from provenance replay"
                )
            for surface, path, binding in surfaces:
                relative = variant_roots[variant] / runner.SURFACE_FILES[surface]
                declared = artifacts_by_path.get(relative)
                if (
                    declared is None
                    or declared.kind != surface
                    or declared.sha256 != binding["sha256"]
                    or declared.bytes != binding["bytes"]
                    or path != root.joinpath(*relative.parts).resolve(strict=True)
                ):
                    raise EvidenceError(
                        f"archived {variant} surface {surface} differs from provenance replay"
                    )
        except EvidenceError:
            raise
        except Exception as error:
            raise EvidenceError(
                f"archived leakage provenance replay failed: {error}"
            ) from error
    if seen != {"left", "right"}:
        raise EvidenceError("leakage provenance omitted a secret-only variant")


def _public_json_shape(value: Any) -> Any:
    if isinstance(value, dict):
        if any(not isinstance(key, str) for key in value):
            raise EvidenceError("differential JSON object key is not text")
        return {key: _public_json_shape(value[key]) for key in sorted(value)}
    if isinstance(value, list):
        return [_public_json_shape(item) for item in value]
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        if not math.isfinite(value):
            raise EvidenceError("differential JSON number is not finite")
        return "number"
    if isinstance(value, str):
        return "string"
    raise EvidenceError("differential JSON contains an unsupported value")


def _validate_differential_pair_manifest(
    path: Path,
    *,
    commit: str,
    root: Path,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> list[tuple[str, int]]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(
            f"cannot read differential_pair_manifest: {error}"
        ) from error
    manifest = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "left_root",
            "right_root",
            "pairs",
            "passed",
        },
        "differential_pair_manifest",
    )
    if (
        manifest["version"] != MANIFEST_VERSION
        or manifest["protocol"] != PROTOCOL
        or manifest["commit"] != commit
        or manifest["passed"] is not True
    ):
        raise EvidenceError(
            "differential pair manifest does not bind the release candidate"
        )
    pairs = manifest["pairs"]
    if not isinstance(pairs, list) or not pairs:
        raise EvidenceError("differential pair manifest must contain artifact pairs")
    left_root = _relative_path(
        manifest["left_root"], "differential_pair_manifest.left_root"
    )
    right_root = _relative_path(
        manifest["right_root"], "differential_pair_manifest.right_root"
    )
    if (
        left_root == right_root
        or left_root.is_relative_to(right_root)
        or right_root.is_relative_to(left_root)
    ):
        raise EvidenceError("differential roots must be distinct and non-overlapping")
    for label, relative_root in (("left", left_root), ("right", right_root)):
        directory = root.joinpath(*relative_root.parts)
        if directory.is_symlink() or not directory.is_dir():
            raise EvidenceError(f"differential {label} root is not a real directory")
    ordering: list[tuple[str, str]] = []
    surfaces: set[str] = set()
    referenced_paths: set[PurePosixPath] = set()
    bindings: list[tuple[str, int]] = []
    for index, value in enumerate(pairs):
        label = f"differential_pair_manifest.pairs[{index}]"
        pair = _exact_fields(
            value,
            {"surface", "relative_name", "left", "right"},
            label,
        )
        surface = pair["surface"]
        if surface not in REQUIRED_DIFFERENTIAL_ARTIFACT_KINDS:
            raise EvidenceError(f"{label}.surface is not a privacy surface")
        relative_name = _relative_path(pair["relative_name"], f"{label}.relative_name")
        if relative_name.as_posix() != DIFFERENTIAL_SURFACE_FILES[surface]:
            raise EvidenceError(f"{label}.relative_name is not canonical for its surface")
        order_key = (surface, relative_name.as_posix())
        ordering.append(order_key)
        surfaces.add(surface)
        left_path = _validate_artifact_reference(
            pair["left"],
            label=f"{label}.left",
            expected_kind=surface,
            artifacts_by_path=artifacts_by_path,
        )
        right_path = _validate_artifact_reference(
            pair["right"],
            label=f"{label}.right",
            expected_kind=surface,
            artifacts_by_path=artifacts_by_path,
        )
        if (
            left_path != left_root / relative_name
            or right_path != right_root / relative_name
        ):
            raise EvidenceError(
                "differential pair paths do not match their declared roots and relative name"
            )
        if (
            left_path == right_path
            or left_path in referenced_paths
            or right_path in referenced_paths
        ):
            raise EvidenceError(
                "differential pair paths must be distinct and single-use"
            )
        referenced_paths.update((left_path, right_path))
        left = artifacts_by_path[left_path]
        right = artifacts_by_path[right_path]
        if left.bytes != right.bytes and surface not in {
            "restricted_audit_source",
            "restricted_packet_source",
        }:
            raise EvidenceError("differential pair byte sizes differ")
        if surface in REQUIRED_DIFFERENTIAL_STATE_CHANGES and left.sha256 == right.sha256:
            raise EvidenceError(
                f"differential state surface {surface} did not change with the secret variant"
            )
        bindings.extend(((left.sha256, left.bytes), (right.sha256, right.bytes)))
        if relative_name.suffix.lower() == ".json":
            try:
                left_json = json.loads(
                    root.joinpath(*left_path.parts).read_text(encoding="utf-8")
                )
                right_json = json.loads(
                    root.joinpath(*right_path.parts).read_text(encoding="utf-8")
                )
            except (OSError, UnicodeError, json.JSONDecodeError) as error:
                raise EvidenceError(
                    f"cannot parse differential JSON pair: {error}"
                ) from error
            if _public_json_shape(left_json) != _public_json_shape(right_json):
                raise EvidenceError("differential pair JSON public shapes differ")
    if ordering != sorted(ordering) or len(ordering) != len(set(ordering)):
        raise EvidenceError("differential pairs must be canonically ordered and unique")
    if surfaces != set(REQUIRED_DIFFERENTIAL_ARTIFACT_KINDS):
        raise EvidenceError(
            "differential pair manifest does not cover every privacy surface"
        )
    rooted_artifacts = {
        artifact.path
        for artifact in artifacts_by_path.values()
        if artifact.path.is_relative_to(left_root)
        or artifact.path.is_relative_to(right_root)
    }
    if rooted_artifacts != referenced_paths:
        raise EvidenceError(
            "differential roots contain an unpaired or undeclared archive artifact"
        )
    return bindings


def _validate_leakage_report(
    path: Path, artifacts: Sequence[Artifact], root: Path, commit: str
) -> None:
    """Require a clean differential bound to every archived capture byte."""

    try:
        report = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read leakage report: {error}") from error
    record = _exact_fields(
        report,
        {
            "version",
            "passed",
            "canary_manifest",
            "scanned_artifacts",
            "scanned_files",
            "scanned_bytes",
            "canary_names",
            "findings",
            "differential",
            "traffic_count_manifests",
            "traffic_count_mismatches",
        },
        "leakage_report",
    )
    if record["version"] != MANIFEST_VERSION or record["passed"] is not True:
        raise EvidenceError("leakage report must be a passing V1 report")
    scanned_files = record["scanned_files"]
    scanned_bytes = record["scanned_bytes"]
    if (
        isinstance(scanned_files, bool)
        or not isinstance(scanned_files, int)
        or scanned_files <= 0
        or isinstance(scanned_bytes, bool)
        or not isinstance(scanned_bytes, int)
        or scanned_bytes <= 0
    ):
        raise EvidenceError("leakage report must scan a non-empty artifact set")
    canary_names = record["canary_names"]
    if (
        not isinstance(canary_names, list)
        or any(not isinstance(name, str) for name in canary_names)
        or canary_names != sorted(set(canary_names))
        or not set(REQUIRED_LEAKAGE_CANARY_NAMES).issubset(canary_names)
    ):
        raise EvidenceError(
            "leakage report lacks account, asset, alias, amount, memo, or capsule canaries"
        )
    if record["findings"] != [] or record["traffic_count_mismatches"] != []:
        raise EvidenceError("leakage report contains a canary or traffic-count finding")

    declared_bindings = Counter(
        (artifact.sha256, artifact.bytes)
        for artifact in artifacts
        if artifact.kind != "leakage_report"
    )
    raw_scanned = record["scanned_artifacts"]
    if not isinstance(raw_scanned, list) or not raw_scanned:
        raise EvidenceError("leakage report must bind its scanned artifacts")
    scanned = [
        _parse_file_binding(value, f"leakage_report.scanned_artifacts[{index}]")
        for index, value in enumerate(raw_scanned)
    ]
    if scanned != sorted(scanned):
        raise EvidenceError("leakage report scanned bindings must be sorted")
    scanned_bindings = Counter(scanned)
    if scanned_bindings - declared_bindings:
        raise EvidenceError("leakage report scanned an unarchived artifact")
    if scanned_files != len(scanned) or scanned_bytes != sum(
        byte_count for _, byte_count in scanned
    ):
        raise EvidenceError("leakage report scan totals do not match its bindings")
    required_bindings = Counter(
        (artifact.sha256, artifact.bytes)
        for artifact in artifacts
        if artifact.kind in REQUIRED_LEAKAGE_ARTIFACT_KINDS
    )
    if required_bindings - scanned_bindings:
        raise EvidenceError(
            "leakage report did not scan every archived privacy surface"
        )

    pair_manifests = [
        artifact
        for artifact in artifacts
        if artifact.kind == "differential_pair_manifest"
    ]
    if len(pair_manifests) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one differential pair manifest"
        )
    pair_bindings = Counter(
        _validate_differential_pair_manifest(
            root.joinpath(*pair_manifests[0].path.parts),
            commit=commit,
            root=root,
            artifacts_by_path={artifact.path: artifact for artifact in artifacts},
        )
    )
    if pair_bindings - scanned_bindings:
        raise EvidenceError(
            "differential pair manifest references an unscanned artifact"
        )
    try:
        pair_document = json.loads(
            root.joinpath(*pair_manifests[0].path.parts).read_text(encoding="utf-8")
        )
        pair_left = _relative_path(
            pair_document["left_root"], "differential_pair_manifest.left_root"
        )
        pair_right = _relative_path(
            pair_document["right_root"], "differential_pair_manifest.right_root"
        )
    except (OSError, UnicodeError, json.JSONDecodeError, KeyError) as error:
        raise EvidenceError(f"cannot replay differential roots: {error}") from error
    recomputed_differential = _recompute_archived_differential(
        root.joinpath(*pair_left.parts), root.joinpath(*pair_right.parts)
    )

    canary_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "canary_manifest"
    ]
    if len(canary_artifacts) != 1:
        raise EvidenceError("evidence bundle must contain exactly one canary manifest")
    canary_binding = _parse_file_binding(
        record["canary_manifest"], "leakage_report.canary_manifest"
    )
    expected_canary = (canary_artifacts[0].sha256, canary_artifacts[0].bytes)
    if canary_binding != expected_canary:
        raise EvidenceError("leakage report used a different canary manifest")
    archived_canary_names = _load_canary_names(
        root.joinpath(*canary_artifacts[0].path.parts)
    )
    if canary_names != archived_canary_names:
        raise EvidenceError("leakage report canary names do not match its manifest")
    _verify_archived_canary_scan(
        root.joinpath(*canary_artifacts[0].path.parts), artifacts, root
    )

    count_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "traffic_count_manifest"
    ]
    if len(count_artifacts) != 2:
        raise EvidenceError(
            "evidence bundle must contain exactly two traffic-count manifests"
        )
    raw_count_bindings = record["traffic_count_manifests"]
    if not isinstance(raw_count_bindings, list) or len(raw_count_bindings) != 2:
        raise EvidenceError("leakage report must bind two traffic-count manifests")
    count_bindings = [
        _parse_file_binding(value, f"leakage_report.traffic_count_manifests[{index}]")
        for index, value in enumerate(raw_count_bindings)
    ]
    if count_bindings != sorted(count_bindings) or Counter(count_bindings) != Counter(
        (artifact.sha256, artifact.bytes) for artifact in count_artifacts
    ):
        raise EvidenceError(
            "leakage report traffic-count bindings do not match archive"
        )
    archived_counts: dict[str, dict[str, int]] = {}
    for artifact in count_artifacts:
        variant = next(
            (
                candidate
                for candidate in ("left", "right")
                if artifact.path.name == f"traffic-counts-{candidate}.json"
            ),
            None,
        )
        if variant is None or variant in archived_counts:
            raise EvidenceError("traffic-count manifest filenames are non-canonical")
        archived_counts[variant] = _load_traffic_count_manifest(
            root.joinpath(*artifact.path.parts)
        )
    if set(archived_counts) != {"left", "right"}:
        raise EvidenceError("traffic-count manifests omit a secret-only variant")
    if archived_counts["left"] != archived_counts["right"]:
        raise EvidenceError("archived differential traffic counts do not match")

    provenance_artifacts = [
        artifact
        for artifact in artifacts
        if artifact.kind == "leakage_capture_provenance"
    ]
    _validate_archived_leakage_provenance(
        provenance_artifacts=provenance_artifacts,
        artifacts_by_path={artifact.path: artifact for artifact in artifacts},
        root=root,
        commit=commit,
        variant_roots={"left": pair_left, "right": pair_right},
        archived_counts=archived_counts,
    )

    differential = _exact_fields(
        record["differential"],
        {
            "left_only",
            "right_only",
            "size_mismatches",
            "json_shape_mismatches",
            "packet_length_mismatches",
        },
        "leakage_report.differential",
    )
    if differential != recomputed_differential:
        raise EvidenceError("leakage differential report differs from archived replay")
    if any(differential.values()):
        raise EvidenceError("leakage report contains a public shape or size finding")


def _finite_nonnegative(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise EvidenceError(f"{label} must be numeric")
    rendered = float(value)
    if not math.isfinite(rendered) or rendered < 0:
        raise EvidenceError(f"{label} must be finite and non-negative")
    return rendered


def _load_benchmark_raw(
    paths: Sequence[Path],
    commit: str,
    hardware_sha256: str,
    hardware_profile_sha256: str,
    configuration_sha256_by_participants: dict[int, str],
) -> dict[tuple[str, int], dict[str, Any]]:
    """Validate the raw benchmark matrix retained in the publication bundle."""

    expected_fields = {
        "version",
        "protocol",
        "commit",
        "hardware_sha256",
        "hardware_profile_sha256",
        "configuration_sha256",
        "profile",
        "participants",
        "seed",
        "run",
        "warmup",
        "stages_ms",
        *_BENCHMARK_RESOURCE_FIELDS,
    }
    buckets: dict[tuple[str, int], dict[str, Any]] = {}
    identities: set[tuple[str, int, int, int, bool]] = set()
    for path in paths:
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeError) as error:
            raise EvidenceError(
                f"cannot read benchmark raw evidence: {error}"
            ) from error
        for line_number, line in enumerate(lines, 1):
            if not line.strip():
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise EvidenceError(
                    f"benchmark raw {path}:{line_number} is invalid JSON: {error}"
                ) from error
            row = _exact_fields(record, expected_fields, f"benchmark_raw:{line_number}")
            if (
                row["version"] != MANIFEST_VERSION
                or row["protocol"] != PROTOCOL
                or row["commit"] != commit
            ):
                raise EvidenceError(
                    "benchmark raw sample must bind the release protocol and commit"
                )
            profile = row["profile"]
            participants = row["participants"]
            seed = row["seed"]
            run = row["run"]
            warmup = row["warmup"]
            if (
                profile not in _BENCHMARK_PROFILES
                or participants not in REQUIRED_PARTICIPANTS
            ):
                raise EvidenceError(
                    "benchmark raw sample has unsupported profile or participants"
                )
            if (
                row["hardware_sha256"] != hardware_sha256
                or row["hardware_profile_sha256"] != hardware_profile_sha256
                or row["configuration_sha256"]
                != configuration_sha256_by_participants[participants]
            ):
                raise EvidenceError(
                    "benchmark raw sample used different hardware or configuration"
                )
            if (
                isinstance(seed, bool)
                or not isinstance(seed, int)
                or seed < 0
                or isinstance(run, bool)
                or not isinstance(run, int)
                or run < 0
                or not isinstance(warmup, bool)
            ):
                raise EvidenceError("benchmark raw sample identity is invalid")
            identity = (profile, participants, seed, run, warmup)
            if identity in identities:
                raise EvidenceError(f"duplicate benchmark raw identity {identity}")
            identities.add(identity)
            stages = row["stages_ms"]
            required_stages = (
                _BENCHMARK_PRIVATE_STAGES
                if profile == "private"
                else ("global_finality", "end_to_end")
            )
            if not isinstance(stages, dict) or set(stages) != set(required_stages):
                raise EvidenceError("benchmark raw stage set is invalid")
            for stage, value in stages.items():
                _finite_nonnegative(value, f"benchmark_raw.stages_ms.{stage}")
            for field in _BENCHMARK_RESOURCE_FIELDS:
                _finite_nonnegative(row[field], f"benchmark_raw.{field}")
            bucket = buckets.setdefault(
                (profile, participants), {"warmups": 0, "measured": 0, "seeds": set()}
            )
            if warmup:
                bucket["warmups"] += 1
            else:
                bucket["measured"] += 1
                bucket["seeds"].add(seed)
    expected_buckets = {
        (profile, participants)
        for profile in _BENCHMARK_PROFILES
        for participants in REQUIRED_PARTICIPANTS
    }
    if set(buckets) != expected_buckets:
        raise EvidenceError("benchmark raw matrix is incomplete")
    for key, bucket in buckets.items():
        if bucket["warmups"] < 5 or bucket["measured"] < 30 or len(bucket["seeds"]) < 2:
            raise EvidenceError(f"benchmark raw bucket {key} lacks required samples")
    return buckets


def _validate_statistical_summary(value: Any, expected_count: int, label: str) -> None:
    summary = _exact_fields(
        value,
        {"count", "mad", "p50", "p50_ci95", "p95", "p95_ci95", "p99", "p99_ci95"},
        label,
    )
    if summary["count"] != expected_count:
        raise EvidenceError(f"{label}.count does not match raw measured runs")
    for field in ("mad", "p50", "p95", "p99"):
        _finite_nonnegative(summary[field], f"{label}.{field}")
    for field in ("p50_ci95", "p95_ci95", "p99_ci95"):
        interval = summary[field]
        if not isinstance(interval, list) or len(interval) != 2:
            raise EvidenceError(f"{label}.{field} must be a two-value interval")
        low = _finite_nonnegative(interval[0], f"{label}.{field}[0]")
        high = _finite_nonnegative(interval[1], f"{label}.{field}[1]")
        if low > high:
            raise EvidenceError(f"{label}.{field} is reversed")


def _regenerate_benchmark_report(
    raw_paths: Sequence[Path], bootstrap_iterations: int
) -> dict[str, Any]:
    reporter_path = Path(__file__).with_name("private_settlement_benchmark_report.py")
    module_name = "_private_settlement_benchmark_report_for_release"
    spec = importlib.util.spec_from_file_location(module_name, reporter_path)
    if spec is None or spec.loader is None:
        raise EvidenceError("cannot load the strict benchmark reporter")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(module_name)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
        return module.build_report(module.load_jsonl(raw_paths), bootstrap_iterations)
    except Exception as error:
        raise EvidenceError(f"benchmark raw evidence is invalid: {error}") from error
    finally:
        if previous is None:
            del sys.modules[module_name]
        else:
            sys.modules[module_name] = previous


def _validate_benchmark_report(
    path: Path,
    raw: dict[tuple[str, int], dict[str, Any]],
    raw_paths: Sequence[Path],
    commit: str,
    hardware_sha256: str,
    hardware_profile_sha256: str,
    configuration_sha256_by_participants: dict[int, str],
) -> None:
    """Require a passing report whose sample identities match retained raw data."""

    try:
        report = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read benchmark report: {error}") from error
    record = _exact_fields(
        report,
        {
            "version",
            "protocol",
            "commit",
            "environment",
            "requirements",
            "profiles",
            "regressions",
            "passed",
        },
        "benchmark_report",
    )
    if (
        record["version"] != MANIFEST_VERSION
        or record["protocol"] != PROTOCOL
        or record["commit"] != commit
        or record["passed"] is not True
    ):
        raise EvidenceError("benchmark report must be a passing V1 report")
    if record["regressions"] != []:
        raise EvidenceError("benchmark report contains release regressions")
    environment = _exact_fields(
        record["environment"],
        {
            "hardware_sha256",
            "hardware_profile_sha256",
            "configuration_sha256_by_participants",
        },
        "benchmark_report.environment",
    )
    expected_configurations = {
        str(participants): configuration_sha256_by_participants[participants]
        for participants in REQUIRED_PARTICIPANTS
    }
    if (
        environment["hardware_sha256"] != hardware_sha256
        or environment["hardware_profile_sha256"] != hardware_profile_sha256
        or environment["configuration_sha256_by_participants"]
        != expected_configurations
    ):
        raise EvidenceError("benchmark report used different hardware or configs")
    requirements = _exact_fields(
        record["requirements"],
        {
            "participants",
            "minimum_warmups",
            "minimum_measured",
            "minimum_seeds",
            "bootstrap_iterations",
        },
        "benchmark_report.requirements",
    )
    _exact_list(
        requirements["participants"],
        REQUIRED_PARTICIPANTS,
        "benchmark_report.requirements.participants",
    )
    for field, minimum in (
        ("minimum_warmups", 5),
        ("minimum_measured", 30),
        ("minimum_seeds", 2),
        ("bootstrap_iterations", 100),
    ):
        value = requirements[field]
        if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
            raise EvidenceError(f"benchmark_report.requirements.{field} is too small")
    profiles = record["profiles"]
    if not isinstance(profiles, dict) or set(profiles) != set(_BENCHMARK_PROFILES):
        raise EvidenceError("benchmark report profiles are incomplete")
    for profile in _BENCHMARK_PROFILES:
        participant_rows = profiles[profile]
        expected_participants = {str(value) for value in REQUIRED_PARTICIPANTS}
        if (
            not isinstance(participant_rows, dict)
            or set(participant_rows) != expected_participants
        ):
            raise EvidenceError(
                f"benchmark report {profile} participant matrix is incomplete"
            )
        for participants in REQUIRED_PARTICIPANTS:
            label = f"benchmark_report.profiles.{profile}.{participants}"
            bucket = _exact_fields(
                participant_rows[str(participants)],
                {"measured_runs", "seeds", "stages_ms", "resources"},
                label,
            )
            raw_bucket = raw[(profile, participants)]
            if bucket["measured_runs"] != raw_bucket["measured"]:
                raise EvidenceError(
                    f"{label}.measured_runs does not match raw evidence"
                )
            expected_seeds = sorted(raw_bucket["seeds"])
            if bucket["seeds"] != expected_seeds:
                raise EvidenceError(f"{label}.seeds does not match raw evidence")
            stages = bucket["stages_ms"]
            required_stages = (
                _BENCHMARK_PRIVATE_STAGES
                if profile == "private"
                else ("global_finality", "end_to_end")
            )
            if not isinstance(stages, dict) or set(stages) != set(required_stages):
                raise EvidenceError(f"{label}.stages_ms is incomplete")
            for stage in required_stages:
                _validate_statistical_summary(
                    stages[stage], raw_bucket["measured"], f"{label}.stages_ms.{stage}"
                )
            resources = bucket["resources"]
            if not isinstance(resources, dict) or set(resources) != set(
                _BENCHMARK_RESOURCE_FIELDS
            ):
                raise EvidenceError(f"{label}.resources is incomplete")
            for field in _BENCHMARK_RESOURCE_FIELDS:
                _validate_statistical_summary(
                    resources[field],
                    raw_bucket["measured"],
                    f"{label}.resources.{field}",
                )

    regenerated = _regenerate_benchmark_report(
        raw_paths, requirements["bootstrap_iterations"]
    )
    for field in (
        "version",
        "protocol",
        "commit",
        "environment",
        "requirements",
        "profiles",
    ):
        if record[field] != regenerated[field]:
            raise EvidenceError(
                "benchmark report statistics do not match archived raw samples"
            )


def verify_bundle(manifest_path: Path) -> dict[str, Any]:
    """Verify a manifest, every declared artifact, and the exact file inventory."""

    root = manifest_path.parent.resolve(strict=True)
    document = _read_strict_json_file(
        manifest_path,
        maximum_bytes=_MAX_RELEASE_MANIFEST_BYTES,
        label="release manifest",
    )
    manifest, artifacts = parse_manifest(document)

    declared = {artifact.path.as_posix() for artifact in artifacts}
    actual: set[str] = set()
    total_bytes = 0
    for path in root.rglob("*"):
        if path.is_symlink():
            raise EvidenceError(f"evidence bundle must not contain symlinks: {path}")
        if path.is_file() and path.resolve() != manifest_path.resolve():
            actual.add(path.relative_to(root).as_posix())
    if actual != declared:
        raise EvidenceError(
            f"evidence file inventory mismatch; missing={sorted(declared - actual)} "
            f"unlisted={sorted(actual - declared)}"
        )

    for artifact in artifacts:
        path = root.joinpath(*artifact.path.parts)
        resolved = path.resolve(strict=True)
        if not resolved.is_relative_to(root) or not resolved.is_file():
            raise EvidenceError(
                f"artifact escapes bundle root or is not a file: {artifact.path}"
            )
        byte_count = resolved.stat().st_size
        if byte_count != artifact.bytes:
            raise EvidenceError(
                f"artifact byte count mismatch for {artifact.path}: "
                f"expected {artifact.bytes}, got {byte_count}"
            )
        digest = _sha256(resolved)
        if digest != artifact.sha256:
            raise EvidenceError(f"artifact SHA-256 mismatch for {artifact.path}")
        total_bytes += byte_count

    artifacts_by_path = {artifact.path: artifact for artifact in artifacts}
    artifact_kind_counts = Counter(artifact.kind for artifact in artifacts)
    for kind in _EXACT_ONE_SOURCE_ARTIFACT_KINDS:
        if artifact_kind_counts[kind] != 1:
            raise EvidenceError(f"evidence bundle must contain exactly one {kind}")
    hardware_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "hardware_description"
    ]
    if len(hardware_artifacts) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one hardware description"
        )
    hardware_profile_sha256 = _validate_hardware_description(
        root.joinpath(*hardware_artifacts[0].path.parts), commit=manifest["commit"]
    )
    configuration_manifests = [
        artifact for artifact in artifacts if artifact.kind == "configuration_manifest"
    ]
    if len(configuration_manifests) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one configuration manifest"
        )
    configuration_digests = _validate_configuration_manifest(
        root.joinpath(*configuration_manifests[0].path.parts),
        commit=manifest["commit"],
        artifacts_by_path=artifacts_by_path,
    )

    fault_reports = [
        artifact
        for artifact in artifacts
        if artifact.kind == "real_network_fault_report"
    ]
    fault_raw = [
        artifact for artifact in artifacts if artifact.kind == "real_network_fault_raw"
    ]
    if len(fault_reports) != 1 or not fault_raw:
        raise EvidenceError(
            "evidence bundle must contain one fault report and non-empty raw fault evidence"
        )
    _validate_fault_report(
        root.joinpath(*fault_reports[0].path.parts),
        raw_artifacts=fault_raw,
        artifacts=artifacts,
        root=root,
        commit=manifest["commit"],
        hardware_sha256=hardware_artifacts[0].sha256,
        configuration_sha256_by_participants=configuration_digests,
    )

    leakage_report_paths = [
        root.joinpath(*artifact.path.parts)
        for artifact in artifacts
        if artifact.kind == "leakage_report"
    ]
    if len(leakage_report_paths) != 1:
        raise EvidenceError("evidence bundle must contain exactly one leakage report")
    _validate_leakage_report(
        leakage_report_paths[0], artifacts, root, manifest["commit"]
    )

    source_manifests = [
        artifact for artifact in artifacts if artifact.kind == "source_manifest"
    ]
    if len(source_manifests) != 1:
        raise EvidenceError("evidence bundle must contain exactly one source manifest")
    source_manifest_artifact = source_manifests[0]
    (
        source_transcript,
        source_tree,
        source_tracked_file_count,
        workspace_manifest_sha256,
        source_archive_reference,
        source_commit_reference,
        source_lockfile_reference,
        source_path_list_reference,
    ) = _validate_source_manifest(
        root.joinpath(*source_manifest_artifact.path.parts),
        commit=manifest["commit"],
        expected_sha256=source_manifest_artifact.sha256,
        expected_bytes=source_manifest_artifact.bytes,
        artifacts_by_path=artifacts_by_path,
    )
    source_commit_artifact = artifacts_by_path[source_commit_reference]
    committed_tree = _validate_source_commit(
        root.joinpath(*source_commit_reference.parts),
        manifest["commit"],
        expected_sha256=source_commit_artifact.sha256,
        expected_bytes=source_commit_artifact.bytes,
    )
    if committed_tree != source_tree:
        raise EvidenceError("source manifest tree differs from the release commit")
    source_archive_path = root.joinpath(*source_archive_reference.parts)
    source_archive_artifact = artifacts_by_path[source_archive_reference]
    source_lockfile_artifact = artifacts_by_path[source_lockfile_reference]
    source_path_list_artifact = artifacts_by_path[source_path_list_reference]
    _read_stable_bounded_artifact(
        root.joinpath(*source_lockfile_reference.parts),
        maximum_bytes=_MAX_SOURCE_LOCKFILE_BYTES,
        expected_sha256=source_lockfile_artifact.sha256,
        expected_bytes=source_lockfile_artifact.bytes,
        label="source_lockfile",
    )
    source_path_list_path = root.joinpath(*source_path_list_reference.parts)

    gate_transcripts: list[PurePosixPath] = [source_transcript]
    formal_source_bindings: FormalSourceBindings | None = None
    for artifact_kind in PASS_REPORT_GATES:
        gate_artifacts = [
            artifact for artifact in artifacts if artifact.kind == artifact_kind
        ]
        if len(gate_artifacts) != 1:
            raise EvidenceError(
                f"evidence bundle must contain exactly one {artifact_kind}"
            )
        artifact = gate_artifacts[0]
        gate_transcript, gate_formal_source_bindings = _validate_pass_report(
            root.joinpath(*artifact.path.parts),
            artifact_kind=artifact_kind,
            commit=manifest["commit"],
            expected_sha256=artifact.sha256,
            expected_bytes=artifact.bytes,
            artifacts_by_path=artifacts_by_path,
            source_tree=source_tree,
            source_tracked_file_count=source_tracked_file_count,
            source_archive_path=source_archive_path,
            source_archive_sha256=source_archive_artifact.sha256,
            source_archive_bytes=source_archive_artifact.bytes,
            workspace_manifest_sha256=workspace_manifest_sha256,
            source_lockfile_sha256=source_lockfile_artifact.sha256,
            source_lockfile_bytes=source_lockfile_artifact.bytes,
            source_path_list_path=source_path_list_path,
            source_path_list_sha256=source_path_list_artifact.sha256,
            source_path_list_bytes=source_path_list_artifact.bytes,
        )
        gate_transcripts.append(gate_transcript)
        if gate_formal_source_bindings is not None:
            if formal_source_bindings is not None:
                raise EvidenceError("formal source package was validated more than once")
            formal_source_bindings = gate_formal_source_bindings
    if formal_source_bindings is None:
        raise EvidenceError("release inventory did not authenticate the formal source package")
    randomized_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "randomized_seed_report"
    ]
    if len(randomized_artifacts) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one randomized seed report"
        )
    gate_transcripts.append(
        _validate_randomized_seed_report(
            root.joinpath(*randomized_artifacts[0].path.parts),
            commit=manifest["commit"],
            minimum_seeds=manifest["qualification"]["randomized_seeds"],
            artifacts_by_path=artifacts_by_path,
        )
    )
    soak_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "soak_report"
    ]
    if len(soak_artifacts) != 1:
        raise EvidenceError("evidence bundle must contain exactly one soak report")
    gate_transcripts.append(
        _validate_soak_report(
            root.joinpath(*soak_artifacts[0].path.parts),
            commit=manifest["commit"],
            minimum_seconds=manifest["qualification"]["soak_seconds"],
            artifacts_by_path=artifacts_by_path,
        )
    )
    formal_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "formal_model_report"
    ]
    if len(formal_artifacts) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one formal model report"
        )
    gate_transcripts.append(
        _validate_formal_model_report(
            root.joinpath(*formal_artifacts[0].path.parts),
            root=root,
            commit=manifest["commit"],
            expected_sha256=formal_artifacts[0].sha256,
            expected_bytes=formal_artifacts[0].bytes,
            formal_source_bindings=formal_source_bindings,
            artifacts_by_path=artifacts_by_path,
        )
    )
    custody_artifacts = [
        artifact
        for artifact in artifacts
        if artifact.kind == "auditor_key_custody_report"
    ]
    if len(custody_artifacts) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one auditor key custody report"
        )
    gate_transcripts.append(
        _validate_auditor_key_custody_report(
            root.joinpath(*custody_artifacts[0].path.parts),
            commit=manifest["commit"],
            artifacts_by_path=artifacts_by_path,
        )
    )
    reproducible_artifacts = [
        artifact
        for artifact in artifacts
        if artifact.kind == "reproducible_build_report"
    ]
    if len(reproducible_artifacts) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one reproducible build report"
        )
    gate_transcripts.extend(
        _validate_reproducible_build_report(
            root.joinpath(*reproducible_artifacts[0].path.parts),
            commit=manifest["commit"],
            artifacts_by_path=artifacts_by_path,
        )
    )
    release_binaries = [
        artifact for artifact in artifacts if artifact.kind == "release_binary"
    ]
    sbom_artifacts = [artifact for artifact in artifacts if artifact.kind == "sbom"]
    if len(sbom_artifacts) != 1:
        raise EvidenceError("evidence bundle must contain exactly one SBOM")
    _validate_cyclonedx_sbom(
        root.joinpath(*sbom_artifacts[0].path.parts),
        commit=manifest["commit"],
        release_binaries=release_binaries,
    )
    audit_attestations = [
        artifact for artifact in artifacts if artifact.kind == "audit_attestation"
    ]
    if len(audit_attestations) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one audit attestation"
        )
    _validate_audit_attestation(
        root.joinpath(*audit_attestations[0].path.parts),
        commit=manifest["commit"],
        audit_manifest=manifest["independent_audit"],
        artifacts_by_path=artifacts_by_path,
    )
    if len(gate_transcripts) != len(set(gate_transcripts)):
        raise EvidenceError("release command gates must use distinct transcripts")

    benchmark_raw_paths = [
        root.joinpath(*artifact.path.parts)
        for artifact in artifacts
        if artifact.kind == "benchmark_raw"
    ]
    benchmark_report_paths = [
        root.joinpath(*artifact.path.parts)
        for artifact in artifacts
        if artifact.kind == "benchmark_report"
    ]
    if len(benchmark_report_paths) != 1:
        raise EvidenceError("evidence bundle must contain exactly one benchmark report")
    _validate_benchmark_report(
        benchmark_report_paths[0],
        _load_benchmark_raw(
            benchmark_raw_paths,
            manifest["commit"],
            hardware_artifacts[0].sha256,
            hardware_profile_sha256,
            configuration_digests,
        ),
        benchmark_raw_paths,
        manifest["commit"],
        hardware_artifacts[0].sha256,
        hardware_profile_sha256,
        configuration_digests,
    )

    canonical_manifest = json.dumps(
        manifest, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    bundle_binding = hashlib.sha256(
        canonical_manifest
        + b"\n"
        + b"\n".join(
            f"{artifact.path}\t{artifact.bytes}\t{artifact.sha256}".encode()
            for artifact in artifacts
        )
    ).hexdigest()
    return {
        "version": MANIFEST_VERSION,
        "protocol": PROTOCOL,
        "commit": manifest["commit"],
        "doi": manifest["doi"],
        "artifact_count": len(artifacts),
        "artifact_bytes": total_bytes,
        "bundle_binding_sha256": bundle_binding,
        "passed": True,
    }


def _write_report(report: dict[str, Any], output: Path | None) -> None:
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if output is None:
        sys.stdout.write(rendered)
    else:
        output.write_text(rendered, encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    """Run the release evidence validator."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path, help="path to release-manifest-v1.json")
    parser.add_argument("--output", type=Path, help="optional validation report path")
    args = parser.parse_args(argv)
    try:
        report = verify_bundle(args.manifest)
        _write_report(report, args.output)
    except (EvidenceError, OSError) as error:
        print(f"atomic private settlement evidence rejected: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
