#!/usr/bin/env python3
"""Verify and archive one SSH-signed Sumeragi v2 release identity.

The first-release verifier accepts only Git SSH signatures.  Every executable
and policy input is protected by a caller-supplied SHA-256 digest, copied into
one private evidence directory, and used from that stable copy.  The exact
copies used for verification are then published as release evidence.  The
attestation is a marker: prerequisites are made durable before it is linked.

The release-host owner and every owner of an ancestor of the private evidence
directory are part of the trust boundary. The verifier rejects symlinks and
revalidates inodes, but Unix pathname APIs cannot exclude a malicious same-UID
namespace swap by an already-trusted parent-directory owner.
"""

from __future__ import annotations

import argparse
import base64
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import selectors
import signal
import stat
import subprocess
import sys
import time
from typing import Any


_IDENTITY_KEYS = {
    "schema_version",
    "head_commit",
    "head_tree",
    "index_tree",
    "workspace_source_manifest_sha256",
    "cargo_lock_sha256",
}
_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_OBJECT_ID_RE = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_SSH_FINGERPRINT_RE = re.compile(r"SHA256:[A-Za-z0-9+/]{43}")
_SHELL_SAFE_PATH_RE = re.compile(r"/[A-Za-z0-9_./+:-]+")
_TRAILER_VERSION = "Sumeragi-V2-Release-Identity-Version"
_TRAILER_MANIFEST = "Sumeragi-V2-Source-Manifest-SHA256"
_TRAILER_LOCK = "Sumeragi-V2-Cargo-Lock-SHA256"
_TRAILER_KEYS = (_TRAILER_VERSION, _TRAILER_MANIFEST, _TRAILER_LOCK)
_SSH_ARMOR_BEGIN = b"-----BEGIN SSH SIGNATURE-----"
_SSH_ARMOR_END = b"-----END SSH SIGNATURE-----"
_UNSUPPORTED_ARMOR_MARKERS = (
    b"-----BEGIN PGP SIGNATURE-----",
    b"-----BEGIN SIGNED MESSAGE-----",
    b"-----BEGIN CERTIFICATE-----",
)
_MAX_IDENTITY_BYTES = 64 * 1024
_MAX_LOCK_BYTES = 128 * 1024 * 1024
_MAX_POLICY_BYTES = 16 * 1024 * 1024
_MAX_TOOL_BYTES = 512 * 1024 * 1024
_MAX_QUERY_OUTPUT_BYTES = 64 * 1024
_MAX_RAW_COMMIT_BYTES = 16 * 1024 * 1024
_MAX_VERIFY_OUTPUT_BYTES = 4 * 1024 * 1024
_MAX_SHOW_OUTPUT_BYTES = 1024 * 1024
_COMMAND_TIMEOUT_SECONDS = 120
_EVIDENCE_DIRECTORY_MODE = 0o700
_TOOL_MODE = 0o500
_DATA_MODE = 0o400


class VerificationError(RuntimeError):
    """The release signature or one of its closed inputs is invalid."""


@dataclass(frozen=True)
class FileSnapshot:
    """Bytes and stable file identity captured without following symlinks."""

    path: Path
    data: bytes
    device: int
    inode: int
    mode: int


@dataclass(frozen=True)
class CommandResult:
    """One bounded command result and the exact argument vector used."""

    argv: tuple[str, ...]
    returncode: int
    stdout: bytes
    stderr: bytes


@dataclass
class EvidenceDirectory:
    """The single private directory in which all evidence is published."""

    path: Path
    descriptor: int
    device: int
    inode: int


@dataclass(frozen=True)
class OutputTarget:
    """One absent final evidence pathname."""

    label: str
    path: Path
    name: str
    mode: int
    marker: bool = False


@dataclass
class StagedArtifact:
    """One staged inode which may later be linked to its final name."""

    target: OutputTarget
    temporary_name: str
    device: int
    inode: int
    sha256: str
    size_bytes: int
    published: bool = False


def _canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode("utf-8")


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _absolute(path: Path) -> Path:
    if not path.is_absolute():
        path = Path.cwd() / path
    return Path(os.path.abspath(path))


def _read_regular_file(
    path: Path,
    label: str,
    *,
    maximum_bytes: int,
    executable: bool = False,
) -> FileSnapshot:
    """Read a resolved regular file while rejecting every symlink component."""

    path = _absolute(path)
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
    except OSError as error:
        raise VerificationError(f"{label} is unavailable") from error
    if resolved != path:
        raise VerificationError(f"{label} path must be resolved and non-symlinked")
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise VerificationError(f"{label} must be a regular non-symlink file")
    if executable and before.st_mode & 0o111 == 0:
        raise VerificationError(f"{label} is not executable")
    if before.st_size > maximum_bytes:
        raise VerificationError(f"{label} exceeds its closed size limit")

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise VerificationError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
            or stat.S_IMODE(opened.st_mode) != stat.S_IMODE(before.st_mode)
        ):
            raise VerificationError(f"{label} changed while it was opened")
        chunks: list[bytes] = []
        total = 0
        while True:
            remaining = maximum_bytes + 1 - total
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if total > maximum_bytes:
                raise VerificationError(f"{label} exceeds its closed size limit")
        after = os.fstat(descriptor)
        if (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
            stat.S_IMODE(after.st_mode),
        ) != (
            opened.st_dev,
            opened.st_ino,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
            stat.S_IMODE(opened.st_mode),
        ):
            raise VerificationError(f"{label} changed while it was read")
        return FileSnapshot(
            path,
            b"".join(chunks),
            opened.st_dev,
            opened.st_ino,
            stat.S_IMODE(opened.st_mode),
        )
    finally:
        os.close(descriptor)


def _require_unchanged_file(
    snapshot: FileSnapshot,
    label: str,
    *,
    maximum_bytes: int,
    executable: bool = False,
) -> None:
    current = _read_regular_file(
        snapshot.path,
        label,
        maximum_bytes=maximum_bytes,
        executable=executable,
    )
    if (
        (current.device, current.inode, current.mode)
        != (snapshot.device, snapshot.inode, snapshot.mode)
        or current.data != snapshot.data
    ):
        raise VerificationError(f"{label} changed during verification")


def _require_digest(value: str, label: str) -> str:
    if _DIGEST_RE.fullmatch(value) is None:
        raise VerificationError(f"{label} must be one lowercase SHA-256 digest")
    return value


def _require_snapshot_digest(
    snapshot: FileSnapshot, expected: str, label: str
) -> None:
    if _sha256(snapshot.data) != expected:
        raise VerificationError(f"{label} does not match its protected SHA-256")


def _load_identity(path: Path) -> tuple[dict[str, Any], FileSnapshot]:
    snapshot = _read_regular_file(
        path, "release identity", maximum_bytes=_MAX_IDENTITY_BYTES
    )
    try:
        value = json.loads(snapshot.data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise VerificationError("release identity is not canonical UTF-8 JSON") from error
    if not isinstance(value, dict) or set(value) != _IDENTITY_KEYS:
        raise VerificationError("release identity fields do not match the exact schema")
    if type(value["schema_version"]) is not int or value["schema_version"] != 1:
        raise VerificationError("release identity has the wrong schema version")
    for field in ("head_commit", "head_tree", "index_tree"):
        item = value[field]
        if not isinstance(item, str) or _OBJECT_ID_RE.fullmatch(item) is None:
            raise VerificationError(f"release identity field {field} is invalid")
    if not (
        len(value["head_commit"])
        == len(value["head_tree"])
        == len(value["index_tree"])
    ):
        raise VerificationError("release identity mixes Git object formats")
    for field in ("workspace_source_manifest_sha256", "cargo_lock_sha256"):
        item = value[field]
        if not isinstance(item, str) or _DIGEST_RE.fullmatch(item) is None:
            raise VerificationError(f"release identity field {field} is invalid")
    if value["head_tree"] != value["index_tree"]:
        raise VerificationError("release identity does not describe one clean tree")
    if snapshot.data != _canonical_json(value):
        raise VerificationError("release identity JSON is not canonical")
    return value, snapshot


def _validate_root(path: Path) -> Path:
    path = _absolute(path)
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise VerificationError("release root is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise VerificationError("release root must be a non-symlink directory")
    return resolved


def _validate_tool(path: Path, label: str) -> FileSnapshot:
    if not path.is_absolute():
        raise VerificationError(f"{label} path must be absolute and resolved")
    supplied = Path(os.path.abspath(path))
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise VerificationError(f"{label} is unavailable") from error
    if supplied != path or resolved != path:
        raise VerificationError(f"{label} path must be absolute and resolved")
    snapshot = _read_regular_file(
        path,
        label,
        maximum_bytes=_MAX_TOOL_BYTES,
        executable=True,
    )
    if (
        label == "pinned Git executable"
        and sys.platform == "darwin"
        and snapshot.path == Path("/usr/bin/git")
    ):
        raise VerificationError(
            "/usr/bin/git is an Apple developer-tool launcher, not a closed Git executable; "
            "supply the resolved `xcrun --find git` binary"
        )
    if (
        label == "pinned ssh-keygen executable"
        and sys.platform == "darwin"
        and snapshot.path == Path("/usr/bin/ssh-keygen")
    ):
        raise VerificationError(
            "/usr/bin/ssh-keygen is an Apple platform binary whose copied inode "
            "cannot execute; supply a relocatable checksum-pinned ssh-keygen"
        )
    return snapshot


def _validate_fingerprint(value: str) -> str:
    if _SSH_FINGERPRINT_RE.fullmatch(value) is None:
        raise VerificationError(
            "expected signer fingerprint must be one OpenSSH SHA256 fingerprint"
        )
    return value


def _validate_allowed_signers_policy(data: bytes) -> None:
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise VerificationError("SSH allowed-signers policy must be UTF-8 text") from error
    if "\r" in text or "\0" in text or not text.endswith("\n"):
        raise VerificationError("SSH allowed-signers policy must be LF-only text")
    active = [line for line in text.splitlines() if line and not line.startswith("#")]
    if len(active) != 1:
        raise VerificationError(
            "SSH allowed-signers file must contain exactly one active key"
        )
    folded = "\n".join(active).casefold()
    if "cert-authority" in folded or "-cert-v01@openssh.com" in folded:
        raise VerificationError(
            "SSH certificate-authority and certificate keys are not accepted in v1"
        )
    if "valid-after=" in folded or "valid-before=" in folded:
        raise VerificationError(
            "time-bounded SSH allowed-signers policies are not accepted in v1"
        )


def _output_targets(args: argparse.Namespace) -> list[OutputTarget]:
    return [
        OutputTarget("attestation", args.attestation_output, "", _DATA_MODE, True),
        OutputTarget(
            "verify transcript", args.verify_transcript_output, "", _DATA_MODE
        ),
        OutputTarget("raw commit", args.raw_commit_output, "", _DATA_MODE),
        OutputTarget("Cargo.lock archive", args.cargo_lock_output, "", _DATA_MODE),
        OutputTarget(
            "SSH allowed-signers archive",
            args.ssh_allowed_signers_output,
            "",
            _DATA_MODE,
        ),
        OutputTarget(
            "SSH revocation-policy archive",
            args.ssh_revocation_output,
            "",
            _DATA_MODE,
        ),
        OutputTarget("Git archive", args.git_archive_output, "", _TOOL_MODE),
        OutputTarget(
            "ssh-keygen archive", args.ssh_keygen_archive_output, "", _TOOL_MODE
        ),
    ]


def _prepare_evidence_directory(
    raw_targets: list[OutputTarget], root: Path
) -> tuple[EvidenceDirectory, dict[str, OutputTarget]]:
    normalized: list[OutputTarget] = []
    parents: set[Path] = set()
    for target in raw_targets:
        path = _absolute(target.path)
        if not path.name:
            raise VerificationError(f"{target.label} output path is invalid")
        try:
            resolved_parent = path.parent.resolve(strict=True)
        except OSError as error:
            raise VerificationError(
                f"{target.label} output parent is unavailable"
            ) from error
        if resolved_parent != path.parent:
            raise VerificationError(
                f"{target.label} output parent must be resolved and non-symlinked"
            )
        if _SHELL_SAFE_PATH_RE.fullmatch(str(path)) is None:
            raise VerificationError(
                f"{target.label} output path must use shell-safe ASCII characters"
            )
        if resolved_parent == root or root in resolved_parent.parents:
            raise VerificationError("release evidence directory must be outside the source root")
        if os.path.lexists(path):
            raise VerificationError(f"{target.label} output already exists")
        parents.add(resolved_parent)
        normalized.append(
            OutputTarget(target.label, path, path.name, target.mode, target.marker)
        )
    if len(parents) != 1:
        raise VerificationError(
            "all release evidence outputs must share one private directory"
        )
    identities = {(target.path.parent, target.name) for target in normalized}
    if len(identities) != len(normalized):
        raise VerificationError("release evidence output paths must be distinct")

    parent = next(iter(parents))
    try:
        metadata = parent.lstat()
    except OSError as error:
        raise VerificationError("release evidence directory is unavailable") from error
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != _EVIDENCE_DIRECTORY_MODE
    ):
        raise VerificationError(
            "release evidence directory must be owner-owned with exact mode 0700"
        )
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(parent, flags)
    except OSError as error:
        raise VerificationError(
            "release evidence directory could not be opened safely"
        ) from error
    opened = os.fstat(descriptor)
    if (
        (opened.st_dev, opened.st_ino) != (metadata.st_dev, metadata.st_ino)
        or stat.S_IMODE(opened.st_mode) != _EVIDENCE_DIRECTORY_MODE
        or opened.st_uid != os.geteuid()
    ):
        os.close(descriptor)
        raise VerificationError("release evidence directory changed while it was opened")
    directory = EvidenceDirectory(parent, descriptor, opened.st_dev, opened.st_ino)
    return directory, {target.label: target for target in normalized}


def _revalidate_evidence_directory(directory: EvidenceDirectory) -> None:
    try:
        metadata = directory.path.lstat()
    except OSError as error:
        raise VerificationError("release evidence directory became unavailable") from error
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or (metadata.st_dev, metadata.st_ino)
        != (directory.device, directory.inode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != _EVIDENCE_DIRECTORY_MODE
    ):
        raise VerificationError("release evidence directory changed during verification")


def _write_all(descriptor: int, data: bytes, label: str) -> None:
    view = memoryview(data)
    while view:
        try:
            written = os.write(descriptor, view)
        except InterruptedError:
            continue
        if written <= 0:
            raise VerificationError(f"{label} write did not progress")
        view = view[written:]


def _stage_artifact(
    directory: EvidenceDirectory, target: OutputTarget, data: bytes
) -> StagedArtifact:
    temporary_name = f".{target.name}.stage.{secrets.token_hex(16)}"
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(
            temporary_name,
            flags,
            0o600,
            dir_fd=directory.descriptor,
        )
    except OSError as error:
        raise VerificationError(f"{target.label} could not be staged") from error
    try:
        _write_all(descriptor, data, target.label)
        os.fchmod(descriptor, target.mode)
        os.fsync(descriptor)
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or stat.S_IMODE(opened.st_mode) != target.mode
            or opened.st_size != len(data)
        ):
            raise VerificationError(f"{target.label} staged inode is invalid")
        return StagedArtifact(
            target,
            temporary_name,
            opened.st_dev,
            opened.st_ino,
            _sha256(data),
            len(data),
        )
    except BaseException as original_error:
        try:
            os.unlink(temporary_name, dir_fd=directory.descriptor)
        except OSError as cleanup_error:
            raise VerificationError(
                f"{target.label} staging cleanup failed"
            ) from cleanup_error
        raise original_error
    finally:
        os.close(descriptor)


def _staged_path(directory: EvidenceDirectory, artifact: StagedArtifact) -> Path:
    return directory.path / artifact.temporary_name


def _owned_unlink(
    directory: EvidenceDirectory, name: str, artifact: StagedArtifact
) -> bool:
    try:
        metadata = os.stat(name, dir_fd=directory.descriptor, follow_symlinks=False)
    except FileNotFoundError:
        return False
    except OSError:
        return False
    if (
        not stat.S_ISREG(metadata.st_mode)
        or (metadata.st_dev, metadata.st_ino) != (artifact.device, artifact.inode)
    ):
        return False
    try:
        os.unlink(name, dir_fd=directory.descriptor)
    except OSError:
        return False
    return True


def _revalidate_artifact_inode(
    directory: EvidenceDirectory, name: str, artifact: StagedArtifact
) -> None:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(name, flags, dir_fd=directory.descriptor)
    except OSError as error:
        raise VerificationError(
            f"{artifact.target.label} inode could not be reopened"
        ) from error
    try:
        before = os.fstat(descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or (before.st_dev, before.st_ino) != (artifact.device, artifact.inode)
            or stat.S_IMODE(before.st_mode) != artifact.target.mode
            or before.st_size != artifact.size_bytes
        ):
            raise VerificationError(f"{artifact.target.label} inode metadata changed")
        digest = hashlib.sha256()
        observed_size = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
            observed_size += len(chunk)
            if observed_size > artifact.size_bytes:
                raise VerificationError(f"{artifact.target.label} inode grew")
        after = os.fstat(descriptor)
        if (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
            stat.S_IMODE(after.st_mode),
        ) != (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
            stat.S_IMODE(before.st_mode),
        ):
            raise VerificationError(
                f"{artifact.target.label} inode changed while it was re-hashed"
            )
        if observed_size != artifact.size_bytes or digest.hexdigest() != artifact.sha256:
            raise VerificationError(f"{artifact.target.label} inode hash changed")
    finally:
        os.close(descriptor)


def _cleanup_artifacts(
    directory: EvidenceDirectory, artifacts: list[StagedArtifact]
) -> None:
    ordered = sorted(artifacts, key=lambda item: not item.target.marker)
    failures: list[str] = []
    for artifact in ordered:
        if artifact.published:
            if _owned_unlink(directory, artifact.target.name, artifact):
                artifact.published = False
            else:
                failures.append(f"published {artifact.target.label}")
        if artifact.temporary_name:
            if _owned_unlink(directory, artifact.temporary_name, artifact):
                artifact.temporary_name = ""
            else:
                failures.append(f"staged {artifact.target.label}")
    try:
        os.fsync(directory.descriptor)
    except OSError:
        failures.append("evidence-directory fsync")
    if failures:
        raise VerificationError(
            "release evidence cleanup failed: " + ", ".join(failures)
        )


def _publish_one(directory: EvidenceDirectory, artifact: StagedArtifact) -> None:
    if not artifact.temporary_name:
        raise VerificationError(f"{artifact.target.label} has no staged inode")
    _revalidate_artifact_inode(directory, artifact.temporary_name, artifact)
    try:
        os.link(
            artifact.temporary_name,
            artifact.target.name,
            src_dir_fd=directory.descriptor,
            dst_dir_fd=directory.descriptor,
            follow_symlinks=False,
        )
    except OSError as error:
        raise VerificationError(
            f"{artifact.target.label} output publication failed"
        ) from error
    artifact.published = True
    try:
        published = os.stat(
            artifact.target.name,
            dir_fd=directory.descriptor,
            follow_symlinks=False,
        )
    except OSError as error:
        raise VerificationError(
            f"{artifact.target.label} output could not be revalidated"
        ) from error
    if (published.st_dev, published.st_ino) != (artifact.device, artifact.inode):
        raise VerificationError(f"{artifact.target.label} output inode changed")
    try:
        os.unlink(artifact.temporary_name, dir_fd=directory.descriptor)
    except OSError as error:
        raise VerificationError(
            f"{artifact.target.label} staging link could not be retired"
        ) from error
    artifact.temporary_name = ""
    _revalidate_artifact_inode(directory, artifact.target.name, artifact)


def _publish_artifacts(
    directory: EvidenceDirectory, artifacts: list[StagedArtifact]
) -> None:
    markers = [artifact for artifact in artifacts if artifact.target.marker]
    if len(markers) != 1:
        raise VerificationError("release evidence requires exactly one attestation marker")
    marker = markers[0]
    prerequisites = [artifact for artifact in artifacts if not artifact.target.marker]
    _revalidate_evidence_directory(directory)
    for artifact in prerequisites:
        _publish_one(directory, artifact)
    os.fsync(directory.descriptor)
    _revalidate_evidence_directory(directory)
    for artifact in prerequisites:
        _revalidate_artifact_inode(directory, artifact.target.name, artifact)
    _publish_one(directory, marker)
    os.fsync(directory.descriptor)
    _revalidate_evidence_directory(directory)


def _closed_environment(private_home: Path) -> dict[str, str]:
    """Return the complete environment admitted to pinned release tools."""

    environment = {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_SYSTEM": "/dev/null",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": str(private_home),
        "LANG": "C",
        "LANGUAGE": "C",
        "LC_ALL": "C",
        "PATH": os.defpath,
        "TZ": "UTC",
        "XDG_CONFIG_HOME": str(private_home),
    }
    if sys.platform == "darwin":
        # CoreFoundation otherwise synthesizes this variable in subprocesses.
        # Pinning it makes the admitted environment explicit and reproducible.
        environment["__CF_USER_TEXT_ENCODING"] = f"0x{os.geteuid():X}:0x1:0xE"
    return environment


def _abort_process(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except (OSError, ProcessLookupError):
        try:
            process.kill()
        except OSError:
            pass
    try:
        process.wait(timeout=5)
    except (OSError, subprocess.TimeoutExpired):
        pass


def _run_bounded(
    executable: Path,
    arguments: list[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    maximum_output_bytes: int,
) -> CommandResult:
    argv = (str(executable), *arguments)
    try:
        process = subprocess.Popen(
            argv,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            start_new_session=True,
        )
    except OSError as error:
        raise VerificationError("pinned Git execution failed") from error
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    streams = {
        process.stdout.fileno(): ("stdout", process.stdout),
        process.stderr.fileno(): ("stderr", process.stderr),
    }
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    for descriptor, (label, stream) in streams.items():
        os.set_blocking(descriptor, False)
        selector.register(descriptor, selectors.EVENT_READ, (label, stream))
    deadline = time.monotonic() + _COMMAND_TIMEOUT_SECONDS
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                _abort_process(process)
                raise VerificationError("pinned Git execution exceeded its timeout")
            events = selector.select(min(remaining, 0.25))
            for key, _ in events:
                label, stream = key.data
                try:
                    chunk = os.read(key.fd, 64 * 1024)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fd)
                    stream.close()
                    continue
                buffers[label].extend(chunk)
                if sum(len(value) for value in buffers.values()) > maximum_output_bytes:
                    _abort_process(process)
                    raise VerificationError(
                        "pinned Git output exceeds its closed size limit"
                    )
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            _abort_process(process)
            raise VerificationError("pinned Git execution exceeded its timeout")
        try:
            returncode = process.wait(timeout=remaining)
        except subprocess.TimeoutExpired as error:
            _abort_process(process)
            raise VerificationError("pinned Git execution exceeded its timeout") from error
    except BaseException:
        if process.poll() is None:
            _abort_process(process)
        raise
    finally:
        selector.close()
        for stream in (process.stdout, process.stderr):
            if not stream.closed:
                stream.close()
    return CommandResult(
        argv,
        returncode,
        bytes(buffers["stdout"]),
        bytes(buffers["stderr"]),
    )


def _run_git(
    git_bin: Path,
    root: Path,
    environment: dict[str, str],
    arguments: list[str],
    *,
    maximum_output_bytes: int,
    check: bool = True,
) -> CommandResult:
    result = _run_bounded(
        git_bin,
        arguments,
        cwd=root,
        environment=environment,
        maximum_output_bytes=maximum_output_bytes,
    )
    if check and result.returncode != 0:
        raise VerificationError("pinned Git rejected a required identity query")
    return result


def _one_ascii_git_line(result: CommandResult, label: str) -> str:
    data = result.stdout
    if (
        result.stderr
        or not data.endswith(b"\n")
        or b"\n" in data[:-1]
        or b"\r" in data
        or b"\0" in data
    ):
        raise VerificationError(f"pinned Git returned malformed {label}")
    try:
        return data[:-1].decode("ascii")
    except UnicodeDecodeError as error:
        raise VerificationError(f"pinned Git returned malformed {label}") from error


def _git_top_level(
    git_bin: Path,
    root: Path,
    environment: dict[str, str],
) -> Path:
    result = _run_git(
        git_bin,
        root,
        environment,
        ["rev-parse", "--show-toplevel"],
        maximum_output_bytes=_MAX_QUERY_OUTPUT_BYTES,
    )
    data = result.stdout
    if (
        result.stderr
        or not data.endswith(b"\n")
        or b"\n" in data[:-1]
        or b"\r" in data
        or b"\0" in data
    ):
        raise VerificationError("pinned Git returned a malformed top-level path")
    try:
        return Path(os.fsdecode(data[:-1])).resolve(strict=True)
    except OSError as error:
        raise VerificationError("pinned Git returned an unavailable top-level path") from error


def _head_and_tree(
    git_bin: Path,
    root: Path,
    environment: dict[str, str],
) -> tuple[str, str]:
    head = _one_ascii_git_line(
        _run_git(
            git_bin,
            root,
            environment,
            ["rev-parse", "--verify", "HEAD^{commit}"],
            maximum_output_bytes=_MAX_QUERY_OUTPUT_BYTES,
        ),
        "HEAD",
    )
    tree = _one_ascii_git_line(
        _run_git(
            git_bin,
            root,
            environment,
            ["rev-parse", "--verify", f"{head}^{{tree}}"],
            maximum_output_bytes=_MAX_QUERY_OUTPUT_BYTES,
        ),
        "immutable HEAD tree",
    )
    if _OBJECT_ID_RE.fullmatch(head) is None or _OBJECT_ID_RE.fullmatch(tree) is None:
        raise VerificationError("pinned Git returned an invalid object identity")
    return head, tree


def _commit_object_id(raw_commit: bytes, hexadecimal_length: int) -> str:
    framed = b"commit " + str(len(raw_commit)).encode("ascii") + b"\0" + raw_commit
    if hexadecimal_length == 40:
        return hashlib.sha1(framed, usedforsecurity=False).hexdigest()
    if hexadecimal_length == 64:
        return hashlib.sha256(framed).hexdigest()
    raise VerificationError("release identity uses an unsupported Git object format")


def _commit_headers_and_message(
    raw_commit: bytes,
) -> tuple[list[tuple[bytes, list[bytes]]], bytes]:
    raw_headers, separator, message = raw_commit.partition(b"\n\n")
    if not separator or b"\r" in raw_headers or b"\0" in raw_headers:
        raise VerificationError("raw commit has malformed LF-only headers")
    records: list[tuple[bytes, list[bytes]]] = []
    for line in raw_headers.split(b"\n"):
        if line.startswith(b" "):
            if not records:
                raise VerificationError("raw commit has an orphan folded header")
            records[-1][1].append(line[1:])
            continue
        key, marker, value = line.partition(b" ")
        if not marker or not key or any(byte < 0x21 or byte > 0x7E for byte in key):
            raise VerificationError("raw commit has a malformed header")
        records.append((key, [value]))
    return records, message


def _validate_ssh_signature_header(records: list[tuple[bytes, list[bytes]]]) -> None:
    signatures = [(key, values) for key, values in records if key.startswith(b"gpgsig")]
    if len(signatures) != 1:
        raise VerificationError("raw commit must contain exactly one SSH signature header")
    key, values = signatures[0]
    if key != b"gpgsig":
        raise VerificationError("raw commit uses an unsupported signature header")
    signature = b"\n".join(values)
    if any(marker in signature for marker in _UNSUPPORTED_ARMOR_MARKERS):
        raise VerificationError("PGP and X509 commit signatures are not accepted")
    lines = signature.split(b"\n")
    if len(lines) < 3 or lines[0] != _SSH_ARMOR_BEGIN or lines[-1] != _SSH_ARMOR_END:
        raise VerificationError("raw commit does not contain exact SSH signature armor")
    encoded = b"".join(lines[1:-1])
    if not encoded:
        raise VerificationError("raw commit SSH signature armor is empty")
    try:
        base64.b64decode(encoded, validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise VerificationError("raw commit SSH signature armor is malformed") from error


def _validate_commit_message(raw_commit: bytes, identity: dict[str, Any]) -> None:
    records, message = _commit_headers_and_message(raw_commit)
    if b"\r" in message or b"\0" in message:
        raise VerificationError("raw commit has a malformed LF-only message")
    _validate_ssh_signature_header(records)
    trees = []
    for key, values in records:
        if key == b"tree":
            if len(values) != 1:
                raise VerificationError("raw commit has a folded tree header")
            try:
                trees.append(values[0].decode("ascii"))
            except UnicodeDecodeError as error:
                raise VerificationError("raw commit has a malformed tree header") from error
    if trees != [identity["head_tree"]]:
        raise VerificationError("raw commit tree does not match the release identity")
    try:
        message_text = message.decode("utf-8")
    except UnicodeDecodeError as error:
        raise VerificationError("raw commit message is not UTF-8") from error
    if not message_text.endswith("\n"):
        raise VerificationError("raw commit message lacks a terminal LF")

    expected = [
        f"{_TRAILER_VERSION}: 1",
        f"{_TRAILER_MANIFEST}: {identity['workspace_source_manifest_sha256']}",
        f"{_TRAILER_LOCK}: {identity['cargo_lock_sha256']}",
    ]
    lines = message_text[:-1].split("\n")
    recognized: list[int] = []
    folded_keys = {key.casefold() for key in _TRAILER_KEYS}
    for index, line in enumerate(lines):
        key, marker, _ = line.partition(":")
        if marker and key.casefold() in folded_keys:
            recognized.append(index)
    terminal_indexes = list(range(len(lines) - 3, len(lines)))
    if (
        len(lines) < 5
        or lines[-3:] != expected
        or lines[-4] != ""
        or not lines[-5]
        or recognized != terminal_indexes
    ):
        raise VerificationError(
            "raw commit lacks the exact terminal Sumeragi v2 release trailer block"
        )


def _signature_config(
    ssh_keygen: Path,
    allowed_signers: Path,
    revocation_file: Path,
) -> list[str]:
    return [
        "-c",
        "gpg.format=ssh",
        "-c",
        "gpg.minTrustLevel=fully",
        "-c",
        f"gpg.ssh.program={ssh_keygen}",
        "-c",
        f"gpg.ssh.allowedSignersFile={allowed_signers}",
        "-c",
        f"gpg.ssh.revocationFile={revocation_file}",
        "-c",
        f"gpg.program={ssh_keygen}",
        "-c",
        f"gpg.openpgp.program={ssh_keygen}",
        "-c",
        f"gpg.x509.program={ssh_keygen}",
    ]


def _verification_metadata(result: CommandResult) -> tuple[str, str, str, str]:
    if not result.stdout.endswith(b"\0\n"):
        raise VerificationError("pinned Git returned malformed signature metadata")
    fields = result.stdout[:-2].split(b"\0")
    if len(fields) != 4:
        raise VerificationError("pinned Git returned malformed signature metadata")
    try:
        decoded = tuple(field.decode("utf-8") for field in fields)
    except UnicodeDecodeError as error:
        raise VerificationError("pinned Git returned non-UTF-8 signature metadata") from error
    for value in decoded:
        if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
            raise VerificationError("pinned Git returned malformed signature metadata")
    status, fingerprint, primary_fingerprint, signer = decoded
    if status != "G":
        raise VerificationError("HEAD does not have a trusted SSH signature status")
    if _SSH_FINGERPRINT_RE.fullmatch(fingerprint) is None:
        raise VerificationError("HEAD signature has no valid SSH signer fingerprint")
    if not signer:
        raise VerificationError("HEAD signature has no allowed-signers principal")
    if primary_fingerprint:
        raise VerificationError(
            "SSH signature metadata unexpectedly contains a primary-key fingerprint"
        )
    return status, fingerprint, primary_fingerprint, signer


def _command_evidence(
    result: CommandResult,
    replay_substitutions: dict[str, str] | None = None,
) -> dict[str, Any]:
    replay_argv = list(result.argv)
    if replay_substitutions:
        replay_argv = [
            _replace_paths(argument, replay_substitutions) for argument in replay_argv
        ]
    return {
        "argv": list(result.argv),
        "replay_argv": replay_argv,
        "exit_status": result.returncode,
        "stdout_base64": base64.b64encode(result.stdout).decode("ascii"),
        "stdout_sha256": _sha256(result.stdout),
        "stdout_size_bytes": len(result.stdout),
        "stderr_base64": base64.b64encode(result.stderr).decode("ascii"),
        "stderr_sha256": _sha256(result.stderr),
        "stderr_size_bytes": len(result.stderr),
    }


def _replace_paths(value: str, substitutions: dict[str, str]) -> str:
    for source in sorted(substitutions, key=len, reverse=True):
        value = value.replace(source, substitutions[source])
    return value


def _artifact(
    data: bytes, mode: int, archive_name: str | None = None
) -> dict[str, int | str]:
    artifact: dict[str, int | str] = {
        "mode": f"{mode:04o}",
        "sha256": _sha256(data),
        "size_bytes": len(data),
    }
    if archive_name is not None:
        artifact["archive_name"] = archive_name
    return artifact


def _protected_artifact(
    data: bytes, mode: int, archive_name: str, protected_sha256: str
) -> dict[str, int | str]:
    return {
        "archive_name": archive_name,
        "mode": f"{mode:04o}",
        "observed_sha256": _sha256(data),
        "protected_sha256": protected_sha256,
        "size_bytes": len(data),
    }


def verify(args: argparse.Namespace) -> None:
    root = _validate_root(args.root)
    identity, identity_snapshot = _load_identity(args.identity)
    expected_git_sha = _require_digest(args.expected_git_sha256, "expected Git digest")
    expected_ssh_sha = _require_digest(
        args.expected_ssh_keygen_sha256, "expected ssh-keygen digest"
    )
    expected_allowed_sha = _require_digest(
        args.expected_ssh_allowed_signers_sha256,
        "expected SSH allowed-signers digest",
    )
    expected_revocation_sha = _require_digest(
        args.expected_ssh_revocation_sha256,
        "expected SSH revocation-policy digest",
    )
    expected_fingerprint = _validate_fingerprint(args.expected_signer_fingerprint)

    git_snapshot = _validate_tool(args.git_bin, "pinned Git executable")
    ssh_snapshot = _validate_tool(args.ssh_keygen_bin, "pinned ssh-keygen executable")
    allowed_snapshot = _read_regular_file(
        args.ssh_allowed_signers,
        "SSH allowed-signers file",
        maximum_bytes=_MAX_POLICY_BYTES,
    )
    if not allowed_snapshot.data:
        raise VerificationError("SSH allowed-signers file must not be empty")
    _validate_allowed_signers_policy(allowed_snapshot.data)
    revocation_snapshot = _read_regular_file(
        args.ssh_revocation_file,
        "SSH revocation-policy file",
        maximum_bytes=_MAX_POLICY_BYTES,
    )
    _require_snapshot_digest(git_snapshot, expected_git_sha, "pinned Git executable")
    _require_snapshot_digest(
        ssh_snapshot, expected_ssh_sha, "pinned ssh-keygen executable"
    )
    _require_snapshot_digest(
        allowed_snapshot, expected_allowed_sha, "SSH allowed-signers file"
    )
    _require_snapshot_digest(
        revocation_snapshot, expected_revocation_sha, "SSH revocation-policy file"
    )

    lock_snapshot = _read_regular_file(
        root / "Cargo.lock",
        "workspace Cargo.lock",
        maximum_bytes=_MAX_LOCK_BYTES,
    )
    if _sha256(lock_snapshot.data) != identity["cargo_lock_sha256"]:
        raise VerificationError("workspace Cargo.lock does not match the release identity")

    directory, targets = _prepare_evidence_directory(_output_targets(args), root)
    staged: list[StagedArtifact] = []
    success = False
    try:
        git_artifact = _stage_artifact(
            directory, targets["Git archive"], git_snapshot.data
        )
        staged.append(git_artifact)
        ssh_artifact = _stage_artifact(
            directory, targets["ssh-keygen archive"], ssh_snapshot.data
        )
        staged.append(ssh_artifact)
        allowed_artifact = _stage_artifact(
            directory,
            targets["SSH allowed-signers archive"],
            allowed_snapshot.data,
        )
        staged.append(allowed_artifact)
        revocation_artifact = _stage_artifact(
            directory,
            targets["SSH revocation-policy archive"],
            revocation_snapshot.data,
        )
        staged.append(revocation_artifact)

        stable_git = _staged_path(directory, git_artifact)
        stable_ssh = _staged_path(directory, ssh_artifact)
        stable_allowed = _staged_path(directory, allowed_artifact)
        stable_revocation = _staged_path(directory, revocation_artifact)
        environment = _closed_environment(directory.path)

        ssh_probe = _run_bounded(
            stable_ssh,
            ["-?"],
            cwd=root,
            environment=environment,
            maximum_output_bytes=_MAX_QUERY_OUTPUT_BYTES,
        )
        if ssh_probe.returncode < 0:
            raise VerificationError(
                "stable private ssh-keygen copy could not execute on this platform"
            )

        if _git_top_level(stable_git, root, environment) != root:
            raise VerificationError("release root is not the exact Git top-level")
        head, tree = _head_and_tree(stable_git, root, environment)
        if head != identity["head_commit"] or tree != identity["head_tree"]:
            raise VerificationError("current HEAD/tree does not match the release identity")

        commit_oid = identity["head_commit"]
        raw_result = _run_git(
            stable_git,
            root,
            environment,
            ["cat-file", "commit", commit_oid],
            maximum_output_bytes=_MAX_RAW_COMMIT_BYTES,
        )
        if raw_result.stderr:
            raise VerificationError("pinned Git returned malformed raw commit output")
        raw_commit = raw_result.stdout
        if _commit_object_id(raw_commit, len(commit_oid)) != commit_oid:
            raise VerificationError("raw commit bytes do not reproduce HEAD")
        _validate_commit_message(raw_commit, identity)

        signature_config = _signature_config(
            stable_ssh, stable_allowed, stable_revocation
        )
        verify_result = _run_git(
            stable_git,
            root,
            environment,
            [*signature_config, "verify-commit", "--raw", commit_oid],
            maximum_output_bytes=_MAX_VERIFY_OUTPUT_BYTES,
            check=False,
        )
        if verify_result.returncode != 0:
            raise VerificationError("Git cryptographic verification of the candidate failed")
        show_result = _run_git(
            stable_git,
            root,
            environment,
            [
                *signature_config,
                "show",
                "--no-patch",
                "--format=%G?%x00%GF%x00%GP%x00%GS%x00",
                commit_oid,
            ],
            maximum_output_bytes=_MAX_SHOW_OUTPUT_BYTES,
        )
        status, fingerprint, primary_fingerprint, signer = _verification_metadata(
            show_result
        )
        if fingerprint != expected_fingerprint:
            raise VerificationError(
                "candidate signer fingerprint does not match protected policy"
            )

        final_head, final_tree = _head_and_tree(stable_git, root, environment)
        if (final_head, final_tree) != (head, tree):
            raise VerificationError("HEAD/tree changed during signature verification")
        if _git_top_level(stable_git, root, environment) != root:
            raise VerificationError("release Git top-level changed during verification")
        _require_unchanged_file(
            identity_snapshot,
            "release identity",
            maximum_bytes=_MAX_IDENTITY_BYTES,
        )
        _require_unchanged_file(
            lock_snapshot,
            "workspace Cargo.lock",
            maximum_bytes=_MAX_LOCK_BYTES,
        )
        _require_unchanged_file(
            git_snapshot,
            "pinned Git executable",
            maximum_bytes=_MAX_TOOL_BYTES,
            executable=True,
        )
        _require_unchanged_file(
            ssh_snapshot,
            "pinned ssh-keygen executable",
            maximum_bytes=_MAX_TOOL_BYTES,
            executable=True,
        )
        _require_unchanged_file(
            allowed_snapshot,
            "SSH allowed-signers file",
            maximum_bytes=_MAX_POLICY_BYTES,
        )
        _require_unchanged_file(
            revocation_snapshot,
            "SSH revocation-policy file",
            maximum_bytes=_MAX_POLICY_BYTES,
        )
        _revalidate_evidence_directory(directory)

        tools = {
            "git": {
                "archive_name": targets["Git archive"].name,
                "mode": "0500",
                "observed_sha256": _sha256(git_snapshot.data),
                "protected_sha256": expected_git_sha,
                "size_bytes": len(git_snapshot.data),
                "source_path": str(git_snapshot.path),
            },
            "ssh_keygen": {
                "archive_name": targets["ssh-keygen archive"].name,
                "mode": "0500",
                "observed_sha256": _sha256(ssh_snapshot.data),
                "protected_sha256": expected_ssh_sha,
                "size_bytes": len(ssh_snapshot.data),
                "source_path": str(ssh_snapshot.path),
            },
        }
        policies = {
            "expected_signer_fingerprint": expected_fingerprint,
            "signature_format": "ssh",
            "ssh_allowed_signers": _protected_artifact(
                allowed_snapshot.data,
                _DATA_MODE,
                targets["SSH allowed-signers archive"].name,
                expected_allowed_sha,
            ),
            "ssh_revocation": _protected_artifact(
                revocation_snapshot.data,
                _DATA_MODE,
                targets["SSH revocation-policy archive"].name,
                expected_revocation_sha,
            ),
        }
        archive_names = {
            "cargo_lock": targets["Cargo.lock archive"].name,
            "git": targets["Git archive"].name,
            "raw_commit": targets["raw commit"].name,
            "ssh_allowed_signers": targets["SSH allowed-signers archive"].name,
            "ssh_keygen": targets["ssh-keygen archive"].name,
            "ssh_revocation": targets["SSH revocation-policy archive"].name,
            "verify_transcript": targets["verify transcript"].name,
        }
        replay_substitutions = {
            str(stable_git): "${EVIDENCE_DIRECTORY}/" + archive_names["git"],
            str(stable_ssh): "${EVIDENCE_DIRECTORY}/" + archive_names["ssh_keygen"],
            str(stable_allowed): (
                "${EVIDENCE_DIRECTORY}/" + archive_names["ssh_allowed_signers"]
            ),
            str(stable_revocation): (
                "${EVIDENCE_DIRECTORY}/" + archive_names["ssh_revocation"]
            ),
            str(directory.path): "${EVIDENCE_DIRECTORY}",
        }
        transcript = _canonical_json(
            {
                "schema_version": 2,
                "archive_names": archive_names,
                "candidate_commit_oid": commit_oid,
                "environment": environment,
                "policy_overrides": signature_config,
                "policies": policies,
                "replay": {
                    "candidate_root": "${CANDIDATE_ROOT}",
                    "evidence_directory": "${EVIDENCE_DIRECTORY}",
                    "environment": {
                        key: _replace_paths(value, replay_substitutions)
                        for key, value in environment.items()
                    },
                    "policy_overrides": [
                        _replace_paths(value, replay_substitutions)
                        for value in signature_config
                    ],
                },
                "tools": tools,
                "commands": {
                    "show_signature_metadata": _command_evidence(
                        show_result, replay_substitutions
                    ),
                    "verify_commit": _command_evidence(
                        verify_result, replay_substitutions
                    ),
                },
                "tool_probes": {
                    "ssh_keygen_usage": _command_evidence(
                        ssh_probe, replay_substitutions
                    ),
                },
            }
        )
        transcript_artifact = _stage_artifact(
            directory, targets["verify transcript"], transcript
        )
        staged.append(transcript_artifact)
        raw_artifact = _stage_artifact(
            directory, targets["raw commit"], raw_commit
        )
        staged.append(raw_artifact)
        lock_artifact = _stage_artifact(
            directory, targets["Cargo.lock archive"], lock_snapshot.data
        )
        staged.append(lock_artifact)

        evidence = {
            "cargo_lock": _artifact(
                lock_snapshot.data, _DATA_MODE, archive_names["cargo_lock"]
            ),
            "git": _artifact(git_snapshot.data, _TOOL_MODE, archive_names["git"]),
            "raw_commit": _artifact(
                raw_commit, _DATA_MODE, archive_names["raw_commit"]
            ),
            "ssh_allowed_signers": _artifact(
                allowed_snapshot.data,
                _DATA_MODE,
                archive_names["ssh_allowed_signers"],
            ),
            "ssh_keygen": _artifact(
                ssh_snapshot.data, _TOOL_MODE, archive_names["ssh_keygen"]
            ),
            "ssh_revocation": _artifact(
                revocation_snapshot.data,
                _DATA_MODE,
                archive_names["ssh_revocation"],
            ),
            "verify_transcript": _artifact(
                transcript, _DATA_MODE, archive_names["verify_transcript"]
            ),
        }
        attestation = _canonical_json(
            {
                "schema_version": 2,
                "release_identity": identity,
                "release_identity_sha256": _sha256(identity_snapshot.data),
                "tools": tools,
                "policies": policies,
                "verification": {
                    "status": status,
                    "signer_fingerprint": fingerprint,
                    "primary_key_fingerprint": primary_fingerprint,
                    "allowed_signers_principal": signer,
                },
                "evidence": evidence,
            }
        )
        attestation_artifact = _stage_artifact(
            directory, targets["attestation"], attestation
        )
        staged.append(attestation_artifact)
        _publish_artifacts(directory, staged)
        success = True
    finally:
        cleanup_failure: BaseException | None = None
        try:
            if not success:
                _cleanup_artifacts(directory, staged)
        except BaseException as error:
            cleanup_failure = error
        try:
            os.close(directory.descriptor)
        except OSError as error:
            if not success and cleanup_failure is None:
                cleanup_failure = VerificationError(
                    "release evidence directory close failed after rollback"
                )
        if cleanup_failure is not None:
            raise cleanup_failure


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--identity", type=Path, required=True)
    parser.add_argument("--git-bin", type=Path, required=True)
    parser.add_argument("--expected-git-sha256", required=True)
    parser.add_argument("--ssh-keygen-bin", type=Path, required=True)
    parser.add_argument("--expected-ssh-keygen-sha256", required=True)
    parser.add_argument("--expected-signer-fingerprint", required=True)
    parser.add_argument("--ssh-allowed-signers", type=Path, required=True)
    parser.add_argument("--expected-ssh-allowed-signers-sha256", required=True)
    parser.add_argument("--ssh-revocation-file", type=Path, required=True)
    parser.add_argument("--expected-ssh-revocation-sha256", required=True)
    parser.add_argument("--attestation-output", type=Path, required=True)
    parser.add_argument("--verify-transcript-output", type=Path, required=True)
    parser.add_argument("--raw-commit-output", type=Path, required=True)
    parser.add_argument("--cargo-lock-output", type=Path, required=True)
    parser.add_argument("--ssh-allowed-signers-output", type=Path, required=True)
    parser.add_argument("--ssh-revocation-output", type=Path, required=True)
    parser.add_argument("--git-archive-output", type=Path, required=True)
    parser.add_argument("--ssh-keygen-archive-output", type=Path, required=True)
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        verify(args)
    except (VerificationError, OSError) as error:
        print(
            f"Sumeragi v2 release identity verification failed: {error}",
            file=sys.stderr,
        )
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
