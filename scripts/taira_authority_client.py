#!/usr/bin/env python3
"""Fixed client for the installed Taira release-authority services.

Production callers cannot select the verifier binary, public binding, Unix
socket, service identity, or role.  The native client authenticates those
installed objects before it reads stdin or inherited artifact descriptors.
Python entry points call :func:`preflight` before touching caller paths, then
pass already-opened, identity-checked files to the native client.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
import sys
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import NoReturn


CLIENT_REQUEST_SCHEMA = "iroha.taira.authority-client-request.v1"
CLIENT_VERIFICATION_SCHEMA = "iroha.taira.authority-client-verification.v1"
CLIENT_RESULT_SCHEMA = "iroha.taira.authority-client-result.v1"
CLIENT_STATUS_SCHEMA = "iroha.taira.authority-client-status.v1"
DURABLE_RECEIPT_SCHEMA = "iroha.taira.authority-durable-receipt.v1"
RUN_ID_DOMAIN = b"iroha:taira:authority-run-id:v1\0"
OPERATION_ID_DOMAIN = b"iroha:taira:authority-operation-id:v1\0"
SHA256_RE = re.compile(r"[0-9a-f]{64}")
ARTIFACT_NAME_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._/-]{0,511}")
MAX_BINDING_BYTES = 1024 * 1024
MAX_NATIVE_CLIENT_BYTES = 512 * 1024 * 1024
MAX_CLIENT_INPUT_BYTES = 64 * 1024 * 1024
MAX_CLIENT_OUTPUT_BYTES = 64 * 1024 * 1024
CLIENT_TIMEOUT_SECONDS = 120
DEPLOY_DISPOSITIONS = ("dry-run", "apply")
DEPLOY_OUTCOMES = ("success", "rolled-back", "rollback-failed")

ROLE_LABELS = (
    "native-evidence",
    "privacy-protocol-origin",
    "privacy-governance",
    "qualification",
    "deploy-issuance",
    "rollout-observation",
    "public-soak-observation",
    "public-soak-replay-admission",
)


@dataclass(frozen=True)
class AuthorityRole:
    """One compile-time production authority installation."""

    role: str
    service_id: str
    administrator_id: str
    binding_path: Path
    request_socket: Path
    state_directory: Path


def _installation_roots() -> tuple[Path, Path, Path, Path]:
    if sys.platform == "darwin":
        return (
            Path("/private/etc/iroha/taira-authorities/v1"),
            Path("/private/var/run/iroha/taira-authorities/v1"),
            Path("/private/var/db/iroha/taira-authorities/v1"),
            Path("/usr/local/libexec/iroha/taira_release_authority"),
        )
    return (
        Path("/etc/iroha/taira-authorities/v1"),
        Path("/run/iroha/taira-authorities/v1"),
        Path("/var/lib/iroha/taira-authorities/v1"),
        Path("/usr/libexec/iroha/taira_release_authority"),
    )


FIXED_CONFIG_ROOT, FIXED_RUNTIME_ROOT, FIXED_STATE_ROOT, FIXED_VERIFIER_BINARY = (
    _installation_roots()
)


def _registered_role(role: str) -> AuthorityRole:
    return AuthorityRole(
        role=role,
        service_id=f"taira-authority-{role}-v1",
        administrator_id=f"taira-authority-{role}-administrator-v1",
        binding_path=FIXED_CONFIG_ROOT / role / "binding-v1.norito",
        request_socket=FIXED_RUNTIME_ROOT / role / "request-v1.sock",
        state_directory=FIXED_STATE_ROOT / role / "state-v1",
    )


ROLE_REGISTRY = {
    "native-evidence": _registered_role("native-evidence"),
    "privacy-protocol-origin": _registered_role("privacy-protocol-origin"),
    "privacy-governance": _registered_role("privacy-governance"),
    "qualification": _registered_role("qualification"),
    "deploy-issuance": _registered_role("deploy-issuance"),
    "rollout-observation": _registered_role("rollout-observation"),
    "public-soak-observation": _registered_role("public-soak-observation"),
    "public-soak-replay-admission": _registered_role(
        "public-soak-replay-admission"
    ),
}


class TairaAuthorityClientError(RuntimeError):
    """The fixed native authority client or one authenticated role refused."""


@dataclass(frozen=True)
class Artifact:
    """One named artifact transferred by descriptor, never by authority path."""

    name: str
    path: Path
    maximum: int | None = None


@dataclass(frozen=True)
class AuthorityResult:
    """One authenticated native authority result and its canonical sidecars."""

    role: str
    operation_id: str
    run_id: str
    status: str
    authority_envelope: Mapping[str, object]
    durable_receipt: Mapping[str, object]
    artifact_manifest: tuple[Mapping[str, object], ...] = ()

    @property
    def authority_envelope_bytes(self) -> bytes:
        """Return the canonical authority-envelope sidecar bytes."""

        return canonical_json_bytes(self.authority_envelope)

    @property
    def durable_receipt_bytes(self) -> bytes:
        """Return the canonical durable-receipt sidecar bytes."""

        return canonical_json_bytes(self.durable_receipt)


@dataclass
class _OpenedArtifact:
    descriptor: int
    manifest: dict[str, object]
    identity: tuple[int, ...]


def _fail(message: str) -> NoReturn:
    raise TairaAuthorityClientError(message)


def canonical_json_bytes(value: object) -> bytes:
    """Encode the canonical ASCII JSON accepted by the native client."""

    try:
        return (
            json.dumps(
                value,
                allow_nan=False,
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise TairaAuthorityClientError(
            f"authority client value is not canonical JSON: {error}"
        ) from error


def _pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            _fail(f"authority client JSON repeats field {key!r}")
        value[key] = item
    return value


def _reject_constant(value: str) -> NoReturn:
    _fail(f"authority client JSON contains forbidden number {value}")


def decode_canonical_json(payload: bytes, label: str) -> dict[str, object]:
    """Decode one exact canonical object while rejecting duplicate fields."""

    if not payload or len(payload) > MAX_CLIENT_OUTPUT_BYTES:
        _fail(f"{label} violates its byte bound")
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_pairs,
            parse_constant=_reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise TairaAuthorityClientError(f"{label} is not strict JSON") from error
    if not isinstance(value, dict) or canonical_json_bytes(value) != payload:
        _fail(f"{label} is not one canonical JSON object")
    return value


def _role(label: str) -> AuthorityRole:
    try:
        return ROLE_REGISTRY[label]
    except KeyError as error:
        raise TairaAuthorityClientError(
            f"unknown fixed Taira authority role {label!r}"
        ) from error


def _sha256(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256_RE.fullmatch(value) is None
        or value == "0" * 64
    ):
        _fail(f"{label} is not one nonzero lowercase SHA-256 digest")
    return value


def _identity(info: os.stat_result) -> tuple[int, ...]:
    return (
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


def _fixed_verifier_binary() -> Path:
    """Return the platform production path without consulting mutable inputs."""

    return _installation_roots()[3]


def _fixed_binary_identity(path: Path) -> tuple[int, ...]:
    try:
        before = path.lstat()
    except OSError as error:
        raise TairaAuthorityClientError(
            "fixed native Taira authority verifier is unavailable"
        ) from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_uid != 0
        or before.st_mode & 0o022
        or not before.st_mode & 0o111
        or before.st_size <= 0
        or before.st_size > MAX_NATIVE_CLIENT_BYTES
    ):
        _fail("fixed native Taira authority verifier has an unsafe identity")
    return _identity(before)


def _native_environment() -> dict[str, str]:
    return {"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"}


def _invoke_native_client(
    command: str,
    role: str,
    payload: bytes | None,
    opened: Sequence[_OpenedArtifact] = (),
) -> dict[str, object]:
    """Invoke only the fixed native client command and recheck its identity."""

    _role(role)
    verifier = _fixed_verifier_binary()
    before = _fixed_binary_identity(verifier)
    if payload is not None and (not payload or len(payload) > MAX_CLIENT_INPUT_BYTES):
        _fail("native Taira authority request violates its byte bound")
    argv = [str(verifier), command, "--role", role]
    for artifact in opened:
        argv.extend(("--artifact-fd", str(artifact.descriptor)))
    try:
        completed = subprocess.run(
            argv,
            input=payload,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=CLIENT_TIMEOUT_SECONDS,
            env=_native_environment(),
            cwd="/",
            close_fds=True,
            pass_fds=tuple(artifact.descriptor for artifact in opened),
            restore_signals=True,
        )
    except subprocess.TimeoutExpired as error:
        raise TairaAuthorityClientError(
            f"authenticated Taira authority {role} timed out"
        ) from error
    except (OSError, subprocess.SubprocessError) as error:
        raise TairaAuthorityClientError(
            f"cannot execute fixed native Taira authority verifier for {role}"
        ) from error
    if _fixed_verifier_binary() != verifier or _fixed_binary_identity(verifier) != before:
        _fail("fixed native Taira authority verifier changed during execution")
    if (
        len(completed.stdout) > MAX_CLIENT_OUTPUT_BYTES
        or len(completed.stderr) > MAX_CLIENT_OUTPUT_BYTES
    ):
        _fail(f"authenticated Taira authority {role} emitted oversized output")
    if completed.returncode != 0:
        _fail(
            f"authenticated Taira authority {role} refused {command} "
            f"with status {completed.returncode}"
        )
    return decode_canonical_json(completed.stdout, "native Taira authority response")


def preflight(
    role: str, *, require_signing: bool = True
) -> Mapping[str, object]:
    """Authenticate the fixed verifier, installed binding, and live service.

    Historical verification authenticates a revoked service so already-issued
    receipts remain verifiable.  Operations that can sign or consume state use
    the default and require a ready, non-revoked service.
    """

    registered = _role(role)
    value = _invoke_native_client("status", role, None)
    expected = {
        "schema",
        "role",
        "status",
        "service_id",
        "administrator_id",
        "service_uid",
        "client_uid",
        "binding_sha256",
        "key_revision",
        "policy_revision",
        "audit_sequence",
        "audit_head",
        "revoked",
    }
    if set(value) != expected:
        _fail(f"authenticated Taira authority {role} returned a noncanonical status")
    ready = value["status"] == "ready" and value["revoked"] is False
    revoked = value["status"] == "revoked" and value["revoked"] is True
    if (
        value["schema"] != CLIENT_STATUS_SCHEMA
        or value["role"] != role
        or value["service_id"] != registered.service_id
        or value["administrator_id"] != registered.administrator_id
        or not (ready if require_signing else ready or revoked)
    ):
        _fail(f"authenticated Taira authority {role} is not ready")
    _sha256(value["binding_sha256"], "authority binding digest")
    _sha256(value["audit_head"], "authority audit head")
    for field in ("key_revision", "policy_revision", "audit_sequence"):
        item = value[field]
        if isinstance(item, bool) or not isinstance(item, int) or item <= 0:
            _fail(f"authority status {field} must be positive")
    service_uid = value["service_uid"]
    client_uid = value["client_uid"]
    if (
        isinstance(service_uid, bool)
        or not isinstance(service_uid, int)
        or service_uid < 0
        or service_uid >= 2**32 - 1
        or (role == "qualification") != (service_uid == 0)
    ):
        _fail("authority status service_uid is invalid")
    if (
        isinstance(client_uid, bool)
        or not isinstance(client_uid, int)
        or client_uid <= 0
        or client_uid >= 2**32 - 1
        or client_uid == service_uid
    ):
        _fail("authority status client_uid is invalid")
    return value


def _artifact_name(value: str) -> str:
    if not isinstance(value, str) or ARTIFACT_NAME_RE.fullmatch(value) is None:
        _fail("authority artifact name is not canonical")
    pure = PurePosixPath(value)
    if (
        pure.is_absolute()
        or str(pure) != value
        or any(part in ("", ".", "..") for part in pure.parts)
    ):
        _fail("authority artifact name is not a canonical relative name")
    return value


def _hash_descriptor(descriptor: int, maximum: int | None) -> str:
    os.lseek(descriptor, 0, os.SEEK_SET)
    digest = hashlib.sha256()
    total = 0
    while True:
        chunk = os.read(descriptor, 1024 * 1024)
        if not chunk:
            break
        total += len(chunk)
        if maximum is not None and total > maximum:
            _fail("authority artifact exceeds its role-specific byte bound")
        digest.update(chunk)
    os.lseek(descriptor, 0, os.SEEK_SET)
    return digest.hexdigest()


def _open_artifacts(
    artifacts: Sequence[Artifact], *, service_uid: int
) -> list[_OpenedArtifact]:
    if len(artifacts) > 256:
        _fail("authority artifact manifest exceeds 256 descriptors")
    opened: list[_OpenedArtifact] = []
    names: set[str] = set()
    file_ids: set[tuple[int, int]] = set()
    try:
        for ordinal, artifact in enumerate(artifacts):
            name = _artifact_name(artifact.name)
            if name in names:
                _fail(f"authority artifact manifest repeats {name!r}")
            names.add(name)
            path = Path(artifact.path)
            before = path.lstat()
            if (
                not stat.S_ISREG(before.st_mode)
                or stat.S_ISLNK(before.st_mode)
                or before.st_nlink != 1
                or before.st_uid not in (0, service_uid)
                or before.st_mode & 0o222
                or before.st_size <= 0
                or (artifact.maximum is not None and before.st_size > artifact.maximum)
            ):
                _fail(f"authority artifact {name!r} has an unsafe identity")
            file_id = (before.st_dev, before.st_ino)
            if file_id in file_ids:
                _fail("authority artifact manifest aliases one file identity")
            file_ids.add(file_id)
            flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
            if hasattr(os, "O_NOFOLLOW"):
                flags |= os.O_NOFOLLOW
            descriptor = os.open(path, flags)
            try:
                current = os.fstat(descriptor)
                identity = _identity(current)
                if identity != _identity(before):
                    _fail(f"authority artifact {name!r} changed while opening")
                digest = _hash_descriptor(descriptor, artifact.maximum)
                if _identity(os.fstat(descriptor)) != identity:
                    _fail(f"authority artifact {name!r} changed while hashing")
            except BaseException:
                os.close(descriptor)
                raise
            opened.append(
                _OpenedArtifact(
                    descriptor=descriptor,
                    manifest={
                        "ordinal": ordinal,
                        "name": name,
                        "size": before.st_size,
                        "sha256": digest,
                    },
                    identity=identity,
                )
            )
        return opened
    except BaseException:
        _close_artifacts(opened)
        raise


def _recheck_artifacts(opened: Sequence[_OpenedArtifact]) -> None:
    for artifact in opened:
        if _identity(os.fstat(artifact.descriptor)) != artifact.identity:
            _fail(f"authority artifact {artifact.manifest['name']!r} mutated")
        digest = _hash_descriptor(artifact.descriptor, artifact.identity[6])
        if (
            digest != artifact.manifest["sha256"]
            or _identity(os.fstat(artifact.descriptor)) != artifact.identity
        ):
            _fail(f"authority artifact {artifact.manifest['name']!r} mutated")


def _close_artifacts(opened: Sequence[_OpenedArtifact]) -> None:
    for artifact in opened:
        try:
            os.close(artifact.descriptor)
        except OSError:
            pass


def derive_run_id(role: str, subject: Mapping[str, object]) -> str:
    """Derive the lookup key that must already have an administrator assignment."""

    _role(role)
    subject_sha256 = hashlib.sha256(canonical_json_bytes(subject)[:-1]).digest()
    return hashlib.sha256(
        RUN_ID_DOMAIN + _length_frame(role.encode("ascii")) + _length_frame(subject_sha256)
    ).hexdigest()


def _length_frame(payload: bytes) -> bytes:
    return len(payload).to_bytes(8, "big") + payload


def _operation_id(
    role: str,
    run_id: str,
    subject: Mapping[str, object],
    manifest: Sequence[Mapping[str, object]],
) -> str:
    subject_sha256 = hashlib.sha256(canonical_json_bytes(subject)[:-1]).digest()
    manifest_sha256 = hashlib.sha256(canonical_json_bytes(list(manifest))[:-1]).digest()
    return hashlib.sha256(
        OPERATION_ID_DOMAIN
        + _length_frame(role.encode("ascii"))
        + _length_frame(bytes.fromhex(run_id))
        + _length_frame(subject_sha256)
        + _length_frame(manifest_sha256)
    ).hexdigest()


def _result(
    value: Mapping[str, object],
    *,
    role: str,
    run_id: str,
    operation_id: str,
    statuses: set[str],
    artifact_manifest: Sequence[Mapping[str, object]] = (),
) -> AuthorityResult:
    fields = {
        "schema",
        "role",
        "operation_id",
        "status",
        "authority_envelope",
        "durable_receipt",
    }
    if set(value) != fields:
        _fail(f"authenticated Taira authority {role} returned noncanonical fields")
    envelope = value["authority_envelope"]
    receipt = value["durable_receipt"]
    if (
        value["schema"] != CLIENT_RESULT_SCHEMA
        or value["role"] != role
        or value["operation_id"] != operation_id
        or value["status"] not in statuses
        or not isinstance(envelope, dict)
        or not isinstance(receipt, dict)
    ):
        _fail(f"authenticated Taira authority {role} returned a mismatched result")
    return AuthorityResult(
        role=role,
        operation_id=operation_id,
        run_id=run_id,
        status=str(value["status"]),
        authority_envelope=envelope,
        durable_receipt=receipt,
        artifact_manifest=tuple(dict(row) for row in artifact_manifest),
    )


def authorize(
    role: str,
    subject: Mapping[str, object],
    *,
    artifacts: Sequence[Artifact] = (),
    run_id: str | None = None,
    disposition: str | None = None,
) -> AuthorityResult:
    """Authorize one canonical subject under a preassigned run.

    The deploy role additionally requires ``dry-run`` or ``apply``.  The
    disposition is deliberately outside the signed structural subject and ID
    derivation so a dry-run and the later under-lock consumption address the
    same administrator-assigned lease.  Dry-run validation never consumes it.
    """

    _role(role)
    status = preflight(role)
    service_uid = int(status["service_uid"])
    if role == "deploy-issuance":
        if disposition not in DEPLOY_DISPOSITIONS:
            _fail("deploy issuance requires a canonical lease disposition")
    elif disposition is not None:
        _fail("only deploy issuance accepts a lease disposition")
    canonical_json_bytes(subject)
    assigned = derive_run_id(role, subject) if run_id is None else _sha256(run_id, "run ID")
    opened: list[_OpenedArtifact] = []
    try:
        opened = _open_artifacts(artifacts, service_uid=service_uid)
        manifest = [artifact.manifest for artifact in opened]
        operation_id = _operation_id(role, assigned, subject, manifest)
        request = {
            "artifact_manifest": manifest,
            "operation_id": operation_id,
            "role": role,
            "run_id": assigned,
            "schema": CLIENT_REQUEST_SCHEMA,
            "subject": dict(subject),
        }
        statuses = {"authorized", "replayed"}
        if disposition is not None:
            request["disposition"] = disposition
            statuses = {"verified"} if disposition == "dry-run" else {"authorized", "replayed"}
        value = _invoke_native_client(
            "authorize", role, canonical_json_bytes(request), opened
        )
        _recheck_artifacts(opened)
        return _result(
            value,
            role=role,
            run_id=assigned,
            operation_id=operation_id,
            statuses=statuses,
            artifact_manifest=manifest,
        )
    except OSError as error:
        raise TairaAuthorityClientError(
            f"cannot capture authority artifacts for {role}"
        ) from error
    finally:
        _close_artifacts(opened)


def verify_receipt(
    role: str,
    subject: Mapping[str, object],
    *,
    authority_envelope: Mapping[str, object],
    durable_receipt: Mapping[str, object],
    artifacts: Sequence[Artifact] = (),
    run_id: str | None = None,
    operation_id: str | None = None,
) -> AuthorityResult:
    """Historically verify sidecars without consuming or re-signing state."""

    _role(role)
    status = preflight(role, require_signing=False)
    service_uid = int(status["service_uid"])
    canonical_json_bytes(subject)
    canonical_json_bytes(authority_envelope)
    canonical_json_bytes(durable_receipt)
    assigned = derive_run_id(role, subject) if run_id is None else _sha256(run_id, "run ID")
    opened: list[_OpenedArtifact] = []
    try:
        opened = _open_artifacts(artifacts, service_uid=service_uid)
        manifest = [artifact.manifest for artifact in opened]
        expected_operation = _operation_id(role, assigned, subject, manifest)
        if operation_id is not None and _sha256(operation_id, "operation ID") != expected_operation:
            _fail("receipt operation ID differs from the canonical request")
        request = {
            "artifact_manifest": manifest,
            "authority_envelope": dict(authority_envelope),
            "durable_receipt": dict(durable_receipt),
            "operation_id": expected_operation,
            "role": role,
            "run_id": assigned,
            "schema": CLIENT_VERIFICATION_SCHEMA,
            "subject": dict(subject),
        }
        value = _invoke_native_client(
            "verify-receipt", role, canonical_json_bytes(request), opened
        )
        _recheck_artifacts(opened)
        return _result(
            value,
            role=role,
            run_id=assigned,
            operation_id=expected_operation,
            statuses={"valid"},
            artifact_manifest=manifest,
        )
    except OSError as error:
        raise TairaAuthorityClientError(
            f"cannot capture authority artifacts for {role}"
        ) from error
    finally:
        _close_artifacts(opened)


def finalize_deployment(
    subject: Mapping[str, object],
    *,
    lease: AuthorityResult,
    outcome: str,
    result_sha256: str,
) -> AuthorityResult:
    """Durably bind one consumed deploy lease to its terminal result.

    Finalization sends the original manifest but no descriptors.  The native
    authority compares it with the persisted apply consumption, records the
    result once, and recovers a byte-identical response for an exact retry.
    """

    role = "deploy-issuance"
    preflight(role)
    canonical_json_bytes(subject)
    if (
        lease.role != role
        or lease.status not in {"authorized", "replayed"}
        or not lease.artifact_manifest
    ):
        _fail("deployment finalization requires one consumed native lease")
    if outcome not in DEPLOY_OUTCOMES:
        _fail("deployment finalization outcome is noncanonical")
    result_digest = _sha256(result_sha256, "deployment result SHA-256")
    assigned = derive_run_id(role, subject)
    manifest = [dict(row) for row in lease.artifact_manifest]
    operation_id = _operation_id(role, assigned, subject, manifest)
    if lease.run_id != assigned or lease.operation_id != operation_id:
        _fail("deployment lease differs from the canonical finalization subject")
    request = {
        "artifact_manifest": manifest,
        "deployment_result": {
            "outcome": outcome,
            "result_sha256": result_digest,
        },
        "disposition": "finalize",
        "operation_id": operation_id,
        "role": role,
        "run_id": assigned,
        "schema": CLIENT_REQUEST_SCHEMA,
        "subject": dict(subject),
    }
    value = _invoke_native_client(
        "authorize", role, canonical_json_bytes(request)
    )
    return _result(
        value,
        role=role,
        run_id=assigned,
        operation_id=operation_id,
        statuses={"finalized", "replayed"},
        artifact_manifest=manifest,
    )
