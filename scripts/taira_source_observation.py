"""Observe nonignored Git inputs without claiming build provenance.

Uses Git, the standard library and an injected command runner. Unchanged tracked
files are represented by HEAD and its binary diff; only nonignored untracked
contents are hashed. No builds, writes, credentials or network are required.
"""
from __future__ import annotations

from collections.abc import Callable
import hashlib
import os
from pathlib import Path
import re
import stat
import subprocess
from typing import Any, NoReturn

Runner = Callable[..., subprocess.CompletedProcess[str]]
LOWER_GIT_COMMIT_RE = re.compile(r"[0-9a-f]{40}")

class SourceObservationError(RuntimeError):
    """The current source could not be observed consistently."""


def fail(message: str) -> NoReturn:
    raise SourceObservationError(message)


def source_observation_field(observation: Any, name: bytes, value: bytes) -> None:
    """Feed one length-delimited field into the worktree-observation digest."""

    observation.update(len(name).to_bytes(4, "big"))
    observation.update(name)
    observation.update(len(value).to_bytes(8, "big"))
    observation.update(value)


def untracked_source_content(path: Path, metadata: os.stat_result) -> tuple[bytes, bytes]:
    """Return one stable untracked entry type and content digest."""

    if stat.S_ISLNK(metadata.st_mode):
        try:
            target = os.fsencode(os.readlink(path))
            after = path.lstat()
        except OSError as error:
            fail(f"cannot inspect untracked source symlink {path}: {error}")
        if (after.st_dev, after.st_ino, after.st_mtime_ns, after.st_ctime_ns) != (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_mtime_ns,
            metadata.st_ctime_ns,
        ):
            fail(f"untracked source changed while hashing it: {path}")
        return b"symlink", hashlib.sha256(target).digest()
    if not stat.S_ISREG(metadata.st_mode):
        fail(f"untracked source is not a regular file or symlink: {path}")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open untracked source {path}: {error}")
    digest = hashlib.sha256()
    try:
        try:
            opened = os.fstat(descriptor)
            if (opened.st_dev, opened.st_ino) != (metadata.st_dev, metadata.st_ino):
                fail(f"untracked source changed while opening it: {path}")
            with os.fdopen(descriptor, "rb", closefd=True) as stream:
                descriptor = -1
                while chunk := stream.read(1024 * 1024):
                    digest.update(chunk)
                after = os.fstat(stream.fileno())
        except OSError as error:
            fail(f"cannot hash untracked source {path}: {error}")
    finally:
        if descriptor >= 0:
            try:
                os.close(descriptor)
            except OSError as error:
                fail(f"cannot close qualifying executable {path}: {error}")
    try:
        pathname_after = path.lstat()
    except OSError as error:
        fail(f"cannot re-inspect untracked source {path}: {error}")
    if (
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
        pathname_after.st_dev,
        pathname_after.st_ino,
        pathname_after.st_size,
        pathname_after.st_mtime_ns,
        pathname_after.st_ctime_ns,
    ) != (
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    ):
        fail(f"untracked source changed while hashing it: {path}")
    return b"file", digest.digest()


def current_source_observation(
    root: Path, run: Runner, *, required_branch: str | None = None,
) -> dict[str, str]:
    """Observe HEAD and the non-ignored worktree without claiming build custody.

    This digest is a pre/post race detector.  It cannot prove which inputs
    Cargo, rustc, build scripts, dependency caches, or repository/user Cargo
    configuration consumed, so the public report states that limitation
    explicitly instead of presenting the observation as source provenance.
    """

    branch = (
        run(
            ["git", "branch", "--show-current"],
            cwd=root,
            timeout=20,
        ).stdout
        or ""
    ).strip()
    if required_branch is not None and branch != required_branch:
        fail(
            "Taira Inrou qualification requires branch "
            f"`{required_branch}`, found `{branch or 'detached HEAD'}`"
        )
    git_head = (
        run(["git", "rev-parse", "HEAD"], cwd=root, timeout=20).stdout or ""
    ).strip()
    if LOWER_GIT_COMMIT_RE.fullmatch(git_head) is None:
        fail("Taira Inrou qualification could not resolve one canonical Git HEAD")
    # Abbreviations can change with Git object inventory even when every source
    # byte is unchanged. Hash canonical full object names and actual file bytes,
    # without color, external diff tools or text conversion presentation.
    tracked_diff = (
        run(
            [
                "git", "diff", "--binary", "--full-index", "--no-color",
                "--no-ext-diff", "--no-textconv", "HEAD", "--", ".",
            ],
            cwd=root,
            timeout=60,
        ).stdout
        or ""
    )
    untracked_output = (
        run(
            ["git", "ls-files", "--others", "--exclude-standard", "-z"],
            cwd=root,
            timeout=30,
        ).stdout
        or ""
    )
    untracked = sorted(path for path in untracked_output.split("\0") if path)
    observation = hashlib.sha256()
    source_observation_field(
        observation,
        b"domain",
        b"iroha.taira.nonignored-worktree-observation.v1",
    )
    source_observation_field(observation, b"git-head", git_head.encode("ascii"))
    source_observation_field(
        observation,
        b"tracked-diff",
        tracked_diff.encode("utf-8", errors="surrogateescape"),
    )
    source_observation_field(
        observation,
        b"untracked-count",
        len(untracked).to_bytes(8, "big"),
    )
    for relative in untracked:
        relative_path = Path(relative)
        if relative_path.is_absolute() or ".." in relative_path.parts:
            fail(f"Git reported an unsafe untracked source path: {relative}")
        path = root / relative_path
        try:
            metadata = path.lstat()
        except OSError as error:
            fail(f"cannot inspect untracked source {path}: {error}")
        entry_type, content_digest = untracked_source_content(path, metadata)
        source_observation_field(
            observation,
            b"untracked-path",
            relative.encode("utf-8", errors="surrogateescape"),
        )
        source_observation_field(
            observation,
            b"untracked-mode",
            stat.S_IMODE(metadata.st_mode).to_bytes(4, "big"),
        )
        source_observation_field(observation, b"untracked-type", entry_type)
        source_observation_field(
            observation,
            b"untracked-size",
            metadata.st_size.to_bytes(8, "big"),
        )
        source_observation_field(
            observation,
            b"untracked-content-sha256",
            content_digest,
        )
    return {
        "branch": branch,
        "git_head": git_head,
        "observation_scope": "git_head_tracked_diff_nonignored_untracked",
        "observed_nonignored_worktree_sha256": observation.hexdigest(),
        "cargo_source_consumption": "not_proven",
    }
