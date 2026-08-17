#!/usr/bin/env python3
"""Assemble the source-bound Privacy v1 handoff consumed by BOI.

The input artifact handoff is inert data from an untrusted build job.  This
command is intended to run in the sealed, secret-free qualification
environment.  Qualification issuance is deliberately disabled until the
installed controller provides the pinned runtime-sandbox and authority-only
signer-broker contract named below.  Once provisioned, the runtime side must
independently re-admit the signed Taira candidate, replay the wheel and ABI-22
native validators, and return only bounded results to the authority side before
the immutable BOI directory can be signed.

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
import time
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
    from . import taira_authority_client
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
        stable_read_path,
        stable_read_relative,
    )
    from .release_manifest_signing import (
        ReleaseManifestSignatureError,
        sign_release_manifest,
        verify_release_manifest,
    )
except ImportError:
    import check_native_sdk_abi22_artifact as abi22
    import taira_authority_client
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
        stable_read_path,
        stable_read_relative,
    )
    from release_manifest_signing import (
        ReleaseManifestSignatureError,
        sign_release_manifest,
        verify_release_manifest,
    )


SCHEMA = "iroha.privacy-v1.boi-handoff-inventory"
SCHEMA_VERSION = 1
SOURCE_HANDOFF_SCHEMA = admission.BOI_SOURCE_HANDOFF_SCHEMA
SOURCE_HANDOFF_KIND = admission.BOI_SOURCE_HANDOFF_KIND
SOURCE_HANDOFF_MANIFEST = admission.BOI_SOURCE_HANDOFF_MANIFEST
OUTPUT_INVENTORY = "boi-privacy-v1-inventory.json"
QUALIFIED_HANDOFF_MANIFEST = "handoff-inventory-v1.json"
QUALIFIED_HANDOFF_KIND = "privacy-v1-boi-qualified"
QUALIFIED_HANDOFF_SCHEMA = "iroha.taira.release_handoff"
PROBE_TRANSCRIPT_PATH = "qualification/probe-transcript-v1.json"
QUALIFICATION_PAYLOAD_INVENTORY_PATH = (
    "qualification/qualified-payload-inventory-v1.json"
)
QUALIFICATION_ENVELOPE_PATH = "qualification/linux-boi-qualification-v1.json"
NATIVE_AUTHORITY_ENVELOPE_PATH = (
    "qualification/native-qualification-authority-envelope-v1.json"
)
NATIVE_AUTHORITY_RECEIPT_PATH = (
    "qualification/native-qualification-durable-receipt-v1.json"
)
QUALIFICATION_SIGNATURE_PATH = (
    "qualification/linux-boi-qualification-v1.json.sig"
)
QUALIFICATION_PUBLIC_KEY_PATH = (
    "qualification/linux-boi-qualification-v1.json.pub"
)
QUALIFICATION_ENVELOPE_SCHEMA = "iroha.taira.linux_boi_qualification"
QUALIFICATION_ENVELOPE_SCHEMA_VERSION = 1
QUALIFICATION_REPLAY_DOMAIN = b"iroha.taira.linux-boi-qualification-replay.v1\0"
QUALIFICATION_LIFETIME_SECONDS = 6 * 60 * 60
QUALIFICATION_CLOCK_SKEW_SECONDS = 5 * 60
BOI_QUALIFICATION_ISOLATION_CONTRACT = (
    "iroha.taira.boi-native-isolation-broker.v1"
)
BOI_QUALIFICATION_RUN_BINDING_CONTRACT = (
    "iroha.taira.boi-authenticated-run-nonce.v1"
)
BOI_COMPLETE_SOURCE_IDENTITY_ATTESTATION_CONTRACT = (
    "iroha.taira.complete-source-identity-attestation.v1"
)
BOI_QUALIFICATION_ISSUANCE_BARRIER = (
    "missing preprovisioned iroha.taira.boi-native-isolation-broker.v1: "
    "candidate archive parsing, ABI loading/symbol inspection, wheel and worker "
    "probes must run under the attested runtime UID/GID with no_new_privs, "
    "closed inherited fds, a scrubbed environment, RLIMIT and stdout/stderr "
    "bounds, a network-denying sandbox, a new session/process-group kill, and "
    "residual-descendant validation; the distinct pinned qualification signer "
    "must be reachable only through an authority-UID-authenticated endpoint "
    "inaccessible to runtime, after every runtime child has exited and candidate "
    "hashes have been rechecked; missing preprovisioned "
    "iroha.taira.boi-authenticated-run-nonce.v1: caller workflow run ID/attempt "
    "must not authorize qualification or replay identity; missing preprovisioned "
    "iroha.taira.complete-source-identity-attestation.v1: a root-owned authority "
    "record must independently bind source commit, DPN validator release commit, "
    "the exact canonical Cargo.lock digest, and workspace source-manifest digest "
    "(or one stronger immutable candidate identity); caller-echoed values are not "
    "release authority"
)
FIXED_CARGO_LOCK_SHA256 = (
    "cd9e829e454171f17540abeb7fd1aa14129252082bd8b076a0199b0ffa4e3f79"
)
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
TRUST_ID_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,127}")

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
MAX_WHEEL_PROBE_RESULT_BYTES = 256 * 1024

EXACT12_PROTOCOL_IDS = tuple(row[0] for row in privacy_evidence.OUTCOMES)
JINDO_PROTOCOL_ID = "iroha-jindo-polynomial-commitment-v0"
JINDO_LIMITATION = "missing-distribution-wide-knowledge-soundness-evidence"
CAPABILITY_BINDING_SCHEMA = "iroha.taira.exact12-runtime-capability-binding"
CAPABILITY_TUPLE_FIELDS = frozenset(
    {
        "activation_state",
        "committed_height",
        "compiled_profile_status",
        "engine_id",
        "engine_manifest_digest",
        "execution_mode",
        "limitation",
        "manifest_digest",
        "network_available",
        "operation_schema",
        "parameter_digest",
        "parameter_id",
        "privacy_feature_mask",
        "proof_system_id",
        "protocol_id",
        "readiness",
        "statement_schema_digest",
        "unavailable_reason",
        "verifier_digest",
    }
)
if len(EXACT12_PROTOCOL_IDS) != 12 or len(set(EXACT12_PROTOCOL_IDS)) != 12:
    raise RuntimeError("BOI Exact12 runtime binding protocol order differs")

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
CANDIDATE_ARCHIVE_DIRECTORY = "candidate/admission"
CANDIDATE_AUTHORITY_DIRECTORY = "candidate/authority"

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
    authority_dir: Path
    authority_files: Mapping[str, StableFile]
    release_signer_fingerprint_sha256: str
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


@dataclass(frozen=True)
class QualifiedBoiSnapshot:
    """Stable identity of one independently verified qualified BOI handoff."""

    root: Path
    files: Mapping[str, StableFile]
    handoff_inventory_sha256: str
    boi_inventory_sha256: str
    candidate_archive_sha256: str
    candidate_boi_artifact_inventory_sha256: str
    candidate_release_manifest_sha256: str
    qualification_receipt_id: str
    probe_transcript_sha256: str
    qualification_signer_fingerprint_sha256: str
    trusted_qualification_public_key: Path
    trusted_qualification_public_key_state: StableFile
    source: Mapping[str, object]


def _fail(message: str) -> NoReturn:
    raise BoiHandoffError(message)


def _sha256(value: object, label: str, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must not be all zero")
    return value


def require_boi_qualification_isolation(
    trusted_qualification_external_signer_sha256: object,
) -> None:
    """Authenticate the fixed qualification service before caller path access."""

    del trusted_qualification_external_signer_sha256
    try:
        taira_authority_client.preflight("qualification")
    except taira_authority_client.TairaAuthorityClientError as error:
        raise BoiHandoffError(
            f"{BOI_QUALIFICATION_ISSUANCE_BARRIER}: {error}"
        ) from error


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


def _trust_id(value: object, label: str) -> str:
    if not isinstance(value, str) or TRUST_ID_RE.fullmatch(value) is None:
        _fail(f"{label} must be one canonical trust identifier")
    return value


def _positive_run_number(value: object, label: str, maximum_digits: int) -> int:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value <= 0
        or len(str(value)) > maximum_digits
    ):
        _fail(f"{label} must be one bounded positive integer")
    return value


def _qualification_replay_namespace(
    source_commit: str, workflow_run_id: int, workflow_run_attempt: int
) -> str:
    return (
        "iroha.taira.linux-boi-qualification.v1:"
        f"{source_commit}:{workflow_run_id}:{workflow_run_attempt}"
    )


def _qualification_receipt_id(envelope_payload: bytes) -> str:
    return hashlib.sha256(QUALIFICATION_REPLAY_DOMAIN + envelope_payload).hexdigest()


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
    release_signer_fingerprint = _sha256(
        args.trusted_signing_fingerprint, "release signer fingerprint"
    )
    archive = Path(os.path.abspath(args.candidate_archive))
    authority_dir = _canonical_directory(
        Path(os.path.abspath(args.candidate_authority_dir)),
        "candidate authority directory",
    )
    replay_ledger = Path(os.path.abspath(args.candidate_replay_ledger))
    verifier = Path(os.path.abspath(args.release_manifest_verifier))
    archive_info = stable_hash_path(
        archive, max_size=native_authority.MAX_ARCHIVE_LOGICAL_BYTES
    )
    try:
        if scan_inventory_paths(authority_dir) != list(admission.FINAL_AUTHORITY_FILES):
            _fail("candidate authority must contain exactly manifest/signature/key")
        authority_files = {
            relative: stable_hash_relative(authority_dir, relative)
            for relative in admission.FINAL_AUTHORITY_FILES
        }
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(f"cannot capture candidate authority: {exc}") from exc
    try:
        result = admission.verify_admission(
            archive_path=archive,
            authority_dir=authority_dir,
            expected_source=source,
            expected_receipt_id=_sha256(
                args.expected_receipt_id, "qualification receipt ID"
            ),
            replay_ledger_path=replay_ledger,
            trusted_signing_fingerprint=release_signer_fingerprint,
            release_manifest_verifier_path=verifier,
            trusted_release_manifest_verifier_sha256=_sha256(
                args.trusted_release_manifest_verifier_sha256,
                "native release-manifest verifier digest",
            ),
            now_unix=args.now_unix,
        )
    except Exception as exc:
        raise BoiHandoffError(f"signed candidate admission failed: {exc}") from exc
    if (
        result.get("verified") is not True
        or result.get("source") != source.as_dict()
        or result.get("signer_fingerprint_sha256")
        != release_signer_fingerprint
    ):
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
        authority_dir=authority_dir,
        authority_files=authority_files,
        release_signer_fingerprint_sha256=release_signer_fingerprint,
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
    if (
        authority_files["release_manifest.json"].sha256
        != candidate.release_manifest_sha256
    ):
        _fail("candidate authority manifest differs from its admission result")
    try:
        if scan_inventory_paths(authority_dir) != list(admission.FINAL_AUTHORITY_FILES):
            _fail("candidate authority inventory changed during BOI admission")
        for relative, before in authority_files.items():
            if stable_hash_relative(authority_dir, relative) != before:
                _fail(f"candidate authority file changed: {relative!r}")
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(f"cannot recheck candidate authority: {exc}") from exc
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


def _validate_capability_binding_result(value: object) -> dict[str, object]:
    """Require one bounded canonical all-12 runtime/network binding result."""

    if not isinstance(value, dict) or set(value) != {
        "manifest_protocol_tuples",
        "protocol_count",
        "required_network_protocol_tuples",
        "schema",
        "schema_version",
    }:
        _fail("Exact12 runtime capability binding fields differ")
    if (
        value.get("schema") != CAPABILITY_BINDING_SCHEMA
        or value.get("schema_version") != 1
        or value.get("protocol_count") != len(EXACT12_PROTOCOL_IDS)
    ):
        _fail("Exact12 runtime capability binding identity differs")
    manifest_rows = value.get("manifest_protocol_tuples")
    required_rows = value.get("required_network_protocol_tuples")
    if (
        not isinstance(manifest_rows, list)
        or not isinstance(required_rows, list)
        or len(manifest_rows) != len(EXACT12_PROTOCOL_IDS)
        or len(required_rows) != len(EXACT12_PROTOCOL_IDS)
        or manifest_rows != required_rows
    ):
        _fail("Exact12 manifest and local required capability tuples differ")
    manifest_digest: str | None = None
    for index, (row, protocol_id) in enumerate(
        zip(manifest_rows, EXACT12_PROTOCOL_IDS)
    ):
        if not isinstance(row, dict) or set(row) != CAPABILITY_TUPLE_FIELDS:
            _fail(f"Exact12 capability tuple {index} fields differ")
        if row.get("protocol_id") != protocol_id:
            _fail(f"Exact12 capability tuple {index} protocol order differs")
        digest = _sha256(
            row.get("manifest_digest"),
            f"Exact12 capability tuple {index} manifest digest",
        )
        if manifest_digest is None:
            manifest_digest = digest
        elif digest != manifest_digest:
            _fail("Exact12 capability tuples do not share one manifest digest")
        committed_height = row.get("committed_height")
        feature_mask = row.get("privacy_feature_mask")
        if (
            not isinstance(committed_height, int)
            or isinstance(committed_height, bool)
            or committed_height <= 0
            or not isinstance(feature_mask, int)
            or isinstance(feature_mask, bool)
            or not 0 <= feature_mask <= (1 << 64) - 1
        ):
            _fail(f"Exact12 capability tuple {index} numeric fields differ")
        for field in (
            "operation_schema",
            "execution_mode",
            "proof_system_id",
            "engine_id",
        ):
            if (
                not isinstance(row.get(field), str)
                or TRUST_ID_RE.fullmatch(str(row[field])) is None
            ):
                _fail(f"Exact12 capability tuple {index} {field} differs")
        for field in (
            "parameter_id",
            "parameter_digest",
            "verifier_digest",
            "statement_schema_digest",
            "engine_manifest_digest",
        ):
            _sha256(
                row.get(field),
                f"Exact12 capability tuple {index} {field}",
            )
        expected_readiness = (
            "available-experimental"
            if protocol_id == JINDO_PROTOCOL_ID
            else "available"
        )
        expected_limitation = (
            JINDO_LIMITATION if protocol_id == JINDO_PROTOCOL_ID else None
        )
        if (
            row.get("network_available") is not True
            or row.get("compiled_profile_status") != "available"
            or row.get("readiness") != expected_readiness
            or row.get("activation_state") != "active"
            or row.get("unavailable_reason") is not None
            or row.get("limitation") != expected_limitation
        ):
            _fail(f"Exact12 capability tuple {protocol_id} is not release-ready")
    return value


def _probe_native_wheel(
    root: Path,
    captured: Mapping[str, StableFile],
    native_member: str,
    capability_payload: bytes,
    compiled_catalog_payload: bytes,
    python: str,
) -> dict[str, object]:
    source = r"""
import importlib.machinery
import importlib.util
import json
import pathlib
import sys

extension = pathlib.Path(sys.argv[1])
archive = pathlib.Path(sys.argv[2]).read_bytes()
catalog = pathlib.Path(sys.argv[3]).read_bytes()
controller_path = pathlib.Path(sys.argv[4])
worker_path = pathlib.Path(sys.argv[5])
worker_sha256 = sys.argv[6]
name = "iroha_python._crypto"
loader = importlib.machinery.ExtensionFileLoader(name, str(extension))
spec = importlib.util.spec_from_loader(name, loader)
if spec is None:
    raise SystemExit("native extension has no import specification")
module = importlib.util.module_from_spec(spec)
loader.exec_module(module)
required = (
    "connect_norito_bridge_abi_version",
    "privacy_compiled_profile_catalog_v1",
    "privacy_exact12_capability_manifest_v1",
    "privacy_validate_compiled_profile_catalog_v1",
    "privacy_validate_exact12_capability_manifest_v1",
)
if any(not callable(getattr(module, item, None)) for item in required):
    raise SystemExit("native wheel omits a required Privacy v1 function")
if module.connect_norito_bridge_abi_version() != 22:
    raise SystemExit("native wheel ABI is not exactly 22")
if module.privacy_validate_compiled_profile_catalog_v1(catalog) != 0:
    raise SystemExit("native wheel rejected the standalone ABI compiled catalog")
if bytes(module.privacy_compiled_profile_catalog_v1()) != catalog:
    raise SystemExit("native wheel compiled catalog differs from the standalone ABI")
if module.privacy_validate_exact12_capability_manifest_v1(archive) != 0:
    raise SystemExit("native wheel rejected the Exact12 capability manifest")
admitted_manifest = module.privacy_exact12_capability_manifest_v1(archive)
canonical_archive = getattr(admitted_manifest, "canonical_archive", None)
if not isinstance(canonical_archive, (bytes, bytearray, memoryview)):
    raise SystemExit("native wheel admission omitted canonical manifest bytes")
if bytes(canonical_archive) != archive:
    raise SystemExit("native wheel compiled capability bytes differ")
expected_protocol_ids = __EXACT12_PROTOCOL_IDS__
jindo_protocol_id = __JINDO_PROTOCOL_ID__
jindo_limitation = __JINDO_LIMITATION__
tuple_fields = frozenset(__CAPABILITY_TUPLE_FIELDS__)

def require_tuple_semantics(row, protocol_id):
    if not isinstance(row, dict) or set(row) != tuple_fields:
        raise SystemExit("native wheel capability tuple fields differ")
    if row.get("protocol_id") != protocol_id:
        raise SystemExit("native wheel capability tuple order differs")
    readiness = (
        "available-experimental"
        if protocol_id == jindo_protocol_id
        else "available"
    )
    limitation = jindo_limitation if protocol_id == jindo_protocol_id else None
    if (
        row.get("network_available") is not True
        or row.get("compiled_profile_status") != "available"
        or row.get("readiness") != readiness
        or row.get("activation_state") != "active"
        or row.get("unavailable_reason") is not None
        or row.get("limitation") != limitation
    ):
        raise SystemExit("native wheel capability tuple is not release-ready")

def normalize(value):
    if isinstance(value, (bytes, bytearray, memoryview)):
        return bytes(value).hex()
    if value is None or isinstance(value, (bool, int, str)):
        return value
    if isinstance(value, dict):
        if any(not isinstance(key, str) for key in value):
            raise SystemExit("native wheel capability tuple key is not text")
        return {key: normalize(value[key]) for key in sorted(value)}
    if isinstance(value, (list, tuple)):
        return [normalize(item) for item in value]
    raise SystemExit("native wheel capability tuple value is not canonical")

manifest_rows = [dict(row) for row in admitted_manifest.protocol_tuples()]
if [row.get("protocol_id") for row in manifest_rows] != list(expected_protocol_ids):
    raise SystemExit("native wheel capability tuple protocol order differs")
required_rows = []
for protocol_id, row in zip(expected_protocol_ids, manifest_rows):
    require_tuple_semantics(row, protocol_id)
    required = dict(admitted_manifest.require_network_capability(protocol_id))
    require_tuple_semantics(required, protocol_id)
    if required != row:
        raise SystemExit("native wheel local profile differs from committed tuple")
    required_rows.append(required)
binding = {
    "manifest_protocol_tuples": normalize(manifest_rows),
    "protocol_count": len(expected_protocol_ids),
    "required_network_protocol_tuples": normalize(required_rows),
    "schema": "iroha.taira.exact12-runtime-capability-binding",
    "schema_version": 1,
}
sys.stdout.write(json.dumps(
    binding,
    indent=2,
    sort_keys=True,
    ensure_ascii=True,
    allow_nan=False,
) + "\n")
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
    source = (
        source.replace("__EXACT12_PROTOCOL_IDS__", repr(EXACT12_PROTOCOL_IDS))
        .replace("__JINDO_PROTOCOL_ID__", repr(JINDO_PROTOCOL_ID))
        .replace("__JINDO_LIMITATION__", repr(JINDO_LIMITATION))
        .replace(
            "__CAPABILITY_TUPLE_FIELDS__",
            repr(tuple(sorted(CAPABILITY_TUPLE_FIELDS))),
        )
    )
    with tempfile.TemporaryDirectory(prefix="privacy-v1-boi-wheel-") as raw:
        temporary = Path(raw).resolve(strict=True)
        extension = temporary / PurePosixPath(native_member).name
        manifest = temporary / "exact12-capability-manifest-v1.norito"
        catalog = temporary / "compiled-profile-catalog-v1.norito"
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
            exclusive_write_bytes(catalog, compiled_catalog_payload, mode=0o600)
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
                        str(catalog),
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
            stderr.seek(0, os.SEEK_END)
            stderr_size = stderr.tell()
            if stderr_size > 64 * 1024:
                _fail("native wheel probe emitted excessive diagnostic output")
            stderr.seek(0)
            diagnostic = stderr.read()
            if result.returncode != 0:
                detail = diagnostic.decode("utf-8", "replace").strip()
                _fail("native wheel probe failed" + (f": {detail}" if detail else ""))
            if diagnostic:
                _fail("native wheel probe emitted diagnostics on success")
            stdout.seek(0, os.SEEK_END)
            stdout_size = stdout.tell()
            if not 1 <= stdout_size <= MAX_WHEEL_PROBE_RESULT_BYTES:
                _fail("native wheel probe result violates its byte bound")
            stdout.seek(0)
            binding_payload = stdout.read()
            binding = _canonical_object(
                binding_payload, "native wheel Exact12 capability binding"
            )
            return _validate_capability_binding_result(binding)


def _validate_abi_runtime(path: Path) -> bytes:
    """Execute the exact ABI surface and its compiled-catalog byte contract."""

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

    try:
        library = ctypes.CDLL(path)
        getter = library.iroha_privacy_compiled_profile_catalog_v1
        validator = library.iroha_privacy_validate_compiled_profile_catalog_v1
        free_buffer = library.iroha_privacy_free_buffer
        getter.argtypes = [
            ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)),
            ctypes.POINTER(ctypes.c_ulong),
        ]
        getter.restype = ctypes.c_int32
        validator.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.c_ulong]
        validator.restype = ctypes.c_int32
        free_buffer.argtypes = [ctypes.POINTER(ctypes.c_uint8)]
        free_buffer.restype = None

        def get_catalog() -> bytes:
            pointer = ctypes.POINTER(ctypes.c_uint8)()
            length = ctypes.c_ulong(0)
            status = getter(ctypes.byref(pointer), ctypes.byref(length))
            if (
                status != 0
                or not bool(pointer)
                or length.value < 16
                or length.value > MAX_CAPABILITY_BYTES
            ):
                _fail("ABI22 compiled-profile catalog getter failed closed")
            try:
                payload = ctypes.string_at(pointer, length.value)
                copied = (ctypes.c_uint8 * len(payload)).from_buffer_copy(payload)
                if validator(copied, len(payload)) != 0:
                    _fail("ABI22 rejected its canonical compiled-profile catalog")
                return payload
            finally:
                free_buffer(pointer)

        first_catalog = get_catalog()
        if first_catalog[:4] != b"NRT0" or not any(first_catalog[4:]):
            _fail("ABI22 compiled-profile catalog is not canonical Norito bytes")
        if get_catalog() != first_catalog:
            _fail("ABI22 compiled-profile catalog getter is not byte-stable")
        return first_catalog
    except (AttributeError, OSError, TypeError, ValueError) as exc:
        raise BoiHandoffError(
            f"ABI22 compiled-profile catalog replay failed: {exc}"
        ) from exc


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
    wheel_probe: Callable[
        [Path, Mapping[str, StableFile], str, bytes, bytes, str],
        Mapping[str, object],
    ] | None,
    abi_runtime_validator: Callable[[Path], bytes] | None,
    require_native_execution: bool = True,
) -> dict[str, object]:
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
    _validate_elf_aarch64(
        _read_captured(root, WORKER_PATH, captured)[:64], "IPWW native worker"
    )
    native_member, _controller = _wheel_layout(root, captured)
    compiled_catalog = b""
    capability_binding: dict[str, object] = {}
    if require_native_execution:
        if wheel_probe is None or abi_runtime_validator is None:
            _fail("native BOI validation callbacks are absent")
        compiled_catalog = abi_runtime_validator(root / ABI_LIBRARY_PATH)
        if stable_hash_relative(root, ABI_LIBRARY_PATH) != captured[ABI_LIBRARY_PATH]:
            _fail("ABI22 native library changed during replay")
        capability_binding = _validate_capability_binding_result(
            wheel_probe(
                root,
                captured,
                native_member,
                capability,
                compiled_catalog,
                python,
            )
        )
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
    if not require_native_execution:
        return {}
    catalog_sha256 = hashlib.sha256(compiled_catalog).hexdigest()
    return {
        "abi22": {
            "abi_version": 22,
            "compiled_profile_catalog_sha256": catalog_sha256,
            "library_sha256": captured[ABI_LIBRARY_PATH].sha256,
            "privacy_c_exports": list(abi22.APPROVED_PRIVACY_C_EXPORTS),
            "result": "passed",
        },
        "python_wheel": {
            "capability_binding": capability_binding,
            "capability_binding_sha256": hashlib.sha256(
                canonical_json_bytes(capability_binding)
            ).hexdigest(),
            "capability_manifest_sha256": captured[CAPABILITY_PATH].sha256,
            "compiled_profile_catalog_sha256": catalog_sha256,
            "native_member": native_member,
            "result": "passed",
            "wheel_sha256": captured[WHEEL_PATH].sha256,
        },
    }


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


def _candidate_archive_relative(candidate: AuthenticatedCandidate) -> str:
    name = candidate.archive.name
    if (
        not name.endswith(".tar.gz")
        or len(name.encode("utf-8")) > 256
        or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", name) is None
    ):
        _fail("candidate archive basename is not a bounded portable name")
    return f"{CANDIDATE_ARCHIVE_DIRECTORY}/{name}"


def _recheck_candidate_files(candidate: AuthenticatedCandidate) -> None:
    if stable_hash_path(candidate.archive) != candidate.archive_info:
        _fail("candidate archive changed during BOI qualification")
    try:
        if scan_inventory_paths(candidate.authority_dir) != list(
            admission.FINAL_AUTHORITY_FILES
        ):
            _fail("candidate authority inventory changed during BOI qualification")
        for relative, before in candidate.authority_files.items():
            if stable_hash_relative(candidate.authority_dir, relative) != before:
                _fail(f"candidate authority file changed: {relative!r}")
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(f"cannot recheck candidate authority: {exc}") from exc


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
        wheel_probe=None,
        abi_runtime_validator=None,
        require_native_execution=False,
    )
    return captured


def _qualification_authority_artifacts(
    artifact_root: Path,
    candidate: AuthenticatedCandidate,
) -> tuple[taira_authority_client.Artifact, ...]:
    """Build the closed descriptor inventory executed by the native sandbox."""

    artifacts = [
        taira_authority_client.Artifact(
            f"source/{spec.path}",
            artifact_root / spec.path,
            maximum=spec.maximum,
        )
        for spec in ARTIFACT_SPECS
    ]
    artifacts.append(
        taira_authority_client.Artifact(
            "candidate/archive",
            candidate.archive,
            maximum=candidate.archive_info.size,
        )
    )
    artifacts.extend(
        taira_authority_client.Artifact(
            f"candidate/authority/{relative}",
            candidate.authority_dir / relative,
            maximum=MAX_HANDOFF_MANIFEST_BYTES,
        )
        for relative in admission.FINAL_AUTHORITY_FILES
    )
    return tuple(artifacts)


def _qualification_authority_subject(
    candidate: AuthenticatedCandidate,
    *,
    source: Mapping[str, object],
    controller_digest: str,
    host_id: str,
    installation_id: str,
    issued_at_unix: int,
    replay_namespace: str,
    workflow_run_id: int,
    workflow_run_attempt: int,
) -> dict[str, object]:
    """Build the exact stable subject shared by issuance and verification."""

    return {
        "candidate": {
            "archive_sha256": candidate.archive_info.sha256,
            "artifact_handoff_sha256": candidate.artifact_handoff_sha256,
            "boi_artifact_inventory_sha256": candidate.boi_artifact_inventory_sha256,
            "exact12_matrix_sha256": candidate.exact12_matrix_sha256,
            "release_manifest_sha256": candidate.release_manifest_sha256,
            "validator_binary_sha256": candidate.validator_binary_sha256,
        },
        "controller": {
            "closure_digest": controller_digest,
            "host_id": host_id,
            "installation_id": installation_id,
        },
        "expected": {
            "abi_version": 22,
            "protocol_ids": list(EXACT12_PROTOCOL_IDS),
            "qualification_contract": BOI_QUALIFICATION_ISOLATION_CONTRACT,
            "run_binding_contract": BOI_QUALIFICATION_RUN_BINDING_CONTRACT,
        },
        "issued_at_unix": issued_at_unix,
        "replay_namespace": replay_namespace,
        "source": dict(source),
        "workflow": {
            "run_attempt": workflow_run_attempt,
            "run_id": workflow_run_id,
        },
    }


def _authority_probe_results(
    result: taira_authority_client.AuthorityResult,
    captured: Mapping[str, StableFile],
    artifact_root: Path,
) -> dict[str, object]:
    """Validate the bounded native-sandbox results returned by the authority."""

    claims = result.authority_envelope.get("claims")
    role_result = claims.get("role_result") if isinstance(claims, dict) else None
    raw = (
        role_result.get("probe_results")
        if isinstance(role_result, dict)
        else None
    )
    if not isinstance(raw, dict) or set(raw) != {"abi22", "python_wheel"}:
        _fail("qualification authority omitted exact native probe results")
    abi_result = raw["abi22"]
    wheel_result = raw["python_wheel"]
    if not isinstance(abi_result, dict) or not isinstance(wheel_result, dict):
        _fail("qualification authority native probe results are not objects")
    native_member, _controller = _wheel_layout(artifact_root, captured)
    abi_fields = {
        "abi_version",
        "compiled_profile_catalog_sha256",
        "library_sha256",
        "privacy_c_exports",
        "result",
    }
    wheel_fields = {
        "capability_binding",
        "capability_binding_sha256",
        "capability_manifest_sha256",
        "compiled_profile_catalog_sha256",
        "native_member",
        "result",
        "wheel_sha256",
    }
    if set(abi_result) != abi_fields or set(wheel_result) != wheel_fields:
        _fail("qualification authority native probe result fields differ")
    catalog_sha256 = _sha256(
        abi_result["compiled_profile_catalog_sha256"],
        "authority compiled-profile catalog digest",
    )
    capability_binding = _validate_capability_binding_result(
        wheel_result["capability_binding"]
    )
    if (
        abi_result["abi_version"] != 22
        or abi_result["library_sha256"] != captured[ABI_LIBRARY_PATH].sha256
        or abi_result["privacy_c_exports"] != list(abi22.APPROVED_PRIVACY_C_EXPORTS)
        or abi_result["result"] != "passed"
        or wheel_result["capability_manifest_sha256"]
        != captured[CAPABILITY_PATH].sha256
        or wheel_result["compiled_profile_catalog_sha256"] != catalog_sha256
        or wheel_result["native_member"] != native_member
        or wheel_result["result"] != "passed"
        or wheel_result["wheel_sha256"] != captured[WHEEL_PATH].sha256
        or wheel_result["capability_binding_sha256"]
        != hashlib.sha256(canonical_json_bytes(capability_binding)).hexdigest()
    ):
        _fail("qualification authority native probe results differ from artifacts")
    return {"abi22": dict(abi_result), "python_wheel": dict(wheel_result)}


def assemble_boi_handoff(
    artifact_root: Path,
    output: Path,
    candidate: AuthenticatedCandidate,
    *,
    python: str = sys.executable,
    wheel_probe: Callable[
        [Path, Mapping[str, StableFile], str, bytes, bytes, str],
        Mapping[str, object],
    ] = _probe_native_wheel,
    abi_runtime_validator: Callable[[Path], bytes] = _validate_abi_runtime,
    qualification_external_signer: Path,
    qualification_signing_public_key: Path,
    trusted_qualification_signing_fingerprint: str,
    qualification_host_id: str,
    qualification_installation_id: str,
    controller_closure_digest: str,
    workflow_run_id: int,
    workflow_run_attempt: int,
    release_manifest_verifier_path: Path,
    trusted_release_manifest_verifier_sha256: str,
    qualification_issued_at_unix: int | None = None,
    qualification_signer: Callable[..., Mapping[str, object]] = (
        sign_release_manifest
    ),
) -> dict[str, object]:
    """Validate every input and create one closed, immutable BOI directory."""

    # Direct callers receive the same fail-before-input guarantee as the CLI.
    require_boi_qualification_isolation(None)
    # Native execution is owned by the installed qualification service.  These
    # legacy callbacks remain accepted for source compatibility but are never
    # invoked by the production path.
    del python, wheel_probe, abi_runtime_validator
    artifact_root = _canonical_directory(artifact_root, "BOI source handoff")
    source = _source_identity(candidate.source)
    qualification_fingerprint = _sha256(
        trusted_qualification_signing_fingerprint,
        "trusted BOI qualification signing fingerprint",
    )
    release_fingerprint = _sha256(
        candidate.release_signer_fingerprint_sha256,
        "authenticated candidate release signing fingerprint",
    )
    if qualification_fingerprint == release_fingerprint:
        _fail("release and BOI qualification signing identities must be distinct")
    controller_digest = _sha256(
        controller_closure_digest, "BOI qualification controller closure digest"
    )
    verifier_sha256 = _sha256(
        trusted_release_manifest_verifier_sha256,
        "trusted qualification verifier digest",
    )
    host_id = _trust_id(qualification_host_id, "BOI qualification host ID")
    installation_id = _trust_id(
        qualification_installation_id, "BOI qualification installation ID"
    )
    run_id = _positive_run_number(workflow_run_id, "workflow run ID", 20)
    run_attempt = _positive_run_number(
        workflow_run_attempt, "workflow run attempt", 10
    )
    issued_at = (
        int(time.time())
        if qualification_issued_at_unix is None
        else qualification_issued_at_unix
    )
    if not isinstance(issued_at, int) or isinstance(issued_at, bool) or issued_at <= 0:
        _fail("BOI qualification issue time must be one positive Unix second")
    expires_at = issued_at + QUALIFICATION_LIFETIME_SECONDS
    replay_namespace = _qualification_replay_namespace(
        str(source["commit"]), run_id, run_attempt
    )
    if candidate.archive_info.sha256 == "0" * 64 or candidate.archive_info.size <= 0:
        _fail("BOI handoff requires one nonempty admitted candidate archive")
    _validate_candidate_receipts(candidate)
    if set(candidate.authority_files) != set(admission.FINAL_AUTHORITY_FILES):
        _fail("BOI handoff requires the exact signed candidate authority")
    if (
        candidate.authority_files["release_manifest.json"].sha256
        != candidate.release_manifest_sha256
    ):
        _fail("candidate release manifest differs from its authenticated digest")
    candidate_archive_path = _candidate_archive_relative(candidate)
    _recheck_candidate_files(candidate)
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
        python=sys.executable,
        wheel_probe=None,
        abi_runtime_validator=None,
        require_native_execution=False,
    )
    authority_subject = _qualification_authority_subject(
        candidate,
        source=source,
        controller_digest=controller_digest,
        host_id=host_id,
        installation_id=installation_id,
        issued_at_unix=issued_at,
        replay_namespace=replay_namespace,
        workflow_run_id=run_id,
        workflow_run_attempt=run_attempt,
    )
    try:
        authority_result = taira_authority_client.authorize(
            "qualification",
            authority_subject,
            artifacts=_qualification_authority_artifacts(artifact_root, candidate),
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise BoiHandoffError(
            f"native qualification authority refused the candidate: {error}"
        ) from error
    probe_results = _authority_probe_results(
        authority_result, captured, artifact_root
    )
    abi_probe_result = canonical_json_bytes(probe_results["abi22"])
    wheel_probe_result = canonical_json_bytes(probe_results["python_wheel"])
    probe_transcript = canonical_json_bytes(
        {
            "abi22": probe_results["abi22"],
            "python_wheel": probe_results["python_wheel"],
            "schema": "iroha.taira.linux_boi_probe_transcript",
            "schema_version": 1,
        }
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
        PROBE_TRANSCRIPT_PATH: probe_transcript,
        NATIVE_AUTHORITY_ENVELOPE_PATH: authority_result.authority_envelope_bytes,
        NATIVE_AUTHORITY_RECEIPT_PATH: authority_result.durable_receipt_bytes,
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
            _write_streamed(
                candidate.archive.parent,
                candidate.archive.name,
                install_root / candidate_archive_path,
                candidate.archive_info,
                executable=False,
            )
            for relative in admission.FINAL_AUTHORITY_FILES:
                _write_streamed(
                    candidate.authority_dir,
                    relative,
                    install_root / CANDIDATE_AUTHORITY_DIRECTORY / relative,
                    candidate.authority_files[relative],
                    executable=False,
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
                    PROBE_TRANSCRIPT_PATH: "linux-native-probe-transcript",
                    NATIVE_AUTHORITY_ENVELOPE_PATH: (
                        "native-qualification-authority-envelope"
                    ),
                    NATIVE_AUTHORITY_RECEIPT_PATH: (
                        "native-qualification-durable-receipt"
                    ),
                    candidate_archive_path: "signed-candidate-archive",
                    **{
                        f"{CANDIDATE_AUTHORITY_DIRECTORY}/{relative}": (
                            "signed-candidate-authority"
                        )
                        for relative in admission.FINAL_AUTHORITY_FILES
                    },
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
                    "archive_path": candidate_archive_path,
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
                    "release_signer_fingerprint_sha256": release_fingerprint,
                    "release_manifest_sha256": candidate.release_manifest_sha256,
                    "authority_directory": CANDIDATE_AUTHORITY_DIRECTORY,
                    "validator_binary_sha256": candidate.validator_binary_sha256,
                },
                "contract": _qualified_contract(),
                "ready": True,
                "schema": SCHEMA,
                "schema_version": SCHEMA_VERSION,
                "source": source,
            }
            inventory_payload = canonical_json_bytes(inventory)
            exclusive_write_bytes(
                install_root / OUTPUT_INVENTORY, inventory_payload, mode=0o600
            )
            payload_rows = []
            for relative in sorted([OUTPUT_INVENTORY, *roles]):
                info = stable_hash_relative(install_root, relative)
                payload_rows.append(
                    {
                        "path": relative,
                        "sha256": info.sha256,
                        "size": info.size,
                    }
                )
            qualification_payload_inventory = (
                json.dumps(
                    {
                        "files": payload_rows,
                        "kind": "privacy-v1-boi-qualification-payload",
                        "schema": QUALIFIED_HANDOFF_SCHEMA,
                        "schema_version": 1,
                    },
                    ensure_ascii=True,
                    sort_keys=True,
                    separators=(",", ":"),
                )
                + "\n"
            ).encode("ascii")
            exclusive_write_bytes(
                install_root / QUALIFICATION_PAYLOAD_INVENTORY_PATH,
                qualification_payload_inventory,
                mode=0o600,
            )
            envelope = {
                "candidate": {
                    "archive_sha256": candidate.archive_info.sha256,
                    "release_manifest_sha256": candidate.release_manifest_sha256,
                    "signed_boi_artifact_inventory_sha256": (
                        candidate.boi_artifact_inventory_sha256
                    ),
                },
                "controller": {
                    "closure_digest": controller_digest,
                    "host_id": host_id,
                    "installation_id": installation_id,
                    "role": "linux-boi-qualification",
                },
                "expires_at_unix": expires_at,
                "issued_at_unix": issued_at,
                "payload": {
                    "boi_inventory_sha256": hashlib.sha256(
                        inventory_payload
                    ).hexdigest(),
                    "qualified_payload_inventory_sha256": hashlib.sha256(
                        qualification_payload_inventory
                    ).hexdigest(),
                    "source_artifact_inventory_sha256": (
                        candidate.boi_artifact_inventory_sha256
                    ),
                },
                "probes": {
                    "abi22_result_sha256": hashlib.sha256(
                        abi_probe_result
                    ).hexdigest(),
                    "python_wheel_result_sha256": hashlib.sha256(
                        wheel_probe_result
                    ).hexdigest(),
                    "transcript_sha256": hashlib.sha256(
                        probe_transcript
                    ).hexdigest(),
                },
                "replay_namespace": replay_namespace,
                "schema": QUALIFICATION_ENVELOPE_SCHEMA,
                "schema_version": QUALIFICATION_ENVELOPE_SCHEMA_VERSION,
                "signer": {
                    "algorithm": "ed25519",
                    "qualification_public_key_fingerprint_sha256": (
                        qualification_fingerprint
                    ),
                    "release_public_key_fingerprint_sha256": release_fingerprint,
                },
                "source": source,
                "workflow": {
                    "run_attempt": run_attempt,
                    "run_id": run_id,
                },
            }
            qualification_envelope = canonical_json_bytes(envelope)
            envelope_path = install_root / QUALIFICATION_ENVELOPE_PATH
            signature_path = install_root / QUALIFICATION_SIGNATURE_PATH
            public_key_path = install_root / QUALIFICATION_PUBLIC_KEY_PATH
            envelope_path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            exclusive_write_bytes(envelope_path, qualification_envelope, mode=0o600)
            qualification_signer(
                envelope_path,
                qualification_external_signer,
                qualification_signing_public_key,
                qualification_fingerprint,
                signature_path,
                public_key_path,
                release_manifest_verifier_path,
                verifier_sha256,
            )
            embedded_public_key = stable_hash_relative(
                install_root, QUALIFICATION_PUBLIC_KEY_PATH
            )
            embedded_signature = stable_hash_relative(
                install_root, QUALIFICATION_SIGNATURE_PATH
            )
            if (
                embedded_public_key.size != 32
                or embedded_public_key.sha256 != qualification_fingerprint
                or embedded_signature.size != 64
            ):
                _fail("BOI qualification signer returned a noncanonical identity")
            transport_rows = []
            transport_members = sorted(
                [
                    OUTPUT_INVENTORY,
                    QUALIFICATION_PAYLOAD_INVENTORY_PATH,
                    QUALIFICATION_ENVELOPE_PATH,
                    QUALIFICATION_SIGNATURE_PATH,
                    QUALIFICATION_PUBLIC_KEY_PATH,
                    *roles,
                ]
            )
            for relative in transport_members:
                info = stable_hash_relative(install_root, relative)
                transport_rows.append(
                    {
                        "path": relative,
                        "sha256": info.sha256,
                        "size": info.size,
                    }
                )
            transport_payload = (
                json.dumps(
                    {
                        "files": transport_rows,
                        "kind": QUALIFIED_HANDOFF_KIND,
                        "schema": QUALIFIED_HANDOFF_SCHEMA,
                        "schema_version": 1,
                    },
                    ensure_ascii=True,
                    sort_keys=True,
                    separators=(",", ":"),
                )
                + "\n"
            ).encode("ascii")
            exclusive_write_bytes(
                install_root / QUALIFIED_HANDOFF_MANIFEST,
                transport_payload,
                mode=0o600,
            )
            expected = sorted([QUALIFIED_HANDOFF_MANIFEST, *transport_members])
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
            if (
                stable_hash_relative(install_root, QUALIFIED_HANDOFF_MANIFEST).sha256
                != hashlib.sha256(transport_payload).hexdigest()
            ):
                _fail("assembled BOI transport inventory changed after installation")
            if (
                stable_hash_relative(
                    install_root, QUALIFICATION_PAYLOAD_INVENTORY_PATH
                ).sha256
                != hashlib.sha256(qualification_payload_inventory).hexdigest()
                or stable_hash_relative(
                    install_root, QUALIFICATION_ENVELOPE_PATH
                ).sha256
                != hashlib.sha256(qualification_envelope).hexdigest()
            ):
                _fail("assembled signed BOI qualification envelope changed")
            _recheck_candidate_files(candidate)
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
        except (OSError, ReleaseArtifactError, ReleaseManifestSignatureError) as exc:
            raise BoiHandoffError(f"cannot install BOI handoff: {exc}") from exc
    return {
        "candidate_archive_sha256": candidate.archive_info.sha256,
        "inventory_sha256": hashlib.sha256(inventory_payload).hexdigest(),
        "handoff_inventory_sha256": hashlib.sha256(transport_payload).hexdigest(),
        "output": str(output),
        "probe_transcript_sha256": hashlib.sha256(probe_transcript).hexdigest(),
        "qualification_receipt_id": _qualification_receipt_id(
            qualification_envelope
        ),
        "ready": True,
        "source": source,
    }


def _qualified_contract() -> dict[str, object]:
    return {
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
    }


def verify_qualified_boi_handoff(
    root: Path,
    *,
    candidate_archive: Path,
    candidate_authority_dir: Path,
    expected_source: admission.SourceIdentity,
    expected_receipt_id: str,
    replay_ledger_path: Path,
    trusted_signing_fingerprint: str,
    trusted_qualification_public_key_path: Path,
    trusted_qualification_signing_fingerprint: str,
    expected_qualification_host_id: str,
    expected_qualification_installation_id: str,
    expected_controller_closure_digest: str,
    expected_workflow_run_id: int,
    expected_workflow_run_attempt: int,
    release_manifest_verifier_path: Path,
    trusted_release_manifest_verifier_sha256: str,
    now_unix: int | None = None,
    qualification_signature_verifier: Callable[..., Mapping[str, object]] = (
        verify_release_manifest
    ),
) -> QualifiedBoiSnapshot:
    """Independently authenticate and bind one closed qualified BOI handoff."""

    require_boi_qualification_isolation(None)
    root = _canonical_directory(Path(os.path.abspath(root)), "qualified BOI handoff")
    external_archive = Path(os.path.abspath(candidate_archive))
    external_authority = _canonical_directory(
        Path(os.path.abspath(candidate_authority_dir)),
        "deployment candidate authority",
    )
    external_archive_info = stable_hash_path(
        external_archive, max_size=native_authority.MAX_ARCHIVE_LOGICAL_BYTES
    )
    qualification_fingerprint = _sha256(
        trusted_qualification_signing_fingerprint,
        "trusted BOI qualification signing fingerprint",
    )
    release_fingerprint = _sha256(
        trusted_signing_fingerprint, "trusted release signing fingerprint"
    )
    if qualification_fingerprint == release_fingerprint:
        _fail("release and BOI qualification signing identities must be distinct")
    expected_host_id = _trust_id(
        expected_qualification_host_id, "expected BOI qualification host ID"
    )
    expected_installation_id = _trust_id(
        expected_qualification_installation_id,
        "expected BOI qualification installation ID",
    )
    expected_controller_digest = _sha256(
        expected_controller_closure_digest,
        "expected BOI qualification controller closure digest",
    )
    expected_run_id = _positive_run_number(
        expected_workflow_run_id, "expected workflow run ID", 20
    )
    expected_run_attempt = _positive_run_number(
        expected_workflow_run_attempt, "expected workflow run attempt", 10
    )
    try:
        if scan_inventory_paths(external_authority) != list(
            admission.FINAL_AUTHORITY_FILES
        ):
            _fail("deployment candidate authority inventory is not exact")
        external_authority_files = {
            relative: stable_hash_relative(external_authority, relative)
            for relative in admission.FINAL_AUTHORITY_FILES
        }
        manifest_info, manifest_payload = stable_read_relative(
            root,
            QUALIFIED_HANDOFF_MANIFEST,
            max_size=MAX_HANDOFF_MANIFEST_BYTES,
            return_payload=True,
        )
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(f"cannot capture qualified BOI handoff: {exc}") from exc
    assert manifest_payload is not None
    transport = _canonical_object(
        manifest_payload, "qualified BOI handoff inventory", compact=True
    )
    if (
        set(transport) != {"files", "kind", "schema", "schema_version"}
        or transport.get("kind") != QUALIFIED_HANDOFF_KIND
        or transport.get("schema") != QUALIFIED_HANDOFF_SCHEMA
        or transport.get("schema_version") != 1
    ):
        _fail("qualified BOI handoff inventory identity is unsupported")
    raw_transport_rows = transport.get("files")
    if not isinstance(raw_transport_rows, list) or not raw_transport_rows:
        _fail("qualified BOI handoff inventory is empty")
    captured: dict[str, StableFile] = {
        QUALIFIED_HANDOFF_MANIFEST: manifest_info
    }
    transport_paths: list[str] = []
    for row in raw_transport_rows:
        if not isinstance(row, dict) or set(row) != {"path", "sha256", "size"}:
            _fail("qualified BOI handoff inventory row fields are not exact")
        relative = row.get("path")
        if (
            not isinstance(relative, str)
            or relative == QUALIFIED_HANDOFF_MANIFEST
            or PurePosixPath(relative).is_absolute()
            or PurePosixPath(relative).as_posix() != relative
            or any(part in {"", ".", ".."} for part in PurePosixPath(relative).parts)
        ):
            _fail("qualified BOI handoff contains a noncanonical path")
        try:
            info = stable_hash_relative(root, relative)
        except ReleaseArtifactError as exc:
            raise BoiHandoffError(str(exc)) from exc
        if (
            row.get("sha256") != info.sha256
            or row.get("size") != info.size
            or info.size <= 0
        ):
            _fail(f"qualified BOI handoff row differs: {relative!r}")
        transport_paths.append(relative)
        captured[relative] = info
    if transport_paths != sorted(set(transport_paths)):
        _fail("qualified BOI handoff inventory paths are reordered or repeated")
    try:
        if scan_inventory_paths(root) != sorted(captured):
            _fail("qualified BOI handoff tree is not the exact closed inventory")
        _, inventory_payload = stable_read_relative(
            root, OUTPUT_INVENTORY, max_size=MAX_HANDOFF_MANIFEST_BYTES,
            return_payload=True,
        )
        _, qualification_payload_inventory = stable_read_relative(
            root,
            QUALIFICATION_PAYLOAD_INVENTORY_PATH,
            max_size=MAX_HANDOFF_MANIFEST_BYTES,
            return_payload=True,
        )
        _, qualification_envelope = stable_read_relative(
            root,
            QUALIFICATION_ENVELOPE_PATH,
            max_size=MAX_HANDOFF_MANIFEST_BYTES,
            return_payload=True,
        )
        _, probe_transcript = stable_read_relative(
            root,
            PROBE_TRANSCRIPT_PATH,
            max_size=MAX_HANDOFF_MANIFEST_BYTES,
            return_payload=True,
        )
        _, native_authority_envelope = stable_read_relative(
            root,
            NATIVE_AUTHORITY_ENVELOPE_PATH,
            max_size=taira_authority_client.MAX_CLIENT_OUTPUT_BYTES,
            return_payload=True,
        )
        _, native_authority_receipt = stable_read_relative(
            root,
            NATIVE_AUTHORITY_RECEIPT_PATH,
            max_size=taira_authority_client.MAX_CLIENT_OUTPUT_BYTES,
            return_payload=True,
        )
        _, embedded_qualification_public_key = stable_read_relative(
            root,
            QUALIFICATION_PUBLIC_KEY_PATH,
            max_size=32,
            return_payload=True,
        )
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(str(exc)) from exc
    assert inventory_payload is not None
    assert qualification_payload_inventory is not None
    assert qualification_envelope is not None
    assert probe_transcript is not None
    assert native_authority_envelope is not None
    assert native_authority_receipt is not None
    assert embedded_qualification_public_key is not None
    trusted_qualification_key = Path(trusted_qualification_public_key_path)
    try:
        if (
            not trusted_qualification_key.is_absolute()
            or Path(os.path.abspath(trusted_qualification_key))
            != trusted_qualification_key
            or trusted_qualification_key.resolve(strict=True)
            != trusted_qualification_key
            or trusted_qualification_key.is_symlink()
        ):
            _fail("trusted BOI qualification public key path is not canonical")
        trusted_qualification_key_state, trusted_qualification_key_payload = (
            stable_read_path(trusted_qualification_key, max_size=32)
        )
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(
            f"cannot capture trusted BOI qualification public key: {exc}"
        ) from exc
    if (
        trusted_qualification_key_state.size != 32
        or trusted_qualification_key_state.sha256 != qualification_fingerprint
        or embedded_qualification_public_key != trusted_qualification_key_payload
    ):
        _fail("qualified BOI public key differs from the separately pinned key")
    try:
        signature_result = qualification_signature_verifier(
            root / QUALIFICATION_ENVELOPE_PATH,
            root / QUALIFICATION_SIGNATURE_PATH,
            root / QUALIFICATION_PUBLIC_KEY_PATH,
            qualification_fingerprint,
            release_manifest_verifier_path,
            trusted_release_manifest_verifier_sha256,
        )
    except ReleaseManifestSignatureError as exc:
        raise BoiHandoffError(
            f"BOI qualification envelope signature is invalid: {exc}"
        ) from exc
    if (
        signature_result.get("signature_verified") is not True
        or signature_result.get("signer_fingerprint_sha256")
        != qualification_fingerprint
        or signature_result.get("manifest_sha256")
        != hashlib.sha256(qualification_envelope).hexdigest()
    ):
        _fail("BOI qualification signature verifier returned a mismatched result")
    payload_transport = _canonical_object(
        qualification_payload_inventory,
        "signed BOI qualification payload inventory",
        compact=True,
    )
    if (
        set(payload_transport) != {"files", "kind", "schema", "schema_version"}
        or payload_transport.get("kind")
        != "privacy-v1-boi-qualification-payload"
        or payload_transport.get("schema") != QUALIFIED_HANDOFF_SCHEMA
        or payload_transport.get("schema_version") != 1
    ):
        _fail("signed BOI qualification payload inventory identity differs")
    payload_rows = payload_transport.get("files")
    if not isinstance(payload_rows, list) or not payload_rows:
        _fail("signed BOI qualification payload inventory is empty")
    payload_paths: list[str] = []
    forbidden_payload_paths = {
        QUALIFIED_HANDOFF_MANIFEST,
        QUALIFICATION_PAYLOAD_INVENTORY_PATH,
        QUALIFICATION_ENVELOPE_PATH,
        QUALIFICATION_SIGNATURE_PATH,
        QUALIFICATION_PUBLIC_KEY_PATH,
    }
    for row in payload_rows:
        if not isinstance(row, dict) or set(row) != {"path", "sha256", "size"}:
            _fail("signed BOI qualification payload row fields are not exact")
        relative = row.get("path")
        info = captured.get(relative) if isinstance(relative, str) else None
        if (
            info is None
            or relative in forbidden_payload_paths
            or row.get("sha256") != info.sha256
            or row.get("size") != info.size
        ):
            _fail("signed BOI qualification payload row differs from exact bytes")
        payload_paths.append(relative)
    if payload_paths != sorted(set(payload_paths)):
        _fail("signed BOI qualification payload paths are reordered or repeated")

    envelope = _canonical_object(
        qualification_envelope, "signed BOI qualification envelope"
    )
    envelope_fields = {
        "candidate",
        "controller",
        "expires_at_unix",
        "issued_at_unix",
        "payload",
        "probes",
        "replay_namespace",
        "schema",
        "schema_version",
        "signer",
        "source",
        "workflow",
    }
    if (
        set(envelope) != envelope_fields
        or envelope.get("schema") != QUALIFICATION_ENVELOPE_SCHEMA
        or envelope.get("schema_version") != QUALIFICATION_ENVELOPE_SCHEMA_VERSION
    ):
        _fail("signed BOI qualification envelope fields differ")
    controller = envelope.get("controller")
    if not isinstance(controller, dict) or controller != {
        "closure_digest": expected_controller_digest,
        "host_id": expected_host_id,
        "installation_id": expected_installation_id,
        "role": "linux-boi-qualification",
    }:
        _fail("signed BOI qualification controller identity differs")
    workflow = envelope.get("workflow")
    if not isinstance(workflow, dict) or workflow != {
        "run_attempt": expected_run_attempt,
        "run_id": expected_run_id,
    }:
        _fail("signed BOI qualification workflow identity differs")
    expected_replay_namespace = _qualification_replay_namespace(
        expected_source.commit, expected_run_id, expected_run_attempt
    )
    if envelope.get("replay_namespace") != expected_replay_namespace:
        _fail("signed BOI qualification replay namespace differs")
    signer = envelope.get("signer")
    if not isinstance(signer, dict) or signer != {
        "algorithm": "ed25519",
        "qualification_public_key_fingerprint_sha256": qualification_fingerprint,
        "release_public_key_fingerprint_sha256": release_fingerprint,
    }:
        _fail("signed BOI qualification signer policy differs")
    issued_at = envelope.get("issued_at_unix")
    expires_at = envelope.get("expires_at_unix")
    current_time = int(time.time()) if now_unix is None else now_unix
    if (
        not isinstance(current_time, int)
        or isinstance(current_time, bool)
        or current_time <= 0
        or not isinstance(issued_at, int)
        or isinstance(issued_at, bool)
        or not isinstance(expires_at, int)
        or isinstance(expires_at, bool)
        or issued_at <= 0
        or expires_at - issued_at != QUALIFICATION_LIFETIME_SECONDS
        or issued_at > current_time + QUALIFICATION_CLOCK_SKEW_SECONDS
        or current_time > expires_at
    ):
        _fail("signed BOI qualification envelope is expired or outside policy")
    normalized_source = _source_identity(envelope.get("source"))
    if normalized_source != _source_identity(expected_source.as_dict()):
        _fail("signed BOI qualification source differs")

    probe_value = _canonical_object(
        probe_transcript, "signed BOI native probe transcript"
    )
    if (
        set(probe_value) != {"abi22", "python_wheel", "schema", "schema_version"}
        or probe_value.get("schema")
        != "iroha.taira.linux_boi_probe_transcript"
        or probe_value.get("schema_version") != 1
    ):
        _fail("signed BOI native probe transcript fields differ")
    abi_probe = probe_value.get("abi22")
    wheel_probe = probe_value.get("python_wheel")
    abi_probe_fields = {
        "abi_version",
        "compiled_profile_catalog_sha256",
        "library_sha256",
        "privacy_c_exports",
        "result",
    }
    wheel_probe_fields = {
        "capability_binding",
        "capability_binding_sha256",
        "capability_manifest_sha256",
        "compiled_profile_catalog_sha256",
        "native_member",
        "result",
        "wheel_sha256",
    }
    if (
        not isinstance(abi_probe, dict)
        or set(abi_probe) != abi_probe_fields
        or not isinstance(wheel_probe, dict)
        or set(wheel_probe) != wheel_probe_fields
    ):
        _fail("signed BOI native probe result fields differ")
    catalog_sha256 = _sha256(
        abi_probe.get("compiled_profile_catalog_sha256"),
        "signed standalone compiled catalog digest",
    )
    capability_binding = _validate_capability_binding_result(
        wheel_probe.get("capability_binding")
    )
    capability_binding_sha256 = hashlib.sha256(
        canonical_json_bytes(capability_binding)
    ).hexdigest()
    native_member, _controller_member = _wheel_layout(root, captured)
    if (
        abi_probe.get("abi_version") != 22
        or abi_probe.get("library_sha256") != captured[ABI_LIBRARY_PATH].sha256
        or abi_probe.get("privacy_c_exports")
        != list(abi22.APPROVED_PRIVACY_C_EXPORTS)
        or abi_probe.get("result") != "passed"
        or wheel_probe.get("capability_manifest_sha256")
        != captured[CAPABILITY_PATH].sha256
        or wheel_probe.get("capability_binding_sha256")
        != capability_binding_sha256
        or wheel_probe.get("compiled_profile_catalog_sha256") != catalog_sha256
        or wheel_probe.get("native_member") != native_member
        or wheel_probe.get("result") != "passed"
        or wheel_probe.get("wheel_sha256") != captured[WHEEL_PATH].sha256
    ):
        _fail("signed BOI native probe results differ from qualified artifacts")
    probes = envelope.get("probes")
    expected_probes = {
        "abi22_result_sha256": hashlib.sha256(
            canonical_json_bytes(abi_probe)
        ).hexdigest(),
        "python_wheel_result_sha256": hashlib.sha256(
            canonical_json_bytes(wheel_probe)
        ).hexdigest(),
        "transcript_sha256": hashlib.sha256(probe_transcript).hexdigest(),
    }
    if not isinstance(probes, dict) or probes != expected_probes:
        _fail("signed BOI probe transcript/result digests differ")
    qualification_receipt_id = _qualification_receipt_id(
        qualification_envelope
    )
    try:
        replay_snapshot = admission.load_replay_ledger(
            Path(os.path.abspath(replay_ledger_path))
        )
    except admission.TairaRolloutAdmissionError as exc:
        raise BoiHandoffError(f"cannot inspect BOI replay ledger: {exc}") from exc
    if qualification_receipt_id in replay_snapshot.consumed_receipt_ids:
        _fail("signed BOI qualification receipt was already consumed")

    inventory = _canonical_object(inventory_payload, "qualified BOI inventory")
    if (
        set(inventory)
        != {"artifacts", "candidate", "contract", "ready", "schema", "schema_version", "source"}
        or inventory.get("schema") != SCHEMA
        or inventory.get("schema_version") != SCHEMA_VERSION
        or inventory.get("ready") is not True
        or inventory.get("contract") != _qualified_contract()
    ):
        _fail("qualified BOI semantic inventory identity differs")
    inventory_source = _source_identity(inventory.get("source"))
    if inventory_source != normalized_source:
        _fail("qualified BOI handoff is bound to a different source")

    candidate_value = inventory.get("candidate")
    candidate_fields = {
        "archive_path", "archive_sha256", "artifact_handoff_sha256",
        "authority_directory", "boi_artifact_inventory_sha256",
        "exact12_matrix_sha256", "linux_validator_binary_sha256",
        "macos_validator_binary_sha256", "privacy_protocol_receipt_id",
        "qualification_receipt_id", "release_manifest_sha256",
        "release_signer_fingerprint_sha256",
        "validator_binary_sha256",
    }
    if not isinstance(candidate_value, dict) or set(candidate_value) != candidate_fields:
        _fail("qualified BOI candidate fields are not exact")
    embedded_archive_relative = candidate_value.get("archive_path")
    if (
        not isinstance(embedded_archive_relative, str)
        or not embedded_archive_relative.startswith(CANDIDATE_ARCHIVE_DIRECTORY + "/")
        or PurePosixPath(embedded_archive_relative).parent.as_posix()
        != CANDIDATE_ARCHIVE_DIRECTORY
    ):
        _fail("qualified BOI candidate archive path is not exact")
    if candidate_value.get("authority_directory") != CANDIDATE_AUTHORITY_DIRECTORY:
        _fail("qualified BOI candidate authority path is not exact")
    embedded_archive = root / embedded_archive_relative
    embedded_authority = root / CANDIDATE_AUTHORITY_DIRECTORY
    embedded_args = argparse.Namespace(
        candidate_archive=embedded_archive,
        candidate_authority_dir=embedded_authority,
        candidate_replay_ledger=Path(os.path.abspath(replay_ledger_path)),
        expected_source_commit=expected_source.commit,
        expected_dpn_validator_release_commit=(
            expected_source.dpn_validator_release_commit
        ),
        expected_cargo_lock_sha256=expected_source.cargo_lock_sha256,
        expected_workspace_source_manifest_sha256=(
            expected_source.workspace_source_manifest_sha256
        ),
        expected_receipt_id=expected_receipt_id,
        trusted_signing_fingerprint=trusted_signing_fingerprint,
        release_manifest_verifier=release_manifest_verifier_path,
        trusted_release_manifest_verifier_sha256=(
            trusted_release_manifest_verifier_sha256
        ),
        now_unix=now_unix,
    )
    embedded = authenticate_candidate(embedded_args)
    if (
        embedded.archive.name != external_archive.name
        or embedded.archive_info.sha256 != external_archive_info.sha256
        or embedded.archive_info.size != external_archive_info.size
    ):
        _fail("qualified BOI handoff contains a different signed candidate archive")
    for relative in admission.FINAL_AUTHORITY_FILES:
        inside = embedded.authority_files[relative]
        outside = external_authority_files[relative]
        if inside.sha256 != outside.sha256 or inside.size != outside.size:
            _fail(f"qualified BOI candidate authority differs: {relative!r}")

    expected_candidate = {
        "archive_path": embedded_archive_relative,
        "archive_sha256": embedded.archive_info.sha256,
        "artifact_handoff_sha256": embedded.artifact_handoff_sha256,
        "authority_directory": CANDIDATE_AUTHORITY_DIRECTORY,
        "boi_artifact_inventory_sha256": embedded.boi_artifact_inventory_sha256,
        "exact12_matrix_sha256": embedded.exact12_matrix_sha256,
        "linux_validator_binary_sha256": embedded.native_validator_binary_sha256,
        "macos_validator_binary_sha256": embedded.validator_binary_sha256,
        "privacy_protocol_receipt_id": embedded.privacy_protocol_receipt_id,
        "qualification_receipt_id": embedded.qualification_receipt_id,
        "release_signer_fingerprint_sha256": (
            embedded.release_signer_fingerprint_sha256
        ),
        "release_manifest_sha256": embedded.release_manifest_sha256,
        "validator_binary_sha256": embedded.validator_binary_sha256,
    }
    if candidate_value != expected_candidate:
        _fail("qualified BOI candidate binding differs from signed admission")
    envelope_candidate = envelope.get("candidate")
    expected_envelope_candidate = {
        "archive_sha256": embedded.archive_info.sha256,
        "release_manifest_sha256": embedded.release_manifest_sha256,
        "signed_boi_artifact_inventory_sha256": (
            embedded.boi_artifact_inventory_sha256
        ),
    }
    if (
        not isinstance(envelope_candidate, dict)
        or envelope_candidate != expected_envelope_candidate
    ):
        _fail("signed BOI qualification candidate binding differs")
    envelope_payload_binding = envelope.get("payload")
    expected_payload_binding = {
        "boi_inventory_sha256": hashlib.sha256(inventory_payload).hexdigest(),
        "qualified_payload_inventory_sha256": hashlib.sha256(
            qualification_payload_inventory
        ).hexdigest(),
        "source_artifact_inventory_sha256": (
            embedded.boi_artifact_inventory_sha256
        ),
    }
    if (
        not isinstance(envelope_payload_binding, dict)
        or envelope_payload_binding != expected_payload_binding
    ):
        _fail("signed BOI qualification payload binding differs")

    try:
        native_envelope_value = taira_authority_client.decode_canonical_json(
            native_authority_envelope,
            "native qualification authority envelope",
        )
        native_receipt_value = taira_authority_client.decode_canonical_json(
            native_authority_receipt,
            "native qualification durable receipt",
        )
        native_verification = taira_authority_client.verify_receipt(
            "qualification",
            _qualification_authority_subject(
                embedded,
                source=normalized_source,
                controller_digest=expected_controller_digest,
                host_id=expected_host_id,
                installation_id=expected_installation_id,
                issued_at_unix=issued_at,
                replay_namespace=expected_replay_namespace,
                workflow_run_id=expected_run_id,
                workflow_run_attempt=expected_run_attempt,
            ),
            authority_envelope=native_envelope_value,
            durable_receipt=native_receipt_value,
            artifacts=_qualification_authority_artifacts(root, embedded),
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise BoiHandoffError(
            f"native qualification receipt verification failed: {error}"
        ) from error
    native_probe_results = _authority_probe_results(
        native_verification, captured, root
    )
    if (
        native_probe_results["abi22"] != abi_probe
        or native_probe_results["python_wheel"] != wheel_probe
    ):
        _fail("native qualification receipt probe results differ from handoff")

    expected_roles = {spec.path: spec.role for spec in ARTIFACT_SPECS}
    expected_roles.update(
        {
            CANDIDATE_ADMISSION_PATH: "candidate-admission",
            SOURCE_HANDOFF_COPY_PATH: "source-artifact-inventory",
            NATIVE_RECEIPT_NORITO_PATH: "native-release-receipt-authoritative",
            NATIVE_RECEIPT_JSON_PATH: "native-release-receipt-projection",
            QUALIFICATION_RECEIPT_PATH: "four-peer-qualification-receipt",
            PROTOCOL_RECEIPT_PATH: "exact12-four-peer-receipt",
            PROBE_TRANSCRIPT_PATH: "linux-native-probe-transcript",
            NATIVE_AUTHORITY_ENVELOPE_PATH: (
                "native-qualification-authority-envelope"
            ),
            NATIVE_AUTHORITY_RECEIPT_PATH: (
                "native-qualification-durable-receipt"
            ),
            embedded_archive_relative: "signed-candidate-archive",
            **{
                f"{CANDIDATE_AUTHORITY_DIRECTORY}/{relative}": (
                    "signed-candidate-authority"
                )
                for relative in admission.FINAL_AUTHORITY_FILES
            },
        }
    )
    raw_rows = inventory.get("artifacts")
    if not isinstance(raw_rows, list) or len(raw_rows) != len(expected_roles):
        _fail("qualified BOI semantic artifact count differs")
    semantic_paths: list[str] = []
    for row in raw_rows:
        if not isinstance(row, dict) or set(row) != {"path", "role", "sha256", "size"}:
            _fail("qualified BOI semantic artifact row fields are not exact")
        relative = row.get("path")
        if not isinstance(relative, str) or expected_roles.get(relative) != row.get("role"):
            _fail("qualified BOI semantic artifact role differs")
        info = captured.get(relative)
        if info is None or row.get("sha256") != info.sha256 or row.get("size") != info.size:
            _fail(f"qualified BOI semantic artifact bytes differ: {relative!r}")
        semantic_paths.append(relative)
    if semantic_paths != sorted(expected_roles):
        _fail("qualified BOI semantic artifact paths are reordered or incomplete")
    expected_payload_paths = {OUTPUT_INVENTORY, *expected_roles}
    if set(payload_paths) != expected_payload_paths:
        _fail("signed BOI qualification payload inventory is not exact")
    expected_transport_paths = {
        *expected_payload_paths,
        QUALIFICATION_PAYLOAD_INVENTORY_PATH,
        QUALIFICATION_ENVELOPE_PATH,
        QUALIFICATION_SIGNATURE_PATH,
        QUALIFICATION_PUBLIC_KEY_PATH,
    }
    if set(transport_paths) != expected_transport_paths:
        _fail("qualified BOI transport and semantic inventories diverge")

    try:
        _, source_inventory_payload = stable_read_relative(
            root, SOURCE_HANDOFF_COPY_PATH,
            max_size=admission.MAX_BOI_ARTIFACT_INVENTORY_BYTES,
            return_payload=True,
        )
        _, candidate_admission_payload = stable_read_relative(
            root, CANDIDATE_ADMISSION_PATH, max_size=admission.MAX_JSON_BYTES,
            return_payload=True,
        )
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(str(exc)) from exc
    assert source_inventory_payload is not None
    assert candidate_admission_payload is not None
    if (
        source_inventory_payload != embedded.boi_artifact_inventory
        or hashlib.sha256(source_inventory_payload).hexdigest()
        != embedded.boi_artifact_inventory_sha256
    ):
        _fail("qualified BOI source inventory differs from the signed candidate")
    try:
        source_inventory = admission._validate_boi_artifact_inventory(
            source_inventory_payload,
            expected_source=expected_source,
            expected_exact12_matrix_sha256=embedded.exact12_matrix_sha256,
        )
    except admission.TairaRolloutAdmissionError as exc:
        raise BoiHandoffError(str(exc)) from exc
    semantic_by_path = {str(row["path"]): row for row in raw_rows}
    source_rows = source_inventory.get("files")
    if not isinstance(source_rows, list) or len(source_rows) != len(ARTIFACT_SPECS):
        _fail("qualified BOI source inventory result is not exact")
    for source_row in source_rows:
        if not isinstance(source_row, dict):
            _fail("qualified BOI source inventory row is malformed")
        relative = str(source_row.get("path", ""))
        semantic_row = semantic_by_path.get(relative)
        if (
            semantic_row is None
            or semantic_row["sha256"] != source_row.get("sha256")
            or semantic_row["size"] != source_row.get("size")
        ):
            _fail(f"qualified BOI source artifact differs: {relative!r}")
    if candidate_admission_payload != _candidate_admission_payload(embedded):
        _fail("qualified BOI admission projection differs from signed candidate")
    receipt_pairs = {
        QUALIFICATION_RECEIPT_PATH: embedded.qualification_receipt,
        PROTOCOL_RECEIPT_PATH: embedded.privacy_protocol_receipt,
        NATIVE_RECEIPT_NORITO_PATH: embedded.native_receipt_norito,
        NATIVE_RECEIPT_JSON_PATH: embedded.native_receipt_json,
    }
    for relative, expected_payload in receipt_pairs.items():
        try:
            _, actual_payload = stable_read_relative(
                root, relative, return_payload=True
            )
        except ReleaseArtifactError as exc:
            raise BoiHandoffError(str(exc)) from exc
        if actual_payload != expected_payload:
            _fail(f"qualified BOI receipt differs from signed candidate: {relative!r}")

    snapshot = QualifiedBoiSnapshot(
        root=root,
        files=captured,
        handoff_inventory_sha256=manifest_info.sha256,
        boi_inventory_sha256=captured[OUTPUT_INVENTORY].sha256,
        candidate_archive_sha256=embedded.archive_info.sha256,
        candidate_boi_artifact_inventory_sha256=(
            embedded.boi_artifact_inventory_sha256
        ),
        candidate_release_manifest_sha256=embedded.release_manifest_sha256,
        qualification_receipt_id=qualification_receipt_id,
        probe_transcript_sha256=hashlib.sha256(probe_transcript).hexdigest(),
        qualification_signer_fingerprint_sha256=qualification_fingerprint,
        trusted_qualification_public_key=trusted_qualification_key,
        trusted_qualification_public_key_state=(
            trusted_qualification_key_state
        ),
        source=normalized_source,
    )
    recheck_qualified_boi_handoff(snapshot)
    if stable_hash_path(external_archive) != external_archive_info:
        _fail("deployment candidate archive changed during BOI verification")
    for relative, before in external_authority_files.items():
        if stable_hash_relative(external_authority, relative) != before:
            _fail(f"deployment candidate authority changed: {relative!r}")
    return snapshot


def recheck_qualified_boi_handoff(snapshot: QualifiedBoiSnapshot) -> None:
    """Reject any qualified-handoff path, byte, or ordering change after admission."""

    try:
        if scan_inventory_paths(snapshot.root) != sorted(snapshot.files):
            _fail("qualified BOI handoff inventory changed after verification")
        for relative, before in snapshot.files.items():
            if stable_hash_relative(snapshot.root, relative) != before:
                _fail(f"qualified BOI handoff changed: {relative!r}")
        if (
            stable_hash_path(
                snapshot.trusted_qualification_public_key, max_size=32
            )
            != snapshot.trusted_qualification_public_key_state
        ):
            _fail("trusted BOI qualification public key changed after verification")
    except ReleaseArtifactError as exc:
        raise BoiHandoffError(f"cannot recheck qualified BOI handoff: {exc}") from exc


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
    parser.add_argument("--qualification-external-signer", type=Path, required=True)
    parser.add_argument(
        "--trusted-qualification-external-signer-sha256", required=True
    )
    parser.add_argument(
        "--qualification-signing-public-key", type=Path, required=True
    )
    parser.add_argument(
        "--trusted-qualification-signing-fingerprint", required=True
    )
    parser.add_argument("--qualification-host-id", required=True)
    parser.add_argument("--qualification-installation-id", required=True)
    parser.add_argument("--controller-closure-digest", required=True)
    parser.add_argument("--workflow-run-id", type=int, required=True)
    parser.add_argument("--workflow-run-attempt", type=int, required=True)
    parser.add_argument("--release-manifest-verifier", type=Path, required=True)
    parser.add_argument("--trusted-release-manifest-verifier-sha256", required=True)
    parser.add_argument("--python", default=sys.executable)
    parser.add_argument("--now-unix", type=int)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        require_boi_qualification_isolation(
            args.trusted_qualification_external_signer_sha256
        )
        candidate = authenticate_candidate(args)
        result = assemble_boi_handoff(
            Path(os.path.abspath(args.artifact_handoff_root)),
            Path(os.path.abspath(args.output)),
            candidate,
            python=args.python,
            qualification_external_signer=args.qualification_external_signer,
            qualification_signing_public_key=(
                args.qualification_signing_public_key
            ),
            trusted_qualification_signing_fingerprint=(
                args.trusted_qualification_signing_fingerprint
            ),
            qualification_host_id=args.qualification_host_id,
            qualification_installation_id=args.qualification_installation_id,
            controller_closure_digest=args.controller_closure_digest,
            workflow_run_id=args.workflow_run_id,
            workflow_run_attempt=args.workflow_run_attempt,
            release_manifest_verifier_path=args.release_manifest_verifier,
            trusted_release_manifest_verifier_sha256=(
                args.trusted_release_manifest_verifier_sha256
            ),
            qualification_issued_at_unix=args.now_unix,
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
