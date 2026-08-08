#!/usr/bin/env python3
"""Assemble the source-bound Privacy v1 handoff consumed by BOI.

The input artifact handoff is inert data from an untrusted build job.  This
command is intended to run in the sealed, secret-free qualification
environment.  It independently re-admits the signed Taira candidate, requires
that candidate's distinct authenticated BOI inventory digest and bytes to name
the exact input inventory, replays the wheel and ABI-22 native validators, and
only then creates an immutable BOI directory.  The macOS build-handoff digest
is retained as qualification evidence but is never accepted as the BOI binding.

No signing key, wallet witness, network credential, or deployment endpoint is
accepted.  Missing release evidence is a hard failure; this command cannot
create a provisional or ``not available`` bundle.
"""

from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import NoReturn

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 compatibility.
    import tomli as tomllib

try:
    from . import check_native_sdk_abi22_artifact as abi22
    from . import taira_privacy_protocol_receipt as privacy_evidence
    from . import taira_release_authority as native_authority
    from . import taira_rollout_admission as admission
    from .release_artifact_contract import (
        ReleaseArtifactError,
        StableFile,
        canonical_json_bytes,
        exclusive_output_fd,
        exclusive_write_bytes,
        load_json_object,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_open_relative,
        stable_read_relative,
    )
except ImportError:
    import check_native_sdk_abi22_artifact as abi22
    import taira_privacy_protocol_receipt as privacy_evidence
    import taira_release_authority as native_authority
    import taira_rollout_admission as admission
    from release_artifact_contract import (
        ReleaseArtifactError,
        StableFile,
        canonical_json_bytes,
        exclusive_output_fd,
        exclusive_write_bytes,
        load_json_object,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_open_relative,
        stable_read_relative,
    )


SCHEMA = "iroha.privacy-v1.boi-handoff-inventory"
SCHEMA_VERSION = 1
SOURCE_HANDOFF_SCHEMA = admission.BOI_SOURCE_HANDOFF_SCHEMA
SOURCE_HANDOFF_KIND = admission.BOI_SOURCE_HANDOFF_KIND
SOURCE_HANDOFF_MANIFEST = admission.BOI_SOURCE_HANDOFF_MANIFEST
OUTPUT_INVENTORY = "boi-privacy-v1-inventory.json"
FIXED_CARGO_LOCK_SHA256 = (
    "cd9e829e454171f17540abeb7fd1aa14129252082bd8b076a0199b0ffa4e3f79"
)
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")

MAX_CAPABILITY_BYTES = 256 * 1024
MAX_WHEEL_BYTES = 1024 * 1024 * 1024
MAX_WORKER_BYTES = 512 * 1024 * 1024
MAX_ABI_LIBRARY_BYTES = 512 * 1024 * 1024
MAX_HEADER_BYTES = 4 * 1024 * 1024
MAX_SCHEMA_BYTES = 16 * 1024 * 1024
MAX_CONFIG_BYTES = 1024 * 1024
MAX_LOCK_BYTES = 32 * 1024 * 1024
MAX_MATRIX_BYTES = 1024 * 1024
MAX_NATIVE_RECEIPT_BYTES = 64 * 1024 * 1024
MAX_HANDOFF_MANIFEST_BYTES = 4 * 1024 * 1024
MAX_WHEEL_MEMBERS = 100_000
MAX_WHEEL_LOGICAL_BYTES = 2 * 1024 * 1024 * 1024

CAPABILITY_PATH = "capability/exact12-capability-manifest-v1.norito"
WHEEL_PATH = "sdk/iroha_python_privacy_v1.whl"
WORKER_PATH = "worker/iroha_privacy_wallet_worker"
ABI_LIBRARY_PATH = "abi22/libconnect_norito_bridge.so"
ABI_HEADER_PATH = "abi22/connect_norito_bridge.h"
ABI_SYMBOLS_PATH = "abi22/privacy-exports-v1.txt"
ABI_EVIDENCE_PATH = "abi22/native-artifact-v1.json"
CAPABILITY_SCHEMA_PATH = "schemas/exact12-capability-manifest-v1.json"
WORKER_SCHEMA_PATH = "schemas/privacy-wallet-ipc-v1.json"
CONFIG_PATH = "config/privacy-v1.example.toml"
CARGO_LOCK_PATH = "source/Cargo.lock"
MATRIX_PATH = "source/exact12-v1.tsv"
SOURCE_MANIFEST_PATH = "source/workspace-source-manifest.sha256"

CANDIDATE_ADMISSION_PATH = "provenance/candidate-admission-v1.json"
SOURCE_HANDOFF_COPY_PATH = "provenance/source-artifact-handoff-v1.json"
NATIVE_RECEIPT_NORITO_PATH = "receipts/native-release-receipt-v1.norito"
NATIVE_RECEIPT_JSON_PATH = "receipts/native-release-receipt-v1.json"
QUALIFICATION_RECEIPT_PATH = "receipts/four-peer-receipt-v2.json"
PROTOCOL_RECEIPT_PATH = "receipts/privacy-protocol-four-peer-receipt-v2.json"

CAPABILITY_SCHEMA_ID = "iroha://schemas/privacy/exact12-capability-manifest-v1"
WORKER_SCHEMA_ID = "iroha://schemas/privacy/ipww-v1"


@dataclass(frozen=True)
class ArtifactSpec:
    """One exact input-to-output artifact contract."""

    path: str
    role: str
    maximum: int
    executable: bool = False


ARTIFACT_SPECS = (
    ArtifactSpec(CAPABILITY_PATH, "exact12-capability-manifest", MAX_CAPABILITY_BYTES),
    ArtifactSpec(WHEEL_PATH, "native-python-wheel", MAX_WHEEL_BYTES),
    ArtifactSpec(WORKER_PATH, "ipww-native-worker", MAX_WORKER_BYTES, True),
    ArtifactSpec(ABI_LIBRARY_PATH, "abi22-native-library", MAX_ABI_LIBRARY_BYTES, True),
    ArtifactSpec(ABI_HEADER_PATH, "abi22-c-header", MAX_HEADER_BYTES),
    ArtifactSpec(ABI_SYMBOLS_PATH, "abi22-symbol-evidence", MAX_HEADER_BYTES),
    ArtifactSpec(
        ABI_EVIDENCE_PATH, "abi22-artifact-evidence", abi22.MAX_MANIFEST_BYTES
    ),
    ArtifactSpec(CAPABILITY_SCHEMA_PATH, "exact12-capability-schema", MAX_SCHEMA_BYTES),
    ArtifactSpec(WORKER_SCHEMA_PATH, "ipww-schema", MAX_SCHEMA_BYTES),
    ArtifactSpec(CONFIG_PATH, "sample-configuration", MAX_CONFIG_BYTES),
    ArtifactSpec(CARGO_LOCK_PATH, "canonical-cargo-lock", MAX_LOCK_BYTES),
    ArtifactSpec(MATRIX_PATH, "exact12-protocol-matrix", MAX_MATRIX_BYTES),
    ArtifactSpec(SOURCE_MANIFEST_PATH, "workspace-source-manifest", 65),
)
SOURCE_ARTIFACT_PATHS = tuple(spec.path for spec in ARTIFACT_SPECS)
if tuple(sorted(SOURCE_ARTIFACT_PATHS)) != admission.BOI_SOURCE_ARTIFACT_PATHS:
    raise RuntimeError("BOI assembler and signed-candidate inventories diverged")


class BoiHandoffError(RuntimeError):
    """The proposed BOI handoff is incomplete, mutable, or unauthenticated."""


@dataclass(frozen=True)
class AuthenticatedCandidate:
    """Evidence returned only after signed-candidate admission succeeds."""

    source: Mapping[str, object]
    artifact_handoff_sha256: str
    boi_artifact_inventory_sha256: str
    boi_artifact_inventory: bytes
    archive: Path
    archive_info: StableFile
    release_manifest_sha256: str
    native_validator_binary_sha256: str
    validator_binary_sha256: str
    exact12_matrix_sha256: str
    qualification_receipt_id: str
    privacy_protocol_receipt_id: str
    qualification_receipt: bytes
    privacy_protocol_receipt: bytes
    native_receipt_norito: bytes
    native_receipt_json: bytes


def _fail(message: str) -> NoReturn:
    raise BoiHandoffError(message)


def _sha256(value: object, label: str, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must not be all zero")
    return value


def _json_sha256(value: object, label: str) -> str:
    if isinstance(value, str):
        return _sha256(value, label)
    if (
        isinstance(value, list)
        and len(value) == 32
        and all(
            isinstance(item, int) and not isinstance(item, bool) and 0 <= item <= 255
            for item in value
        )
    ):
        return _sha256(bytes(value).hex(), label)
    _fail(f"{label} is not one canonical 32-byte digest")


def _commit(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or COMMIT_RE.fullmatch(value) is None
        or value == "0" * 40
    ):
        _fail(f"{label} must be one nonzero lowercase 40-hex object id")
    return value


def _canonical_directory(path: Path, label: str) -> Path:
    if not path.is_absolute():
        _fail(f"{label} must be an absolute path")
    try:
        resolved = path.resolve(strict=True)
        info = path.lstat()
    except OSError as exc:
        raise BoiHandoffError(f"cannot inspect {label}: {exc}") from exc
    if resolved != path or stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
        _fail(f"{label} must be one canonical non-symlink directory")
    if info.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
        _fail(f"{label} must not be group- or world-writable")
    return path


def _canonical_object(
    payload: bytes, label: str, *, compact: bool = False
) -> dict[str, object]:
    try:
        value = load_json_object(payload, label)
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(str(exc)) from exc
    rendered = (
        (
            json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
            + "\n"
        ).encode("ascii")
        if compact
        else canonical_json_bytes(value)
    )
    if rendered != payload:
        _fail(f"{label} is not canonical deterministic JSON")
    return value


def _source_identity(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != {
        "cargo_lock_sha256",
        "commit",
        "dpn_validator_release_commit",
        "workspace_source_manifest_sha256",
    }:
        _fail("candidate source identity fields are not exact")
    source = {
        "cargo_lock_sha256": _sha256(value["cargo_lock_sha256"], "Cargo.lock digest"),
        "commit": _commit(value["commit"], "source commit"),
        "dpn_validator_release_commit": _commit(
            value["dpn_validator_release_commit"], "DPN validator release commit"
        ),
        "workspace_source_manifest_sha256": _sha256(
            value["workspace_source_manifest_sha256"], "workspace source manifest"
        ),
    }
    if source["cargo_lock_sha256"] != FIXED_CARGO_LOCK_SHA256:
        _fail("candidate is not bound to the frozen Privacy v1 Cargo.lock")
    return source


def _read_candidate_native_receipts(nested_archive: Path) -> tuple[bytes, bytes]:
    prefix = nested_archive.name.removesuffix(".tar.gz")
    expected = {
        f"{prefix}/{native_authority.EVIDENCE_PATHS['receipt_norito']}": (
            "norito",
            MAX_NATIVE_RECEIPT_BYTES,
        ),
        f"{prefix}/{native_authority.EVIDENCE_PATHS['receipt_json']}": (
            "json",
            MAX_NATIVE_RECEIPT_BYTES,
        ),
    }
    captured: dict[str, bytes] = {}
    seen: set[str] = set()
    count = 0
    try:
        with tarfile.open(nested_archive, mode="r:gz") as archive:
            for member in archive:
                count += 1
                if count > native_authority.MAX_ARCHIVE_MEMBERS:
                    _fail(
                        "nested native release archive exceeds its member-count bound"
                    )
                name = member.name.removesuffix("/") if member.isdir() else member.name
                if name in seen:
                    _fail(f"nested native release archive repeats member {name!r}")
                seen.add(name)
                contract = expected.get(name)
                if contract is None:
                    continue
                if not member.isfile() or member.issparse() or member.size <= 0:
                    _fail(f"native release receipt is not a regular file: {name!r}")
                label, maximum = contract
                if member.size > maximum:
                    _fail(f"native release {label} receipt exceeds its byte bound")
                stream = archive.extractfile(member)
                if stream is None:
                    _fail(f"native release receipt cannot be read: {name!r}")
                payload = stream.read(member.size + 1)
                if len(payload) != member.size:
                    _fail(f"native release receipt is truncated: {name!r}")
                captured[label] = payload
    except (OSError, tarfile.TarError) as exc:
        raise BoiHandoffError(
            f"cannot inspect admitted native release receipts: {exc}"
        ) from exc
    if set(captured) != {"norito", "json"}:
        _fail("admitted candidate omits a native release receipt projection")
    return captured["norito"], captured["json"]


def authenticate_candidate(args: argparse.Namespace) -> AuthenticatedCandidate:
    """Re-admit the signed candidate and retain its authenticated receipts."""

    source = admission.SourceIdentity(
        _commit(args.expected_source_commit, "source commit"),
        _commit(args.expected_dpn_validator_release_commit, "DPN release commit"),
        _sha256(args.expected_cargo_lock_sha256, "Cargo.lock digest"),
        _sha256(args.expected_workspace_source_manifest_sha256, "source manifest"),
    )
    _source_identity(source.as_dict())
    archive = Path(os.path.abspath(args.candidate_archive))
    authority_dir = Path(os.path.abspath(args.candidate_authority_dir))
    replay_ledger = Path(os.path.abspath(args.candidate_replay_ledger))
    verifier = Path(os.path.abspath(args.release_manifest_verifier))
    archive_info = stable_hash_path(
        archive, max_size=native_authority.MAX_ARCHIVE_LOGICAL_BYTES
    )
    try:
        result = admission.verify_admission(
            archive_path=archive,
            authority_dir=authority_dir,
            expected_source=source,
            expected_receipt_id=_sha256(
                args.expected_receipt_id, "qualification receipt ID"
            ),
            replay_ledger_path=replay_ledger,
            trusted_signing_fingerprint=_sha256(
                args.trusted_signing_fingerprint, "release signer fingerprint"
            ),
            release_manifest_verifier_path=verifier,
            trusted_release_manifest_verifier_sha256=_sha256(
                args.trusted_release_manifest_verifier_sha256,
                "native release-manifest verifier digest",
            ),
            now_unix=args.now_unix,
        )
    except Exception as exc:
        raise BoiHandoffError(f"signed candidate admission failed: {exc}") from exc
    if result.get("verified") is not True or result.get("source") != source.as_dict():
        _fail("candidate admission did not return the exact verified source identity")

    with tempfile.TemporaryDirectory(prefix="privacy-v1-boi-candidate-") as raw:
        root = Path(raw).resolve(strict=True)
        try:
            inventory = admission._extract_final_archive(archive, archive_info, root)
            _, admission_payload = stable_read_relative(
                root,
                admission.ADMISSION_MANIFEST_PATH,
                max_size=admission.MAX_JSON_BYTES,
                return_payload=True,
            )
            _, qualification_payload = stable_read_relative(
                root,
                admission.MACOS_RECEIPT_PATH,
                max_size=admission.MAX_JSON_BYTES,
                return_payload=True,
            )
            _, protocol_payload = stable_read_relative(
                root,
                admission.PRIVACY_PROTOCOL_RECEIPT_PATH,
                max_size=privacy_evidence.MAX_RECEIPT_BYTES,
                return_payload=True,
            )
            _, boi_inventory_payload = stable_read_relative(
                root,
                admission.BOI_ARTIFACT_INVENTORY_PATH,
                max_size=admission.MAX_BOI_ARTIFACT_INVENTORY_BYTES,
                return_payload=True,
            )
        except (ReleaseArtifactError, admission.TairaRolloutAdmissionError) as exc:
            raise BoiHandoffError(
                f"cannot replay admitted candidate inventory: {exc}"
            ) from exc
        assert admission_payload is not None
        assert qualification_payload is not None
        assert protocol_payload is not None
        assert boi_inventory_payload is not None
        manifest = _canonical_object(admission_payload, "candidate admission manifest")
        linux = manifest.get("linux_arm64")
        if not isinstance(linux, dict) or not isinstance(
            linux.get("archive_path"), str
        ):
            _fail("candidate admission manifest omits its Linux archive path")
        linux_relative = str(linux["archive_path"])
        if linux_relative not in inventory:
            _fail("candidate admission inventory omits its Linux release archive")
        native_norito, native_json = _read_candidate_native_receipts(
            root / linux_relative
        )

    protocol = _canonical_object(protocol_payload, "privacy protocol receipt")
    protocol_candidate = protocol.get("candidate")
    if not isinstance(protocol_candidate, dict):
        _fail("privacy protocol receipt omits its candidate binding")
    matrix_sha256 = _sha256(
        protocol_candidate.get("exact12_matrix_sha256"), "Exact12 matrix digest"
    )
    try:
        native_receipt_value = load_json_object(
            native_json, "admitted native release JSON receipt"
        )
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(str(exc)) from exc
    candidate = AuthenticatedCandidate(
        source=_source_identity(result["source"]),
        artifact_handoff_sha256=_sha256(
            result.get("artifact_handoff_sha256"), "artifact handoff digest"
        ),
        boi_artifact_inventory_sha256=_sha256(
            result.get("boi_artifact_inventory_sha256"),
            "BOI artifact inventory digest",
        ),
        boi_artifact_inventory=boi_inventory_payload,
        archive=archive,
        archive_info=archive_info,
        release_manifest_sha256=_sha256(
            result.get("release_manifest_sha256"), "candidate release manifest digest"
        ),
        native_validator_binary_sha256=_json_sha256(
            native_receipt_value.get("validator_binary_sha256"),
            "native Linux validator binary digest",
        ),
        validator_binary_sha256=_sha256(
            result.get("validator_binary_sha256"), "validator binary digest"
        ),
        exact12_matrix_sha256=matrix_sha256,
        qualification_receipt_id=_sha256(
            result.get("receipt_id"), "qualification receipt ID"
        ),
        privacy_protocol_receipt_id=_sha256(
            result.get("privacy_protocol_receipt_id"), "privacy protocol receipt ID"
        ),
        qualification_receipt=qualification_payload,
        privacy_protocol_receipt=protocol_payload,
        native_receipt_norito=native_norito,
        native_receipt_json=native_json,
    )
    if stable_hash_path(archive) != archive_info:
        _fail("candidate archive changed while BOI admission evidence was captured")
    return candidate


def _validate_source_handoff(
    root: Path,
    *,
    source: Mapping[str, object],
    exact12_matrix_sha256: str,
    inventory_sha256: str,
    inventory_payload: bytes,
) -> tuple[dict[str, StableFile], bytes]:
    try:
        actual = scan_inventory_paths(root)
        manifest_info, manifest_payload = stable_read_relative(
            root,
            SOURCE_HANDOFF_MANIFEST,
            max_size=MAX_HANDOFF_MANIFEST_BYTES,
            return_payload=True,
        )
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(str(exc)) from exc
    assert manifest_payload is not None
    expected_paths = sorted([SOURCE_HANDOFF_MANIFEST, *SOURCE_ARTIFACT_PATHS])
    if actual != expected_paths:
        _fail("BOI source handoff inventory is not the exact first-release file set")
    if manifest_info.sha256 != inventory_sha256:
        _fail(
            "BOI source handoff is not the BOI inventory admitted by the candidate"
        )
    if manifest_payload != inventory_payload:
        _fail("BOI source handoff inventory bytes differ from the signed candidate")
    try:
        admission._validate_boi_artifact_inventory(
            manifest_payload,
            expected_source=admission.SourceIdentity(
                commit=str(source["commit"]),
                dpn_validator_release_commit=str(
                    source["dpn_validator_release_commit"]
                ),
                cargo_lock_sha256=str(source["cargo_lock_sha256"]),
                workspace_source_manifest_sha256=str(
                    source["workspace_source_manifest_sha256"]
                ),
            ),
            expected_exact12_matrix_sha256=exact12_matrix_sha256,
        )
    except admission.TairaRolloutAdmissionError as exc:
        raise BoiHandoffError(str(exc)) from exc
    manifest = _canonical_object(
        manifest_payload, "BOI source handoff manifest", compact=True
    )
    if set(manifest) != {"files", "kind", "schema", "schema_version"}:
        _fail("BOI source handoff manifest fields are not exact")
    if manifest != {
        "files": manifest["files"],
        "kind": SOURCE_HANDOFF_KIND,
        "schema": SOURCE_HANDOFF_SCHEMA,
        "schema_version": 1,
    }:
        _fail("BOI source handoff manifest identity is unsupported")
    rows = manifest["files"]
    if not isinstance(rows, list) or len(rows) != len(ARTIFACT_SPECS):
        _fail("BOI source handoff manifest has the wrong artifact count")
    specs = {spec.path: spec for spec in ARTIFACT_SPECS}
    captured: dict[str, StableFile] = {}
    row_paths: list[str] = []
    for row in rows:
        if not isinstance(row, dict) or set(row) != {"path", "sha256", "size"}:
            _fail("BOI source handoff artifact row fields are not exact")
        path = row["path"]
        if not isinstance(path, str) or path not in specs:
            _fail("BOI source handoff contains an unknown artifact path")
        if (
            not isinstance(row["sha256"], str)
            or SHA256_RE.fullmatch(row["sha256"]) is None
            or not isinstance(row["size"], int)
            or isinstance(row["size"], bool)
            or row["size"] <= 0
        ):
            _fail(f"BOI source handoff metadata is invalid for {path!r}")
        spec = specs[path]
        try:
            info = stable_hash_relative(root, path, max_size=spec.maximum)
        except ReleaseArtifactError as exc:
            raise BoiHandoffError(str(exc)) from exc
        if row["sha256"] != info.sha256 or row["size"] != info.size:
            _fail(f"BOI source handoff metadata differs for {path!r}")
        row_paths.append(path)
        captured[path] = info
    if row_paths != sorted(SOURCE_ARTIFACT_PATHS):
        _fail("BOI source handoff rows must be unique and sorted")
    return captured, manifest_payload


def _read_captured(root: Path, path: str, captured: Mapping[str, StableFile]) -> bytes:
    try:
        info, payload = stable_read_relative(
            root,
            path,
            max_size=captured[path].size,
            return_payload=True,
        )
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(str(exc)) from exc
    assert payload is not None
    if info != captured[path]:
        _fail(f"BOI source artifact changed after inventory capture: {path!r}")
    return payload


def _validate_elf_aarch64(payload: bytes, label: str) -> None:
    if len(payload) < 64 or payload[:4] != b"\x7fELF":
        _fail(f"{label} is not a 64-bit ELF binary")
    if payload[4:6] != b"\x02\x01" or payload[6] != 1:
        _fail(f"{label} is not canonical little-endian ELF64")
    file_type = int.from_bytes(payload[16:18], "little")
    machine = int.from_bytes(payload[18:20], "little")
    if file_type not in {2, 3} or machine != 183:
        _fail(f"{label} is not an executable/shared Linux aarch64 artifact")


def _validate_header(payload: bytes) -> None:
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise BoiHandoffError("ABI22 header is not UTF-8") from exc
    if not text.endswith("\n") or "\r" in text or "\0" in text:
        _fail("ABI22 header is not canonical LF-delimited text")
    exports = re.findall(r"\b(iroha_privacy_[a-z0-9_]+)\s*\(", text)
    if sorted(exports) != sorted(abi22.APPROVED_PRIVACY_C_EXPORTS):
        _fail("ABI22 header does not declare exactly the five approved privacy exports")


def _validate_symbols(payload: bytes) -> None:
    expected = "".join(f"{name}\n" for name in abi22.APPROVED_PRIVACY_C_EXPORTS).encode(
        "ascii"
    )
    if payload != expected:
        _fail("ABI22 symbol evidence is not the exact ordered five-export inventory")


def _validate_schema(payload: bytes, expected_id: str, label: str) -> None:
    value = _canonical_object(payload, label)
    required = {"$id", "$schema", "type"}
    if not required.issubset(value):
        _fail(f"{label} omits its JSON Schema identity")
    if (
        value["$id"] != expected_id
        or value["$schema"] != "https://json-schema.org/draft/2020-12/schema"
        or value["type"] != "object"
    ):
        _fail(f"{label} has the wrong first-release schema identity")


def _wheel_layout(root: Path, captured: Mapping[str, StableFile]) -> tuple[str, str]:
    try:
        with stable_open_relative(
            root, WHEEL_PATH, expected=captured[WHEEL_PATH]
        ) as descriptor:
            with os.fdopen(os.dup(descriptor), "rb") as stream:
                with zipfile.ZipFile(stream) as wheel:
                    infos = wheel.infolist()
                    if not infos or len(infos) > MAX_WHEEL_MEMBERS:
                        _fail("Python wheel has an invalid member count")
                    names: list[str] = []
                    logical = 0
                    for info in infos:
                        name = info.filename
                        pure = PurePosixPath(name)
                        if (
                            not name
                            or name.endswith("/")
                            or pure.is_absolute()
                            or any(part in {"", ".", ".."} for part in pure.parts)
                            or pure.as_posix() != name
                        ):
                            _fail("Python wheel contains a noncanonical member")
                        mode = info.external_attr >> 16
                        if stat.S_ISLNK(mode) or info.flag_bits & 0x1:
                            _fail("Python wheel contains a link or encrypted member")
                        if info.file_size <= 0 or info.file_size > MAX_WHEEL_BYTES:
                            _fail("Python wheel member violates its byte bound")
                        logical += info.file_size
                        if logical > MAX_WHEEL_LOGICAL_BYTES:
                            _fail("Python wheel exceeds its logical byte bound")
                        if (
                            info.compress_size == 0
                            or info.file_size > info.compress_size * 500
                        ):
                            _fail(
                                "Python wheel contains an excessive compression ratio"
                            )
                        names.append(name)
                    if names != list(dict.fromkeys(names)):
                        _fail("Python wheel contains duplicate members")
                    native = [
                        name
                        for name in names
                        if PurePosixPath(name).parent.as_posix() == "iroha_python"
                        and PurePosixPath(name).name.startswith("_crypto.")
                        and name.endswith(".so")
                    ]
                    if len(native) != 1:
                        _fail(
                            "Python wheel must contain exactly one native _crypto aarch64 module"
                        )
                    controller = "iroha_python/privacy_wallet_worker.py"
                    if controller not in names:
                        _fail(
                            "Python wheel omits the thin authenticated IPWW controller"
                        )
                    wheel_metadata = [
                        name for name in names if name.endswith(".dist-info/WHEEL")
                    ]
                    if len(wheel_metadata) != 1:
                        _fail(
                            "Python wheel must contain exactly one WHEEL metadata file"
                        )
                    metadata = wheel.read(wheel_metadata[0])
                    try:
                        metadata_text = metadata.decode("utf-8")
                    except UnicodeDecodeError as exc:
                        raise BoiHandoffError(
                            "Python WHEEL metadata is not UTF-8"
                        ) from exc
                    if (
                        "Root-Is-Purelib: false\n" not in metadata_text
                        or re.search(r"(?m)^Tag: .*aarch64$", metadata_text) is None
                    ):
                        _fail("Python wheel is not an aarch64 native wheel")
                    with wheel.open(native[0]) as native_stream:
                        native_header = native_stream.read(64)
                    _validate_elf_aarch64(native_header, "Python native module")
                    return native[0], controller
    except (OSError, ReleaseArtifactError, zipfile.BadZipFile, RuntimeError) as exc:
        if isinstance(exc, BoiHandoffError):
            raise
        raise BoiHandoffError(f"cannot inspect native Python wheel: {exc}") from exc


def _probe_native_wheel(
    root: Path,
    captured: Mapping[str, StableFile],
    native_member: str,
    capability_payload: bytes,
    python: str,
) -> None:
    source = r"""
import importlib.machinery
import importlib.util
import pathlib
import sys

extension = pathlib.Path(sys.argv[1])
archive = pathlib.Path(sys.argv[2]).read_bytes()
controller_path = pathlib.Path(sys.argv[3])
worker_path = pathlib.Path(sys.argv[4])
worker_sha256 = sys.argv[5]
name = "iroha_python._crypto"
loader = importlib.machinery.ExtensionFileLoader(name, str(extension))
spec = importlib.util.spec_from_loader(name, loader)
if spec is None:
    raise SystemExit("native extension has no import specification")
module = importlib.util.module_from_spec(spec)
loader.exec_module(module)
required = (
    "connect_norito_bridge_abi_version",
    "privacy_exact12_capability_manifest_v1",
    "privacy_validate_exact12_capability_manifest_v1",
)
if any(not callable(getattr(module, item, None)) for item in required):
    raise SystemExit("native wheel omits a required Privacy v1 function")
if module.connect_norito_bridge_abi_version() != 22:
    raise SystemExit("native wheel ABI is not exactly 22")
if module.privacy_validate_exact12_capability_manifest_v1(archive) != 0:
    raise SystemExit("native wheel rejected the Exact12 capability manifest")
if bytes(module.privacy_exact12_capability_manifest_v1()) != archive:
    raise SystemExit("native wheel compiled capability bytes differ")
controller_loader = importlib.machinery.SourceFileLoader(
    "iroha_privacy_wallet_worker_controller", str(controller_path)
)
controller_spec = importlib.util.spec_from_loader(
    "iroha_privacy_wallet_worker_controller", controller_loader
)
if controller_spec is None:
    raise SystemExit("privacy wallet controller has no import specification")
controller_module = importlib.util.module_from_spec(controller_spec)
sys.modules[controller_spec.name] = controller_module
controller_loader.exec_module(controller_module)
with controller_module.PrivacyWalletWorkerControllerV1(
    worker_path, expected_worker_sha256=worker_sha256
) as controller:
    controller.ping()
"""
    with tempfile.TemporaryDirectory(prefix="privacy-v1-boi-wheel-") as raw:
        temporary = Path(raw).resolve(strict=True)
        extension = temporary / PurePosixPath(native_member).name
        manifest = temporary / "exact12-capability-manifest-v1.norito"
        controller = temporary / "privacy_wallet_worker.py"
        worker = temporary / "iroha_privacy_wallet_worker"
        try:
            with stable_open_relative(
                root, WHEEL_PATH, expected=captured[WHEEL_PATH]
            ) as descriptor:
                with os.fdopen(os.dup(descriptor), "rb") as stream:
                    with zipfile.ZipFile(stream) as wheel:
                        with wheel.open(native_member) as member:
                            with exclusive_output_fd(extension, mode=0o755) as target:
                                total = 0
                                while chunk := member.read(1024 * 1024):
                                    total += len(chunk)
                                    if total > MAX_ABI_LIBRARY_BYTES:
                                        _fail(
                                            "native wheel module exceeds its extraction bound"
                                        )
                                    view = memoryview(chunk)
                                    while view:
                                        written = os.write(target, view)
                                        if written <= 0:
                                            _fail(
                                                "short write extracting native wheel module"
                                            )
                                        view = view[written:]
                                os.fsync(target)
                        controller_payload = wheel.read(
                            "iroha_python/privacy_wallet_worker.py"
                        )
                        if not 1 <= len(controller_payload) <= 4 * 1024 * 1024:
                            _fail(
                                "native wheel controller exceeds its extraction bound"
                            )
                        exclusive_write_bytes(
                            controller, controller_payload, mode=0o600
                        )
            _write_streamed(
                root,
                WORKER_PATH,
                worker,
                captured[WORKER_PATH],
                executable=True,
            )
            exclusive_write_bytes(manifest, capability_payload, mode=0o600)
        except (OSError, ReleaseArtifactError, zipfile.BadZipFile) as exc:
            raise BoiHandoffError(f"cannot stage native wheel probe: {exc}") from exc
        environment = {"PATH": os.defpath, "PYTHONHASHSEED": "0"}
        if os.name == "nt":
            for name in ("SYSTEMROOT", "WINDIR"):
                if value := os.environ.get(name):
                    environment[name] = value
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            try:
                result = subprocess.run(
                    [
                        python,
                        "-I",
                        "-S",
                        "-c",
                        source,
                        str(extension),
                        str(manifest),
                        str(controller),
                        str(worker),
                        captured[WORKER_PATH].sha256,
                    ],
                    cwd=temporary,
                    env=environment,
                    stdin=subprocess.DEVNULL,
                    stdout=stdout,
                    stderr=stderr,
                    check=False,
                    timeout=120,
                )
            except (OSError, subprocess.TimeoutExpired) as exc:
                raise BoiHandoffError(
                    f"native wheel probe could not complete: {exc}"
                ) from exc
            if result.returncode != 0:
                stderr.seek(0, os.SEEK_END)
                size = stderr.tell()
                if size > 64 * 1024:
                    _fail("native wheel probe emitted excessive diagnostic output")
                stderr.seek(0)
                detail = stderr.read().decode("utf-8", "replace").strip()
                _fail("native wheel probe failed" + (f": {detail}" if detail else ""))


def _validate_abi_runtime(path: Path) -> None:
    try:
        version = abi22.probe_artifact("c-jni", path)
        symbols = abi22.inspect_exported_symbols(path, required=True)
        exports = abi22.validate_privacy_c_exports(
            () if symbols is None else symbols, require_exact=True
        )
    except abi22.ArtifactContractError as exc:
        raise BoiHandoffError(f"ABI22 native replay failed: {exc}") from exc
    if version != 22 or exports != abi22.APPROVED_PRIVACY_C_EXPORTS:
        _fail("ABI22 native replay did not return the exact approved surface")


def _validate_sample_config(payload: bytes, captured: Mapping[str, StableFile]) -> None:
    try:
        value = tomllib.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise BoiHandoffError("BOI sample configuration is not canonical TOML") from exc
    if set(value) != {"privacy_v1"} or not isinstance(value["privacy_v1"], dict):
        _fail("BOI sample configuration must contain only [privacy_v1]")
    expected = {
        "abi22_library": ABI_LIBRARY_PATH,
        "abi22_sha256": captured[ABI_LIBRARY_PATH].sha256,
        "capability_manifest": CAPABILITY_PATH,
        "network_availability_source": "torii-committed-capability-manifest",
        "python_wheel": WHEEL_PATH,
        "python_wheel_sha256": captured[WHEEL_PATH].sha256,
        "witness_crosses_ffi": False,
        "worker": WORKER_PATH,
        "worker_sha256": captured[WORKER_PATH].sha256,
    }
    if value["privacy_v1"] != expected:
        _fail("BOI sample configuration does not bind the exact admitted artifacts")


def _validate_artifacts(
    root: Path,
    captured: Mapping[str, StableFile],
    *,
    source: Mapping[str, object],
    exact12_matrix_sha256: str,
    python: str,
    wheel_probe: Callable[[Path, Mapping[str, StableFile], str, bytes, str], None],
    abi_runtime_validator: Callable[[Path], None],
) -> None:
    source = _source_identity(source)
    cargo = _read_captured(root, CARGO_LOCK_PATH, captured)
    if hashlib.sha256(cargo).hexdigest() != FIXED_CARGO_LOCK_SHA256:
        _fail("BOI Cargo.lock bytes do not match the frozen release lock")
    source_manifest = _read_captured(root, SOURCE_MANIFEST_PATH, captured)
    expected_source_line = (
        str(source["workspace_source_manifest_sha256"]) + "\n"
    ).encode("ascii")
    if source_manifest != expected_source_line:
        _fail("BOI source-manifest file differs from the admitted candidate")

    matrix = _read_captured(root, MATRIX_PATH, captured)
    if hashlib.sha256(matrix).hexdigest() != exact12_matrix_sha256:
        _fail("BOI protocol matrix differs from the admitted qualification receipt")
    try:
        native_authority._validate_exact12_matrix(matrix)
    except native_authority.TairaReleaseAuthorityError as exc:
        raise BoiHandoffError(str(exc)) from exc

    capability = _read_captured(root, CAPABILITY_PATH, captured)
    if len(capability) < 16 or capability[:4] != b"NRT0" or not any(capability[4:]):
        _fail("Exact12 capability manifest is not a nonempty canonical Norito archive")

    header = _read_captured(root, ABI_HEADER_PATH, captured)
    symbols = _read_captured(root, ABI_SYMBOLS_PATH, captured)
    _validate_header(header)
    _validate_symbols(symbols)
    abi_manifest_path = root / ABI_EVIDENCE_PATH
    try:
        abi_manifest = abi22.load_manifest(abi_manifest_path)
    except abi22.ArtifactContractError as exc:
        raise BoiHandoffError(f"ABI22 evidence is invalid: {exc}") from exc
    if (
        abi_manifest["sdk"] != "c-jni"
        or abi_manifest["source_commit"] != source["commit"]
        or abi_manifest["workspace_source_manifest_sha256"]
        != source["workspace_source_manifest_sha256"]
        or abi_manifest["artifact_sha256"] != captured[ABI_LIBRARY_PATH].sha256
        or abi_manifest["artifact_size"] != captured[ABI_LIBRARY_PATH].size
        or abi_manifest["privacy_c_exports"] != list(abi22.APPROVED_PRIVACY_C_EXPORTS)
        or abi_manifest["privacy_c_exports_inspected"] is not True
    ):
        _fail("ABI22 evidence is not bound to the admitted source and native library")
    _validate_elf_aarch64(
        _read_captured(root, ABI_LIBRARY_PATH, captured)[:64], "ABI22 native library"
    )
    abi_runtime_validator(root / ABI_LIBRARY_PATH)
    if stable_hash_relative(root, ABI_LIBRARY_PATH) != captured[ABI_LIBRARY_PATH]:
        _fail("ABI22 native library changed during replay")

    _validate_elf_aarch64(
        _read_captured(root, WORKER_PATH, captured)[:64], "IPWW native worker"
    )
    native_member, _controller = _wheel_layout(root, captured)
    wheel_probe(root, captured, native_member, capability, python)
    if stable_hash_relative(root, WHEEL_PATH) != captured[WHEEL_PATH]:
        _fail("native Python wheel changed during replay")
    if stable_hash_relative(root, WORKER_PATH) != captured[WORKER_PATH]:
        _fail("IPWW native worker changed during replay")

    _validate_schema(
        _read_captured(root, CAPABILITY_SCHEMA_PATH, captured),
        CAPABILITY_SCHEMA_ID,
        "Exact12 capability JSON Schema",
    )
    _validate_schema(
        _read_captured(root, WORKER_SCHEMA_PATH, captured),
        WORKER_SCHEMA_ID,
        "IPWW JSON Schema",
    )
    _validate_sample_config(_read_captured(root, CONFIG_PATH, captured), captured)


def _write_streamed(
    source_root: Path,
    relative: str,
    target: Path,
    expected: StableFile,
    *,
    executable: bool,
) -> None:
    target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    mode = 0o755 if executable else 0o600
    try:
        with stable_open_relative(source_root, relative, expected=expected) as source:
            with exclusive_output_fd(target, mode=mode) as destination:
                digest = hashlib.sha256()
                total = 0
                while chunk := os.read(source, 1024 * 1024):
                    digest.update(chunk)
                    total += len(chunk)
                    view = memoryview(chunk)
                    while view:
                        written = os.write(destination, view)
                        if written <= 0:
                            _fail(f"short write while installing {relative!r}")
                        view = view[written:]
                os.fsync(destination)
                if digest.hexdigest() != expected.sha256 or total != expected.size:
                    _fail(f"source bytes changed while installing {relative!r}")
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(str(exc)) from exc


def _publish_directory_noreplace(
    staging: Path,
    destination: Path,
    parent_fd: int,
) -> None:
    """Atomically publish a sibling directory without replacing a raced path."""

    if (
        staging.parent != destination.parent
        or not staging.name
        or not destination.name
        or Path(staging.name).name != staging.name
        or Path(destination.name).name != destination.name
    ):
        _fail("BOI publication paths must be canonical siblings")
    library = ctypes.CDLL(None, use_errno=True)
    if sys.platform == "darwin" and hasattr(library, "renameatx_np"):
        rename = library.renameatx_np
        flag = 0x00000004  # RENAME_EXCL
    elif sys.platform.startswith("linux") and hasattr(library, "renameat2"):
        rename = library.renameat2
        flag = 0x00000001  # RENAME_NOREPLACE
    else:
        _fail("atomic no-replace BOI publication is unavailable on this platform")
    rename.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    ]
    rename.restype = ctypes.c_int
    result = rename(
        parent_fd,
        os.fsencode(staging.name),
        parent_fd,
        os.fsencode(destination.name),
        flag,
    )
    if result == 0:
        return
    error_number = ctypes.get_errno()
    if error_number in {errno.EEXIST, errno.ENOTEMPTY}:
        _fail("BOI output appeared before exclusive atomic publication")
    raise OSError(error_number, os.strerror(error_number), os.fspath(destination))


def _candidate_admission_payload(candidate: AuthenticatedCandidate) -> bytes:
    return canonical_json_bytes(
        {
            "artifact_handoff_sha256": candidate.artifact_handoff_sha256,
            "boi_artifact_inventory_sha256": (
                candidate.boi_artifact_inventory_sha256
            ),
            "candidate_archive_sha256": candidate.archive_info.sha256,
            "exact12_matrix_sha256": candidate.exact12_matrix_sha256,
            "privacy_protocol_receipt_id": candidate.privacy_protocol_receipt_id,
            "qualification_receipt_id": candidate.qualification_receipt_id,
            "release_manifest_sha256": candidate.release_manifest_sha256,
            "schema": "iroha.privacy-v1.boi-candidate-admission",
            "schema_version": 1,
            "native_validator_binary_sha256": candidate.native_validator_binary_sha256,
            "source": _source_identity(candidate.source),
            "validator_binary_sha256": candidate.validator_binary_sha256,
            "verified": True,
        }
    )


def _validate_candidate_receipts(candidate: AuthenticatedCandidate) -> None:
    qualification = _canonical_object(
        candidate.qualification_receipt, "admitted four-peer qualification receipt"
    )
    if (
        qualification.get("schema") != admission.MACOS_RECEIPT_SCHEMA
        or qualification.get("schema_version") != admission.MACOS_RECEIPT_SCHEMA_VERSION
        or qualification.get("source") != candidate.source
        or qualification.get("receipt_id") != candidate.qualification_receipt_id
        or qualification.get("artifact_handoff_sha256")
        != candidate.artifact_handoff_sha256
        or qualification.get("validator_binary_sha256")
        != candidate.validator_binary_sha256
    ):
        _fail("four-peer qualification receipt differs from admitted candidate")

    protocol = _canonical_object(
        candidate.privacy_protocol_receipt, "admitted Exact12 four-peer receipt"
    )
    binding = protocol.get("candidate")
    if (
        protocol.get("schema") != privacy_evidence.RECEIPT_SCHEMA
        or protocol.get("schema_version") != privacy_evidence.RECEIPT_SCHEMA_VERSION
        or protocol.get("receipt_id") != candidate.privacy_protocol_receipt_id
        or not isinstance(binding, dict)
        or binding.get("source") != candidate.source
        or binding.get("artifact_handoff_sha256") != candidate.artifact_handoff_sha256
        or binding.get("validator_binary_sha256") != candidate.validator_binary_sha256
        or binding.get("exact12_matrix_sha256") != candidate.exact12_matrix_sha256
    ):
        _fail("Exact12 four-peer receipt differs from admitted candidate")
    if (
        len(candidate.native_receipt_norito) < 16
        or candidate.native_receipt_norito[:4] != b"NRT0"
    ):
        _fail("admitted native release receipt is not a Norito archive")
    try:
        native_json = load_json_object(
            candidate.native_receipt_json, "admitted native release JSON receipt"
        )
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(str(exc)) from exc
    expected_native_fields = {
        "all_native_stages_passed",
        "build_profile",
        "cargo_lock_sha256",
        "command_manifest",
        "contains_canonical_proof_artifacts",
        "contains_witnesses",
        "exact12_matrix_sha256",
        "expectations",
        "fixed_stage_count",
        "isolation_policy_enforced",
        "runner_binary_sha256",
        "schema_version",
        "source_sha256",
        "stage_artifacts",
        "validator_binary_sha256",
        "x509_resource",
    }
    if set(native_json) != expected_native_fields:
        _fail("admitted native release JSON receipt fields are not exact")

    if (
        native_json["schema_version"] != 1
        or native_json["build_profile"] != "release"
        or native_json["fixed_stage_count"] != 48
        or native_json["all_native_stages_passed"] is not True
        or native_json["contains_witnesses"] is not False
        or native_json["contains_canonical_proof_artifacts"] is not True
        or native_json["isolation_policy_enforced"] is not True
        or _json_sha256(native_json["source_sha256"], "native receipt source digest")
        != candidate.source["workspace_source_manifest_sha256"]
        or _json_sha256(
            native_json["cargo_lock_sha256"], "native receipt Cargo.lock digest"
        )
        != candidate.source["cargo_lock_sha256"]
        or _json_sha256(
            native_json["exact12_matrix_sha256"], "native receipt matrix digest"
        )
        != candidate.exact12_matrix_sha256
        or _json_sha256(
            native_json["validator_binary_sha256"], "native receipt validator digest"
        )
        != candidate.native_validator_binary_sha256
    ):
        _fail("native release receipt differs from the admitted source or candidate")
    _json_sha256(native_json["runner_binary_sha256"], "native receipt runner digest")
    for name in (
        "command_manifest",
        "expectations",
        "stage_artifacts",
        "x509_resource",
    ):
        pair = native_json[name]
        if not isinstance(pair, dict) or set(pair) != {"json_sha256", "norito_sha256"}:
            _fail(f"native release receipt {name} digest pair is malformed")
        _json_sha256(pair["json_sha256"], f"native receipt {name} JSON digest")
        _json_sha256(pair["norito_sha256"], f"native receipt {name} Norito digest")


def validate_candidate_boi_artifact_handoff(
    artifact_root: Path,
    *,
    source: Mapping[str, object],
    exact12_matrix_sha256: str,
    inventory_sha256: str,
    inventory_payload: bytes,
) -> dict[str, StableFile]:
    """Perform the candidate authority's platform-independent BOI rebind.

    Runtime loading remains the Linux qualification authority's job.  Before a
    candidate can be signed, this pass nevertheless validates the exact source,
    Cargo.lock, Exact12 matrix, ELF identities, wheel layout, ABI evidence,
    schemas, configuration, and every byte digest in the closed inventory.
    """

    artifact_root = _canonical_directory(
        artifact_root, "candidate BOI source handoff"
    )
    normalized_source = _source_identity(source)
    normalized_matrix = _sha256(exact12_matrix_sha256, "Exact12 matrix digest")
    normalized_inventory = _sha256(inventory_sha256, "BOI inventory digest")
    if hashlib.sha256(inventory_payload).hexdigest() != normalized_inventory:
        _fail("candidate BOI inventory bytes differ from their bound digest")
    captured, _ = _validate_source_handoff(
        artifact_root,
        source=normalized_source,
        exact12_matrix_sha256=normalized_matrix,
        inventory_sha256=normalized_inventory,
        inventory_payload=inventory_payload,
    )
    _validate_artifacts(
        artifact_root,
        captured,
        source=normalized_source,
        exact12_matrix_sha256=normalized_matrix,
        python=sys.executable,
        wheel_probe=lambda *_args: None,
        abi_runtime_validator=lambda _path: None,
    )
    return captured


def assemble_boi_handoff(
    artifact_root: Path,
    output: Path,
    candidate: AuthenticatedCandidate,
    *,
    python: str = sys.executable,
    wheel_probe: Callable[
        [Path, Mapping[str, StableFile], str, bytes, str], None
    ] = _probe_native_wheel,
    abi_runtime_validator: Callable[[Path], None] = _validate_abi_runtime,
) -> dict[str, object]:
    """Validate every input and create one closed, immutable BOI directory."""

    artifact_root = _canonical_directory(artifact_root, "BOI source handoff")
    source = _source_identity(candidate.source)
    if candidate.archive_info.sha256 == "0" * 64 or candidate.archive_info.size <= 0:
        _fail("BOI handoff requires one nonempty admitted candidate archive")
    _validate_candidate_receipts(candidate)
    captured, source_manifest_payload = _validate_source_handoff(
        artifact_root,
        source=candidate.source,
        exact12_matrix_sha256=candidate.exact12_matrix_sha256,
        inventory_sha256=candidate.boi_artifact_inventory_sha256,
        inventory_payload=candidate.boi_artifact_inventory,
    )
    _validate_artifacts(
        artifact_root,
        captured,
        source=candidate.source,
        exact12_matrix_sha256=candidate.exact12_matrix_sha256,
        python=python,
        wheel_probe=wheel_probe,
        abi_runtime_validator=abi_runtime_validator,
    )
    if not output.is_absolute():
        _fail("BOI output directory must be an absolute path")
    if output.exists() or output.is_symlink():
        _fail("BOI output directory must be fresh")

    admission_payload = _candidate_admission_payload(candidate)
    generated = {
        CANDIDATE_ADMISSION_PATH: admission_payload,
        SOURCE_HANDOFF_COPY_PATH: source_manifest_payload,
        NATIVE_RECEIPT_NORITO_PATH: candidate.native_receipt_norito,
        NATIVE_RECEIPT_JSON_PATH: candidate.native_receipt_json,
        QUALIFICATION_RECEIPT_PATH: candidate.qualification_receipt,
        PROTOCOL_RECEIPT_PATH: candidate.privacy_protocol_receipt,
    }
    if any(not payload for payload in generated.values()):
        _fail("admitted candidate contains an empty required receipt")

    parent = _canonical_directory(output.parent, "BOI output parent")
    try:
        temporary = tempfile.TemporaryDirectory(
            prefix=f".{output.name}.pending-", dir=parent
        )
    except OSError as exc:
        raise BoiHandoffError(
            f"cannot create private BOI staging directory: {exc}"
        ) from exc
    with temporary as raw_stage:
        install_root = Path(raw_stage).resolve(strict=True)
        try:
            install_root.chmod(0o700)
            for spec in ARTIFACT_SPECS:
                _write_streamed(
                    artifact_root,
                    spec.path,
                    install_root / spec.path,
                    captured[spec.path],
                    executable=spec.executable,
                )
            for relative, payload in sorted(generated.items()):
                target = install_root / relative
                target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                exclusive_write_bytes(target, payload, mode=0o600)

            roles = {spec.path: spec.role for spec in ARTIFACT_SPECS}
            roles.update(
                {
                    CANDIDATE_ADMISSION_PATH: "candidate-admission",
                    SOURCE_HANDOFF_COPY_PATH: "source-artifact-inventory",
                    NATIVE_RECEIPT_NORITO_PATH: "native-release-receipt-authoritative",
                    NATIVE_RECEIPT_JSON_PATH: "native-release-receipt-projection",
                    QUALIFICATION_RECEIPT_PATH: "four-peer-qualification-receipt",
                    PROTOCOL_RECEIPT_PATH: "exact12-four-peer-receipt",
                }
            )
            rows = []
            for relative in sorted(roles):
                info = stable_hash_relative(install_root, relative)
                rows.append(
                    {
                        "path": relative,
                        "role": roles[relative],
                        "sha256": info.sha256,
                        "size": info.size,
                    }
                )
            inventory = {
                "artifacts": rows,
                "candidate": {
                    "archive_sha256": candidate.archive_info.sha256,
                    "artifact_handoff_sha256": candidate.artifact_handoff_sha256,
                    "boi_artifact_inventory_sha256": (
                        candidate.boi_artifact_inventory_sha256
                    ),
                    "exact12_matrix_sha256": candidate.exact12_matrix_sha256,
                    "linux_validator_binary_sha256": (
                        candidate.native_validator_binary_sha256
                    ),
                    "macos_validator_binary_sha256": candidate.validator_binary_sha256,
                    "privacy_protocol_receipt_id": candidate.privacy_protocol_receipt_id,
                    "qualification_receipt_id": candidate.qualification_receipt_id,
                    "release_manifest_sha256": candidate.release_manifest_sha256,
                    "validator_binary_sha256": candidate.validator_binary_sha256,
                },
                "contract": {
                    "abi_version": 22,
                    "availability_source": "torii-committed-capability-manifest",
                    "jindo_assurance": "available-experimental",
                    "jindo_missing_evidence": (
                        "MissingDistributionWideKnowledgeSoundnessEvidence"
                    ),
                    "privacy_c_exports": list(abi22.APPROVED_PRIVACY_C_EXPORTS),
                    "protocol_count": 12,
                    "wallet_bundle_wire": "IPWB/1",
                    "wallet_worker_wire": "IPWW/1",
                    "witness_crosses_ffi": False,
                },
                "ready": True,
                "schema": SCHEMA,
                "schema_version": SCHEMA_VERSION,
                "source": source,
            }
            inventory_payload = canonical_json_bytes(inventory)
            exclusive_write_bytes(
                install_root / OUTPUT_INVENTORY, inventory_payload, mode=0o600
            )
            expected = sorted([OUTPUT_INVENTORY, *roles])
            if scan_inventory_paths(install_root) != expected:
                _fail(
                    "assembled BOI directory does not have the exact closed inventory"
                )
            for row in rows:
                info = stable_hash_relative(install_root, str(row["path"]))
                if info.sha256 != row["sha256"] or info.size != row["size"]:
                    _fail(f"assembled BOI artifact changed: {row['path']!r}")
            if (
                stable_hash_relative(install_root, OUTPUT_INVENTORY).sha256
                != hashlib.sha256(inventory_payload).hexdigest()
            ):
                _fail("assembled BOI inventory changed after installation")
            if stable_hash_path(candidate.archive) != candidate.archive_info:
                _fail("candidate archive changed while the BOI handoff was assembled")
            for current, directories, files in os.walk(install_root, topdown=False):
                current_path = Path(current)
                for name in files:
                    path = current_path / name
                    executable = path.relative_to(install_root).as_posix() in {
                        WORKER_PATH,
                        ABI_LIBRARY_PATH,
                    }
                    path.chmod(0o555 if executable else 0o444)
                for name in directories:
                    (current_path / name).chmod(0o555)
            install_root.chmod(0o555)
            staging_stat = os.stat(install_root, follow_symlinks=False)
            parent_path_stat = os.stat(parent, follow_symlinks=False)
            parent_fd = os.open(
                parent,
                os.O_RDONLY
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_DIRECTORY", 0)
                | getattr(os, "O_NOFOLLOW", 0),
            )
            try:
                opened_parent_stat = os.fstat(parent_fd)
                if not stat.S_ISDIR(opened_parent_stat.st_mode) or (
                    opened_parent_stat.st_dev,
                    opened_parent_stat.st_ino,
                ) != (parent_path_stat.st_dev, parent_path_stat.st_ino):
                    _fail("BOI output parent changed before publication")
                _publish_directory_noreplace(install_root, output, parent_fd)
                published_stat = os.stat(
                    output.name, dir_fd=parent_fd, follow_symlinks=False
                )
                if not stat.S_ISDIR(published_stat.st_mode) or (
                    published_stat.st_dev,
                    published_stat.st_ino,
                ) != (staging_stat.st_dev, staging_stat.st_ino):
                    _fail("published BOI directory identity changed")
                os.fsync(parent_fd)
                closed_parent_stat = os.stat(parent, follow_symlinks=False)
                if (closed_parent_stat.st_dev, closed_parent_stat.st_ino) != (
                    opened_parent_stat.st_dev,
                    opened_parent_stat.st_ino,
                ):
                    _fail("BOI output parent changed during publication")
            finally:
                os.close(parent_fd)
        except (OSError, ReleaseArtifactError) as exc:
            raise BoiHandoffError(f"cannot install BOI handoff: {exc}") from exc
    return {
        "candidate_archive_sha256": candidate.archive_info.sha256,
        "inventory_sha256": hashlib.sha256(inventory_payload).hexdigest(),
        "output": str(output),
        "ready": True,
        "source": source,
    }


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--artifact-handoff-root", type=Path, required=True)
    parser.add_argument("--candidate-archive", type=Path, required=True)
    parser.add_argument("--candidate-authority-dir", type=Path, required=True)
    parser.add_argument("--candidate-replay-ledger", type=Path, required=True)
    parser.add_argument("--expected-source-commit", required=True)
    parser.add_argument("--expected-dpn-validator-release-commit", required=True)
    parser.add_argument("--expected-cargo-lock-sha256", required=True)
    parser.add_argument("--expected-workspace-source-manifest-sha256", required=True)
    parser.add_argument("--expected-receipt-id", required=True)
    parser.add_argument("--trusted-signing-fingerprint", required=True)
    parser.add_argument("--release-manifest-verifier", type=Path, required=True)
    parser.add_argument("--trusted-release-manifest-verifier-sha256", required=True)
    parser.add_argument("--python", default=sys.executable)
    parser.add_argument("--now-unix", type=int)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        candidate = authenticate_candidate(args)
        result = assemble_boi_handoff(
            Path(os.path.abspath(args.artifact_handoff_root)),
            Path(os.path.abspath(args.output)),
            candidate,
            python=args.python,
        )
    except (
        BoiHandoffError,
        OSError,
        ReleaseArtifactError,
        abi22.ArtifactContractError,
        admission.TairaRolloutAdmissionError,
    ) as exc:
        print(f"Privacy v1 BOI handoff refused: {exc}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(canonical_json_bytes(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
