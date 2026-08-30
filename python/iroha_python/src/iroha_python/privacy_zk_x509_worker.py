"""Hash-pinned controller for the isolated native zk-X.509 worker.

The controller never opens a signer seed, X.509 witness, or owner bundle.  A
native bundle-writer process consumes owner-only input files and returns only a
SHA-256 receipt. A separate authenticated one-shot worker can return a
non-consuming semantic-admission receipt bound to the exact owner inode, or
consume that bundle and return only a complete versioned signed transaction.
The executable, signed source commit, worker source closure, compiled profile,
and protocol are all exact constructor pins, so no PATH lookup or compatibility
fallback is possible.
"""

from __future__ import annotations

import hashlib
import hmac
import importlib.util
import json
import os
import secrets
import stat
import struct
import subprocess
import sys
from dataclasses import dataclass
from enum import IntEnum
from pathlib import Path
from typing import Final

PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1: Final = 1
PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1: Final = 1
PRIVACY_ZK_X509_WORKER_PROTOCOL_ID_V1: Final = "iroha-zk-x509-stark-p256-v0"
PRIVACY_ZK_X509_WORKER_SOURCE_CLOSURE_SCHEMA_V1: Final = (
    "path-and-length-framed-sha256("
    "ci/privacy_zk_x509_worker_source_closure_v1.txt):v3"
)
PRIVACY_ZK_X509_WORKER_MAX_FRAME_BYTES_V1: Final = 12 * 1024 * 1024
PRIVACY_ZK_X509_WORKER_MAX_SIGNED_TRANSACTION_BYTES_V1: Final = 9 * 1024 * 1024
PRIVACY_ZK_X509_WORKER_MAX_KAT_PROOF_BYTES_V1: Final = 8_212_538

_MAGIC_V1 = b"X5PW"
_AUTH_KEY_BYTES_V1 = 32
_AUTH_TAG_BYTES_V1 = 32
_DIGEST_BYTES_V1 = 32
_MAX_WORKER_BINARY_BYTES_V1 = 512 * 1024 * 1024
_MAX_PUBLIC_REQUEST_BYTES_V1 = 1024 * 1024
_MAX_PATH_BYTES_V1 = 4096
_RESPONSE_OK_V1 = 0
_RESPONSE_ERROR_V1 = 1
_QUALIFIED_ISOLATION_CONTRACT_V1 = (
    "iroha.zk-x509.qualified-linux-aarch64-launcher.v1"
)
_UNAVAILABLE_ISOLATION_CONTRACT_V1 = _QUALIFIED_ISOLATION_CONTRACT_V1 + ":unavailable"
_ISOLATION_PACKAGE_DOMAIN_V1 = (
    b"iroha.privacy.zk-x509.qualified-linux-launcher-package.v1"
)
_ISOLATION_POLICY_V1 = (
    b"target=aarch64-unknown-linux-gnu;kernel-min=6.3;static-elf=true;"
    b"openat2=resolve-beneath+no-symlinks+no-magiclinks;"
    b"executable-memfd=mfd-exec+seal-exec+seal-write+seal-grow+seal-shrink+seal-seal;"
    b"attestation-memfd=mfd-noexec-seal+seal-exec+seal-write+seal-grow+seal-shrink+seal-seal;"
    b"uid=nonzero-equal-real+effective+saved+fs;"
    b"capabilities=effective+permitted+inheritable+ambient-zero;"
    b"landlock-abi-min=3;seccomp-tsync=true;seccomp-future-syscalls=deny;"
    b"seccomp-pidfd+privileged=deny;no-new-privs=true;dumpable=false;"
    b"cgroup-v2=true;memory-max=12884901888;memory-swap-max=0;"
    b"memory-oom-group=1;pids-max=6;cpu-max=max+period-100000;"
    b"rlimit-as=34359738368;rlimit-core=0;"
    b"fd-closure=stdio+64-72-bootstrap+stdio+one-data-runtime;"
    b"wall-ms=300000"
)
_RELEASE_EVIDENCE_DOMAIN_V1 = b"iroha.privacy.zk-x509.worker-release-evidence.v1"
_EXACT_LAUNCH_SOURCE_V1 = Path(__file__).resolve().with_name(
    "privacy_wallet_worker.py"
)


class PrivacyZkX509WorkerCommandV1(IntEnum):
    """Closed authenticated X5PW command registry."""

    IDENTITY = 1
    EXECUTE = 2
    ADMIT_BUNDLE = 3


class PrivacyZkX509WorkerErrorCodeV1(IntEnum):
    """Stable non-secret error classes returned by the native process."""

    INVALID_REQUEST = 1
    PROFILE_UNAVAILABLE = 2
    CUSTODY = 3
    WITNESS = 4
    PROOF = 5
    FINALIZATION = 6
    ISOLATION_UNAVAILABLE = 7


class PrivacyZkX509WorkerErrorV1(RuntimeError):
    """Local fail-closed controller rejection."""


class PrivacyZkX509WorkerRemoteErrorV1(PrivacyZkX509WorkerErrorV1):
    """Authenticated non-secret failure emitted by the native worker."""

    def __init__(self, code: PrivacyZkX509WorkerErrorCodeV1) -> None:
        super().__init__(f"zk-X509 worker rejected request ({int(code)})")
        self.code = code


@dataclass(frozen=True)
class PrivacyZkX509WorkerIdentityV1:
    """Exact immutable identity proven before any owner bundle is named."""

    artifact_sha256: str
    cargo_lock_sha256: str
    compiled_profile_sha256: str
    expectations_json_sha256: str
    expectations_norito_sha256: str
    isolation_package_sha256: str | None
    kat_proof_bytes: int
    kat_proof_sha256: str
    protocol_id: str
    protocol_profile_sha256: str
    protocol_version: int
    public_request_schema_version: int
    qualified_isolation_ready: bool
    release_evidence_ready: bool
    release_evidence_sha256: str
    resource_certificate_sha256: str
    isolation_contract: str
    soundness_certificate_sha256: str
    source_allowed_signers_sha256: str
    source_closure_schema: str
    source_commit: str
    source_revocation_sha256: str
    source_sha256: str
    workspace_source_manifest_sha256: str


@dataclass(frozen=True)
class PrivacyZkX509SecretBundleReceiptV1:
    """Non-secret receipt from the owner-only native bundle writer."""

    path: Path
    sha256: bytes


@dataclass(frozen=True)
class PrivacyZkX509SecretBundleAdmissionV1:
    """Authenticated non-consuming admission of one exact owner bundle inode."""

    public_request_sha256: bytes
    secret_bundle_sha256: bytes
    device: int
    inode: int
    size: int
    mode: int
    owner: int


@dataclass(frozen=True)
class PrivacyZkX509SignedActionV1:
    """Public terminal output of one native proof-and-sign operation."""

    transaction_hash: bytes
    proof_sha256: bytes
    versioned_signed_transaction: bytes


@dataclass(frozen=True)
class _WorkerSnapshot:
    path: Path
    device: int
    inode: int
    mode: int
    owner: int
    links: int
    size: int
    modified_ns: int
    sha256: str


def _require_lower_hex(value: str, digits: int, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != digits
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ValueError(f"{label} must be exactly {digits} lowercase hex digits")
    return value


def _require_source_commit(value: str, label: str) -> str:
    value = _require_lower_hex(value, 40, label)
    if value == "0" * 40:
        raise ValueError(f"{label} must be nonzero")
    return value


def _require_nonzero_sha256(value: str, label: str) -> str:
    value = _require_lower_hex(value, 64, label)
    if value == "0" * 64:
        raise ValueError(f"{label} must be nonzero")
    return value


def _qualified_isolation_package_sha256(artifact_sha256: str) -> str:
    """Bind one qualified launcher package to its artifact and policy bytes."""

    artifact_sha256 = _require_nonzero_sha256(
        artifact_sha256, "artifact_sha256"
    )
    digest = hashlib.sha256()
    digest.update(_ISOLATION_PACKAGE_DOMAIN_V1)
    digest.update(bytes.fromhex(artifact_sha256))
    digest.update(hashlib.sha256(_ISOLATION_POLICY_V1).digest())
    return digest.hexdigest()


def _require_digest_bytes(value: bytes, label: str) -> bytes:
    value = bytes(value)
    if len(value) != _DIGEST_BYTES_V1 or not any(value):
        raise ValueError(f"{label} must be one nonzero SHA-256 digest")
    return value


def _release_evidence_sha256(
    protocol_profile_sha256: str,
    kat_proof_bytes: int,
    kat_proof_sha256: str,
    expectations_norito_sha256: str,
    expectations_json_sha256: str,
    soundness_certificate_sha256: str,
    resource_certificate_sha256: str,
) -> str:
    protocol_label = PRIVACY_ZK_X509_WORKER_PROTOCOL_ID_V1.encode("ascii")
    digest = hashlib.sha256()
    digest.update(_RELEASE_EVIDENCE_DOMAIN_V1)
    digest.update(len(protocol_label).to_bytes(2, "big"))
    digest.update(protocol_label)
    digest.update(
        bytes(
            (
                PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1,
                PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1,
            )
        )
    )
    digest.update(bytes.fromhex(protocol_profile_sha256))
    digest.update(kat_proof_bytes.to_bytes(4, "big"))
    digest.update(bytes.fromhex(kat_proof_sha256))
    digest.update(bytes.fromhex(expectations_norito_sha256))
    digest.update(bytes.fromhex(expectations_json_sha256))
    digest.update(bytes.fromhex(soundness_certificate_sha256))
    digest.update(bytes.fromhex(resource_certificate_sha256))
    return digest.hexdigest()


def _absolute_path(value: str | os.PathLike[str], label: str) -> Path:
    text = os.fspath(value)
    if (
        not text
        or len(os.fsencode(text)) > _MAX_PATH_BYTES_V1
        or "\x00" in text
        or not os.path.isabs(text)
        or any(part in (".", "..") for part in Path(text).parts)
    ):
        raise ValueError(f"{label} must be a canonical absolute path")
    return Path(text)


def _inspect_worker(
    path: Path,
    expected_sha256: str,
) -> _WorkerSnapshot:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise PrivacyZkX509WorkerErrorV1("zk-X509 worker is unavailable") from error
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or not metadata.st_mode & stat.S_IXUSR
        or not 1 <= metadata.st_size <= _MAX_WORKER_BINARY_BYTES_V1
    ):
        raise PrivacyZkX509WorkerErrorV1(
            "zk-X509 worker must be one owner-controlled executable regular file"
        )
    digest = hashlib.sha256()
    try:
        with path.open("rb", buffering=0) as source:
            opened = os.fstat(source.fileno())
            if (
                opened.st_dev != metadata.st_dev
                or opened.st_ino != metadata.st_ino
                or opened.st_size != metadata.st_size
                or opened.st_mtime_ns != metadata.st_mtime_ns
            ):
                raise PrivacyZkX509WorkerErrorV1(
                    "zk-X509 worker changed while it was authenticated"
                )
            remaining = opened.st_size
            while remaining:
                chunk = source.read(min(1024 * 1024, remaining))
                if not chunk:
                    raise PrivacyZkX509WorkerErrorV1(
                        "zk-X509 worker was truncated while it was authenticated"
                    )
                digest.update(chunk)
                remaining -= len(chunk)
            if source.read(1):
                raise PrivacyZkX509WorkerErrorV1(
                    "zk-X509 worker grew while it was authenticated"
                )
            closed = os.fstat(source.fileno())
    except OSError as error:
        raise PrivacyZkX509WorkerErrorV1(
            "zk-X509 worker could not be authenticated"
        ) from error
    if (
        closed.st_size != opened.st_size
        or closed.st_mtime_ns != opened.st_mtime_ns
        or closed.st_dev != opened.st_dev
        or closed.st_ino != opened.st_ino
    ):
        raise PrivacyZkX509WorkerErrorV1(
            "zk-X509 worker changed while it was hashed"
        )
    actual_sha256 = digest.hexdigest()
    if not hmac.compare_digest(actual_sha256, expected_sha256):
        raise PrivacyZkX509WorkerErrorV1("zk-X509 worker artifact hash mismatch")
    return _WorkerSnapshot(
        path=path,
        device=metadata.st_dev,
        inode=metadata.st_ino,
        mode=metadata.st_mode,
        owner=metadata.st_uid,
        links=metadata.st_nlink,
        size=metadata.st_size,
        modified_ns=metadata.st_mtime_ns,
        sha256=actual_sha256,
    )


def _same_worker(left: _WorkerSnapshot, right: _WorkerSnapshot) -> bool:
    return left == right


def _load_exact_worker_launch_module_v1():
    """Load the source-closure-pinned exact-inode launch implementation."""

    module_name = "_iroha_privacy_zk_x509_exact_launch"
    loaded = sys.modules.get(module_name)
    if loaded is not None:
        return loaded
    spec = importlib.util.spec_from_file_location(module_name, _EXACT_LAUNCH_SOURCE_V1)
    if spec is None or spec.loader is None:
        raise PrivacyZkX509WorkerErrorV1(
            "zk-X509 exact authenticated launch module is unavailable"
        )
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


def _prepare_exact_worker_launch_v1(
    path: Path,
    expected_sha256: str,
):
    """Authenticate one worker inode and return its non-racy invocation."""

    launch_module = _load_exact_worker_launch_module_v1()
    try:
        return launch_module._prepare_verified_worker_launch_v1(
            path,
            expected_sha256,
        )
    except (OSError, ValueError) as error:
        raise PrivacyZkX509WorkerErrorV1(
            "zk-X509 worker exact authenticated launch failed"
        ) from error


def _encode_frame(
    command: int,
    sequence: int,
    payload: bytes,
    auth_key: bytes | bytearray,
) -> bytes:
    if (
        command
        not in (
            int(PrivacyZkX509WorkerCommandV1.IDENTITY),
            int(PrivacyZkX509WorkerCommandV1.EXECUTE),
            int(PrivacyZkX509WorkerCommandV1.ADMIT_BUNDLE),
        )
        or not 1 <= sequence <= (1 << 64) - 1
        or len(payload) > PRIVACY_ZK_X509_WORKER_MAX_FRAME_BYTES_V1 - 50
        or len(auth_key) != _AUTH_KEY_BYTES_V1
    ):
        raise PrivacyZkX509WorkerErrorV1("invalid X5PW request frame")
    authenticated = b"".join(
        (
            _MAGIC_V1,
            bytes((PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1, command)),
            struct.pack(">Q", sequence),
            struct.pack(">I", len(payload)),
            payload,
        )
    )
    tag = hmac.new(auth_key, authenticated, hashlib.sha256).digest()
    frame = authenticated + tag
    return struct.pack(">I", len(frame)) + frame


def _decode_frame(encoded: bytes, auth_key: bytes) -> tuple[int, int, bytes]:
    if len(encoded) < 4:
        raise PrivacyZkX509WorkerErrorV1("X5PW response is truncated")
    declared = struct.unpack(">I", encoded[:4])[0]
    if (
        declared != len(encoded) - 4
        or not 50 <= declared <= PRIVACY_ZK_X509_WORKER_MAX_FRAME_BYTES_V1
    ):
        raise PrivacyZkX509WorkerErrorV1("X5PW response length is invalid")
    authenticated = encoded[4:-_AUTH_TAG_BYTES_V1]
    tag = encoded[-_AUTH_TAG_BYTES_V1:]
    if not hmac.compare_digest(
        tag, hmac.new(auth_key, authenticated, hashlib.sha256).digest()
    ):
        raise PrivacyZkX509WorkerErrorV1("X5PW response authentication failed")
    if (
        authenticated[:4] != _MAGIC_V1
        or authenticated[4] != PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1
    ):
        raise PrivacyZkX509WorkerErrorV1("X5PW response protocol mismatch")
    command = authenticated[5]
    sequence = struct.unpack(">Q", authenticated[6:14])[0]
    payload_length = struct.unpack(">I", authenticated[14:18])[0]
    payload = authenticated[18:]
    if sequence == 0 or payload_length != len(payload):
        raise PrivacyZkX509WorkerErrorV1("X5PW response framing is invalid")
    return command, sequence, payload


def _canonical_json_bytes(value: dict[str, object]) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
    ).encode("utf-8")


def _hash_public_file(path: Path, maximum: int) -> bytes:
    try:
        before = path.stat()
        if not stat.S_ISREG(before.st_mode) or not 1 <= before.st_size <= maximum:
            raise PrivacyZkX509WorkerErrorV1("public X.509 request file is invalid")
        digest = hashlib.sha256()
        with path.open("rb", buffering=0) as source:
            opened = os.fstat(source.fileno())
            if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns) != (
                before.st_dev,
                before.st_ino,
                before.st_size,
                before.st_mtime_ns,
            ):
                raise PrivacyZkX509WorkerErrorV1(
                    "public X.509 request changed before hashing"
                )
            remaining = opened.st_size
            while remaining:
                chunk = source.read(min(1024 * 1024, remaining))
                if not chunk:
                    raise PrivacyZkX509WorkerErrorV1(
                        "public X.509 request was truncated while hashing"
                    )
                digest.update(chunk)
                remaining -= len(chunk)
            if source.read(1):
                raise PrivacyZkX509WorkerErrorV1(
                    "public X.509 request grew while hashing"
                )
            after = os.fstat(source.fileno())
    except OSError as error:
        raise PrivacyZkX509WorkerErrorV1(
            "public X.509 request could not be hashed"
        ) from error
    if (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns) != (
        opened.st_dev,
        opened.st_ino,
        opened.st_size,
        opened.st_mtime_ns,
    ):
        raise PrivacyZkX509WorkerErrorV1(
            "public X.509 request changed while hashing"
        )
    return digest.digest()


class PrivacyZkX509WorkerControllerV1:
    """One-shot controller pinned to one reviewed native artifact."""

    __slots__ = (
        "_expected_artifact_sha256",
        "_expected_compiled_profile_sha256",
        "_expected_source_commit",
        "_expected_source_sha256",
        "_expected_workspace_source_manifest_sha256",
        "_identity",
        "_worker_path",
    )

    def __init__(
        self,
        worker_path: str | os.PathLike[str],
        *,
        expected_artifact_sha256: str,
        expected_source_commit: str,
        expected_source_sha256: str,
        expected_compiled_profile_sha256: str,
        expected_workspace_source_manifest_sha256: str,
    ) -> None:
        self._worker_path = _absolute_path(worker_path, "worker_path")
        self._expected_artifact_sha256 = _require_nonzero_sha256(
            expected_artifact_sha256, "expected_artifact_sha256"
        )
        self._expected_source_commit = _require_source_commit(
            expected_source_commit, "expected_source_commit"
        )
        self._expected_source_sha256 = _require_nonzero_sha256(
            expected_source_sha256, "expected_source_sha256"
        )
        self._expected_compiled_profile_sha256 = _require_nonzero_sha256(
            expected_compiled_profile_sha256,
            "expected_compiled_profile_sha256",
        )
        self._expected_workspace_source_manifest_sha256 = _require_nonzero_sha256(
            expected_workspace_source_manifest_sha256,
            "expected_workspace_source_manifest_sha256",
        )
        identity_payload = self._invoke(PrivacyZkX509WorkerCommandV1.IDENTITY, b"")
        self._identity = self._parse_identity(identity_payload)

    @property
    def identity(self) -> PrivacyZkX509WorkerIdentityV1:
        return self._identity

    def _invoke(
        self,
        command: PrivacyZkX509WorkerCommandV1,
        payload: bytes,
    ) -> bytes:
        before = _inspect_worker(
            self._worker_path, self._expected_artifact_sha256
        )
        auth_key = bytearray(secrets.token_bytes(_AUTH_KEY_BYTES_V1))
        if len(auth_key) != _AUTH_KEY_BYTES_V1 or not any(auth_key):
            for index in range(len(auth_key)):
                auth_key[index] = 0
            raise PrivacyZkX509WorkerErrorV1(
                "secure X5PW authentication key is unavailable"
            )
        sequence = int.from_bytes(secrets.token_bytes(8), "big") or 1
        request = bytearray(auth_key)
        request.extend(
            _encode_frame(int(command), sequence, bytes(payload), auth_key)
        )
        launch = None
        try:
            launch = _prepare_exact_worker_launch_v1(
                self._worker_path,
                self._expected_artifact_sha256,
            )
            try:
                completed = subprocess.run(
                    [os.fspath(launch.invocation)],
                    input=request,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.DEVNULL,
                    cwd=os.path.abspath(os.sep),
                    env={},
                    close_fds=True,
                    pass_fds=launch.pass_fds,
                    start_new_session=True,
                    check=False,
                    timeout=None,
                )
                launch.authenticate()
            except (OSError, ValueError) as error:
                raise PrivacyZkX509WorkerErrorV1(
                    "failed to start native zk-X509 worker"
                ) from error
            after = _inspect_worker(
                self._worker_path, self._expected_artifact_sha256
            )
            if not _same_worker(before, after):
                raise PrivacyZkX509WorkerErrorV1(
                    "native zk-X509 worker changed during execution"
                )
            if completed.returncode != 0:
                raise PrivacyZkX509WorkerErrorV1(
                    "native zk-X509 worker terminated before an authenticated response"
                )
            response_command, response_sequence, response_payload = _decode_frame(
                completed.stdout, auth_key
            )
            if response_command != int(command) or response_sequence != sequence:
                raise PrivacyZkX509WorkerErrorV1("X5PW response identity mismatch")
            if (
                len(response_payload) >= 3
                and response_payload[0] == _RESPONSE_ERROR_V1
            ):
                if (
                    response_payload[1]
                    != PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1
                ):
                    raise PrivacyZkX509WorkerErrorV1(
                        "X5PW error version mismatch"
                    )
                try:
                    code = PrivacyZkX509WorkerErrorCodeV1(response_payload[2])
                except ValueError as error:
                    raise PrivacyZkX509WorkerErrorV1(
                        "X5PW returned an unknown error code"
                    ) from error
                if len(response_payload) != 3:
                    raise PrivacyZkX509WorkerErrorV1(
                        "X5PW error response contains trailing data"
                    )
                raise PrivacyZkX509WorkerRemoteErrorV1(code)
            return response_payload
        finally:
            if launch is not None:
                launch.close()
            for index in range(len(auth_key)):
                auth_key[index] = 0
            for index in range(len(request)):
                request[index] = 0

    def _parse_identity(self, payload: bytes) -> PrivacyZkX509WorkerIdentityV1:
        if not payload or payload[0] != _RESPONSE_OK_V1:
            raise PrivacyZkX509WorkerErrorV1("X5PW identity response is invalid")
        try:
            raw = payload[1:].decode("utf-8")
            parsed = json.loads(raw)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise PrivacyZkX509WorkerErrorV1(
                "X5PW identity is not canonical JSON"
            ) from error
        expected_keys = {
            "artifact_self_hash_required",
            "cargo_lock_sha256",
            "compiled_profile_sha256",
            "expectations_json_sha256",
            "expectations_norito_sha256",
            "isolation_package_sha256",
            "kat_proof_bytes",
            "kat_proof_sha256",
            "operation",
            "production_profile_ready",
            "protocol_id",
            "protocol_profile_sha256",
            "protocol_version",
            "public_request_schema_version",
            "qualified_isolation_ready",
            "release_evidence_ready",
            "release_evidence_sha256",
            "resource_certificate_sha256",
            "isolation_contract",
            "schema",
            "schema_version",
            "soundness_certificate_sha256",
            "source_allowed_signers_sha256",
            "source_closure_schema",
            "source_commit",
            "source_revocation_sha256",
            "source_sha256",
            "workspace_source_manifest_sha256",
        }
        if (
            not isinstance(parsed, dict)
            or set(parsed) != expected_keys
            or _canonical_json_bytes(parsed) != payload[1:]
            or parsed["artifact_self_hash_required"] is not True
            or parsed["operation"] != "prove-and-sign-zk-x509-action-v1"
            or parsed["protocol_id"] != PRIVACY_ZK_X509_WORKER_PROTOCOL_ID_V1
            or parsed["protocol_version"]
            != PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1
            or parsed["public_request_schema_version"]
            != PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1
            or parsed["schema"] != "iroha.privacy.zk_x509_worker_identity"
            or parsed["schema_version"] != 2
            or parsed["source_closure_schema"]
            != PRIVACY_ZK_X509_WORKER_SOURCE_CLOSURE_SCHEMA_V1
        ):
            raise PrivacyZkX509WorkerErrorV1(
                "X5PW identity does not match the closed protocol"
            )
        unavailable_identity = (
            parsed["production_profile_ready"] is False
            and parsed["qualified_isolation_ready"] is False
            and parsed["isolation_package_sha256"] is None
            and parsed["isolation_contract"]
            == _UNAVAILABLE_ISOLATION_CONTRACT_V1
        )
        if unavailable_identity:
            raise PrivacyZkX509WorkerErrorV1(
                "qualified zk-X509 Linux isolation launcher is unavailable"
            )
        if (
            parsed["production_profile_ready"] is not True
            or parsed["qualified_isolation_ready"] is not True
            or parsed["isolation_contract"]
            != _QUALIFIED_ISOLATION_CONTRACT_V1
        ):
            raise PrivacyZkX509WorkerErrorV1(
                "X5PW identity contains inconsistent qualified isolation evidence"
            )
        try:
            isolation_package_sha256 = _require_nonzero_sha256(
                parsed["isolation_package_sha256"],
                "isolation_package_sha256",
            )
        except ValueError as error:
            raise PrivacyZkX509WorkerErrorV1(
                "X5PW identity isolation package digest is malformed"
            ) from error
        expected_isolation_package_sha256 = (
            _qualified_isolation_package_sha256(
                self._expected_artifact_sha256
            )
        )
        if not hmac.compare_digest(
            isolation_package_sha256,
            expected_isolation_package_sha256,
        ):
            raise PrivacyZkX509WorkerErrorV1(
                "X5PW identity isolation package does not bind the reviewed artifact"
            )
        for field, expected, digits in (
            ("compiled_profile_sha256", self._expected_compiled_profile_sha256, 64),
            ("protocol_profile_sha256", self._expected_compiled_profile_sha256, 64),
            ("source_commit", self._expected_source_commit, 40),
            ("source_sha256", self._expected_source_sha256, 64),
            (
                "workspace_source_manifest_sha256",
                self._expected_workspace_source_manifest_sha256,
                64,
            ),
        ):
            try:
                actual = (
                    _require_source_commit(parsed[field], field)
                    if field == "source_commit"
                    else _require_nonzero_sha256(parsed[field], field)
                )
            except ValueError as error:
                raise PrivacyZkX509WorkerErrorV1(
                    f"X5PW identity {field} is malformed"
                ) from error
            if not hmac.compare_digest(actual, expected):
                raise PrivacyZkX509WorkerErrorV1(
                    f"X5PW identity {field} does not match the reviewed pin"
                )
        release_fields = (
            "cargo_lock_sha256",
            "expectations_json_sha256",
            "expectations_norito_sha256",
            "kat_proof_sha256",
            "release_evidence_sha256",
            "resource_certificate_sha256",
            "soundness_certificate_sha256",
            "source_allowed_signers_sha256",
            "source_revocation_sha256",
        )
        try:
            release_digests = {
                field: _require_nonzero_sha256(parsed[field], field)
                for field in release_fields
            }
        except ValueError as error:
            raise PrivacyZkX509WorkerErrorV1(
                "X5PW release-evidence identity contains a malformed digest"
            ) from error
        kat_proof_bytes = parsed["kat_proof_bytes"]
        if (
            not isinstance(kat_proof_bytes, int)
            or isinstance(kat_proof_bytes, bool)
            or not 1
            <= kat_proof_bytes
            <= PRIVACY_ZK_X509_WORKER_MAX_KAT_PROOF_BYTES_V1
            or hmac.compare_digest(
                release_digests["expectations_norito_sha256"],
                release_digests["expectations_json_sha256"],
            )
        ):
            raise PrivacyZkX509WorkerErrorV1(
                "X5PW release-evidence identity is incomplete"
            )
        expected_release_evidence = _release_evidence_sha256(
            parsed["protocol_profile_sha256"],
            kat_proof_bytes,
            release_digests["kat_proof_sha256"],
            release_digests["expectations_norito_sha256"],
            release_digests["expectations_json_sha256"],
            release_digests["soundness_certificate_sha256"],
            release_digests["resource_certificate_sha256"],
        )
        if (
            parsed["release_evidence_ready"] is not True
            or not hmac.compare_digest(
                release_digests["release_evidence_sha256"],
                expected_release_evidence,
            )
        ):
            raise PrivacyZkX509WorkerErrorV1(
                "X5PW release-evidence identity does not match its constituents"
            )
        return PrivacyZkX509WorkerIdentityV1(
            artifact_sha256=self._expected_artifact_sha256,
            cargo_lock_sha256=release_digests["cargo_lock_sha256"],
            compiled_profile_sha256=self._expected_compiled_profile_sha256,
            expectations_json_sha256=release_digests["expectations_json_sha256"],
            expectations_norito_sha256=release_digests[
                "expectations_norito_sha256"
            ],
            isolation_package_sha256=isolation_package_sha256,
            kat_proof_bytes=kat_proof_bytes,
            kat_proof_sha256=release_digests["kat_proof_sha256"],
            protocol_id=parsed["protocol_id"],
            protocol_profile_sha256=parsed["protocol_profile_sha256"],
            protocol_version=parsed["protocol_version"],
            public_request_schema_version=parsed["public_request_schema_version"],
            qualified_isolation_ready=parsed["qualified_isolation_ready"],
            release_evidence_ready=True,
            release_evidence_sha256=release_digests["release_evidence_sha256"],
            resource_certificate_sha256=release_digests[
                "resource_certificate_sha256"
            ],
            isolation_contract=parsed["isolation_contract"],
            soundness_certificate_sha256=release_digests[
                "soundness_certificate_sha256"
            ],
            source_allowed_signers_sha256=release_digests[
                "source_allowed_signers_sha256"
            ],
            source_closure_schema=parsed["source_closure_schema"],
            source_commit=parsed["source_commit"],
            source_revocation_sha256=release_digests["source_revocation_sha256"],
            source_sha256=parsed["source_sha256"],
            workspace_source_manifest_sha256=parsed[
                "workspace_source_manifest_sha256"
            ],
        )

    def _require_qualified_isolation(self) -> None:
        expected_package_sha256 = _qualified_isolation_package_sha256(
            self._expected_artifact_sha256
        )
        if (
            not self._identity.qualified_isolation_ready
            or self._identity.isolation_contract
            != _QUALIFIED_ISOLATION_CONTRACT_V1
            or self._identity.isolation_package_sha256 is None
            or not hmac.compare_digest(
                self._identity.isolation_package_sha256,
                expected_package_sha256,
            )
        ):
            raise PrivacyZkX509WorkerErrorV1(
                "qualified zk-X509 Linux isolation launcher is unavailable"
            )

    def create_secret_bundle(
        self,
        *,
        public_request_path: str | os.PathLike[str],
        signer_seed_path: str | os.PathLike[str],
        witness_path: str | os.PathLike[str],
        output_path: str | os.PathLike[str],
    ) -> PrivacyZkX509SecretBundleReceiptV1:
        """Invoke the native owner-only writer without reading a secret input."""

        self._require_qualified_isolation()

        public_path = _absolute_path(public_request_path, "public_request_path")
        seed_path = _absolute_path(signer_seed_path, "signer_seed_path")
        witness = _absolute_path(witness_path, "witness_path")
        output = _absolute_path(output_path, "output_path")
        before = _inspect_worker(self._worker_path, self._expected_artifact_sha256)
        launch = _prepare_exact_worker_launch_v1(
            self._worker_path,
            self._expected_artifact_sha256,
        )
        try:
            try:
                completed = subprocess.run(
                    [
                        os.fspath(launch.invocation),
                        "bundle",
                        os.fspath(public_path),
                        os.fspath(seed_path),
                        os.fspath(witness),
                        os.fspath(output),
                    ],
                    stdout=subprocess.PIPE,
                    stderr=subprocess.DEVNULL,
                    cwd=os.path.abspath(os.sep),
                    env={},
                    close_fds=True,
                    pass_fds=launch.pass_fds,
                    start_new_session=True,
                    check=False,
                    timeout=None,
                )
                launch.authenticate()
            except (OSError, ValueError) as error:
                raise PrivacyZkX509WorkerErrorV1(
                    "failed to start native zk-X509 bundle writer"
                ) from error
        finally:
            launch.close()
        after = _inspect_worker(self._worker_path, self._expected_artifact_sha256)
        if not _same_worker(before, after) or completed.returncode != 0:
            raise PrivacyZkX509WorkerErrorV1(
                "native zk-X509 bundle writer failed closed"
            )
        try:
            metadata = output.lstat()
        except OSError as error:
            raise PrivacyZkX509WorkerErrorV1(
                "native zk-X509 bundle writer produced no owner file"
            ) from error
        if (
            len(completed.stdout) != _DIGEST_BYTES_V1
            or not any(completed.stdout)
            or not stat.S_ISREG(metadata.st_mode)
            or stat.S_IMODE(metadata.st_mode) != 0o600
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
        ):
            raise PrivacyZkX509WorkerErrorV1(
                "native zk-X509 bundle receipt or file mode is invalid"
            )
        return PrivacyZkX509SecretBundleReceiptV1(
            path=output,
            sha256=bytes(completed.stdout),
        )

    def execute(
        self,
        *,
        public_request_path: str | os.PathLike[str],
        secret_bundle_path: str | os.PathLike[str],
        secret_bundle_sha256: bytes,
    ) -> PrivacyZkX509SignedActionV1:
        """Consume one native bundle and return only public signed wire."""

        self._require_qualified_isolation()

        public_path = _absolute_path(public_request_path, "public_request_path")
        bundle_path = _absolute_path(secret_bundle_path, "secret_bundle_path")
        public_digest = _hash_public_file(
            public_path, _MAX_PUBLIC_REQUEST_BYTES_V1
        )
        bundle_digest = _require_digest_bytes(
            secret_bundle_sha256, "secret_bundle_sha256"
        )
        request = {
            "schema_version": PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1,
            "public_request_path": os.fspath(public_path),
            "public_request_sha256": list(public_digest),
            "secret_bundle_path": os.fspath(bundle_path),
            "secret_bundle_sha256": list(bundle_digest),
        }
        payload = self._invoke(
            PrivacyZkX509WorkerCommandV1.EXECUTE,
            _canonical_json_bytes(request),
        )
        minimum = 1 + 1 + _DIGEST_BYTES_V1 * 2 + 4
        if (
            len(payload) < minimum
            or payload[0] != _RESPONSE_OK_V1
            or payload[1] != PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1
        ):
            raise PrivacyZkX509WorkerErrorV1("X5PW signed response is invalid")
        transaction_hash = payload[2:34]
        proof_sha256 = payload[34:66]
        transaction_length = struct.unpack(">I", payload[66:70])[0]
        transaction = payload[70:]
        if (
            not any(transaction_hash)
            or not any(proof_sha256)
            or not 1
            <= transaction_length
            <= PRIVACY_ZK_X509_WORKER_MAX_SIGNED_TRANSACTION_BYTES_V1
            or len(transaction) != transaction_length
        ):
            raise PrivacyZkX509WorkerErrorV1(
                "X5PW signed transaction length or digest is invalid"
            )
        return PrivacyZkX509SignedActionV1(
            transaction_hash=transaction_hash,
            proof_sha256=proof_sha256,
            versioned_signed_transaction=transaction,
        )

    def admit_secret_bundle(
        self,
        *,
        public_request_path: str | os.PathLike[str],
        secret_bundle_path: str | os.PathLike[str],
        secret_bundle_sha256: bytes,
    ) -> PrivacyZkX509SecretBundleAdmissionV1:
        """Validate one exact owner bundle without proving, signing, or reading it here."""

        self._require_qualified_isolation()

        public_path = _absolute_path(public_request_path, "public_request_path")
        bundle_path = _absolute_path(secret_bundle_path, "secret_bundle_path")
        public_digest = _hash_public_file(
            public_path, _MAX_PUBLIC_REQUEST_BYTES_V1
        )
        bundle_digest = _require_digest_bytes(
            secret_bundle_sha256, "secret_bundle_sha256"
        )
        request = {
            "schema_version": PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1,
            "public_request_path": os.fspath(public_path),
            "public_request_sha256": list(public_digest),
            "secret_bundle_path": os.fspath(bundle_path),
            "secret_bundle_sha256": list(bundle_digest),
        }
        payload = self._invoke(
            PrivacyZkX509WorkerCommandV1.ADMIT_BUNDLE,
            _canonical_json_bytes(request),
        )
        if (
            len(payload) != 98
            or payload[0] != _RESPONSE_OK_V1
            or payload[1] != PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1
            or not hmac.compare_digest(payload[2:34], public_digest)
            or not hmac.compare_digest(payload[34:66], bundle_digest)
        ):
            raise PrivacyZkX509WorkerErrorV1(
                "X5PW secret-bundle admission response is invalid"
            )
        device, inode, size, mode, owner = struct.unpack(">QQQII", payload[66:])
        try:
            metadata = bundle_path.lstat()
        except OSError as error:
            raise PrivacyZkX509WorkerErrorV1(
                "admitted zk-X509 secret bundle is unavailable"
            ) from error
        if (
            device == 0
            or inode == 0
            or not 1 <= size <= 4 + 1 + 32 + 32 + 4 + 64 * 1024
            or mode != 0o600
            or owner != os.geteuid()
            or not stat.S_ISREG(metadata.st_mode)
            or stat.S_ISLNK(metadata.st_mode)
            or metadata.st_nlink != 1
            or metadata.st_dev != device
            or metadata.st_ino != inode
            or metadata.st_size != size
            or stat.S_IMODE(metadata.st_mode) != mode
            or metadata.st_uid != owner
        ):
            raise PrivacyZkX509WorkerErrorV1(
                "admitted zk-X509 secret bundle changed or has invalid custody"
            )
        return PrivacyZkX509SecretBundleAdmissionV1(
            public_request_sha256=bytes(payload[2:34]),
            secret_bundle_sha256=bytes(payload[34:66]),
            device=device,
            inode=inode,
            size=size,
            mode=mode,
            owner=owner,
        )


__all__ = [
    "PRIVACY_ZK_X509_WORKER_MAX_FRAME_BYTES_V1",
    "PRIVACY_ZK_X509_WORKER_MAX_SIGNED_TRANSACTION_BYTES_V1",
    "PRIVACY_ZK_X509_WORKER_PROTOCOL_ID_V1",
    "PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1",
    "PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1",
    "PRIVACY_ZK_X509_WORKER_SOURCE_CLOSURE_SCHEMA_V1",
    "PrivacyZkX509SecretBundleAdmissionV1",
    "PrivacyZkX509SecretBundleReceiptV1",
    "PrivacyZkX509SignedActionV1",
    "PrivacyZkX509WorkerCommandV1",
    "PrivacyZkX509WorkerControllerV1",
    "PrivacyZkX509WorkerErrorCodeV1",
    "PrivacyZkX509WorkerErrorV1",
    "PrivacyZkX509WorkerIdentityV1",
    "PrivacyZkX509WorkerRemoteErrorV1",
]
