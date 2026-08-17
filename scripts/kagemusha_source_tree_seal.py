#!/usr/bin/env python3
"""Capture and verify the exact clean Kagemusha source closure.

First-release candidate generation requires one SSH signature trusted by the
reviewer's user-level allowed-signers policy on the exact checked-out commit,
an index identical to HEAD, no untracked or ignored files, one tracked mode
``100644`` root ``Cargo.lock`` that is also bound separately, and a full
source-tree identity matching one independently pinned canonical descriptor.
``descriptor`` emits the clean observation that must be reviewed and pinned.
Gitlinks bind their
exact index commit and must be represented by an empty, non-symlink directory.
``identity`` and ``fingerprint`` never accept an unpinned observation.

Repository-local Git signature configuration is untrusted.  The verifier pins
``/usr/bin/ssh-keygen``, reads only the user-level allowed-signers/revocation
paths, snapshots those owner-controlled policies, and overrides every Git
signature-format, trust, policy, and executable setting for verification.
The in-process identity also carries the raw HEAD object's tree, ordered
parents, committer epoch, object digest/size, and the sole verified SSH
principal/key/policy digests so build projections cannot supply those facts.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
import pathlib
import re
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from typing import Any


SOURCE_TREE_DOMAIN = b"iroha.kagemusha.full-source-tree-sha256.v4\0"
SOURCE_DIFF_DOMAIN = b"iroha-source-diff-v1\0"
TRACKED_DIFF_DOMAIN = b"tracked-binary-diff-sha256\0"
UNTRACKED_MANIFEST_DOMAIN = b"untracked-path-blob-manifest-sha256\0"
REVIEWED_SOURCE_CLOSURE_SCHEMA = "iroha.reviewed-source-closure.v1"
SOURCE_IDENTITY_SCHEMA = "iroha.kagemusha.reviewed_source_tree_identity.v1"
ALLOWED_INDEX_MODES = {b"100644", b"100755", b"120000", b"160000"}
ALLOWED_UNTRACKED_MODES = {"100644", "100755"}
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()
MAX_DESCRIPTOR_BYTES = 16 * 1024 * 1024
MAX_CARGO_LOCK_BYTES = 16 * 1024 * 1024
MAX_UNTRACKED_FILE_BYTES = 16 * 1024 * 1024
MAX_UNTRACKED_FILES = 0
MAX_ALLOWED_SIGNERS_BYTES = 64 * 1024
MAX_REVOCATION_BYTES = 16 * 1024 * 1024
REQUIRED_TRACKED_BUILD_INPUT = b"Cargo.lock"
GIT = pathlib.Path("/usr/bin/git")
SSH_KEYGEN = pathlib.Path("/usr/bin/ssh-keygen")
GIT_ARGUMENT_PREFIX = (
    "-c",
    "core.attributesFile=/dev/null",
    "-c",
    "core.excludesFile=/dev/null",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.untrackedCache=false",
)
TRACKED_DIFF_ARGUMENTS = (
    "--no-pager",
    "diff",
    "--binary",
    "--full-index",
    "--no-renames",
    "--diff-algorithm=myers",
    "--no-ext-diff",
    "--no-textconv",
    "--ignore-submodules=none",
    "--cached",
    "HEAD",
    "--",
    ".",
)
DESCRIPTOR_KEYS = {
    "schema",
    "base_commit",
    "source_commit",
    "source_repo_dirty",
    "source_tree_sha256",
    "tracked_binary_diff_sha256",
    "untracked_file_count",
    "untracked_path_mode_blob_oid_manifest",
    "untracked_path_mode_blob_oid_manifest_sha256",
    "ignored_cargo_lock_size_bytes",
    "ignored_cargo_lock_sha256",
    "combined_source_fingerprint_sha256",
}
MANIFEST_ENTRY_KEYS = {
    "blob_sha256",
    "git_blob_oid",
    "git_mode",
    "path",
    "path_bytes_base64",
}


class SourceSealError(RuntimeError):
    """The checkout cannot produce the exact independently pinned source seal."""


@dataclass(frozen=True)
class IndexEntry:
    mode: bytes
    object_id: bytes
    path: bytes


@dataclass(frozen=True)
class OpenedDirectory:
    """One descriptor-retained component in a repository-relative path."""

    parent_descriptor: int
    name: bytes
    descriptor: int
    identity: tuple[int, ...]


@dataclass(frozen=True)
class SignaturePolicy:
    """Stable user-level SSH trust policy used for source authentication."""

    allowed_signers: bytes
    revocation: bytes
    signer: "VerifiedSshSignature | None" = None


@dataclass(frozen=True)
class VerifiedSshSignature:
    """The sole SSH signer admitted by the exact verified trust policy."""

    principal: str
    public_key_sha256: str
    allowed_signers_sha256: str
    revocation_sha256: str


@dataclass(frozen=True)
class SourceAuthority:
    """Facts derived from the verified raw HEAD commit and its exact parents."""

    commit: str
    commit_object_sha256: str
    commit_object_size: int
    committer_epoch: int
    git_tree: str
    ordered_parents: tuple[str, ...]
    ordered_parent_trees: tuple[str, ...]
    signature: VerifiedSshSignature


@dataclass(frozen=True)
class SourceIdentity:
    source_commit: str
    source_tree_sha256: str
    source_repo_dirty: bool
    reviewed_source_closure: dict[str, Any]
    reviewed_source_closure_descriptor_sha256: str
    source_authority: SourceAuthority


def _git_environment() -> dict[str, str]:
    return {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_LITERAL_PATHSPECS": "1",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_PAGER": "cat",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PAGER": "cat",
        "PATH": "/usr/bin:/bin",
        "TZ": "UTC",
    }


def _git(root: pathlib.Path, *arguments: str) -> bytes:
    if not GIT.is_file() or GIT.is_symlink():
        raise SourceSealError("pinned /usr/bin/git is unavailable")
    try:
        return subprocess.run(
            [
                os.fspath(GIT),
                *GIT_ARGUMENT_PREFIX,
                "-C",
                os.fspath(root),
                *arguments,
            ],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=_git_environment(),
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        raise SourceSealError(f"pinned Git failed: {' '.join(arguments)}") from exc


def _global_signature_config_path(
    key: str, *, required: bool
) -> pathlib.Path | None:
    environment = _git_environment()
    environment.pop("GIT_CONFIG_GLOBAL", None)
    home_text = os.environ.get("HOME", "")
    if not home_text:
        raise SourceSealError("HOME is required to select the user signature trust root")
    home = pathlib.Path(home_text)
    if not home.is_absolute() or os.path.normpath(home_text) != home_text:
        raise SourceSealError("HOME must be one absolute normalized path")
    environment["HOME"] = home_text
    try:
        completed = subprocess.run(
            [
                os.fspath(GIT),
                *GIT_ARGUMENT_PREFIX,
                "config",
                "--global",
                "--path",
                "--get",
                key,
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
        )
    except OSError as exc:
        raise SourceSealError("could not read the user signature trust policy") from exc
    if completed.returncode == 1 and not completed.stdout and not required:
        return None
    if completed.returncode != 0:
        raise SourceSealError(f"user Git config must define exactly one {key}")
    payload = completed.stdout
    if (
        not payload.endswith(b"\n")
        or b"\n" in payload[:-1]
        or b"\r" in payload
        or b"\0" in payload
    ):
        raise SourceSealError(f"user Git config has a malformed {key}")
    try:
        value = pathlib.Path(payload[:-1].decode("utf-8"))
    except UnicodeDecodeError as exc:
        raise SourceSealError(f"user Git config has a non-UTF-8 {key}") from exc
    if (
        not value.is_absolute()
        or os.path.normpath(os.fspath(value)) != os.fspath(value)
    ):
        raise SourceSealError(f"user Git config {key} must be absolute and normalized")
    return value


def _open_absolute_parent_directory(
    path: pathlib.Path, label: str
) -> tuple[int, int, bytes, list[OpenedDirectory]]:
    path_text = os.fspath(path)
    if not path.is_absolute() or os.path.normpath(path_text) != path_text:
        raise SourceSealError(f"{label} path must be absolute and normalized")
    components = [os.fsencode(component) for component in path.parts[1:]]
    if not components or any(
        component in (b"", b".", b"..") for component in components
    ):
        raise SourceSealError(f"{label} path is not canonical")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    root_descriptor = os.open(b"/", flags)
    current = root_descriptor
    chain: list[OpenedDirectory] = []
    try:
        for component in components[:-1]:
            before = os.stat(component, dir_fd=current, follow_symlinks=False)
            if not stat.S_ISDIR(before.st_mode) or stat.S_ISLNK(before.st_mode):
                raise SourceSealError(f"{label} path traverses a symlink")
            child = os.open(component, flags, dir_fd=current)
            opened = os.fstat(child)
            if _stable_directory_identity(before) != _stable_directory_identity(opened):
                os.close(child)
                raise SourceSealError(f"{label} ancestor changed while opened")
            chain.append(
                OpenedDirectory(
                    parent_descriptor=current,
                    name=component,
                    descriptor=child,
                    identity=_stable_directory_identity(opened),
                )
            )
            current = child
    except SourceSealError:
        _close_directory_chain(chain)
        os.close(root_descriptor)
        raise
    except OSError as exc:
        _close_directory_chain(chain)
        os.close(root_descriptor)
        raise SourceSealError(f"{label} path is missing or unsafe") from exc
    return root_descriptor, current, components[-1], chain


def _read_bounded_absolute_file(
    path: pathlib.Path,
    label: str,
    maximum_bytes: int,
    *,
    allow_empty: bool,
    owner_controlled: bool,
) -> bytes:
    root_descriptor, parent, name, chain = _open_absolute_parent_directory(path, label)
    descriptor: int | None = None
    try:
        before = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_size > maximum_bytes
            or (not allow_empty and before.st_size == 0)
            or (
                owner_controlled
                and (before.st_uid != os.geteuid() or stat.S_IMODE(before.st_mode) & 0o022)
            )
        ):
            raise SourceSealError(f"{label} must be one bounded regular file")
        flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        descriptor = os.open(name, flags, dir_fd=parent)
        opened_before = os.fstat(descriptor)
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(descriptor, 64 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if total > maximum_bytes:
                raise SourceSealError(f"{label} exceeds its byte bound")
            chunks.append(chunk)
        opened_after = os.fstat(descriptor)
        after = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not (
            _stable_identity(before)
            == _stable_identity(opened_before)
            == _stable_identity(opened_after)
            == _stable_identity(after)
        ):
            raise SourceSealError(f"{label} changed while read")
        payload = b"".join(chunks)
        if (not allow_empty and not payload) or len(payload) != opened_after.st_size:
            raise SourceSealError(f"{label} has an invalid size")
        return payload
    except OSError as exc:
        raise SourceSealError(f"{label} is missing or unsafe") from exc
    finally:
        if descriptor is not None:
            os.close(descriptor)
        try:
            _verify_directory_chain(chain, os.fsencode(path))
        finally:
            os.close(root_descriptor)


def _read_signature_policy_file(
    path: pathlib.Path, label: str, maximum_bytes: int, *, allow_empty: bool
) -> bytes:
    return _read_bounded_absolute_file(
        path,
        label,
        maximum_bytes,
        allow_empty=allow_empty,
        owner_controlled=True,
    )


def _validate_allowed_signers(payload: bytes) -> tuple[str, str]:
    """Return the sole portable principal and SHA-256 of its SSH key blob."""

    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise SourceSealError("SSH allowed-signers policy must be UTF-8") from exc
    if "\r" in text or "\0" in text or not text.endswith("\n"):
        raise SourceSealError("SSH allowed-signers policy must be canonical LF text")
    active = [line for line in text.splitlines() if line and not line.startswith("#")]
    if len(active) != 1:
        raise SourceSealError("SSH allowed-signers policy must contain exactly one active key")
    folded = active[0].casefold()
    if (
        "cert-authority" in folded
        or "-cert-v01@openssh.com" in folded
        or "valid-after=" in folded
        or "valid-before=" in folded
    ):
        raise SourceSealError(
            "SSH allowed-signers policy forbids certificates and time-dependent keys"
        )
    fields = active[0].split()
    if len(fields) < 3:
        raise SourceSealError("SSH allowed-signers policy entry is incomplete")
    principal, key_type, encoded_key = fields[:3]
    if (
        not principal
        or len(principal) > 128
        or any(
            not (character.isascii() and (character.isalnum() or character in "._@+-"))
            for character in principal
        )
    ):
        raise SourceSealError(
            "SSH allowed-signers policy must name exactly one portable principal"
        )
    if (
        not key_type.isascii()
        or not key_type
        or "-cert-" in key_type
        or any(not (character.isalnum() or character in "@._+-") for character in key_type)
    ):
        raise SourceSealError("SSH allowed-signers policy key type is malformed")
    try:
        key_blob = base64.b64decode(encoded_key, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise SourceSealError("SSH allowed-signers public key is not canonical base64") from exc
    if base64.b64encode(key_blob).decode("ascii") != encoded_key or len(key_blob) < 4:
        raise SourceSealError("SSH allowed-signers public key is not canonical")
    type_size = int.from_bytes(key_blob[:4], "big")
    if type_size == 0 or 4 + type_size > len(key_blob):
        raise SourceSealError("SSH allowed-signers public key blob is malformed")
    try:
        blob_key_type = key_blob[4 : 4 + type_size].decode("ascii")
    except UnicodeDecodeError as exc:
        raise SourceSealError("SSH allowed-signers public key type is not ASCII") from exc
    if blob_key_type != key_type:
        raise SourceSealError(
            "SSH allowed-signers public key type differs from its key blob"
        )
    return principal, hashlib.sha256(key_blob).hexdigest()


def _load_signature_policy() -> SignaturePolicy:
    allowed_path = _global_signature_config_path(
        "gpg.ssh.allowedSignersFile", required=True
    )
    assert allowed_path is not None
    revocation_path = _global_signature_config_path(
        "gpg.ssh.revocationFile", required=False
    )
    allowed = _read_signature_policy_file(
        allowed_path,
        "SSH allowed-signers policy",
        MAX_ALLOWED_SIGNERS_BYTES,
        allow_empty=False,
    )
    principal, public_key_sha256 = _validate_allowed_signers(allowed)
    revocation = (
        b""
        if revocation_path is None
        else _read_signature_policy_file(
            revocation_path,
            "SSH revocation policy",
            MAX_REVOCATION_BYTES,
            allow_empty=True,
        )
    )
    return SignaturePolicy(
        allowed_signers=allowed,
        revocation=revocation,
        signer=VerifiedSshSignature(
            principal=principal,
            public_key_sha256=public_key_sha256,
            allowed_signers_sha256=hashlib.sha256(allowed).hexdigest(),
            revocation_sha256=hashlib.sha256(revocation).hexdigest(),
        ),
    )


def _require_one_ssh_signature(raw_commit: bytes) -> None:
    headers, separator, _ = raw_commit.partition(b"\n\n")
    if not separator:
        raise SourceSealError("source commit object has no canonical header boundary")
    lines = headers.split(b"\n")
    signature_indexes = [
        index for index, line in enumerate(lines) if line.startswith(b"gpgsig ")
    ]
    if len(signature_indexes) != 1:
        raise SourceSealError("source commit must contain exactly one SSH signature")
    start = signature_indexes[0]
    signature = [lines[start][len(b"gpgsig ") :]]
    for line in lines[start + 1 :]:
        if not line.startswith(b" "):
            break
        signature.append(line[1:])
    payload = b"\n".join(signature)
    if (
        payload.count(b"-----BEGIN SSH SIGNATURE-----") != 1
        or payload.count(b"-----END SSH SIGNATURE-----") != 1
        or any(
            marker in payload
            for marker in (
                b"-----BEGIN PGP SIGNATURE-----",
                b"-----BEGIN SIGNED MESSAGE-----",
                b"-----BEGIN CERTIFICATE-----",
            )
        )
    ):
        raise SourceSealError("source commit signature must be exactly one SSH signature")


def _commit_tree_and_parents(raw_commit: bytes) -> tuple[str, tuple[str, ...]]:
    """Extract the exact tree and ordered parent list from a raw commit payload."""

    headers, separator, _ = raw_commit.partition(b"\n\n")
    if not separator:
        raise SourceSealError("source commit object has no canonical header boundary")
    lines = headers.split(b"\n")
    trees = [line[5:] for line in lines if line.startswith(b"tree ")]
    parents = [line[7:] for line in lines if line.startswith(b"parent ")]
    if len(trees) != 1 or lines[0] != b"tree " + trees[0]:
        raise SourceSealError("source commit must contain exactly one leading Git tree")

    def commit_id(value: bytes, label: str) -> str:
        if (
            len(value) != 40
            or value == b"0" * 40
            or any(byte not in b"0123456789abcdef" for byte in value)
        ):
            raise SourceSealError(f"source commit {label} is not canonical SHA-1")
        return value.decode("ascii")

    return commit_id(trees[0], "tree"), tuple(
        commit_id(parent, "parent") for parent in parents
    )


def _commit_epoch(raw_commit: bytes) -> int:
    """Extract the exact committer epoch from a raw Git commit payload."""

    headers = raw_commit.partition(b"\n\n")[0]
    committers = [
        line for line in headers.split(b"\n") if line.startswith(b"committer ")
    ]
    if len(committers) != 1:
        raise SourceSealError("source commit must contain exactly one committer header")
    matched = re.fullmatch(
        rb"committer .+ ([0-9]+) ([+-](?:0[0-9]|1[0-4])[0-5][0-9])",
        committers[0],
    )
    if matched is None:
        raise SourceSealError("source commit committer header is malformed")
    epoch_text = matched.group(1)
    if len(epoch_text) > 1 and epoch_text.startswith(b"0"):
        raise SourceSealError("source commit committer epoch is not canonical")
    epoch = int(epoch_text)
    if not 1 <= epoch <= (2**63 - 1):
        raise SourceSealError("source commit committer epoch is outside its bound")
    return epoch


def _source_authority_from_verified_commit(
    root: pathlib.Path,
    commit: str,
    raw_commit: bytes,
    signature: VerifiedSshSignature,
) -> SourceAuthority:
    """Derive source authority solely from a verified raw commit and its parents."""

    if (
        len(commit) != 40
        or commit == "0" * 40
        or any(character not in "0123456789abcdef" for character in commit)
    ):
        raise SourceSealError("verified source commit id is malformed")
    git_tree, ordered_parents = _commit_tree_and_parents(raw_commit)
    ordered_parent_trees: list[str] = []
    for parent in ordered_parents:
        parent_payload = _git(root, "cat-file", "commit", parent)
        parent_tree, _ = _commit_tree_and_parents(parent_payload)
        ordered_parent_trees.append(parent_tree)
    return SourceAuthority(
        commit=commit,
        commit_object_sha256=hashlib.sha256(raw_commit).hexdigest(),
        commit_object_size=len(raw_commit),
        committer_epoch=_commit_epoch(raw_commit),
        git_tree=git_tree,
        ordered_parents=ordered_parents,
        ordered_parent_trees=tuple(ordered_parent_trees),
        signature=signature,
    )


def _write_private_policy_snapshot(
    directory: pathlib.Path, name: str, payload: bytes
) -> pathlib.Path:
    path = directory / name
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(path, flags, 0o600)
    try:
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, payload[offset:])
            if written <= 0:
                raise SourceSealError("could not snapshot the SSH trust policy")
            offset += written
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    return path


def _verify_signed_commit(root: pathlib.Path, commit: bytes) -> SourceAuthority:
    try:
        commit_text = commit.decode("ascii")
    except UnicodeDecodeError as exc:
        raise SourceSealError("source commit is not canonical ASCII") from exc
    raw_commit = _git(root, "cat-file", "commit", commit_text)
    _require_one_ssh_signature(raw_commit)
    if not SSH_KEYGEN.is_file() or SSH_KEYGEN.is_symlink():
        raise SourceSealError("pinned /usr/bin/ssh-keygen is unavailable")
    policy = _load_signature_policy()
    environment = _git_environment()
    try:
        with tempfile.TemporaryDirectory(prefix="iroha-kagemusha-signature-") as temporary:
            directory = pathlib.Path(temporary)
            allowed = _write_private_policy_snapshot(
                directory, "allowed-signers", policy.allowed_signers
            )
            revocation = _write_private_policy_snapshot(
                directory, "revocation", policy.revocation
            )
            signature_config = [
                "-c",
                "gpg.format=ssh",
                "-c",
                "gpg.minTrustLevel=fully",
                "-c",
                f"gpg.ssh.program={SSH_KEYGEN}",
                "-c",
                f"gpg.ssh.allowedSignersFile={allowed}",
                "-c",
                f"gpg.ssh.revocationFile={revocation}",
                "-c",
                f"gpg.program={SSH_KEYGEN}",
                "-c",
                f"gpg.openpgp.program={SSH_KEYGEN}",
                "-c",
                f"gpg.x509.program={SSH_KEYGEN}",
            ]
            completed = subprocess.run(
                [
                    os.fspath(GIT),
                    *GIT_ARGUMENT_PREFIX,
                    "-C",
                    os.fspath(root),
                    *signature_config,
                    "verify-commit",
                    "--raw",
                    commit_text,
                ],
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                env=environment,
            )
    except OSError as exc:
        raise SourceSealError("could not verify the source commit signature") from exc
    if completed.returncode != 0:
        raise SourceSealError(
            "source commit must carry a locally verifiable signature"
        )
    raw_commit_after = _git(root, "cat-file", "commit", commit_text)
    if raw_commit_after != raw_commit:
        raise SourceSealError("source commit object changed while its signature was verified")
    signer = policy.signer
    if signer is None:
        principal, public_key_sha256 = _validate_allowed_signers(
            policy.allowed_signers
        )
        signer = VerifiedSshSignature(
            principal=principal,
            public_key_sha256=public_key_sha256,
            allowed_signers_sha256=hashlib.sha256(policy.allowed_signers).hexdigest(),
            revocation_sha256=hashlib.sha256(policy.revocation).hexdigest(),
        )
    return _source_authority_from_verified_commit(
        root,
        commit_text,
        raw_commit,
        signer,
    )


def _repository_root(root: pathlib.Path) -> pathlib.Path:
    requested = root.resolve(strict=True)
    discovered = pathlib.Path(
        os.fsdecode(_git(requested, "rev-parse", "--show-toplevel")).strip()
    ).resolve(strict=True)
    if discovered != requested:
        raise SourceSealError(
            f"--root must be the exact repository root ({discovered})"
        )
    return requested


def _head(root: pathlib.Path) -> bytes:
    value = _git(root, "rev-parse", "--verify", "HEAD^{commit}").strip()
    if (
        len(value) != 40
        or value == b"0" * 40
        or any(byte not in b"0123456789abcdef" for byte in value)
    ):
        raise SourceSealError("Git HEAD is not one nonzero canonical SHA-1 commit id")
    return value


def status(root: pathlib.Path) -> bytes:
    return _git(
        root,
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    )


def _safe_relative_path(path: bytes, *, allow_cargo_lock: bool = False) -> None:
    if (
        not path
        or path.startswith(b"/")
        or path.endswith(b"/")
        or b"\0" in path
        or any(component in (b"", b".", b"..") for component in path.split(b"/"))
        or path.split(b"/", 1)[0] == b".git"
        or (not allow_cargo_lock and path == REQUIRED_TRACKED_BUILD_INPUT)
    ):
        raise SourceSealError("Git returned an unsafe source path")


def _index_entries(root: pathlib.Path) -> list[IndexEntry]:
    records = _git(root, "ls-files", "--stage", "-z", "--").split(b"\0")
    entries: list[IndexEntry] = []
    seen: set[bytes] = set()
    for record in records:
        if not record:
            continue
        try:
            metadata, path = record.split(b"\t", 1)
            mode, object_id, stage = metadata.split(b" ", 2)
        except ValueError as exc:
            raise SourceSealError("Git returned a malformed index record") from exc
        if mode not in ALLOWED_INDEX_MODES:
            raise SourceSealError(
                f"unsupported Git index mode {os.fsdecode(mode)!r} for {os.fsdecode(path)!r}"
            )
        if len(object_id) != 40 or any(
            byte not in b"0123456789abcdef" for byte in object_id
        ):
            raise SourceSealError("Git returned a non-canonical index object id")
        if stage != b"0":
            raise SourceSealError("the source index contains an unresolved merge stage")
        _safe_relative_path(path, allow_cargo_lock=True)
        if path in seen:
            raise SourceSealError("Git returned a duplicate source path")
        seen.add(path)
        entries.append(IndexEntry(mode=mode, object_id=object_id, path=path))
    if not entries:
        raise SourceSealError("the source index is empty")
    entries.sort(key=lambda entry: entry.path)
    return entries


def _field(hasher: "hashlib._Hash", value: bytes) -> None:
    hasher.update(len(value).to_bytes(8, "big"))
    hasher.update(value)


def _stable_identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _stable_directory_identity(metadata: os.stat_result) -> tuple[int, ...]:
    """Return path identity fields unaffected by unrelated directory entries."""

    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_uid,
        metadata.st_gid,
    )


def _git_mode(metadata: os.stat_result) -> bytes:
    if stat.S_ISREG(metadata.st_mode):
        return b"100755" if metadata.st_mode & 0o111 else b"100644"
    if stat.S_ISLNK(metadata.st_mode):
        return b"120000"
    if stat.S_ISDIR(metadata.st_mode):
        return b"160000"
    return b"unsupported"


def _open_root_directory(root: pathlib.Path) -> tuple[bytes, int, tuple[int, ...]]:
    root_bytes = os.fsencode(root)
    before = os.lstat(root_bytes)
    if not stat.S_ISDIR(before.st_mode) or stat.S_ISLNK(before.st_mode):
        raise SourceSealError("repository root must be one real directory")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(root_bytes, flags)
    opened = os.fstat(descriptor)
    if _stable_directory_identity(before) != _stable_directory_identity(opened):
        os.close(descriptor)
        raise SourceSealError("repository root changed while opened")
    return root_bytes, descriptor, _stable_directory_identity(opened)


def _verify_root_directory(
    root_bytes: bytes, descriptor: int, expected: tuple[int, ...]
) -> None:
    if (
        _stable_directory_identity(os.fstat(descriptor)) != expected
        or _stable_directory_identity(os.lstat(root_bytes)) != expected
    ):
        raise SourceSealError("repository root changed while sealing")


def _close_directory_chain(chain: list[OpenedDirectory]) -> None:
    for opened in reversed(chain):
        os.close(opened.descriptor)


def _open_parent_directory(
    root_descriptor: int, path: bytes
) -> tuple[int, bytes, list[OpenedDirectory]]:
    """Resolve a source parent with descriptor-relative, no-symlink traversal."""

    _safe_relative_path(path, allow_cargo_lock=True)
    components = path.split(b"/")
    current = root_descriptor
    chain: list[OpenedDirectory] = []
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        for component in components[:-1]:
            before = os.stat(component, dir_fd=current, follow_symlinks=False)
            if not stat.S_ISDIR(before.st_mode) or stat.S_ISLNK(before.st_mode):
                raise SourceSealError(
                    f"source path has a non-directory or symlink ancestor: {os.fsdecode(path)}"
                )
            child = os.open(component, flags, dir_fd=current)
            opened = os.fstat(child)
            if _stable_directory_identity(before) != _stable_directory_identity(opened):
                os.close(child)
                raise SourceSealError(
                    f"source ancestor changed while opened: {os.fsdecode(path)}"
                )
            chain.append(
                OpenedDirectory(
                    parent_descriptor=current,
                    name=component,
                    descriptor=child,
                    identity=_stable_directory_identity(opened),
                )
            )
            current = child
    except SourceSealError:
        _close_directory_chain(chain)
        raise
    except OSError as exc:
        _close_directory_chain(chain)
        raise SourceSealError(
            f"source path ancestor is missing or unsafe: {os.fsdecode(path)}"
        ) from exc
    return current, components[-1], chain


def _verify_directory_chain(chain: list[OpenedDirectory], path: bytes) -> None:
    try:
        for opened in chain:
            bound = os.stat(
                opened.name,
                dir_fd=opened.parent_descriptor,
                follow_symlinks=False,
            )
            if (
                _stable_directory_identity(bound) != opened.identity
                or _stable_directory_identity(os.fstat(opened.descriptor))
                != opened.identity
            ):
                raise SourceSealError(
                    f"source ancestor changed while read: {os.fsdecode(path)}"
                )
    except OSError as exc:
        raise SourceSealError(
            f"source ancestor changed while read: {os.fsdecode(path)}"
        ) from exc
    finally:
        _close_directory_chain(chain)


def _hash_regular_file_at(
    root_descriptor: int,
    path: bytes,
    source_hasher: "hashlib._Hash",
    *,
    maximum_bytes: int | None = None,
    require_nonempty: bool = False,
    expected_git_mode: bytes | None = None,
) -> tuple[int, str, str]:
    parent, name, chain = _open_parent_directory(root_descriptor, path)
    descriptor: int | None = None
    try:
        before = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            raise SourceSealError(
                f"source must be a singly linked regular file: {os.fsdecode(path)}"
            )
        if expected_git_mode is not None and _git_mode(before) != expected_git_mode:
            raise SourceSealError(
                f"source mode differs from the signed index: {os.fsdecode(path)}"
            )
        if (
            (require_nonempty and before.st_size <= 0)
            or (maximum_bytes is not None and before.st_size > maximum_bytes)
        ):
            raise SourceSealError(f"source file has an invalid size: {os.fsdecode(path)}")
        flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        descriptor = os.open(name, flags, dir_fd=parent)
        opened_before = os.fstat(descriptor)
        source_hasher.update(opened_before.st_size.to_bytes(8, "big"))
        sha256 = hashlib.sha256()
        blob_oid = hashlib.sha1(
            b"blob " + str(opened_before.st_size).encode("ascii") + b"\0",
            usedforsecurity=False,
        )
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            source_hasher.update(chunk)
            sha256.update(chunk)
            blob_oid.update(chunk)
        opened_after = os.fstat(descriptor)
        after = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not (
            _stable_identity(before)
            == _stable_identity(opened_before)
            == _stable_identity(opened_after)
            == _stable_identity(after)
        ):
            raise SourceSealError(f"source changed while read: {os.fsdecode(path)}")
        if total != opened_after.st_size:
            raise SourceSealError(f"source was truncated while read: {os.fsdecode(path)}")
        return total, sha256.hexdigest(), blob_oid.hexdigest()
    except OSError as exc:
        raise SourceSealError(
            f"source is missing or unsafe: {os.fsdecode(path)}"
        ) from exc
    finally:
        if descriptor is not None:
            os.close(descriptor)
        _verify_directory_chain(chain, path)


def _stable_symlink_bytes_at(root_descriptor: int, path: bytes) -> bytes:
    parent, name, chain = _open_parent_directory(root_descriptor, path)
    try:
        before = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not stat.S_ISLNK(before.st_mode) or before.st_nlink != 1:
            raise SourceSealError(
                f"tracked source differs from signed symlink mode: {os.fsdecode(path)}"
            )
        payload = os.readlink(name, dir_fd=parent)
        after = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not isinstance(payload, bytes):
            payload = os.fsencode(payload)
        if _stable_identity(before) != _stable_identity(after):
            raise SourceSealError(
                f"tracked symlink changed while read: {os.fsdecode(path)}"
            )
        return payload
    except OSError as exc:
        raise SourceSealError(
            f"tracked symlink is missing or unsafe: {os.fsdecode(path)}"
        ) from exc
    finally:
        _verify_directory_chain(chain, path)


def _require_empty_gitlink_directory_at(root_descriptor: int, path: bytes) -> None:
    parent, name, chain = _open_parent_directory(root_descriptor, path)
    descriptor: int | None = None
    try:
        observed = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not stat.S_ISDIR(observed.st_mode) or stat.S_ISLNK(observed.st_mode):
            raise SourceSealError(
                f"tracked gitlink must be a present empty directory: {os.fsdecode(path)}"
            )
        flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        descriptor = os.open(name, flags, dir_fd=parent)
        opened_before = os.fstat(descriptor)
        if _stable_identity(observed) != _stable_identity(opened_before):
            raise SourceSealError(
                f"tracked gitlink changed while inspected: {os.fsdecode(path)}"
            )
        if os.listdir(descriptor):
            raise SourceSealError(
                f"tracked gitlink directory must be empty: {os.fsdecode(path)}"
            )
        opened_after = os.fstat(descriptor)
        after = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not (
            _stable_identity(observed)
            == _stable_identity(opened_before)
            == _stable_identity(opened_after)
            == _stable_identity(after)
        ):
            raise SourceSealError(
                f"tracked gitlink changed while inspected: {os.fsdecode(path)}"
            )
    except OSError as exc:
        raise SourceSealError(
            f"tracked gitlink is missing or unsafe: {os.fsdecode(path)}"
        ) from exc
    finally:
        if descriptor is not None:
            os.close(descriptor)
        _verify_directory_chain(chain, path)


def _untracked_paths(root: pathlib.Path) -> list[bytes]:
    records = _git(
        root,
        "ls-files",
        "--others",
        "--exclude-standard",
        "-z",
        "--",
    ).split(b"\0")
    paths = [path for path in records if path]
    if len(paths) > MAX_UNTRACKED_FILES:
        raise SourceSealError("untracked source inventory exceeds its file-count bound")
    for path in paths:
        _safe_relative_path(path)
    if paths != sorted(set(paths)):
        raise SourceSealError("untracked source paths are not unique and raw-byte sorted")
    return paths


def _ignored_paths(root: pathlib.Path) -> list[bytes]:
    records = _git(
        root,
        "ls-files",
        "--others",
        "--ignored",
        "--exclude-standard",
        "-z",
        "--",
    ).split(b"\0")
    paths = sorted({path for path in records if path})
    for path in paths:
        _safe_relative_path(path, allow_cargo_lock=True)
    return paths


def _canonical_json_bytes(value: Any) -> bytes:
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
    except (TypeError, ValueError, UnicodeError) as exc:
        raise SourceSealError("reviewed source closure is not canonical JSON") from exc


def _untracked_manifest_bytes(entries: list[dict[str, Any]]) -> bytes:
    return b"".join(_canonical_json_bytes(entry) for entry in entries)


def _capture_observed_descriptor(root: pathlib.Path) -> dict[str, Any]:
    root = _repository_root(root)
    root_bytes, root_descriptor, root_identity = _open_root_directory(root)
    try:
        head_before = _head(root)
        _verify_signed_commit(root, head_before)
        diff_before = _git(root, *TRACKED_DIFF_ARGUMENTS)
        untracked_before = _untracked_paths(root)
        if diff_before or untracked_before:
            raise SourceSealError(
                "first-release Kagemusha source index must equal HEAD (empty tracked diff) "
                "and have no untracked files"
            )
        ignored_before = _ignored_paths(root)
        if ignored_before:
            raise SourceSealError("ignored source set must be empty")
        entries = _index_entries(root)
        cargo_lock_entries = [
            entry for entry in entries if entry.path == REQUIRED_TRACKED_BUILD_INPUT
        ]
        if len(cargo_lock_entries) != 1 or cargo_lock_entries[0].mode != b"100644":
            raise SourceSealError(
                "source index must contain exactly one stage-0 tracked mode 100644 root Cargo.lock"
            )
        cargo_lock_entry = cargo_lock_entries[0]
        source_hasher = hashlib.sha256(SOURCE_TREE_DOMAIN)

        for entry in entries:
            _field(source_hasher, b"tracked-source-v1")
            _field(source_hasher, entry.path)
            if entry.mode == b"160000":
                _field(source_hasher, b"gitlink-index-v1")
                _field(source_hasher, entry.object_id)
                _require_empty_gitlink_directory_at(root_descriptor, entry.path)
            elif entry.mode in (b"100644", b"100755"):
                _field(source_hasher, entry.mode)
                _, _, observed_blob_oid = _hash_regular_file_at(
                    root_descriptor,
                    entry.path,
                    source_hasher,
                    expected_git_mode=entry.mode,
                )
                if observed_blob_oid.encode("ascii") != entry.object_id:
                    raise SourceSealError(
                        "tracked source blob differs from the signed index: "
                        f"{os.fsdecode(entry.path)}"
                    )
            elif entry.mode == b"120000":
                payload = _stable_symlink_bytes_at(root_descriptor, entry.path)
                blob_oid = hashlib.sha1(
                    b"blob " + str(len(payload)).encode("ascii") + b"\0" + payload,
                    usedforsecurity=False,
                ).hexdigest()
                if blob_oid.encode("ascii") != entry.object_id:
                    raise SourceSealError(
                        "tracked symlink blob differs from the signed index: "
                        f"{os.fsdecode(entry.path)}"
                    )
                _field(source_hasher, entry.mode)
                _field(source_hasher, payload)
            else:  # Kept exhaustive even if the index parser changes later.
                raise SourceSealError(
                    f"unsupported tracked source mode: {os.fsdecode(entry.path)}"
                )

        untracked_manifest: list[dict[str, Any]] = []
        for path in untracked_before:
            # MAX_UNTRACKED_FILES is zero.  Retain a fail-closed guard if that
            # first-release policy is ever accidentally relaxed in one place.
            raise SourceSealError(
                f"untracked source is forbidden: {os.fsdecode(path)}"
            )

        # Preserve the V1 domain and descriptor field names for existing
        # JSON/Norito consumers even though Cargo.lock is now an ordinary
        # tracked source entry as well as this redundant separate binding.
        _field(source_hasher, b"required-ignored-build-input-v1")
        _field(source_hasher, REQUIRED_TRACKED_BUILD_INPUT)
        _field(source_hasher, b"100644")
        cargo_lock_size, cargo_lock_sha256, cargo_lock_blob_oid = _hash_regular_file_at(
            root_descriptor,
            REQUIRED_TRACKED_BUILD_INPUT,
            source_hasher,
            maximum_bytes=MAX_CARGO_LOCK_BYTES,
            require_nonempty=True,
            expected_git_mode=b"100644",
        )
        if cargo_lock_blob_oid.encode("ascii") != cargo_lock_entry.object_id:
            raise SourceSealError(
                "tracked root Cargo.lock blob differs from the signed index"
            )

        head_after = _head(root)
        diff_after = _git(root, *TRACKED_DIFF_ARGUMENTS)
        untracked_after = _untracked_paths(root)
        ignored_after = _ignored_paths(root)
        cargo_recheck_size, cargo_recheck_sha256, cargo_recheck_blob_oid = (
            _hash_regular_file_at(
                root_descriptor,
                REQUIRED_TRACKED_BUILD_INPUT,
                hashlib.sha256(),
                maximum_bytes=MAX_CARGO_LOCK_BYTES,
                require_nonempty=True,
                expected_git_mode=b"100644",
            )
        )
        _verify_root_directory(root_bytes, root_descriptor, root_identity)
        if (
            head_after != head_before
            or diff_after != diff_before
            or untracked_after != untracked_before
            or ignored_after != ignored_before
            or cargo_recheck_size != cargo_lock_size
            or cargo_recheck_sha256 != cargo_lock_sha256
            or cargo_recheck_blob_oid != cargo_lock_blob_oid
        ):
            raise SourceSealError("Kagemusha source HEAD or closure changed while sealing")
    finally:
        os.close(root_descriptor)

    tracked_binary_diff_sha256 = hashlib.sha256(diff_before).hexdigest()
    untracked_manifest_sha256 = hashlib.sha256(
        _untracked_manifest_bytes(untracked_manifest)
    ).hexdigest()
    combined = hashlib.sha256()
    combined.update(SOURCE_DIFF_DOMAIN)
    combined.update(TRACKED_DIFF_DOMAIN)
    combined.update(bytes.fromhex(tracked_binary_diff_sha256))
    combined.update(UNTRACKED_MANIFEST_DOMAIN)
    combined.update(bytes.fromhex(untracked_manifest_sha256))
    source_repo_dirty = False
    descriptor = {
        "base_commit": head_before.decode("ascii"),
        "combined_source_fingerprint_sha256": combined.hexdigest(),
        "ignored_cargo_lock_sha256": cargo_lock_sha256,
        "ignored_cargo_lock_size_bytes": cargo_lock_size,
        "schema": REVIEWED_SOURCE_CLOSURE_SCHEMA,
        "source_commit": head_before.decode("ascii"),
        "source_repo_dirty": source_repo_dirty,
        "source_tree_sha256": source_hasher.hexdigest(),
        "tracked_binary_diff_sha256": tracked_binary_diff_sha256,
        "untracked_file_count": len(untracked_manifest),
        "untracked_path_mode_blob_oid_manifest": untracked_manifest,
        "untracked_path_mode_blob_oid_manifest_sha256": untracked_manifest_sha256,
    }
    if len(_canonical_json_bytes(descriptor)) > MAX_DESCRIPTOR_BYTES:
        raise SourceSealError("reviewed source closure descriptor exceeds its size bound")
    return descriptor


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise SourceSealError(f"duplicate JSON member: {key}")
        value[key] = item
    return value


def _reject_constant(value: str) -> None:
    raise SourceSealError(f"non-finite JSON number is forbidden: {value}")


def _require_digest(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or re.fullmatch(r"[0-9a-f]{64}", value) is None
        or value == "0" * 64
    ):
        raise SourceSealError(f"{label} must be one nonzero lowercase SHA-256")
    return value


def _require_commit(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or re.fullmatch(r"[0-9a-f]{40}", value) is None
        or value == "0" * 40
    ):
        raise SourceSealError(f"{label} must be one nonzero lowercase SHA-1 commit")
    return value


def _decode_manifest_path(entry: dict[str, Any], label: str) -> bytes:
    display_path = entry["path"]
    encoded_path = entry["path_bytes_base64"]
    if (
        not isinstance(display_path, str)
        or not display_path
        or not isinstance(encoded_path, str)
        or not encoded_path
    ):
        raise SourceSealError(f"{label} path fields must be nonempty strings")
    try:
        path_bytes = base64.b64decode(encoded_path, validate=True)
    except (ValueError, base64.binascii.Error) as exc:
        raise SourceSealError(f"{label} path bytes are not canonical Base64") from exc
    _safe_relative_path(path_bytes)
    if (
        base64.b64encode(path_bytes).decode("ascii") != encoded_path
        or os.fsdecode(path_bytes) != display_path
    ):
        raise SourceSealError(f"{label} path display/base64 binding is not exact")
    return path_bytes


def _validate_descriptor(value: Any, required_commit: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != DESCRIPTOR_KEYS:
        raise SourceSealError("reviewed source closure keys are not exact")
    if value["schema"] != REVIEWED_SOURCE_CLOSURE_SCHEMA:
        raise SourceSealError("reviewed source closure schema is not exact")
    required_commit = _require_commit(required_commit, "required source commit")
    base_commit = _require_commit(value["base_commit"], "base_commit")
    source_commit = _require_commit(value["source_commit"], "source_commit")
    if base_commit != required_commit or source_commit != required_commit:
        raise SourceSealError(
            "reviewed source closure does not use the exact pinned signed base commit"
        )
    for field in (
        "source_tree_sha256",
        "tracked_binary_diff_sha256",
        "untracked_path_mode_blob_oid_manifest_sha256",
        "ignored_cargo_lock_sha256",
        "combined_source_fingerprint_sha256",
    ):
        _require_digest(value[field], field)
    file_count = value["untracked_file_count"]
    cargo_lock_size = value["ignored_cargo_lock_size_bytes"]
    if (
        type(file_count) is not int
        or file_count < 0
        or file_count > MAX_UNTRACKED_FILES
    ):
        raise SourceSealError("untracked_file_count is not one bounded JSON integer")
    if (
        type(cargo_lock_size) is not int
        or cargo_lock_size <= 0
        or cargo_lock_size > MAX_CARGO_LOCK_BYTES
    ):
        raise SourceSealError(
            "ignored_cargo_lock_size_bytes is not one bounded positive JSON integer"
        )
    raw_manifest = value["untracked_path_mode_blob_oid_manifest"]
    if not isinstance(raw_manifest, list) or len(raw_manifest) != file_count:
        raise SourceSealError("untracked manifest count is not exact")
    paths: list[bytes] = []
    for index, raw_entry in enumerate(raw_manifest):
        label = f"untracked_path_mode_blob_oid_manifest[{index}]"
        if not isinstance(raw_entry, dict) or set(raw_entry) != MANIFEST_ENTRY_KEYS:
            raise SourceSealError(f"{label} keys are not exact")
        _require_digest(raw_entry["blob_sha256"], f"{label}.blob_sha256")
        if (
            not isinstance(raw_entry["git_blob_oid"], str)
            or re.fullmatch(r"[0-9a-f]{40}", raw_entry["git_blob_oid"]) is None
        ):
            raise SourceSealError(f"{label}.git_blob_oid is not lowercase SHA-1")
        if raw_entry["git_mode"] not in ALLOWED_UNTRACKED_MODES:
            raise SourceSealError(f"{label}.git_mode is not canonical")
        paths.append(_decode_manifest_path(raw_entry, label))
    if paths != sorted(set(paths)):
        raise SourceSealError(
            "untracked manifest paths are not unique and raw-byte sorted"
        )
    manifest_sha256 = hashlib.sha256(
        _untracked_manifest_bytes(raw_manifest)
    ).hexdigest()
    if manifest_sha256 != value["untracked_path_mode_blob_oid_manifest_sha256"]:
        raise SourceSealError("untracked manifest SHA-256 is not self-consistent")
    combined = hashlib.sha256()
    combined.update(SOURCE_DIFF_DOMAIN)
    combined.update(TRACKED_DIFF_DOMAIN)
    combined.update(bytes.fromhex(value["tracked_binary_diff_sha256"]))
    combined.update(UNTRACKED_MANIFEST_DOMAIN)
    combined.update(bytes.fromhex(manifest_sha256))
    if combined.hexdigest() != value["combined_source_fingerprint_sha256"]:
        raise SourceSealError("combined source fingerprint is not self-consistent")
    derived_dirty = (
        value["tracked_binary_diff_sha256"] != EMPTY_SHA256 or file_count != 0
    )
    if value["source_repo_dirty"] is not derived_dirty:
        raise SourceSealError("source_repo_dirty does not equal the derived closure state")
    if value["source_repo_dirty"] is not False:
        raise SourceSealError("source_repo_dirty must be false for a clean source closure")
    if derived_dirty:
        raise SourceSealError(
            "reviewed Kagemusha source closure must have an empty tracked diff and no "
            "untracked files"
        )
    return value


def _read_descriptor_payload(path: str) -> bytes:
    selected = pathlib.Path(path)
    return _read_bounded_absolute_file(
        selected,
        "reviewed source closure",
        MAX_DESCRIPTOR_BYTES,
        allow_empty=False,
        owner_controlled=True,
    )


def _load_descriptor(
    path: str,
    expected_sha256: str,
    *,
    required_commit: str,
) -> tuple[dict[str, Any], str]:
    expected_sha256 = _require_digest(
        expected_sha256, "reviewed source closure descriptor SHA-256"
    )
    payload = _read_descriptor_payload(path)
    observed_sha256 = hashlib.sha256(payload).hexdigest()
    if observed_sha256 != expected_sha256:
        raise SourceSealError("reviewed source closure descriptor digest differs from its pin")
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_reject_duplicates,
            parse_constant=_reject_constant,
        )
    except (json.JSONDecodeError, UnicodeError, SourceSealError) as exc:
        raise SourceSealError("reviewed source closure is not strict JSON") from exc
    if _canonical_json_bytes(value) != payload:
        raise SourceSealError("reviewed source closure bytes are not canonical")
    return _validate_descriptor(value, required_commit), observed_sha256


def compute_observed_descriptor(root: pathlib.Path) -> dict[str, Any]:
    """Capture the canonical descriptor that must be reviewed independently."""

    return _capture_observed_descriptor(root)


def compute_identity(
    root: pathlib.Path,
    reviewed_source_closure: str,
    reviewed_source_closure_sha256: str,
) -> SourceIdentity:
    root = _repository_root(root)
    required_commit = _head(root).decode("ascii")
    descriptor, descriptor_sha256 = _load_descriptor(
        reviewed_source_closure,
        reviewed_source_closure_sha256,
        required_commit=required_commit,
    )
    observed = _capture_observed_descriptor(root)
    if observed != descriptor:
        raise SourceSealError(
            "current source closure differs from the independently pinned descriptor"
        )
    source_authority = _verify_signed_commit(root, required_commit.encode("ascii"))
    if (
        source_authority.commit != required_commit
        or _head(root).decode("ascii") != required_commit
    ):
        raise SourceSealError(
            "source HEAD changed while its authenticated authority was derived"
        )
    return SourceIdentity(
        source_commit=descriptor["source_commit"],
        source_tree_sha256=descriptor["source_tree_sha256"],
        source_repo_dirty=descriptor["source_repo_dirty"],
        reviewed_source_closure=descriptor,
        reviewed_source_closure_descriptor_sha256=descriptor_sha256,
        source_authority=source_authority,
    )


def compute_fingerprint(
    root: pathlib.Path,
    reviewed_source_closure: str,
    reviewed_source_closure_sha256: str,
) -> str:
    return compute_identity(
        root,
        reviewed_source_closure,
        reviewed_source_closure_sha256,
    ).source_tree_sha256


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "mode",
        choices=("descriptor", "fingerprint", "identity", "status", "paths"),
    )
    parser.add_argument("--root", type=pathlib.Path, required=True)
    parser.add_argument("--reviewed-source-closure")
    parser.add_argument("--reviewed-source-closure-sha256")
    return parser.parse_args()


def _require_review_pin(args: argparse.Namespace) -> tuple[str, str]:
    if not args.reviewed_source_closure or not args.reviewed_source_closure_sha256:
        raise SourceSealError(
            "identity/fingerprint require --reviewed-source-closure and "
            "--reviewed-source-closure-sha256"
        )
    return args.reviewed_source_closure, args.reviewed_source_closure_sha256


def main() -> int:
    args = parse_args()
    root = _repository_root(args.root)
    if args.mode == "descriptor":
        if args.reviewed_source_closure or args.reviewed_source_closure_sha256:
            raise SourceSealError("descriptor observation does not accept a review pin")
        sys.stdout.buffer.write(_canonical_json_bytes(compute_observed_descriptor(root)))
    elif args.mode == "fingerprint":
        path, sha256 = _require_review_pin(args)
        print(compute_fingerprint(root, path, sha256))
    elif args.mode == "identity":
        path, sha256 = _require_review_pin(args)
        identity = compute_identity(root, path, sha256)
        sys.stdout.buffer.write(
            _canonical_json_bytes(
                {
                    "reviewed_source_closure": identity.reviewed_source_closure,
                    "reviewed_source_closure_descriptor_sha256": (
                        identity.reviewed_source_closure_descriptor_sha256
                    ),
                    "schema": SOURCE_IDENTITY_SCHEMA,
                    "source_commit": identity.source_commit,
                    "source_repo_dirty": identity.source_repo_dirty,
                    "source_tree_sha256": identity.source_tree_sha256,
                }
            )
        )
    elif args.mode == "status":
        if args.reviewed_source_closure or args.reviewed_source_closure_sha256:
            raise SourceSealError("status does not accept a review pin")
        value = status(root)
        if value:
            sys.stdout.buffer.write(value)
    else:
        if args.reviewed_source_closure or args.reviewed_source_closure_sha256:
            raise SourceSealError("paths does not accept a review pin")
        for entry in _index_entries(root):
            sys.stdout.buffer.write(entry.path + b"\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, SourceSealError) as exc:
        print(f"Kagemusha source-tree seal failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
