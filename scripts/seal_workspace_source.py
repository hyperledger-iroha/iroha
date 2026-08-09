#!/usr/bin/env python3
"""Make ordinary writes to a detached release worktree fail cooperatively.

This is an integrity guard against editors, build tools, and accidental writes,
not a privilege boundary: the owning uid can restore write bits. Repeated source
identity checks detect state present at checkpoints; detached committed source
removes the mutable caller checkout from the execution path.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path, PurePosixPath
import stat
import sys
from typing import Iterable


class SealError(RuntimeError):
    """A source seal is invalid or cannot be enforced."""


def _relative_writable_paths(values: Iterable[str]) -> frozenset[PurePosixPath]:
    paths = set()
    for value in values:
        path = PurePosixPath(value)
        if path.is_absolute() or not path.parts or ".." in path.parts:
            raise SealError(f"writable path must be a relative child: {value!r}")
        paths.add(path)
    return frozenset(paths)


def _is_writable_path(
    relative: PurePosixPath, writable: frozenset[PurePosixPath]
) -> bool:
    return any(relative == path or path in relative.parents for path in writable)


def _walk_source(
    root: Path, writable: frozenset[PurePosixPath], *, topdown: bool
) -> Iterable[tuple[Path, list[str], list[str]]]:
    for directory, directories, files in os.walk(
        root, topdown=topdown, followlinks=False
    ):
        current = Path(directory)
        retained = []
        for name in directories:
            child = current / name
            relative = PurePosixPath(child.relative_to(root).as_posix())
            if not _is_writable_path(relative, writable):
                retained.append(name)
        directories[:] = retained
        yield current, directories, files


def _validate_symlink_target(
    root: Path,
    link: Path,
    target: Path,
    writable: frozenset[PurePosixPath],
    *,
    form: str,
) -> None:
    try:
        relative = PurePosixPath(target.relative_to(root).as_posix())
    except ValueError as error:
        raise SealError(
            f"source symlink {link.relative_to(root)} {form} target escapes "
            f"the sealed source root: {target}"
        ) from error
    if _is_writable_path(relative, writable):
        raise SealError(
            f"source symlink {link.relative_to(root)} {form} target enters "
            f"a writable output: {relative}"
        )


def _validate_source_symlinks(
    root: Path, writable: frozenset[PurePosixPath]
) -> None:
    """Reject source links that can read outside the immutable source tree."""

    for directory, directories, files in os.walk(root, topdown=True, followlinks=False):
        current = Path(directory)
        for name in (*directories, *files):
            link = current / name
            relative = PurePosixPath(link.relative_to(root).as_posix())
            if _is_writable_path(relative, writable) or not link.is_symlink():
                continue
            raw_target = Path(os.readlink(link))
            target = raw_target if raw_target.is_absolute() else link.parent / raw_target
            lexical_target = Path(os.path.abspath(target))
            _validate_symlink_target(
                root, link, lexical_target, writable, form="lexical"
            )
            resolved_target = target.resolve(strict=False)
            _validate_symlink_target(
                root, link, resolved_target, writable, form="resolved"
            )
        directories[:] = [
            name
            for name in directories
            if not _is_writable_path(
                PurePosixPath((current / name).relative_to(root).as_posix()), writable
            )
        ]


def seal_source_tree(root: Path, writable_paths: Iterable[str] = ("target",)) -> None:
    """Remove write permission from source while preserving executable bits."""

    root = root.resolve(strict=True)
    if not root.is_dir():
        raise SealError(f"source root is not a directory: {root}")
    writable = _relative_writable_paths(writable_paths)
    _validate_source_symlinks(root, writable)
    visited_directories = []
    for directory, _directories, files in _walk_source(root, writable, topdown=True):
        visited_directories.append(directory)
        for name in files:
            path = directory / name
            if path.is_symlink() or not path.is_file():
                continue
            metadata = path.stat()
            if metadata.st_nlink != 1:
                raise SealError(
                    f"source regular file has external hard-link aliases: "
                    f"{path.relative_to(root)}"
                )
            mode = stat.S_IMODE(metadata.st_mode)
            path.chmod(0o555 if mode & 0o111 else 0o444)
    for directory in reversed(visited_directories):
        directory.chmod(0o555)


def verify_source_tree_sealed(
    root: Path, writable_paths: Iterable[str] = ("target",)
) -> None:
    """Reject any writable source file or directory outside designated outputs."""

    root = root.resolve(strict=True)
    writable = _relative_writable_paths(writable_paths)
    _validate_source_symlinks(root, writable)
    violations = []
    for directory, _directories, files in _walk_source(root, writable, topdown=True):
        if stat.S_IMODE(directory.stat().st_mode) & 0o222:
            violations.append(str(directory))
        for name in files:
            path = directory / name
            if path.is_symlink() or not path.is_file():
                continue
            metadata = path.stat()
            if metadata.st_nlink != 1:
                violations.append(f"{path} (hard-linked)")
                continue
            if stat.S_IMODE(metadata.st_mode) & 0o222:
                violations.append(str(path))
    if violations:
        raise SealError("writable source remains under seal: " + ", ".join(violations))


def unseal_source_tree(root: Path) -> None:
    """Restore owner write/traversal permission so a private worktree can be removed."""

    root = root.resolve(strict=True)
    for directory, directories, files in os.walk(root, topdown=True, followlinks=False):
        current = Path(directory)
        current.chmod(stat.S_IMODE(current.stat().st_mode) | 0o700)
        for name in files:
            path = current / name
            if path.is_symlink() or not path.is_file():
                continue
            path.chmod(stat.S_IMODE(path.stat().st_mode) | 0o600)
        directories[:] = [
            name for name in directories if not (current / name).is_symlink()
        ]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--seal", action="store_true")
    action.add_argument("--verify", action="store_true")
    action.add_argument("--unseal", action="store_true")
    parser.add_argument("--root", type=Path, required=True)
    writable_policy = parser.add_mutually_exclusive_group()
    writable_policy.add_argument(
        "--writable",
        action="append",
        default=[],
        help="relative writable subtree (defaults to target)",
    )
    writable_policy.add_argument(
        "--no-writable-paths",
        action="store_true",
        help="seal every path below the source root, including target",
    )
    args = parser.parse_args()
    writable = [] if args.no_writable_paths else (args.writable or ["target"])
    try:
        if args.seal:
            seal_source_tree(args.root, writable)
        elif args.verify:
            verify_source_tree_sealed(args.root, writable)
        else:
            unseal_source_tree(args.root)
    except (OSError, SealError) as error:
        print(f"workspace source seal error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
