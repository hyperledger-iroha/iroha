#!/usr/bin/env python3
"""Produce clean-tree source evidence for AtomicPrivateSettlementV1 releases.

The producer is deliberately narrower than the final release-evidence
validator.  It accepts only one clean committed Git checkout, freezes the
recursive tree inventory, creates the repository's deterministic source seal,
and emits the source artifacts and reports consumed by
``private_settlement_release_evidence.py``.  Output is staged outside the
checkout and published by one directory rename, so a failed capture cannot be
mistaken for passing evidence.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from types import ModuleType
from typing import Any, Iterable, Mapping, Sequence


PROTOCOL = "AtomicPrivateSettlementV1"
REPORT_VERSION = 1
_MAX_INVENTORY_ENTRIES = 1_000_000
_MAX_INVENTORY_PATH_BYTES = 4096
_MAX_LS_TREE_BYTES = 512 * 1024 * 1024
_MAX_COMMIT_BYTES = 16 * 1024 * 1024
_MAX_TRANSCRIPT_BYTES = 16 * 1024 * 1024
_MAX_LOCKFILE_BYTES = 64 * 1024 * 1024
_GIT_MODES = frozenset({"100644", "100755", "120000", "160000"})
_OBJECT_ID = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")


class SourceEvidenceError(RuntimeError):
    """The checkout or requested source-evidence output is invalid."""


def _load_source_manifest_tools() -> ModuleType:
    """Load the repository-owned source-seal implementation by exact path."""

    path = Path(__file__).with_name("compute_workspace_source_manifest.py")
    spec = importlib.util.spec_from_file_location(
        "_private_settlement_workspace_source_manifest", path
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load workspace source-manifest tools: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


_SOURCE_TOOLS = _load_source_manifest_tools()


def _utc_timestamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def _exclusive_write(path: Path, payload: bytes, *, maximum_bytes: int) -> None:
    if len(payload) > maximum_bytes:
        raise SourceEvidenceError(f"refusing oversized source evidence: {path.name}")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    succeeded = False
    try:
        with os.fdopen(descriptor, "wb") as stream:
            descriptor = -1
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        succeeded = True
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        if not succeeded:
            try:
                path.unlink()
            except FileNotFoundError:
                pass


def _bounded_git_stdout(
    repository_root: Path,
    arguments: Sequence[str],
    *,
    maximum_bytes: int,
) -> bytes:
    """Run one read-only Git query while bounding retained stdout."""

    command = _SOURCE_TOOLS._git_command(repository_root, *arguments)
    with tempfile.TemporaryFile() as error_stream:
        process = subprocess.Popen(
            command,
            cwd=repository_root,
            env=_SOURCE_TOOLS._git_read_only_environment(),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=error_stream,
        )
        assert process.stdout is not None
        chunks: list[bytes] = []
        size = 0
        oversized = False
        with process.stdout:
            while chunk := process.stdout.read(1024 * 1024):
                size += len(chunk)
                if size > maximum_bytes:
                    oversized = True
                    break
                chunks.append(chunk)
        return_code = process.wait()
        error_stream.seek(0)
        stderr = error_stream.read(1024 * 1024 + 1)
    if oversized:
        raise SourceEvidenceError(
            f"Git output exceeds the {maximum_bytes}-byte evidence bound"
        )
    if return_code != 0:
        detail = stderr[: 1024 * 1024].decode("utf-8", errors="replace").strip()
        raise SourceEvidenceError(
            f"read-only Git command failed ({shlex.join(command)}): {detail}"
        )
    return b"".join(chunks)


def _validate_inventory_path(raw_path: bytes) -> str:
    if (
        not raw_path
        or len(raw_path) > _MAX_INVENTORY_PATH_BYTES
        or raw_path.startswith(b"/")
        or b"\0" in raw_path
        or any(component in (b"", b".", b"..") for component in raw_path.split(b"/"))
        or raw_path.split(b"/", 1)[0] == b".git"
    ):
        raise SourceEvidenceError("Git tree inventory contains an unsafe path")
    try:
        path = raw_path.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise SourceEvidenceError(
            "Git tree inventory paths must be canonical UTF-8"
        ) from error
    if os.fsencode(path) != raw_path:
        raise SourceEvidenceError(
            "Git tree inventory path cannot be represented exactly by this host"
        )
    return path


def _parse_inventory(raw: bytes, *, oid_hex_chars: int) -> list[dict[str, str]]:
    """Parse canonical ``git ls-tree -rz`` output into verifier rows."""

    if not raw or not raw.endswith(b"\0"):
        raise SourceEvidenceError("Git tree inventory is empty or unterminated")
    records = raw.split(b"\0")[:-1]
    if not records or len(records) > _MAX_INVENTORY_ENTRIES:
        raise SourceEvidenceError("Git tree inventory count is outside its bound")
    entries: list[dict[str, str]] = []
    prior_path: bytes | None = None
    for index, record in enumerate(records):
        header, separator, raw_path = record.partition(b"\t")
        fields = header.split(b" ")
        if not separator or len(fields) != 3:
            raise SourceEvidenceError(f"malformed Git tree row at index {index}")
        try:
            mode, object_type, object_id = (
                field.decode("ascii", errors="strict") for field in fields
            )
        except UnicodeDecodeError as error:
            raise SourceEvidenceError(
                f"non-ASCII Git tree header at index {index}"
            ) from error
        if mode not in _GIT_MODES:
            raise SourceEvidenceError(f"unsupported Git mode {mode!r}")
        expected_type = "commit" if mode == "160000" else "blob"
        if object_type != expected_type:
            raise SourceEvidenceError(
                f"Git object type for {mode} must be {expected_type!r}"
            )
        if (
            len(object_id) != oid_hex_chars
            or re.fullmatch(r"[0-9a-f]+", object_id) is None
        ):
            raise SourceEvidenceError("Git tree row contains an invalid object ID")
        if prior_path is not None and raw_path <= prior_path:
            raise SourceEvidenceError(
                "Git tree inventory paths are duplicated or not raw-byte sorted"
            )
        path = _validate_inventory_path(raw_path)
        entries.append(
            {
                "path": path,
                "mode": mode,
                "object_type": object_type,
                "object_id": object_id,
            }
        )
        prior_path = raw_path
    if not any(entry["path"] == "Cargo.lock" for entry in entries):
        raise SourceEvidenceError("clean release tree does not contain Cargo.lock")
    return entries


def _tree_inventory(
    repository_root: Path, tree: str
) -> tuple[list[dict[str, str]], str, int]:
    raw = _bounded_git_stdout(
        repository_root,
        ("ls-tree", "-r", "-z", "--full-tree", tree),
        maximum_bytes=_MAX_LS_TREE_BYTES,
    )
    entries = _parse_inventory(raw, oid_hex_chars=len(tree))
    return entries, hashlib.sha256(raw).hexdigest(), len(raw)


def _git_object_id(kind: bytes, payload: bytes, oid_hex_chars: int) -> str:
    framed = kind + b" " + str(len(payload)).encode("ascii") + b"\0" + payload
    if oid_hex_chars == 40:
        return hashlib.sha1(framed).hexdigest()
    if oid_hex_chars == 64:
        return hashlib.sha256(framed).hexdigest()
    raise SourceEvidenceError("release checkout uses an unsupported Git object format")


def _raw_commit(repository_root: Path, commit: str, tree: str) -> bytes:
    payload = _bounded_git_stdout(
        repository_root,
        ("cat-file", "commit", commit),
        maximum_bytes=_MAX_COMMIT_BYTES,
    )
    if not payload or _git_object_id(b"commit", payload, len(commit)) != commit:
        raise SourceEvidenceError("raw Git commit body does not hash to release HEAD")
    header, separator, _ = payload.partition(b"\n\n")
    if not separator or header.splitlines()[0] != f"tree {tree}".encode("ascii"):
        raise SourceEvidenceError("raw Git commit body does not bind the release tree")
    return payload


def _copy_lockfile(source: Path, destination: Path, expected_sha256: str) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(destination, flags, 0o600)
    digest = hashlib.sha256()
    succeeded = False
    try:
        with _SOURCE_TOOLS._stable_regular_reader(
            source,
            maximum_size=_MAX_LOCKFILE_BYTES,
            label="release Cargo.lock",
        ) as (input_stream, _):
            with os.fdopen(descriptor, "wb") as output_stream:
                descriptor = -1
                while chunk := input_stream.read(1024 * 1024):
                    output_stream.write(chunk)
                    digest.update(chunk)
                output_stream.flush()
                os.fsync(output_stream.fileno())
        if digest.hexdigest() != expected_sha256:
            raise SourceEvidenceError("Cargo.lock changed after source identity capture")
        succeeded = True
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        if not succeeded:
            try:
                destination.unlink()
            except FileNotFoundError:
                pass


def _file_binding(path: Path, artifact_path: PurePosixPath) -> dict[str, str | int]:
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise SourceEvidenceError(f"artifact is not one regular file: {path}")
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
            size += len(chunk)
    after = path.lstat()
    if (
        (metadata.st_dev, metadata.st_ino, metadata.st_size, metadata.st_mtime_ns)
        != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
        or size != metadata.st_size
    ):
        raise SourceEvidenceError(f"artifact changed while hashed: {path}")
    return {"path": artifact_path.as_posix(), "sha256": digest.hexdigest(), "bytes": size}


def _artifact_path(directory: PurePosixPath, name: str) -> PurePosixPath:
    return directory / PurePosixPath(name)


def _relative_artifact_directory(value: str) -> PurePosixPath:
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in ("", ".", "..") for part in path.parts)
        or "\\" in value
    ):
        raise SourceEvidenceError(
            "artifact directory must be a normalized relative POSIX path"
        )
    return path


def _ensure_directory_chain(root: Path, parts: Iterable[str]) -> Path:
    current = root
    for component in parts:
        current = current / component
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            current.mkdir(mode=0o700)
            metadata = current.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise SourceEvidenceError(
                f"source-evidence output parent is not a real directory: {current}"
            )
    return current


def _same_identity(
    before: Mapping[str, str | int], after: Mapping[str, str | int]
) -> bool:
    return dict(before) == dict(after)


def produce_source_evidence(
    repository_root: Path,
    bundle_root: Path,
    artifact_directory: str = "evidence/source",
    *,
    command: str = "private_settlement_source_evidence.py",
) -> dict[str, Any]:
    """Produce one atomic source-evidence directory and artifact inventory."""

    started_at = _utc_timestamp()
    started = time.monotonic()
    repository_root = repository_root.resolve()
    bundle_metadata = bundle_root.lstat()
    if stat.S_ISLNK(bundle_metadata.st_mode) or not stat.S_ISDIR(bundle_metadata.st_mode):
        raise SourceEvidenceError("bundle root must be a real pre-existing directory")
    bundle_root = bundle_root.resolve()
    relative_directory = _relative_artifact_directory(artifact_directory)
    candidate_output = bundle_root.joinpath(*relative_directory.parts)
    resolved_candidate = candidate_output.resolve(strict=False)
    if resolved_candidate == repository_root or repository_root in resolved_candidate.parents:
        raise SourceEvidenceError("source evidence must be staged outside the checkout")
    if os.path.lexists(candidate_output):
        raise SourceEvidenceError("source-evidence artifact directory already exists")

    identity = _SOURCE_TOOLS.release_source_identity(repository_root)
    commit = str(identity["head_commit"])
    tree = str(identity["head_tree"])
    if (
        _OBJECT_ID.fullmatch(commit) is None
        or _OBJECT_ID.fullmatch(tree) is None
        or len(commit) != len(tree)
        or identity["index_tree"] != tree
    ):
        raise SourceEvidenceError("release source identity is internally inconsistent")
    entries, inventory_sha256, inventory_bytes = _tree_inventory(
        repository_root, tree
    )
    paths = [entry["path"] for entry in entries]
    commit_payload = _raw_commit(repository_root, commit, tree)

    output_parent = _ensure_directory_chain(
        bundle_root, relative_directory.parts[:-1]
    )
    staging = Path(
        tempfile.mkdtemp(prefix=".aps-source-evidence-", dir=output_parent)
    )
    published = False
    try:
        path_list_path = staging / "source-paths.bin"
        source_archive_path = staging / "source.seal"
        source_commit_path = staging / "source.commit"
        source_lockfile_path = staging / "Cargo.lock"
        source_transcript_path = staging / "source-capture.log"
        inventory_transcript_path = staging / "release-inventory.log"
        source_manifest_path = staging / "source_manifest.json"
        inventory_report_path = staging / "release_inventory_report.json"

        _SOURCE_TOOLS.write_source_path_list(path_list_path, paths)
        if _SOURCE_TOOLS.read_source_path_list(path_list_path) != paths:
            raise SourceEvidenceError("retained source path list changed after creation")
        workspace_manifest = str(identity["workspace_source_manifest_sha256"])
        if (
            _SOURCE_TOOLS.workspace_source_manifest_from_path_list(
                repository_root, path_list_path
            )
            != workspace_manifest
        ):
            raise SourceEvidenceError(
                "clean tree path list differs from the release source identity"
            )
        archive_sha256 = _SOURCE_TOOLS.create_source_seal(
            repository_root,
            path_list_path,
            source_archive_path,
            workspace_manifest,
        )
        _exclusive_write(
            source_commit_path, commit_payload, maximum_bytes=_MAX_COMMIT_BYTES
        )
        _copy_lockfile(
            repository_root / "Cargo.lock",
            source_lockfile_path,
            str(identity["cargo_lock_sha256"]),
        )

        repeated_entries, repeated_inventory_sha256, repeated_inventory_bytes = (
            _tree_inventory(repository_root, tree)
        )
        repeated_identity = _SOURCE_TOOLS.release_source_identity(repository_root)
        if (
            repeated_entries != entries
            or repeated_inventory_sha256 != inventory_sha256
            or repeated_inventory_bytes != inventory_bytes
            or not _same_identity(identity, repeated_identity)
            or _SOURCE_TOOLS.workspace_source_manifest_from_path_list(
                repository_root, path_list_path
            )
            != workspace_manifest
        ):
            raise SourceEvidenceError("release source changed during evidence capture")

        path_list_reference = _file_binding(
            path_list_path, _artifact_path(relative_directory, "source-paths.bin")
        )
        archive_reference = _file_binding(
            source_archive_path, _artifact_path(relative_directory, "source.seal")
        )
        if archive_reference["sha256"] != archive_sha256:
            raise SourceEvidenceError("source-seal checksum changed after creation")
        commit_reference = _file_binding(
            source_commit_path, _artifact_path(relative_directory, "source.commit")
        )
        lockfile_reference = _file_binding(
            source_lockfile_path, _artifact_path(relative_directory, "Cargo.lock")
        )

        duration_seconds = max(time.monotonic() - started, 0.000001)
        source_transcript = "\n".join(
            (
                "AtomicPrivateSettlementV1 production source capture",
                f"started_at_utc={started_at}",
                f"command={command}",
                "identity_command=compute_workspace_source_manifest.py --release-identity-json",
                "commit_command=git cat-file commit <head_commit>",
                f"head_commit={commit}",
                f"head_tree={tree}",
                f"index_tree={identity['index_tree']}",
                f"workspace_manifest_sha256={workspace_manifest}",
                f"cargo_lock_sha256={identity['cargo_lock_sha256']}",
                f"source_path_list_sha256={path_list_reference['sha256']}",
                f"source_path_list_bytes={path_list_reference['bytes']}",
                f"source_archive_sha256={archive_reference['sha256']}",
                f"source_archive_bytes={archive_reference['bytes']}",
                "post_capture_identity=unchanged",
                "post_capture_inventory=unchanged",
                f"duration_seconds={duration_seconds:.9f}",
                "exit_code=0",
                "passed=true",
                "",
            )
        ).encode("utf-8")
        _exclusive_write(
            source_transcript_path,
            source_transcript,
            maximum_bytes=_MAX_TRANSCRIPT_BYTES,
        )
        source_transcript_reference = _file_binding(
            source_transcript_path,
            _artifact_path(relative_directory, "source-capture.log"),
        )
        inventory_transcript = "\n".join(
            (
                "AtomicPrivateSettlementV1 canonical release inventory",
                f"started_at_utc={started_at}",
                f"command={command}",
                "inventory_command=git ls-tree -r -z --full-tree <head_tree>",
                f"head_commit={commit}",
                f"head_tree={tree}",
                f"tracked_file_count={len(entries)}",
                f"ls_tree_stdout_sha256={inventory_sha256}",
                f"ls_tree_stdout_bytes={inventory_bytes}",
                "inventory_schema=path,mode,object_type,object_id",
                "post_capture_identity=unchanged",
                "post_capture_inventory=unchanged",
                f"duration_seconds={duration_seconds:.9f}",
                "exit_code=0",
                "passed=true",
                "",
            )
        ).encode("utf-8")
        _exclusive_write(
            inventory_transcript_path,
            inventory_transcript,
            maximum_bytes=_MAX_TRANSCRIPT_BYTES,
        )
        inventory_transcript_reference = _file_binding(
            inventory_transcript_path,
            _artifact_path(relative_directory, "release-inventory.log"),
        )

        source_manifest = {
            "version": REPORT_VERSION,
            "protocol": PROTOCOL,
            "commit": commit,
            "tree": tree,
            "workspace_manifest_sha256": workspace_manifest,
            "worktree_clean": True,
            "tracked_file_count": len(entries),
            "modified": [],
            "untracked": [],
            "source_path_list": path_list_reference,
            "source_archive": archive_reference,
            "source_commit": commit_reference,
            "source_lockfile": lockfile_reference,
            "passed": True,
            "transcript": source_transcript_reference,
        }
        _exclusive_write(
            source_manifest_path,
            _canonical_json(source_manifest),
            maximum_bytes=_MAX_TRANSCRIPT_BYTES,
        )
        inventory_report = {
            "version": REPORT_VERSION,
            "protocol": PROTOCOL,
            "commit": commit,
            "gate": "release_inventory",
            "command": command,
            "exit_code": 0,
            "passed": True,
            "started_at_utc": started_at,
            "duration_seconds": duration_seconds,
            "details": {"tree": tree, "entries": entries},
            "transcript": inventory_transcript_reference,
        }
        _exclusive_write(
            inventory_report_path,
            _canonical_json(inventory_report),
            maximum_bytes=_MAX_LS_TREE_BYTES,
        )

        artifacts = [
            {"kind": "source_path_list", **path_list_reference},
            {"kind": "source_archive", **archive_reference},
            {"kind": "source_commit", **commit_reference},
            {"kind": "source_lockfile", **lockfile_reference},
            {"kind": "operator_log", **source_transcript_reference},
            {"kind": "operator_log", **inventory_transcript_reference},
            {
                "kind": "source_manifest",
                **_file_binding(
                    source_manifest_path,
                    _artifact_path(relative_directory, "source_manifest.json"),
                ),
            },
            {
                "kind": "release_inventory_report",
                **_file_binding(
                    inventory_report_path,
                    _artifact_path(
                        relative_directory, "release_inventory_report.json"
                    ),
                ),
            },
        ]
        summary = {
            "version": REPORT_VERSION,
            "protocol": PROTOCOL,
            "commit": commit,
            "tree": tree,
            "workspace_manifest_sha256": workspace_manifest,
            "artifact_directory": relative_directory.as_posix(),
            "artifacts": artifacts,
        }
        staging.rename(candidate_output)
        published = True
        return summary
    finally:
        if not published and os.path.lexists(staging):
            shutil.rmtree(staging)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repository-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="clean Git checkout to capture",
    )
    parser.add_argument(
        "--bundle-root",
        type=Path,
        required=True,
        help="pre-existing release-bundle root outside the checkout",
    )
    parser.add_argument(
        "--artifact-directory",
        default="evidence/source",
        help="new relative POSIX directory below the bundle root",
    )
    args = parser.parse_args(argv)
    command = shlex.join([sys.executable, *sys.argv])
    try:
        summary = produce_source_evidence(
            args.repository_root,
            args.bundle_root,
            args.artifact_directory,
            command=command,
        )
    except (
        SourceEvidenceError,
        _SOURCE_TOOLS.ActiveGitOperationError,
        _SOURCE_TOOLS.DirtyReleaseSourceError,
        _SOURCE_TOOLS.SourcePathListError,
        _SOURCE_TOOLS.SourceSealError,
        _SOURCE_TOOLS.UnmergedSourceError,
        OSError,
    ) as error:
        print(f"private settlement source evidence failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
