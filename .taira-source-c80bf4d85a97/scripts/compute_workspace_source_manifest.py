#!/usr/bin/env python3
"""Compute a deterministic SHA-256 manifest of checkout source files.

The manifest covers tracked and untracked, non-ignored paths reported by Git,
including deletions, symlink targets, and executable bits. Build artifacts and
other ignored files are deliberately excluded, except for the workspace
``Cargo.lock``: Cargo consumes that file even when repository policy keeps it
untracked, so a release manifest must bind its exact bytes. Unresolved index
entries are rejected because they do not identify one reproducible source
tree. For every enumerated file or symlink entry, the checkout manifest records
all permission bits, not only Git's executable bit. The separate source-seal
check verifies directory modes and rejects source symlinks that escape the
sealed root or enter the writable output. HEAD/tree remains the canonical Git
content-and-executable-mode identity.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import sys
from typing import Iterable


_DOMAIN = b"iroha-workspace-source-manifest-v2\0"
_WORKSPACE_LOCKFILE = "Cargo.lock"
_ACTIVE_GIT_OPERATION_PATHS = (
    ("merge", "MERGE_HEAD"),
    ("cherry-pick", "CHERRY_PICK_HEAD"),
    ("revert", "REVERT_HEAD"),
    ("mailbox apply", "AM_HEAD"),
    ("rebase-apply", "rebase-apply"),
    ("rebase-merge", "rebase-merge"),
    ("sequencer", "sequencer"),
    ("bisect", "BISECT_START"),
)


class UnmergedSourceError(RuntimeError):
    """The Git index contains unresolved source entries."""


class ActiveGitOperationError(RuntimeError):
    """The checkout is in a Git operation that can still replace its source."""


class DirtyReleaseSourceError(RuntimeError):
    """A production release source is not one clean committed Git tree."""


def _git_path(root: Path, name: str) -> Path:
    """Resolve one worktree-specific Git administrative path."""

    result = subprocess.run(
        ["git", "rev-parse", "--git-path", name],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    )
    raw = result.stdout.removesuffix(b"\n")
    if not raw:
        raise RuntimeError(f"git returned an empty administrative path for {name}")
    path = Path(os.fsdecode(raw))
    if not path.is_absolute():
        path = root / path
    # Normalize `..` without following the administrative marker itself. A
    # dangling marker symlink must still count as an active operation.
    return Path(os.path.abspath(path))


def _active_git_operations(root: Path) -> list[str]:
    active = []
    for label, name in _ACTIVE_GIT_OPERATION_PATHS:
        path = _git_path(root, name)
        if os.path.lexists(path):
            active.append(label)
    return active


def _reject_active_git_operations(root: Path) -> None:
    active = _active_git_operations(root)
    if active:
        raise ActiveGitOperationError(
            "workspace has an active Git operation: " + ", ".join(active)
        )


def _git_unmerged_paths(root: Path) -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "--unmerged", "-z"],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    )
    paths: set[str] = set()
    for entry in result.stdout.split(b"\0"):
        if not entry:
            continue
        _, separator, raw_path = entry.partition(b"\t")
        if not separator:
            raise RuntimeError("git returned a malformed unmerged index entry")
        paths.add(os.fsdecode(raw_path))
    return sorted(paths, key=os.fsencode)


def _git_source_paths(root: Path) -> list[str]:
    unmerged = _git_unmerged_paths(root)
    if unmerged:
        rendered = ", ".join(unmerged)
        raise UnmergedSourceError(
            f"workspace contains unresolved merge entries: {rendered}"
        )
    _reject_active_git_operations(root)
    result = subprocess.run(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    )
    paths = {
        os.fsdecode(raw)
        for raw in result.stdout.split(b"\0")
        if raw
    }
    # This workspace intentionally keeps Cargo.lock untracked. Cargo still
    # consumes it, and `--locked` validates it, so excluding it would allow a
    # build input to drift without invalidating release evidence.
    paths.add(_WORKSPACE_LOCKFILE)
    return sorted(paths, key=os.fsencode)


def _git_stdout(root: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", *arguments],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    return result.stdout.strip()


def _git_paths(root: Path, *arguments: str) -> list[str]:
    if not arguments:
        raise ValueError("a Git subcommand is required")
    command, *rest = arguments
    result = subprocess.run(
        ["git", command, "-z", *rest],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    )
    return [os.fsdecode(raw) for raw in result.stdout.split(b"\0") if raw]


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def release_source_identity(root: Path) -> dict[str, str | int]:
    """Return the exact clean committed source identity for a release run."""

    root = root.resolve()
    _reject_active_git_operations(root)
    unmerged = _git_unmerged_paths(root)
    if unmerged:
        raise UnmergedSourceError(
            "workspace contains unresolved merge entries: " + ", ".join(unmerged)
        )

    head_commit = _git_stdout(root, "rev-parse", "--verify", "HEAD^{commit}")
    head_tree = _git_stdout(root, "rev-parse", "--verify", "HEAD^{tree}")
    index_tree = _git_stdout(root, "write-tree")
    if index_tree != head_tree:
        staged = _git_paths(root, "diff", "--cached", "--name-only", "HEAD", "--")
        rendered = ", ".join(staged) if staged else "index tree differs from HEAD"
        raise DirtyReleaseSourceError(f"release index is not HEAD: {rendered}")

    tracked_changes = _git_paths(root, "diff", "--name-only", "--")
    if tracked_changes:
        raise DirtyReleaseSourceError(
            "release worktree has tracked changes: " + ", ".join(tracked_changes)
        )
    untracked = _git_paths(root, "ls-files", "--others", "--exclude-standard")
    if untracked:
        raise DirtyReleaseSourceError(
            "release worktree has non-ignored untracked paths: "
            + ", ".join(untracked)
        )

    lockfile = root / _WORKSPACE_LOCKFILE
    if not lockfile.is_file() or lockfile.is_symlink():
        raise DirtyReleaseSourceError(
            f"release requires a regular workspace {_WORKSPACE_LOCKFILE}"
        )
    return {
        "schema_version": 1,
        "head_commit": head_commit,
        "head_tree": head_tree,
        "index_tree": index_tree,
        "workspace_source_manifest_sha256": workspace_source_manifest(root),
        "cargo_lock_sha256": _sha256_file(lockfile),
    }


def _frame(hasher: "hashlib._Hash", payload: bytes) -> None:
    hasher.update(struct.pack(">Q", len(payload)))
    hasher.update(payload)


def _manifest_for_paths(root: Path, paths: Iterable[str]) -> str:
    hasher = hashlib.sha256(_DOMAIN)
    for relative in sorted(set(paths), key=os.fsencode):
        encoded_path = os.fsencode(relative)
        _frame(hasher, encoded_path)
        path = root / relative
        try:
            metadata = path.lstat()
        except FileNotFoundError:
            hasher.update(b"D")
            continue

        hasher.update(struct.pack(">I", stat.S_IMODE(metadata.st_mode)))
        if stat.S_ISLNK(metadata.st_mode):
            hasher.update(b"L")
            _frame(hasher, os.fsencode(os.readlink(path)))
        elif stat.S_ISREG(metadata.st_mode):
            hasher.update(b"F")
            hasher.update(struct.pack(">Q", metadata.st_size))
            with path.open("rb") as source:
                while chunk := source.read(1024 * 1024):
                    hasher.update(chunk)
        elif stat.S_ISDIR(metadata.st_mode):
            # Gitlinks/submodules appear as directory entries in the parent.
            hasher.update(b"G")
        else:
            hasher.update(b"O")
    return hasher.hexdigest()


def workspace_source_manifest(root: Path) -> str:
    """Return the checkout source manifest rooted at ``root``."""

    root = root.resolve()
    return _manifest_for_paths(root, _git_source_paths(root))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Git checkout root (defaults to this repository)",
    )
    parser.add_argument(
        "--release-identity-json",
        action="store_true",
        help="require one clean committed source and print its canonical identity",
    )
    args = parser.parse_args()
    try:
        if args.release_identity_json:
            print(
                json.dumps(
                    release_source_identity(args.root),
                    sort_keys=True,
                    separators=(",", ":"),
                )
            )
        else:
            print(workspace_source_manifest(args.root))
    except (
        ActiveGitOperationError,
        DirtyReleaseSourceError,
        UnmergedSourceError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"workspace source manifest error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
