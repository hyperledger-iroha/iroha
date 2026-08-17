#!/usr/bin/env python3
"""Build authenticated candidate/publication handoffs for the public-v2 soak.

Both phases are offline and path-only.  They derive the exact prerequisite
identity consumed by ``check_taira_public_v2_24h_soak_evidence.py`` from frozen
handoff bytes and an authenticated installed ``macos-publish`` controller
attestation.  No source identity, receipt ID, artifact digest, signing key,
verifier digest, OCI digest, or controller digest is accepted on the command
line or through the environment.

The current Taira authority barriers remain authoritative: until the native
qualification and rollout-observation authorities are provisioned, the
corresponding phase refuses before inspecting caller-controlled paths.
"""

from __future__ import annotations

import argparse
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import stat
import sys
import tempfile
import time
from typing import Any, NoReturn

try:
    from . import check_taira_public_v2_24h_soak_evidence as soak_checker
    from . import close_taira_publication_handoff as publication_closer
    from . import publish_taira_rollout as publisher
    from . import seal_taira_release_controllers as controllers
    from . import taira_rollout_admission as admission
    from .release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_write_bytes,
    )
    from .release_manifest_signing import (
        ReleaseManifestSignatureError,
        verify_release_manifest,
    )
except ImportError:
    import check_taira_public_v2_24h_soak_evidence as soak_checker
    import close_taira_publication_handoff as publication_closer
    import publish_taira_rollout as publisher
    import seal_taira_release_controllers as controllers
    import taira_rollout_admission as admission
    from release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_write_bytes,
    )
    from release_manifest_signing import (
        ReleaseManifestSignatureError,
        verify_release_manifest,
    )


HANDOFF_SCHEMA = soak_checker.HANDOFF_SCHEMA
SCHEMA_VERSION = 1
CANDIDATE_FIELDS = frozenset(soak_checker.CANDIDATE_IDENTITY_FIELDS)
PUBLICATION_FIELDS = frozenset(soak_checker.PUBLICATION_IDENTITY_FIELDS)
SOURCE_FIELDS = frozenset(soak_checker.SOURCE_FIELDS)
DOCUMENT_FIELDS = frozenset(soak_checker.HANDOFF_DOCUMENT_FIELDS)

MAX_ATTESTATION_BYTES = 4 * 1024 * 1024
MAX_HANDOFF_BYTES = soak_checker.MAX_HANDOFF_BYTES
MAX_SOURCE_BYTES = 1024 * 1024
MAX_RECEIPT_ID_BYTES = 65
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
CANDIDATE_ROOT_RE = re.compile(r"publish-candidate-[1-9][0-9]{0,19}-[1-9][0-9]{0,9}")


class PrerequisiteHandoffError(RuntimeError):
    """A prerequisite could not be authenticated from the supplied bytes."""


@dataclass(frozen=True)
class FileIdentity:
    """Stable identity for one captured regular file."""

    device: int
    inode: int
    mode: int
    links: int
    uid: int
    gid: int
    size: int
    mtime_ns: int
    ctime_ns: int


@dataclass(frozen=True)
class CapturedFile:
    """Bound bytes and identity for one small input file."""

    path: Path
    payload: bytes
    sha256: str
    identity: FileIdentity


@dataclass(frozen=True)
class PublisherTrust:
    """Trust roots replayed from the installed publisher controller."""

    attestation: CapturedFile
    authenticated: Mapping[str, object]
    controller_sha256: str
    controller_uid: int
    controller_gid: int
    handoff_root: Path
    source_commit: str
    signing_fingerprint: str
    verifier_path: Path
    verifier_sha256: str
    verifier: publisher.CapturedFile
    oras_sha256: str
    oras_version: str
    repository: str
    suffix: str


@dataclass(frozen=True)
class CandidateState:
    """A frozen candidate and its replayed admission result."""

    candidate: publisher.Candidate
    source: admission.SourceIdentity
    receipt_id: str
    admission_result: Mapping[str, object]
    admission_payload: bytes


def _fail(message: str) -> NoReturn:
    raise PrerequisiteHandoffError(message)


def _sha256(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256_RE.fullmatch(value) is None
        or value == "0" * 64
    ):
        _fail(f"{label} must be one nonzero lowercase SHA-256 digest")
    return value


def _commit(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or COMMIT_RE.fullmatch(value) is None
        or value == "0" * 40
    ):
        _fail(f"{label} must be one nonzero lowercase 40-hex commit")
    return value


def _integer(
    value: object,
    label: str,
    *,
    minimum: int = 0,
    maximum: int | None = None,
) -> int:
    if (
        type(value) is not int
        or value < minimum
        or (maximum is not None and value > maximum)
    ):
        suffix = "" if maximum is None else f" and <= {maximum}"
        _fail(f"{label} must be an integer >= {minimum}{suffix}")
    return value


def _exact(value: object, fields: frozenset[str] | set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    actual = set(value)
    expected = set(fields)
    if actual != expected:
        _fail(
            f"{label} fields differ: missing={sorted(expected - actual)}, "
            f"extra={sorted(actual - expected)}"
        )
    return value


def _reject_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"JSON contains duplicate key {key!r}")
        result[key] = value
    return result


def _reject_constant(value: str) -> NoReturn:
    _fail(f"JSON contains non-finite number {value!r}")


def _strict_json(payload: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_reject_pairs,
            parse_constant=_reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise PrerequisiteHandoffError(f"{label} is not strict JSON") from error
    if not isinstance(value, dict):
        _fail(f"{label} root must be an object")
    return value


def _compact_json(value: object) -> bytes:
    try:
        return (
            json.dumps(
                value,
                ensure_ascii=True,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise PrerequisiteHandoffError(
            f"handoff is not canonically encodable: {error}"
        ) from error


def _identity(info: os.stat_result) -> FileIdentity:
    return FileIdentity(
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _absolute(path: Path, label: str, *, exists: bool = True) -> Path:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} must use one absolute normalized path")
    comparison = path if exists else path.parent
    try:
        if comparison.resolve(strict=True) != comparison:
            _fail(f"{label} must not traverse symbolic links")
    except OSError as error:
        raise PrerequisiteHandoffError(f"cannot resolve {label}: {path}") from error
    return path


def _capture_file(
    path: Path,
    label: str,
    maximum: int,
    *,
    private: bool = False,
    expected_uid: int | None = None,
    expected_gid: int | None = None,
    exact_mode: int | None = None,
) -> CapturedFile:
    path = _absolute(path, label)
    try:
        before = path.lstat()
    except OSError as error:
        raise PrerequisiteHandoffError(f"cannot inspect {label}: {path}") from error
    allowed_owners = {0, os.geteuid()}
    if expected_uid is not None:
        allowed_owners = {expected_uid}
    mode = stat.S_IMODE(before.st_mode)
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_uid not in allowed_owners
        or (expected_gid is not None and before.st_gid != expected_gid)
        or before.st_size <= 0
        or before.st_size > maximum
        or before.st_mode & 0o022
        or (private and mode not in {0o400, 0o600})
        or (exact_mode is not None and mode != exact_mode)
    ):
        _fail(f"{label} is not one bounded owner-controlled regular file")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise PrerequisiteHandoffError(f"cannot safely open {label}: {path}") from error
    try:
        opened = os.fstat(descriptor)
        if _identity(opened) != _identity(before):
            _fail(f"{label} changed while opening")
        payload = bytearray()
        while len(payload) < before.st_size:
            chunk = os.read(descriptor, min(1024 * 1024, before.st_size - len(payload)))
            if not chunk:
                _fail(f"{label} was truncated while reading")
            payload.extend(chunk)
        if os.read(descriptor, 1):
            _fail(f"{label} grew while reading")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    try:
        named = path.lstat()
    except OSError as error:
        raise PrerequisiteHandoffError(f"{label} path vanished while reading") from error
    if _identity(after) != _identity(before) or _identity(named) != _identity(before):
        _fail(f"{label} changed while reading")
    body = bytes(payload)
    return CapturedFile(path, body, hashlib.sha256(body).hexdigest(), _identity(before))


def _replay_file(captured: CapturedFile, label: str, maximum: int, **kwargs: object) -> None:
    replay = _capture_file(captured.path, label, maximum, **kwargs)
    if replay != captured:
        _fail(f"{label} changed during prerequisite production")


def _directory_identity(path: Path, label: str) -> tuple[int, int]:
    path = _absolute(path, label)
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode):
        _fail(f"{label} must be one non-symlink directory")
    return info.st_dev, info.st_ino


def _reject_output_within(output: Path, root: Path, label: str) -> None:
    output = _absolute(output, "prerequisite output", exists=False)
    root = _absolute(root, label)
    if output == root or root in output.parents:
        _fail(f"prerequisite output must not modify the frozen {label}")


def _distinct_file_identities(files: Sequence[tuple[int, int]], label: str) -> None:
    if len(files) != len(set(files)):
        _fail(f"{label} inputs contain a filesystem alias")


def _publisher_record(
    rows: object,
    *,
    flag: str,
    label: str,
) -> Mapping[str, object]:
    if not isinstance(rows, list):
        _fail(f"publisher controller {label} records are absent")
    matches = [
        row
        for row in rows
        if isinstance(row, dict)
        and row.get("operation") == "publish-rollout"
        and row.get("flag") == flag
    ]
    if len(matches) != 1:
        _fail(f"publisher controller lacks one exact {label} record for {flag}")
    return matches[0]


def _authenticate_controller_attestation(path: Path) -> PublisherTrust:
    captured = _capture_file(
        path,
        "publisher controller attestation",
        MAX_ATTESTATION_BYTES,
        private=True,
    )
    value = _strict_json(captured.payload, "publisher controller attestation")
    if captured.payload != controllers.canonical_json_bytes(value):
        _fail("publisher controller attestation is not canonical compact JSON")
    required_scalars = {
        "controller_digest": _sha256(
            value.get("controller_digest"), "publisher controller digest"
        ),
        "launcher_sha256": _sha256(
            value.get("launcher_sha256"), "publisher launcher digest"
        ),
        "source_commit": _commit(
            value.get("source_commit"), "publisher controller source commit"
        ),
    }
    for field in (
        "controller_version",
        "host_id",
        "installation_id",
        "platform",
        "role",
    ):
        if not isinstance(value.get(field), str) or not value[field]:
            _fail(f"publisher controller attestation {field} is absent")
    uid = _integer(value.get("uid"), "publisher controller UID")
    gid = _integer(value.get("controller_gid"), "publisher controller GID")
    try:
        replay = controllers._attest(
            expected_launcher_sha256=required_scalars["launcher_sha256"],
            expected_controller_digest=required_scalars["controller_digest"],
            expected_version=value["controller_version"],
            expected_host_id=value["host_id"],
            expected_installation_id=value["installation_id"],
            expected_uid=str(uid),
            source_commit=required_scalars["source_commit"],
            platform_name=value["platform"],
            role=value["role"],
        )
    except controllers.ControllerSealError as error:
        raise PrerequisiteHandoffError(
            f"publisher installed-controller attestation failed replay: {error}"
        ) from error
    if replay != value:
        _fail("publisher controller attestation differs from current installed state")
    if value["platform"] != "macos" or value["role"] != "macos-publish":
        _fail("publisher controller attestation is not the macos-publish authority")

    fingerprint_record = _publisher_record(
        value.get("trusted_values"),
        flag="--trusted-signing-fingerprint",
        label="trusted literal",
    )
    oras_version_record = _publisher_record(
        value.get("trusted_values"),
        flag="--expected-oras-version",
        label="trusted literal",
    )
    repository_record = _publisher_record(
        value.get("trusted_values"), flag="--repository", label="trusted literal"
    )
    suffix_record = _publisher_record(
        value.get("trusted_values"), flag="--suffix", label="trusted literal"
    )
    verifier_record = _publisher_record(
        value.get("trusted_executables"),
        flag="--release-manifest-verifier",
        label="trusted executable",
    )
    oras_record = _publisher_record(
        value.get("trusted_executables"),
        flag="--oras",
        label="trusted executable",
    )
    fingerprint = _sha256(
        fingerprint_record.get("value"), "publisher signing fingerprint"
    )
    verifier_sha256 = _sha256(
        verifier_record.get("sha256"), "publisher native verifier digest"
    )
    oras_sha256 = _sha256(oras_record.get("sha256"), "publisher ORAS digest")
    verifier_path_raw = verifier_record.get("path")
    if not isinstance(verifier_path_raw, str):
        _fail("publisher native verifier path is absent")
    verifier_path = Path(verifier_path_raw)
    try:
        verifier = publisher._capture_pinned_executable(
            verifier_path, verifier_sha256, "publisher native verifier"
        )
    except publisher.TairaPublicationError as error:
        raise PrerequisiteHandoffError(str(error)) from error
    handoff_raw = value.get("handoff_root")
    if not isinstance(handoff_raw, str):
        _fail("publisher handoff root is absent")
    handoff_root = _absolute(Path(handoff_raw), "publisher handoff root")
    root_info = handoff_root.lstat()
    if (
        not stat.S_ISDIR(root_info.st_mode)
        or stat.S_ISLNK(root_info.st_mode)
        or root_info.st_uid != uid
        or root_info.st_gid != gid
        or stat.S_IMODE(root_info.st_mode) != 0o711
    ):
        _fail("publisher handoff root differs from the authenticated attestation")
    repository = repository_record.get("value")
    suffix = suffix_record.get("value")
    oras_version = oras_version_record.get("value")
    if not all(isinstance(item, str) for item in (repository, suffix, oras_version)):
        _fail("publisher repository, suffix, or ORAS version trust is absent")
    try:
        repository, suffix, _registry = publisher._validate_repository(
            str(repository), str(suffix)
        )
    except publisher.TairaPublicationError as error:
        raise PrerequisiteHandoffError(str(error)) from error
    if publisher.VERSION_RE.fullmatch(str(oras_version)) is None:
        _fail("publisher attested ORAS version is noncanonical")
    _replay_file(
        captured,
        "publisher controller attestation",
        MAX_ATTESTATION_BYTES,
        private=True,
    )
    return PublisherTrust(
        captured,
        value,
        required_scalars["controller_digest"],
        uid,
        gid,
        handoff_root,
        required_scalars["source_commit"],
        fingerprint,
        verifier_path,
        verifier_sha256,
        verifier,
        oras_sha256,
        str(oras_version),
        repository,
        suffix,
    )


def _replay_publisher_trust(trust: PublisherTrust) -> None:
    """Re-attest the installed closure and every publisher trust record."""

    replay = _authenticate_controller_attestation(trust.attestation.path)
    if replay != trust:
        _fail("publisher installed-controller trust changed during production")


def _require_candidate_authorities() -> None:
    try:
        admission._require_privacy_protocol_controller_origin_authority()
        admission._require_independent_native_evidence_authority()
    except admission.TairaRolloutAdmissionError as error:
        raise PrerequisiteHandoffError(str(error)) from error


def _require_publication_authorities() -> None:
    try:
        publisher._require_authenticated_rollout_observation_authority()
    except publisher.TairaPublicationError as error:
        raise PrerequisiteHandoffError(str(error)) from error
    _require_candidate_authorities()


def _source_identity(value: object, label: str) -> admission.SourceIdentity:
    source = _exact(value, SOURCE_FIELDS, label)
    return admission.SourceIdentity(
        commit=_commit(source["commit"], f"{label}.commit"),
        dpn_validator_release_commit=_commit(
            source["dpn_validator_release_commit"],
            f"{label}.dpn_validator_release_commit",
        ),
        cargo_lock_sha256=_sha256(
            source["cargo_lock_sha256"], f"{label}.cargo_lock_sha256"
        ),
        workspace_source_manifest_sha256=_sha256(
            source["workspace_source_manifest_sha256"],
            f"{label}.workspace_source_manifest_sha256",
        ),
    )


def _capture_candidate_seed(
    root: Path, trust: PublisherTrust
) -> tuple[admission.SourceIdentity, str, CapturedFile, CapturedFile]:
    root = _absolute(root, "candidate root")
    if root.parent != trust.handoff_root or CANDIDATE_ROOT_RE.fullmatch(root.name) is None:
        _fail("candidate root is not one authenticated publisher handoff")
    source_file = _capture_file(
        root / publisher.SOURCE_IDENTITY_NAME,
        "candidate source identity",
        MAX_SOURCE_BYTES,
        expected_uid=trust.controller_uid,
        expected_gid=trust.controller_gid,
        exact_mode=0o444,
    )
    source_value = _strict_json(source_file.payload, "candidate source identity")
    if source_file.payload != publisher._canonical_compact(source_value):
        _fail("candidate source identity is not canonical compact JSON")
    _exact(source_value, {"source", "source_date_epoch"}, "candidate source identity")
    _integer(source_value["source_date_epoch"], "candidate source epoch", minimum=1)
    source = _source_identity(source_value["source"], "candidate source")
    if source.commit != trust.source_commit:
        _fail("candidate source commit differs from the publisher controller")
    receipt_file = _capture_file(
        root / publisher.RECEIPT_ID_NAME,
        "candidate qualification receipt ID",
        MAX_RECEIPT_ID_BYTES,
        expected_uid=trust.controller_uid,
        expected_gid=trust.controller_gid,
        exact_mode=0o444,
    )
    try:
        receipt_text = receipt_file.payload.decode("ascii")
    except UnicodeDecodeError as error:
        raise PrerequisiteHandoffError(
            "candidate qualification receipt ID is not ASCII"
        ) from error
    if not receipt_text.endswith("\n") or receipt_text.count("\n") != 1:
        _fail("candidate qualification receipt ID is noncanonical")
    receipt_id = _sha256(receipt_text[:-1], "candidate qualification receipt ID")
    return source, receipt_id, source_file, receipt_file


def _candidate_file_identities(
    candidate: publisher.Candidate,
) -> list[tuple[int, int]]:
    return [
        (captured.identity.device, captured.identity.inode)
        for captured in candidate.files.values()
    ]


def _validate_admission_result(
    result: object,
    *,
    candidate: publisher.Candidate,
    source: admission.SourceIdentity,
    receipt_id: str,
    trust: PublisherTrust,
) -> Mapping[str, object]:
    if not isinstance(result, dict):
        _fail("candidate admission result must be an object")
    authority_manifest = candidate.files["authority/release_manifest.json"]
    archive = candidate.files[candidate.archive_relative]
    if (
        result.get("schema") != admission.VERIFICATION_SCHEMA
        or result.get("schema_version") != admission.VERIFICATION_SCHEMA_VERSION
        or result.get("verified") is not True
        or result.get("deployment_performed") is not False
        or result.get("peer_count") != admission.PEER_COUNT
        or result.get("source") != source.as_dict()
        or result.get("receipt_id") != receipt_id
        or result.get("archive_sha256") != archive.sha256
        or result.get("release_manifest_sha256") != authority_manifest.sha256
        or result.get("signer_fingerprint_sha256") != trust.signing_fingerprint
        or result.get("release_manifest_verifier_sha256") != trust.verifier_sha256
    ):
        _fail("candidate admission result differs from the exact frozen candidate")
    _sha256(result.get("validator_binary_sha256"), "admitted validator binary digest")
    return result


def _admit_candidate(
    candidate: publisher.Candidate,
    source: admission.SourceIdentity,
    receipt_id: str,
    trust: PublisherTrust,
    temporary_parent: Path,
) -> tuple[Mapping[str, object], bytes]:
    temporary_parent = _absolute(temporary_parent, "prerequisite temporary parent")
    with tempfile.TemporaryDirectory(
        prefix=".taira-prerequisite-admission-", dir=temporary_parent
    ) as raw_temp:
        temp = Path(raw_temp).resolve(strict=True)
        temp_info = temp.lstat()
        if (
            not stat.S_ISDIR(temp_info.st_mode)
            or stat.S_ISLNK(temp_info.st_mode)
            or temp_info.st_uid != os.geteuid()
            or stat.S_IMODE(temp_info.st_mode) != 0o700
        ):
            _fail("admission replay directory is not owner-private")
        ledger = temp / "empty-replay-ledger.json"
        exclusive_write_bytes(
            ledger, admission.canonical_replay_ledger_bytes(()), mode=0o600
        )
        try:
            result = admission.verify_admission(
                archive_path=candidate.archive,
                authority_dir=candidate.authority,
                expected_source=source,
                expected_receipt_id=receipt_id,
                replay_ledger_path=ledger,
                trusted_signing_fingerprint=trust.signing_fingerprint,
                release_manifest_verifier_path=trust.verifier_path,
                trusted_release_manifest_verifier_sha256=trust.verifier_sha256,
                now_unix=int(time.time()),
            )
        except admission.TairaRolloutAdmissionError as error:
            raise PrerequisiteHandoffError(str(error)) from error
        validated = _validate_admission_result(
            result,
            candidate=candidate,
            source=source,
            receipt_id=receipt_id,
            trust=trust,
        )
        payload = canonical_json_bytes(validated)
    return validated, payload


def _capture_and_admit_candidate(
    root: Path,
    trust: PublisherTrust,
    temporary_parent: Path,
) -> CandidateState:
    source, receipt_id, source_seed, receipt_seed = _capture_candidate_seed(root, trust)
    try:
        candidate = publisher._capture_candidate(root, source, receipt_id)
    except publisher.TairaPublicationError as error:
        raise PrerequisiteHandoffError(str(error)) from error
    if (
        candidate.files[publisher.SOURCE_IDENTITY_NAME].sha256 != source_seed.sha256
        or candidate.files[publisher.RECEIPT_ID_NAME].sha256 != receipt_seed.sha256
        or (
            candidate.files[publisher.SOURCE_IDENTITY_NAME].identity.device,
            candidate.files[publisher.SOURCE_IDENTITY_NAME].identity.inode,
        )
        != (source_seed.identity.device, source_seed.identity.inode)
        or (
            candidate.files[publisher.RECEIPT_ID_NAME].identity.device,
            candidate.files[publisher.RECEIPT_ID_NAME].identity.inode,
        )
        != (receipt_seed.identity.device, receipt_seed.identity.inode)
    ):
        _fail("candidate identity files changed between derivation and inventory capture")
    identities = _candidate_file_identities(candidate)
    _distinct_file_identities(identities, "candidate")
    result, admission_payload = _admit_candidate(
        candidate, source, receipt_id, trust, temporary_parent
    )
    try:
        publisher._assert_candidate_unchanged(candidate)
        publisher._assert_file_unchanged(trust.verifier, "publisher native verifier")
    except publisher.TairaPublicationError as error:
        raise PrerequisiteHandoffError(str(error)) from error
    _replay_publisher_trust(trust)
    return CandidateState(candidate, source, receipt_id, result, admission_payload)


def _candidate_identity(state: CandidateState) -> dict[str, str]:
    return {
        "admission_archive_sha256": state.candidate.files[
            state.candidate.archive_relative
        ].sha256,
        "admission_authority_manifest_sha256": state.candidate.files[
            "authority/release_manifest.json"
        ].sha256,
        "handoff_inventory_sha256": state.candidate.files[
            publisher.HANDOFF_MANIFEST
        ].sha256,
        "qualification_receipt_id": state.receipt_id,
        "validator_binary_sha256": str(
            state.admission_result["validator_binary_sha256"]
        ),
    }


def _document(kind: str, source: admission.SourceIdentity, identity: Mapping[str, str]) -> dict[str, object]:
    expected = CANDIDATE_FIELDS if kind == "candidate" else PUBLICATION_FIELDS
    if set(identity) != set(expected):
        _fail(f"{kind} prerequisite identity fields differ from the soak checker")
    return {
        "identity": dict(identity),
        "kind": kind,
        "schema": HANDOFF_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "source": source.as_dict(),
    }


def _load_candidate_document(path: Path) -> tuple[CapturedFile, Mapping[str, str], admission.SourceIdentity]:
    captured = _capture_file(path, "candidate prerequisite handoff", MAX_HANDOFF_BYTES)
    value = _strict_json(captured.payload, "candidate prerequisite handoff")
    if captured.payload != _compact_json(value):
        _fail("candidate prerequisite handoff is not canonical compact JSON")
    _exact(value, DOCUMENT_FIELDS, "candidate prerequisite handoff")
    if (
        value["schema"] != HANDOFF_SCHEMA
        or value["schema_version"] != SCHEMA_VERSION
        or value["kind"] != "candidate"
    ):
        _fail("candidate prerequisite handoff identity differs")
    source = _source_identity(value["source"], "candidate prerequisite source")
    identity_value = _exact(
        value["identity"], CANDIDATE_FIELDS, "candidate prerequisite identity"
    )
    identity: dict[str, str] = {
        field: _sha256(identity_value[field], f"candidate prerequisite {field}")
        for field in CANDIDATE_FIELDS
    }
    return captured, identity, source


def _publication_file_identities(
    files: Mapping[str, publication_closer.Captured],
) -> list[tuple[int, int]]:
    return [(row.identity[0], row.identity[1]) for row in files.values()]


def _capture_publication_root(
    root: Path,
    *,
    trust: PublisherTrust,
    source: admission.SourceIdentity,
    receipt_id: str,
) -> tuple[int, tuple[int, ...], dict[str, publication_closer.Captured]]:
    root = _absolute(root, "publication handoff root")
    if (
        root.parent != trust.handoff_root
        or root.name != publication_closer.OUTPUT_PREFIX + receipt_id
    ):
        _fail("publication root is not the exact root-closed receipt handoff")
    descriptor = -1
    try:
        descriptor, identity = publication_closer._open_bound_directory(
            root,
            "publication handoff root",
            uid=trust.controller_uid,
            gid=trust.controller_gid,
            mode=0o555,
        )
        if sorted(os.listdir(descriptor)) != sorted(publication_closer.TERMINAL_FILES):
            _fail("publication handoff inventory is not exactly seven files")
        files = {
            name: publication_closer._read_frozen_at(
                descriptor,
                name,
                uid=trust.controller_uid,
                gid=trust.controller_gid,
            )
            for name in sorted(publication_closer.TERMINAL_FILES)
        }
        publication_closer._validate_payload_bindings(
            files,
            receipt_id,
            trust.signing_fingerprint,
            expected_source_commit=source.commit,
            expected_dpn_validator_release_commit=(
                source.dpn_validator_release_commit
            ),
            expected_cargo_lock_sha256=source.cargo_lock_sha256,
            expected_workspace_source_manifest_sha256=(
                source.workspace_source_manifest_sha256
            ),
        )
    except publication_closer.PublicationHandoffError as error:
        if descriptor >= 0:
            os.close(descriptor)
        raise PrerequisiteHandoffError(str(error)) from error
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        raise
    return descriptor, identity, files


def _replay_publication_root(
    root: Path,
    descriptor: int,
    identity: tuple[int, ...],
    files: Mapping[str, publication_closer.Captured],
    trust: PublisherTrust,
) -> None:
    try:
        if publication_closer._directory_identity(os.fstat(descriptor)) != identity:
            _fail("publication handoff root changed during validation")
        named = root.lstat()
        if publication_closer._directory_identity(named) != identity:
            _fail("publication handoff root path changed during validation")
        if sorted(os.listdir(descriptor)) != sorted(publication_closer.TERMINAL_FILES):
            _fail("publication handoff inventory changed during validation")
        for captured in files.values():
            publication_closer._replay_at(
                descriptor,
                captured,
                uid=trust.controller_uid,
                gid=trust.controller_gid,
            )
    except publication_closer.PublicationHandoffError as error:
        raise PrerequisiteHandoffError(str(error)) from error


def _receipt_layers(files: Mapping[str, publication_closer.Captured]) -> list[publisher.Layer]:
    rows = (
        (publisher.PUBLICATION_RECEIPT_NAME, publisher.PUBLICATION_RECEIPT_MEDIA_TYPE),
        (publisher.PUBLICATION_SIGNATURE_NAME, publisher.AUTHORITY_SIGNATURE_MEDIA_TYPE),
        (publisher.PUBLICATION_PUBLIC_KEY_NAME, publisher.AUTHORITY_PUBLIC_KEY_MEDIA_TYPE),
    )
    return [
        publisher.Layer(name, media_type, files[name].sha256, len(files[name].payload))
        for name, media_type in rows
    ]


def _digest_payload(payload: bytes, label: str) -> str:
    try:
        value = payload.decode("ascii")
    except UnicodeDecodeError as error:
        raise PrerequisiteHandoffError(f"{label} is not ASCII") from error
    if not value.endswith("\n") or value.count("\n") != 1:
        _fail(f"{label} is not one canonical OCI digest line")
    digest = value[:-1]
    try:
        publisher._oci_digest(digest, label)
    except publisher.TairaPublicationError as error:
        raise PrerequisiteHandoffError(str(error)) from error
    return digest


def _validate_publication_receipt(
    files: Mapping[str, publication_closer.Captured],
    *,
    state: CandidateState,
    trust: PublisherTrust,
    primary_digest: str,
) -> tuple[Mapping[str, object], str]:
    payload = files[publisher.PUBLICATION_RECEIPT_NAME].payload
    value = _strict_json(payload, "publication receipt")
    if payload != canonical_json_bytes(value):
        _fail("publication receipt is not canonical deterministic JSON")
    _exact(
        value,
        {
            "admission_sha256",
            "immutable_reference",
            "issued_at_unix",
            "layers",
            "oras",
            "qualification_receipt_id",
            "repository",
            "schema",
            "schema_version",
            "signing",
            "source",
            "subject",
            "suffix",
            "tag",
            "tagged_reference",
        },
        "publication receipt",
    )
    candidate_layers = publisher._candidate_layers(state.candidate)
    expected_tag = f"taira-{publisher._source_identity_digest(state.source)}"
    if trust.suffix:
        expected_tag += f"-{trust.suffix}"
    expected_reference = f"{trust.repository}@{primary_digest}"
    expected_tagged = f"{trust.repository}:{expected_tag}"
    issued = _integer(
        value["issued_at_unix"],
        "publication issue time",
        minimum=1,
        maximum=publisher.MAX_PUBLICATION_UNIX,
    )
    oras = _exact(value["oras"], {"executable_sha256", "version"}, "publication ORAS")
    signing = _exact(
        value["signing"],
        {"native_verifier_sha256", "signer_fingerprint_sha256"},
        "publication signing",
    )
    subject = _exact(
        value["subject"], {"digest", "media_type", "size"}, "publication subject"
    )
    if (
        value["schema"] != publisher.PUBLICATION_SCHEMA
        or value["schema_version"] != publisher.PUBLICATION_SCHEMA_VERSION
        or value["qualification_receipt_id"] != state.receipt_id
        or value["source"] != state.source.as_dict()
        or value["admission_sha256"]
        != hashlib.sha256(state.admission_payload).hexdigest()
        or value["layers"] != [layer.receipt_row() for layer in candidate_layers]
        or value["repository"] != trust.repository
        or value["suffix"] != trust.suffix
        or value["tag"] != expected_tag
        or value["tagged_reference"] != expected_tagged
        or value["immutable_reference"] != expected_reference
        or oras
        != {"executable_sha256": trust.oras_sha256, "version": trust.oras_version}
        or signing
        != {
            "native_verifier_sha256": trust.verifier_sha256,
            "signer_fingerprint_sha256": trust.signing_fingerprint,
        }
        or subject
        != {
            "digest": primary_digest,
            "media_type": publisher.OCI_MANIFEST_MEDIA_TYPE,
            "size": len(files[publisher.PRIMARY_MANIFEST_NAME].payload),
        }
    ):
        _fail("publication receipt does not bind the exact admitted candidate")
    created = (
        dt.datetime(1970, 1, 1, tzinfo=dt.timezone.utc)
        + dt.timedelta(seconds=issued)
    ).strftime("%Y-%m-%dT%H:%M:%SZ")
    return value, created


def _validate_publication_bytes(
    files: Mapping[str, publication_closer.Captured],
    *,
    publication_root: Path,
    state: CandidateState,
    trust: PublisherTrust,
) -> tuple[str, str]:
    primary_digest = _digest_payload(
        files[publisher.PRIMARY_DIGEST_NAME].payload, "published primary digest"
    )
    receipt_digest = _digest_payload(
        files[publisher.RECEIPT_DIGEST_NAME].payload, "published receipt digest"
    )
    _receipt, created = _validate_publication_receipt(
        files, state=state, trust=trust, primary_digest=primary_digest
    )
    try:
        verification = verify_release_manifest(
            publication_root / publisher.PUBLICATION_RECEIPT_NAME,
            publication_root / publisher.PUBLICATION_SIGNATURE_NAME,
            publication_root / publisher.PUBLICATION_PUBLIC_KEY_NAME,
            trust.signing_fingerprint,
            trust.verifier_path,
            trust.verifier_sha256,
        )
    except ReleaseManifestSignatureError as error:
        raise PrerequisiteHandoffError(
            f"publication receipt signature verification failed: {error}"
        ) from error
    if (
        verification.get("signature_verified") is not True
        or verification.get("manifest_sha256")
        != files[publisher.PUBLICATION_RECEIPT_NAME].sha256
        or verification.get("signer_fingerprint_sha256") != trust.signing_fingerprint
        or verification.get("native_verifier_sha256") != trust.verifier_sha256
    ):
        _fail("publication signature verifier result differs from the captured bytes")
    candidate_layers = publisher._candidate_layers(state.candidate)
    receipt_layers = _receipt_layers(files)
    try:
        publisher._validate_raw_manifest(
            files[publisher.PRIMARY_MANIFEST_NAME].payload,
            digest=primary_digest,
            expected_size=len(files[publisher.PRIMARY_MANIFEST_NAME].payload),
            artifact_type=publisher.PRIMARY_ARTIFACT_TYPE,
            layers=candidate_layers,
            created=created,
            subject=None,
            label="primary OCI manifest",
        )
        publisher._validate_raw_manifest(
            files[publisher.RECEIPT_MANIFEST_NAME].payload,
            digest=receipt_digest,
            expected_size=len(files[publisher.RECEIPT_MANIFEST_NAME].payload),
            artifact_type=publisher.PUBLICATION_ARTIFACT_TYPE,
            layers=receipt_layers,
            created=created,
            subject=(primary_digest, len(files[publisher.PRIMARY_MANIFEST_NAME].payload)),
            label="publication receipt OCI manifest",
        )
    except publisher.TairaPublicationError as error:
        raise PrerequisiteHandoffError(str(error)) from error
    return primary_digest.removeprefix("sha256:"), receipt_digest.removeprefix("sha256:")


def _validate_output_parent(path: Path) -> tuple[int, str, tuple[int, int]]:
    path = _absolute(path, "prerequisite output", exists=False)
    parent = path.parent
    info = parent.lstat()
    if (
        not stat.S_ISDIR(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or info.st_uid not in {0, os.geteuid()}
        or info.st_mode & 0o022
    ):
        _fail("prerequisite output parent is not owner-controlled")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(parent, flags)
    opened = os.fstat(descriptor)
    if (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino):
        os.close(descriptor)
        _fail("prerequisite output parent changed while opening")
    return descriptor, path.name, (opened.st_dev, opened.st_ino)


def _write_atomic_no_replace(path: Path, payload: bytes) -> None:
    parent_fd, leaf, parent_identity = _validate_output_parent(path)
    temporary = f".{leaf}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    descriptor = -1
    installed_identity: tuple[int, int] | None = None
    linked = False
    try:
        flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        descriptor = os.open(temporary, flags, 0o600, dir_fd=parent_fd)
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail("short prerequisite handoff write")
            view = view[written:]
        os.fchmod(descriptor, 0o400)
        os.fsync(descriptor)
        temporary_info = os.fstat(descriptor)
        if (
            not stat.S_ISREG(temporary_info.st_mode)
            or temporary_info.st_uid != os.geteuid()
            or stat.S_IMODE(temporary_info.st_mode) != 0o400
            or temporary_info.st_size != len(payload)
            or temporary_info.st_nlink != 1
        ):
            _fail("prerequisite temporary output identity differs")
        installed_identity = (temporary_info.st_dev, temporary_info.st_ino)
        os.close(descriptor)
        descriptor = -1
        os.link(
            temporary,
            leaf,
            src_dir_fd=parent_fd,
            dst_dir_fd=parent_fd,
            follow_symlinks=False,
        )
        linked = True
        installed = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        if (
            (installed.st_dev, installed.st_ino) != installed_identity
            or not stat.S_ISREG(installed.st_mode)
            or installed.st_nlink != 2
        ):
            _fail("prerequisite output changed during atomic publication")
        os.unlink(temporary, dir_fd=parent_fd)
        final = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        if (
            (final.st_dev, final.st_ino) != installed_identity
            or final.st_nlink != 1
            or stat.S_IMODE(final.st_mode) != 0o400
            or (os.fstat(parent_fd).st_dev, os.fstat(parent_fd).st_ino)
            != parent_identity
        ):
            _fail("prerequisite output identity differs after publication")
        os.fsync(parent_fd)
        linked = False
    except FileExistsError as error:
        raise PrerequisiteHandoffError(
            "prerequisite output already exists; refusing to overwrite it"
        ) from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            os.unlink(temporary, dir_fd=parent_fd)
        except FileNotFoundError:
            pass
        if linked and installed_identity is not None:
            try:
                current = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
                if (current.st_dev, current.st_ino) == installed_identity:
                    os.unlink(leaf, dir_fd=parent_fd)
                    os.fsync(parent_fd)
            except FileNotFoundError:
                pass
        os.close(parent_fd)
    replay = _capture_file(path, "published prerequisite handoff", MAX_HANDOFF_BYTES)
    if replay.payload != payload or stat.S_IMODE(replay.identity.mode) != 0o400:
        _fail("published prerequisite handoff failed byte replay")


def build_candidate_handoff(
    candidate_root: Path,
    controller_attestation: Path,
    output: Path,
) -> dict[str, object]:
    """Replay one candidate admission and atomically publish its soak handoff."""

    # These provenance barriers must remain before every caller-controlled path read.
    _require_candidate_authorities()
    trust = _authenticate_controller_attestation(controller_attestation)
    validation_fd, _leaf, _identity_pair = _validate_output_parent(output)
    os.close(validation_fd)
    _reject_output_within(output, candidate_root, "candidate root")
    state = _capture_and_admit_candidate(candidate_root, trust, output.parent)
    identity = _candidate_identity(state)
    document = _document("candidate", state.source, identity)
    payload = _compact_json(document)
    _write_atomic_no_replace(output, payload)
    return document


def build_publication_handoff(
    candidate_root: Path,
    candidate_handoff: Path,
    publication_root: Path,
    controller_attestation: Path,
    output: Path,
) -> dict[str, object]:
    """Replay one signed OCI publication and publish its exact soak handoff."""

    # The observation and native provenance barriers precede all supplied path I/O.
    _require_publication_authorities()
    trust = _authenticate_controller_attestation(controller_attestation)
    validation_fd, _leaf, _identity_pair = _validate_output_parent(output)
    os.close(validation_fd)
    _reject_output_within(output, candidate_root, "candidate root")
    _reject_output_within(output, publication_root, "publication handoff root")
    candidate_document, candidate_identity, document_source = _load_candidate_document(
        candidate_handoff
    )
    if document_source.commit != trust.source_commit:
        _fail("candidate handoff source differs from the publisher controller")
    state = _capture_and_admit_candidate(candidate_root, trust, output.parent)
    derived_candidate = _candidate_identity(state)
    if document_source != state.source or candidate_identity != derived_candidate:
        _fail("candidate prerequisite handoff differs from the replayed candidate bytes")
    publication_descriptor = -1
    try:
        publication_descriptor, publication_identity, files = _capture_publication_root(
            publication_root,
            trust=trust,
            source=state.source,
            receipt_id=state.receipt_id,
        )
        _distinct_file_identities(
            [
                *_candidate_file_identities(state.candidate),
                *_publication_file_identities(files),
                (
                    candidate_document.identity.device,
                    candidate_document.identity.inode,
                ),
                (trust.attestation.identity.device, trust.attestation.identity.inode),
                (trust.verifier.identity.device, trust.verifier.identity.inode),
            ],
            "candidate/publication",
        )
        if _directory_identity(candidate_root, "candidate root") == _directory_identity(
            publication_root, "publication handoff root"
        ):
            _fail("candidate and publication roots are filesystem aliases")
        primary_manifest_sha256, receipt_manifest_sha256 = _validate_publication_bytes(
            files,
            publication_root=publication_root,
            state=state,
            trust=trust,
        )
        try:
            publisher._assert_candidate_unchanged(state.candidate)
            publisher._assert_file_unchanged(
                trust.verifier, "publisher native verifier"
            )
        except publisher.TairaPublicationError as error:
            raise PrerequisiteHandoffError(str(error)) from error
        _replay_file(
            candidate_document,
            "candidate prerequisite handoff",
            MAX_HANDOFF_BYTES,
        )
        _replay_publisher_trust(trust)
        _replay_publication_root(
            publication_root,
            publication_descriptor,
            publication_identity,
            files,
            trust,
        )
        identity = {
            "admission_archive_sha256": derived_candidate[
                "admission_archive_sha256"
            ],
            "candidate_handoff_sha256": candidate_document.sha256,
            "handoff_inventory_sha256": derived_candidate[
                "handoff_inventory_sha256"
            ],
            "publication_public_key_sha256": files[
                publisher.PUBLICATION_PUBLIC_KEY_NAME
            ].sha256,
            "publication_receipt_sha256": files[
                publisher.PUBLICATION_RECEIPT_NAME
            ].sha256,
            "publication_signature_sha256": files[
                publisher.PUBLICATION_SIGNATURE_NAME
            ].sha256,
            "published_primary_oci_manifest_sha256": primary_manifest_sha256,
            "published_receipt_oci_manifest_sha256": receipt_manifest_sha256,
            "publisher_controller_sha256": trust.controller_sha256,
            "qualification_receipt_id": state.receipt_id,
            "validator_binary_sha256": derived_candidate[
                "validator_binary_sha256"
            ],
        }
        document = _document("publication", state.source, identity)
        payload = _compact_json(document)
        _write_atomic_no_replace(output, payload)
        return document
    finally:
        if publication_descriptor >= 0:
            os.close(publication_descriptor)


def build_parser() -> argparse.ArgumentParser:
    """Build the path-only prerequisite handoff command line."""

    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    subparsers = parser.add_subparsers(dest="command", required=True)
    candidate = subparsers.add_parser("candidate", allow_abbrev=False)
    candidate.add_argument("--candidate-root", type=Path, required=True)
    candidate.add_argument(
        "--publisher-controller-attestation", type=Path, required=True
    )
    candidate.add_argument("--output", type=Path, required=True)
    publication = subparsers.add_parser("publication", allow_abbrev=False)
    publication.add_argument("--candidate-root", type=Path, required=True)
    publication.add_argument("--candidate-handoff", type=Path, required=True)
    publication.add_argument("--publication-root", type=Path, required=True)
    publication.add_argument(
        "--publisher-controller-attestation", type=Path, required=True
    )
    publication.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Produce one prerequisite handoff, refusing without authenticated roots."""

    args = build_parser().parse_args(argv)
    try:
        document = (
            build_candidate_handoff(
                args.candidate_root,
                args.publisher_controller_attestation,
                args.output,
            )
            if args.command == "candidate"
            else build_publication_handoff(
                args.candidate_root,
                args.candidate_handoff,
                args.publication_root,
                args.publisher_controller_attestation,
                args.output,
            )
        )
    except (
        OSError,
        ReleaseArtifactError,
        ReleaseManifestSignatureError,
        PrerequisiteHandoffError,
        admission.TairaRolloutAdmissionError,
        publisher.TairaPublicationError,
        controllers.ControllerSealError,
        publication_closer.PublicationHandoffError,
    ) as error:
        print(f"Taira public-v2 prerequisite handoff refused: {error}", file=sys.stderr)
        return 1
    summary = {
        "kind": document["kind"],
        "output": str(args.output),
        "sha256": hashlib.sha256(_compact_json(document)).hexdigest(),
    }
    sys.stdout.buffer.write(_compact_json(summary))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
