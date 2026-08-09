#!/usr/bin/env python3
"""Resolve and hash the reviewed Sumeragi SDK production-source closure."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
from typing import Any, Iterable, Sequence


MANIFEST_RELATIVE_PATH = PurePosixPath(
    "ci/sumeragi_v2_sdk_source_closure.json"
)
RESOLVER_RELATIVE_PATH = PurePosixPath(
    "ci/resolve_sumeragi_v2_sdk_source_closure.py"
)
MANIFEST_FORMAT = "iroha-sumeragi-v2-sdk-production-source-closure"
MANIFEST_VERSION = 1
EXPECTED_SUITES = frozenset(
    {
        "native-amx-v2-grouped",
        "sumeragi-v2-sdk-diagnostics",
    }
)
EXPECTED_HARNESSES = {
    "native-amx-v2-grouped": PurePosixPath(
        "ci/run_native_amx_v2_grouped_sdk_parity.sh"
    ),
    "sumeragi-v2-sdk-diagnostics": PurePosixPath(
        "ci/run_sumeragi_v2_sdk_diagnostics.sh"
    ),
}
GROUP_NAME_RE = re.compile(r"^[a-z][a-z0-9-]*$")
EXTENSION_RE = re.compile(r"^\.[A-Za-z0-9.]+$")
REGULAR_GIT_MODES = frozenset({"100644", "100755"})


class ClosureError(RuntimeError):
    """A deterministic, user-facing closure validation failure."""


@dataclass(frozen=True)
class ClosureRoot:
    """One exact production directory owned by a manifest group."""

    extensions: tuple[str, ...]
    group: str
    path: PurePosixPath
    recursive: bool

    def sort_key(self) -> tuple[str, str, tuple[str, ...], bool]:
        return (self.path.as_posix(), self.group, self.extensions, self.recursive)


@dataclass(frozen=True)
class SourceClosureManifest:
    """Validated manifest data used to resolve a suite."""

    closure_roots: tuple[ClosureRoot, ...]
    groups: dict[str, tuple[PurePosixPath, ...]]
    suites: dict[str, tuple[str, ...]]


@dataclass(frozen=True)
class GitIndexEntry:
    """A stage-zero Git index entry."""

    mode: str
    object_id: str


def _fail(message: str) -> ClosureError:
    return ClosureError(message)


def _stable_regular_bytes(path: Path, context: str) -> bytes:
    """Read one regular file without following a final symlink or accepting drift."""

    flags = os.O_RDONLY
    flags |= getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise _fail(f"{context} is not a readable regular non-symlink file") from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise _fail(f"{context} must be a regular non-symlink file")
        chunks: list[bytes] = []
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            chunks.append(chunk)
        after = os.fstat(descriptor)
        identity_before = (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
        )
        identity_after = (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        )
        if identity_before != identity_after:
            raise _fail(f"{context} changed while it was read")
        try:
            pathname = os.lstat(path)
        except OSError as error:
            raise _fail(f"{context} disappeared while it was read") from error
        if (
            stat.S_ISLNK(pathname.st_mode)
            or not stat.S_ISREG(pathname.st_mode)
            or pathname.st_dev != after.st_dev
            or pathname.st_ino != after.st_ino
        ):
            raise _fail(f"{context} changed identity while it was read")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def _canonical_relative_path(value: Any, context: str) -> PurePosixPath:
    if not isinstance(value, str) or not value:
        raise _fail(f"{context} must be a non-empty string")
    if "\\" in value or any(ord(character) < 0x20 for character in value):
        raise _fail(f"{context} must use a printable canonical POSIX path")
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or path.as_posix() != value
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise _fail(f"{context} must be a canonical repository-relative path")
    return path


def _object_without_duplicate_keys(
    pairs: Sequence[tuple[str, Any]],
) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise _fail(f"source-closure manifest contains duplicate key {key!r}")
        result[key] = value
    return result


def _require_exact_keys(
    value: Any,
    expected: frozenset[str],
    context: str,
) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise _fail(f"{context} must be an object")
    observed = frozenset(value)
    if observed != expected:
        missing = sorted(expected - observed)
        unexpected = sorted(observed - expected)
        raise _fail(
            f"{context} keys are not exact: missing={missing!r} "
            f"unexpected={unexpected!r}"
        )
    return value


def _sorted_unique_strings(value: Any, context: str) -> tuple[str, ...]:
    if not isinstance(value, list) or not value:
        raise _fail(f"{context} must be a non-empty array")
    if any(not isinstance(item, str) or not item for item in value):
        raise _fail(f"{context} must contain only non-empty strings")
    if value != sorted(value) or len(value) != len(set(value)):
        raise _fail(f"{context} must be strictly sorted and duplicate-free")
    return tuple(value)


def _manifest_from_bytes(raw: bytes) -> SourceClosureManifest:
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise _fail("source-closure manifest must be UTF-8") from error
    try:
        document = json.loads(text, object_pairs_hook=_object_without_duplicate_keys)
    except ClosureError:
        raise
    except (json.JSONDecodeError, RecursionError) as error:
        raise _fail(f"source-closure manifest is not valid JSON: {error}") from error
    top = _require_exact_keys(
        document,
        frozenset({"closure_roots", "format", "groups", "suites", "version"}),
        "source-closure manifest",
    )
    if top["format"] != MANIFEST_FORMAT:
        raise _fail(f"source-closure manifest format must equal {MANIFEST_FORMAT!r}")
    if type(top["version"]) is not int or top["version"] != MANIFEST_VERSION:
        raise _fail(f"source-closure manifest version must equal {MANIFEST_VERSION}")
    canonical = (
        json.dumps(document, ensure_ascii=True, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    if raw != canonical:
        raise _fail(
            "source-closure manifest bytes must use canonical sorted two-space JSON"
        )

    groups_value = top["groups"]
    if not isinstance(groups_value, dict) or not groups_value:
        raise _fail("source-closure manifest groups must be a non-empty object")
    groups: dict[str, tuple[PurePosixPath, ...]] = {}
    all_paths: dict[PurePosixPath, str] = {}
    for group, raw_paths in groups_value.items():
        if not isinstance(group, str) or GROUP_NAME_RE.fullmatch(group) is None:
            raise _fail(f"invalid source-closure group name {group!r}")
        path_strings = _sorted_unique_strings(raw_paths, f"group {group!r}")
        paths = tuple(
            _canonical_relative_path(path, f"group {group!r} path")
            for path in path_strings
        )
        for path in paths:
            prior_group = all_paths.get(path)
            if prior_group is not None:
                raise _fail(
                    f"source path {path.as_posix()!r} is duplicated by groups "
                    f"{prior_group!r} and {group!r}"
                )
            all_paths[path] = group
        groups[group] = paths

    roots_value = top["closure_roots"]
    if not isinstance(roots_value, list) or not roots_value:
        raise _fail("source-closure manifest closure_roots must be non-empty")
    closure_roots: list[ClosureRoot] = []
    for index, raw_root in enumerate(roots_value):
        root = _require_exact_keys(
            raw_root,
            frozenset({"extensions", "group", "path", "recursive"}),
            f"closure_roots[{index}]",
        )
        group = root["group"]
        if not isinstance(group, str) or group not in groups:
            raise _fail(f"closure_roots[{index}].group must name a declared group")
        extensions = _sorted_unique_strings(
            root["extensions"], f"closure_roots[{index}].extensions"
        )
        if any(EXTENSION_RE.fullmatch(extension) is None for extension in extensions):
            raise _fail(
                f"closure_roots[{index}].extensions contains an invalid suffix"
            )
        recursive = root["recursive"]
        if type(recursive) is not bool:
            raise _fail(f"closure_roots[{index}].recursive must be boolean")
        closure_roots.append(
            ClosureRoot(
                extensions=extensions,
                group=group,
                path=_canonical_relative_path(
                    root["path"], f"closure_roots[{index}].path"
                ),
                recursive=recursive,
            )
        )
    if closure_roots != sorted(closure_roots, key=ClosureRoot.sort_key):
        raise _fail("source-closure manifest closure_roots must be strictly sorted")
    root_paths = [root.path for root in closure_roots]
    if len(root_paths) != len(set(root_paths)):
        raise _fail("source-closure manifest closure_roots paths must be unique")

    suites_value = top["suites"]
    if not isinstance(suites_value, dict):
        raise _fail("source-closure manifest suites must be an object")
    if frozenset(suites_value) != EXPECTED_SUITES:
        raise _fail(
            "source-closure manifest suites must be exactly "
            f"{sorted(EXPECTED_SUITES)!r}"
        )
    suites: dict[str, tuple[str, ...]] = {}
    used_groups: set[str] = set()
    for suite, raw_groups in suites_value.items():
        suite_groups = _sorted_unique_strings(raw_groups, f"suite {suite!r}")
        unknown = sorted(set(suite_groups) - groups.keys())
        if unknown:
            raise _fail(f"suite {suite!r} names unknown groups {unknown!r}")
        resolved_paths = {
            path for group in suite_groups for path in groups[group]
        }
        required_paths = {
            MANIFEST_RELATIVE_PATH,
            RESOLVER_RELATIVE_PATH,
            EXPECTED_HARNESSES[suite],
        }
        missing_required = sorted(
            (path.as_posix() for path in required_paths - resolved_paths)
        )
        if missing_required:
            raise _fail(
                f"suite {suite!r} omits required closure inputs "
                f"{missing_required!r}"
            )
        suites[suite] = suite_groups
        used_groups.update(suite_groups)
    unused_groups = sorted(groups.keys() - used_groups)
    if unused_groups:
        raise _fail(f"source-closure manifest has unused groups {unused_groups!r}")
    return SourceClosureManifest(
        closure_roots=tuple(closure_roots),
        groups=groups,
        suites=suites,
    )


def _assert_canonical_root(root: Path) -> Path:
    absolute = Path(os.path.abspath(root))
    canonical = Path(os.path.realpath(root))
    if absolute != canonical or not canonical.is_dir():
        raise _fail("repository root must be an existing canonical directory")
    return canonical


def _relative_path(root: Path, path: Path, context: str) -> PurePosixPath:
    absolute = Path(os.path.abspath(path))
    try:
        relative = absolute.relative_to(root)
    except ValueError as error:
        raise _fail(f"{context} must be inside the repository root") from error
    return _canonical_relative_path(relative.as_posix(), context)


def _assert_no_symlink_components(root: Path, relative: PurePosixPath) -> Path:
    current = root
    for part in relative.parts:
        current = current / part
        try:
            metadata = os.lstat(current)
        except FileNotFoundError as error:
            raise _fail(f"source-closure path is missing: {relative.as_posix()}") from error
        if stat.S_ISLNK(metadata.st_mode):
            raise _fail(
                f"source-closure path traverses a symlink: {relative.as_posix()}"
            )
    return current


def _git_output(root: Path, arguments: Sequence[str]) -> bytes:
    result = subprocess.run(
        ["git", "-C", os.fspath(root), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        diagnostic = result.stderr.decode("utf-8", errors="replace").strip()
        raise _fail(f"Git source tracking query failed: {diagnostic}")
    return result.stdout


def _git_index(root: Path) -> tuple[bytes, dict[PurePosixPath, GitIndexEntry]]:
    top_level_raw = _git_output(root, ["rev-parse", "--show-toplevel"])
    try:
        top_level = Path(top_level_raw.decode("utf-8").strip())
    except UnicodeDecodeError as error:
        raise _fail("Git repository root is not UTF-8") from error
    if Path(os.path.realpath(top_level)) != root:
        raise _fail("--root must name the exact Git worktree root")
    raw = _git_output(root, ["ls-files", "--stage", "-z"])
    entries: dict[PurePosixPath, GitIndexEntry] = {}
    for record in raw.split(b"\0"):
        if not record:
            continue
        try:
            metadata, raw_path = record.split(b"\t", 1)
            mode, object_id, stage = metadata.decode("ascii").split(" ")
            path_text = raw_path.decode("utf-8")
        except (UnicodeDecodeError, ValueError) as error:
            raise _fail("Git index contains an unsupported source entry") from error
        path = _canonical_relative_path(path_text, "Git index path")
        if stage != "0":
            raise _fail(f"Git index path is unmerged: {path.as_posix()}")
        if path in entries:
            raise _fail(f"Git index path is duplicated: {path.as_posix()}")
        entries[path] = GitIndexEntry(mode=mode, object_id=object_id)
    return raw, entries


def _discover_root_paths(
    repo_root: Path,
    closure_root: ClosureRoot,
) -> set[PurePosixPath]:
    root_path = _assert_no_symlink_components(repo_root, closure_root.path)
    try:
        root_metadata = os.lstat(root_path)
    except OSError as error:
        raise _fail(
            f"production closure root is unreadable: {closure_root.path.as_posix()}"
        ) from error
    if not stat.S_ISDIR(root_metadata.st_mode):
        raise _fail(
            f"production closure root must be a directory: "
            f"{closure_root.path.as_posix()}"
        )

    discovered: set[PurePosixPath] = set()
    if closure_root.recursive:
        iterator: Iterable[tuple[str, list[str], list[str]]] = os.walk(root_path)
    else:
        with os.scandir(root_path) as entries_iterator:
            entries = sorted(entries_iterator, key=lambda entry: entry.name)
        for entry in entries:
            if entry.is_symlink():
                relative = (root_path / entry.name).relative_to(repo_root).as_posix()
                raise _fail(f"production closure contains a symlink: {relative}")
        filenames = [
            entry.name for entry in entries if entry.is_file(follow_symlinks=False)
        ]
        directory_names = [
            entry.name for entry in entries if entry.is_dir(follow_symlinks=False)
        ]
        iterator = [(os.fspath(root_path), directory_names, filenames)]
    for directory, directory_names, filenames in iterator:
        directory_names.sort()
        filenames.sort()
        for name in directory_names:
            candidate = Path(directory) / name
            metadata = os.lstat(candidate)
            if stat.S_ISLNK(metadata.st_mode):
                relative = candidate.relative_to(repo_root).as_posix()
                raise _fail(f"production closure contains a symlink: {relative}")
        for name in filenames:
            candidate = Path(directory) / name
            metadata = os.lstat(candidate)
            relative = _canonical_relative_path(
                candidate.relative_to(repo_root).as_posix(),
                "discovered production source path",
            )
            if stat.S_ISLNK(metadata.st_mode):
                raise _fail(
                    f"production closure contains a symlink: {relative.as_posix()}"
                )
            if not name.endswith(closure_root.extensions):
                continue
            if not stat.S_ISREG(metadata.st_mode):
                raise _fail(
                    f"production closure input is not regular: {relative.as_posix()}"
                )
            discovered.add(relative)
        if not closure_root.recursive:
            break
    return discovered


def _suite_paths(
    manifest: SourceClosureManifest,
    suite: str,
) -> tuple[PurePosixPath, ...]:
    if suite not in manifest.suites:
        raise _fail(f"unknown source-closure suite {suite!r}")
    return tuple(
        sorted(
            {
                path
                for group in manifest.suites[suite]
                for path in manifest.groups[group]
            },
            key=PurePosixPath.as_posix,
        )
    )


def _validate_and_hash(
    repo_root: Path,
    manifest_path: Path,
    suite: str,
) -> tuple[tuple[PurePosixPath, str], ...]:
    expected_manifest = repo_root / MANIFEST_RELATIVE_PATH
    if Path(os.path.abspath(manifest_path)) != expected_manifest:
        raise _fail(
            f"manifest path must equal {MANIFEST_RELATIVE_PATH.as_posix()!r} "
            "inside the repository root"
        )
    _assert_no_symlink_components(repo_root, MANIFEST_RELATIVE_PATH)
    manifest_raw = _stable_regular_bytes(manifest_path, "source-closure manifest")
    manifest = _manifest_from_bytes(manifest_raw)
    suite_paths = _suite_paths(manifest, suite)

    index_before, tracked = _git_index(repo_root)
    for path in suite_paths:
        absolute = _assert_no_symlink_components(repo_root, path)
        metadata = os.lstat(absolute)
        if not stat.S_ISREG(metadata.st_mode):
            raise _fail(
                f"source-closure input must be a regular file: {path.as_posix()}"
            )
        index_entry = tracked.get(path)
        if index_entry is None:
            raise _fail(f"source-closure input is untracked: {path.as_posix()}")
        if index_entry.mode not in REGULAR_GIT_MODES:
            raise _fail(
                f"source-closure input has unexpected Git mode "
                f"{index_entry.mode}: {path.as_posix()}"
            )

    discovered_by_group: dict[str, set[PurePosixPath]] = {}
    for closure_root in manifest.closure_roots:
        discovered = _discover_root_paths(repo_root, closure_root)
        group_discovered = discovered_by_group.setdefault(closure_root.group, set())
        overlap = group_discovered & discovered
        if overlap:
            raise _fail(
                "overlapping production closure roots discovered "
                f"{sorted(path.as_posix() for path in overlap)!r}"
            )
        group_discovered.update(discovered)
    for group, discovered in discovered_by_group.items():
        declared = set(manifest.groups[group])
        missing = sorted(path.as_posix() for path in declared - discovered)
        unexpected = sorted(discovered - declared, key=PurePosixPath.as_posix)
        if missing:
            raise _fail(
                f"production group {group!r} declares paths outside its exact "
                f"closure roots: {missing!r}"
            )
        if unexpected:
            first = unexpected[0]
            tracking = "tracked" if first in tracked else "untracked"
            raise _fail(
                f"production group {group!r} has unexpected {tracking} input: "
                f"{first.as_posix()}"
            )

    records: list[tuple[PurePosixPath, str]] = []
    for path in suite_paths:
        payload = _stable_regular_bytes(
            repo_root / path,
            f"source-closure input {path.as_posix()!r}",
        )
        records.append((path, hashlib.sha256(payload).hexdigest()))

    rediscovered_by_group: dict[str, set[PurePosixPath]] = {}
    for closure_root in manifest.closure_roots:
        rediscovered_by_group.setdefault(closure_root.group, set()).update(
            _discover_root_paths(repo_root, closure_root)
        )
    if rediscovered_by_group != discovered_by_group:
        raise _fail("production source closure changed while it was hashed")
    index_after, _tracked_after = _git_index(repo_root)
    if index_after != index_before:
        raise _fail("Git index changed while the source closure was hashed")
    return tuple(records)


def _records_digest(records: Sequence[tuple[PurePosixPath, str]]) -> str:
    manifest_bytes = b"".join(
        f"{path.as_posix()}\t{digest}\n".encode("utf-8")
        for path, digest in records
    )
    return hashlib.sha256(manifest_bytes).hexdigest()


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument(
        "--manifest",
        type=Path,
        help=(
            "manifest path; defaults to "
            f"ROOT/{MANIFEST_RELATIVE_PATH.as_posix()}"
        ),
    )
    parser.add_argument("--suite", required=True, choices=sorted(EXPECTED_SUITES))
    actions = parser.add_mutually_exclusive_group(required=True)
    actions.add_argument("--check", action="store_true")
    actions.add_argument("--print-paths", action="store_true")
    actions.add_argument("--print-records", action="store_true")
    actions.add_argument("--manifest-sha256", action="store_true")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        repo_root = _assert_canonical_root(args.root)
        manifest_path = (
            args.manifest
            if args.manifest is not None
            else repo_root / MANIFEST_RELATIVE_PATH
        )
        manifest_relative = _relative_path(
            repo_root, manifest_path, "source-closure manifest path"
        )
        manifest_path = repo_root / manifest_relative
        records = _validate_and_hash(repo_root, manifest_path, args.suite)
    except ClosureError as error:
        print(f"SDK source closure error: {error}", file=sys.stderr)
        return 1

    if args.print_paths:
        for path, _digest in records:
            print(path.as_posix())
    elif args.print_records:
        for path, digest in records:
            print(f"{path.as_posix()}\t{digest}")
    elif args.manifest_sha256:
        print(_records_digest(records))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
