#!/usr/bin/env python3
"""Create or check the canonical manifest for SoraFS CLI release candidates."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path
from typing import BinaryIO

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 uses the pinned backport.
    import tomli as tomllib


SCHEMA = "sorafs.cli.release-manifest.v1"
MAX_CHECKSUM_MANIFEST_BYTES = 1024 * 1024
MAX_VERSION_MAP_BYTES = 64 * 1024
MAX_RELEASE_MANIFEST_BYTES = 1024 * 1024
MAX_FILES_PER_TARGET = 1024
MAX_ENTRIES_PER_TARGET = 2048
MAX_CANDIDATE_TREE_DEPTH = 16
MAX_TOTAL_FILES = 4096
TARGETS = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
)
TARGET_BINARY_SUFFIXES = {
    target: ".exe" if target == "x86_64-pc-windows-msvc" else ""
    for target in TARGETS
}
COMMON_TARGET_FILES = (
    "version-map.toml",
    "ROLLBACK-YANK.md",
    "CHANGELOG.md",
    "LICENSE",
    "sorafs-release.spdx.json",
    "sorafs-release-vulnerabilities.sarif",
)
SHA256_RE = re.compile(r"[0-9a-f]{64}")
VERSION_RE = re.compile(
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?\Z"
)
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
REPOSITORY_RE = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+")
REF_RE = re.compile(r"refs/(?:heads|tags)/[A-Za-z0-9._/-]{1,240}")


class ManifestError(RuntimeError):
    """Raised when a release candidate or manifest violates the contract."""


def required_candidate_payload_paths(version: str, target: str) -> set[str]:
    """Return the schema-closed release payload required for one native target."""

    suffix = TARGET_BINARY_SUFFIXES[target]
    validator_package = f"sorafs-validate-{version}-{target}"
    cli_package = f"sorafs-cli-{version}-{target}"
    return {
        f"sorafs_cli{suffix}",
        f"sorafs_fetch{suffix}",
        f"sorafs-validate{suffix}",
        "sorafs_cli.help.txt",
        "sorafs_fetch.help.txt",
        "sorafs-validate.help.txt",
        "version-map.toml",
        "ROLLBACK-YANK.md",
        "CHANGELOG.md",
        "LICENSE",
        f"reference-validator/{validator_package}.sha256",
        f"reference-validator/{validator_package}.tar.gz",
        f"reference-validator/{validator_package}.tar.gz.sha256",
        f"reference-validator/{validator_package}.manifest.json",
        f"reference-validator/{validator_package}.manifest.json.sha256",
        (
            f"reference-validator/{validator_package}/"
            f"sorafs-validate{suffix}"
        ),
        f"reference-validator/{validator_package}/HELP.txt",
        (
            f"reference-validator/{validator_package}/include/"
            "sorafs_reference.h"
        ),
        f"reference-validator/{validator_package}/smoke.advert.json",
        f"reference-validator/{validator_package}/smoke.bundle.json",
        f"platform-archive/{cli_package}.tar.gz",
        f"platform-archive/{cli_package}.tar.gz.sha256",
        f"platform-archive/{cli_package}.manifest.json",
        f"platform-archive/{cli_package}.manifest.json.sha256",
        "platform-archive/candidate-package-first.json",
        "platform-archive/candidate-package-replay.json",
        "sorafs-release.spdx.json",
        "sorafs-release-vulnerabilities.sarif",
        f"sorafs-cli-{target}.spdx.json",
        f"sorafs-cli-{target}-vulnerabilities.sarif",
    }


def _validate_metadata(version: str, commit: str, repository: str, ref: str) -> None:
    if VERSION_RE.fullmatch(version) is None:
        raise ManifestError("version must be canonical SemVer")
    if COMMIT_RE.fullmatch(commit) is None:
        raise ManifestError("commit must be a full lowercase 40-hex Git commit")
    if REPOSITORY_RE.fullmatch(repository) is None:
        raise ManifestError("repository must be an owner/name pair")
    if REF_RE.fullmatch(ref) is None or "//" in ref or ref.endswith("/"):
        raise ManifestError("ref must be a canonical branch or tag reference")


def _identity(metadata: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
        metadata.st_nlink,
    )


def _open_regular(path: Path, label: str) -> tuple[BinaryIO, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as exc:
        raise ManifestError(f"cannot inspect {label}: {exc}") from exc
    if not stat.S_ISREG(before.st_mode):
        raise ManifestError(f"{label} must be a regular file")
    if before.st_nlink != 1:
        raise ManifestError(f"{label} must have exactly one hard link")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise ManifestError(f"cannot open {label}: {exc}") from exc
    handle = os.fdopen(descriptor, "rb", closefd=True)
    opened = os.fstat(handle.fileno())
    if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
        handle.close()
        raise ManifestError(f"{label} changed while it was opened")
    return handle, opened


def _hash_regular(path: Path, label: str) -> tuple[str, int]:
    handle, opened = _open_regular(path, label)
    digest = hashlib.sha256()
    try:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
        closed = os.fstat(handle.fileno())
    finally:
        handle.close()
    if _identity(opened) != _identity(closed):
        raise ManifestError(f"{label} changed while it was hashed")
    return digest.hexdigest(), opened.st_size


def _read_bounded_regular(path: Path, label: str, limit: int) -> bytes:
    handle, opened = _open_regular(path, label)
    payload = bytearray()
    try:
        while True:
            chunk = handle.read(65536)
            if not chunk:
                break
            payload.extend(chunk)
            if len(payload) > limit:
                raise ManifestError(f"{label} exceeds the {limit}-byte limit")
        closed = os.fstat(handle.fileno())
    finally:
        handle.close()
    if _identity(opened) != _identity(closed):
        raise ManifestError(f"{label} changed while it was read")
    return bytes(payload)


def _candidate_files(candidate_dir: Path, target: str) -> list[Path]:
    try:
        candidate_metadata = candidate_dir.lstat()
    except OSError as exc:
        raise ManifestError(
            f"cannot inspect candidate directory for {target}: {exc}"
        ) from exc
    if stat.S_ISLNK(candidate_metadata.st_mode) or not stat.S_ISDIR(
        candidate_metadata.st_mode
    ):
        raise ManifestError(f"candidate directory for {target} must be a real directory")

    files: list[Path] = []
    pending: list[tuple[Path, tuple[str, ...], os.stat_result]] = [
        (candidate_dir, (), candidate_metadata)
    ]
    visited_entries = 0
    while pending:
        directory, prefix, expected_metadata = pending.pop()
        relative_directory = "/".join(prefix) if prefix else "."
        try:
            observed_metadata = directory.lstat()
        except OSError as exc:
            raise ManifestError(
                f"candidate directory changed during enumeration: "
                f"{relative_directory}"
            ) from exc
        if (
            stat.S_ISLNK(observed_metadata.st_mode)
            or not stat.S_ISDIR(observed_metadata.st_mode)
            or (observed_metadata.st_dev, observed_metadata.st_ino)
            != (expected_metadata.st_dev, expected_metadata.st_ino)
        ):
            raise ManifestError(
                f"candidate directory changed during enumeration: "
                f"{relative_directory}"
            )
        try:
            entries = []
            with os.scandir(directory) as iterator:
                for entry in iterator:
                    visited_entries += 1
                    if visited_entries > MAX_ENTRIES_PER_TARGET:
                        raise ManifestError(
                            f"candidate for {target} exceeds the "
                            f"{MAX_ENTRIES_PER_TARGET}-entry limit"
                        )
                    entries.append(entry)
        except OSError as exc:
            raise ManifestError(
                f"candidate directory cannot be enumerated safely: "
                f"{relative_directory}"
            ) from exc
        entries.sort(key=lambda entry: entry.name)

        for entry in entries:
            entry_prefix = (*prefix, entry.name)
            relative = "/".join(entry_prefix)
            try:
                metadata = entry.stat(follow_symlinks=False)
            except OSError as exc:
                raise ManifestError(
                    f"candidate path changed during enumeration: {relative}"
                ) from exc
            if stat.S_ISLNK(metadata.st_mode):
                raise ManifestError(
                    f"candidate path must not be a symlink: {relative}"
                )
            if len(entry_prefix) > MAX_CANDIDATE_TREE_DEPTH:
                raise ManifestError(
                    f"candidate for {target} exceeds the "
                    f"{MAX_CANDIDATE_TREE_DEPTH}-level depth limit"
                )
            if stat.S_ISDIR(metadata.st_mode):
                pending.append((directory / entry.name, entry_prefix, metadata))
                continue
            if not stat.S_ISREG(metadata.st_mode):
                raise ManifestError(
                    f"candidate path must be a regular file: {relative}"
                )
            if metadata.st_nlink != 1:
                raise ManifestError(
                    f"candidate path must have exactly one hard link: {relative}"
                )
            files.append(directory / entry.name)
            if len(files) > MAX_FILES_PER_TARGET:
                raise ManifestError(
                    f"candidate for {target} exceeds the "
                    f"{MAX_FILES_PER_TARGET}-file limit"
                )
    if not files:
        raise ManifestError(f"candidate for {target} is empty")
    return sorted(
        files,
        key=lambda path: path.relative_to(candidate_dir).as_posix(),
    )


def _parse_checksums(payload: bytes, target: str) -> dict[str, str]:
    try:
        source = payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ManifestError(f"SHA256SUMS for {target} must be UTF-8") from exc
    if not source.endswith("\n"):
        raise ManifestError(f"SHA256SUMS for {target} must end with a newline")
    checksums: dict[str, str] = {}
    for line_number, line in enumerate(source.splitlines(), start=1):
        match = re.fullmatch(r"([0-9a-f]{64}) [ *](.+)", line)
        if match is None:
            raise ManifestError(
                f"SHA256SUMS for {target} has a malformed line {line_number}"
            )
        digest, relative = match.groups()
        normalized = Path(relative)
        if (
            normalized.is_absolute()
            or relative == "SHA256SUMS"
            or "\\" in relative
            or normalized.as_posix() != relative
            or any(part in ("", ".", "..") for part in normalized.parts)
        ):
            raise ManifestError(
                f"SHA256SUMS for {target} has an unsafe path on line {line_number}"
            )
        if relative in checksums:
            raise ManifestError(
                f"SHA256SUMS for {target} repeats {relative!r}"
            )
        checksums[relative] = digest
    if not checksums:
        raise ManifestError(f"SHA256SUMS for {target} is empty")
    return checksums


def _validate_version_map(payload: bytes, *, version: str, target: str) -> None:
    """Bind the exact embedded map bytes to the release manifest version."""

    try:
        document = tomllib.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise ManifestError(
            f"version-map.toml for {target} must be valid UTF-8 TOML"
        ) from exc
    release_version = document.get("release_version")
    if not isinstance(release_version, str):
        raise ManifestError(
            f"version-map.toml for {target} must declare a string release_version"
        )
    if release_version != version:
        raise ManifestError(
            f"version-map.toml for {target} does not match the release manifest version"
        )


def build_manifest(
    artifacts_dir: Path,
    *,
    version: str,
    commit: str,
    repository: str,
    ref: str,
) -> dict[str, object]:
    """Return the closed manifest for exactly the five reviewed candidates."""

    _validate_metadata(version, commit, repository, ref)
    if artifacts_dir.is_symlink() or not artifacts_dir.is_dir():
        raise ManifestError("artifacts directory must be a real directory")
    expected_dirs = {
        f"sorafs-cli-{version}-{target}": target for target in TARGETS
    }
    actual_dirs = {
        path.name
        for path in artifacts_dir.iterdir()
        if path.is_dir() and not path.is_symlink()
    }
    if actual_dirs != set(expected_dirs):
        missing = sorted(set(expected_dirs) - actual_dirs)
        unexpected = sorted(actual_dirs - set(expected_dirs))
        raise ManifestError(
            "candidate directory inventory mismatch "
            f"(missing={missing}, unexpected={unexpected})"
        )
    unexpected_roots = [
        path.name
        for path in artifacts_dir.iterdir()
        if path.name not in expected_dirs
    ]
    if unexpected_roots:
        raise ManifestError(
            f"unexpected entries in artifacts directory: {sorted(unexpected_roots)}"
        )

    entries: list[dict[str, object]] = []
    common_target_digests: dict[str, str] | None = None
    for target in TARGETS:
        directory_name = f"sorafs-cli-{version}-{target}"
        candidate_dir = artifacts_dir / directory_name
        files = _candidate_files(candidate_dir, target)
        relative_files = {
            path.relative_to(candidate_dir).as_posix(): path for path in files
        }
        expected_files = required_candidate_payload_paths(version, target) | {
            "SHA256SUMS"
        }
        if set(relative_files) != expected_files:
            missing = sorted(expected_files - set(relative_files))
            unexpected = sorted(set(relative_files) - expected_files)
            raise ManifestError(
                f"candidate file inventory mismatch for {target} "
                f"(missing={missing}, unexpected={unexpected})"
            )
        checksum_path = relative_files.get("SHA256SUMS")
        if checksum_path is None:
            raise ManifestError(f"candidate for {target} is missing SHA256SUMS")
        checksum_payload = _read_bounded_regular(
            checksum_path,
            f"SHA256SUMS for {target}",
            MAX_CHECKSUM_MANIFEST_BYTES,
        )
        declared = _parse_checksums(checksum_payload, target)
        covered = set(relative_files) - {"SHA256SUMS"}
        if set(declared) != covered:
            missing = sorted(covered - set(declared))
            unexpected = sorted(set(declared) - covered)
            raise ManifestError(
                f"SHA256SUMS coverage mismatch for {target} "
                f"(missing={missing}, unexpected={unexpected})"
            )

        file_hashes: dict[str, tuple[str, int]] = {}
        for relative, path in relative_files.items():
            label = f"candidate file {directory_name}/{relative}"
            if relative == "version-map.toml":
                version_map = _read_bounded_regular(
                    path,
                    label,
                    MAX_VERSION_MAP_BYTES,
                )
                _validate_version_map(version_map, version=version, target=target)
                digest, size = hashlib.sha256(version_map).hexdigest(), len(version_map)
            else:
                digest, size = _hash_regular(path, label)
            if size == 0:
                raise ManifestError(
                    f"candidate file {directory_name}/{relative} must not be empty"
                )
            file_hashes[relative] = (digest, size)
            if relative != "SHA256SUMS" and declared[relative] != digest:
                raise ManifestError(
                    f"SHA256SUMS digest mismatch for {directory_name}/{relative}"
                )
        observed_common_digests = {
            relative: file_hashes[relative][0]
            for relative in COMMON_TARGET_FILES
        }
        if common_target_digests is None:
            common_target_digests = observed_common_digests
        elif observed_common_digests != common_target_digests:
            drifted = sorted(
                relative
                for relative in COMMON_TARGET_FILES
                if observed_common_digests[relative]
                != common_target_digests[relative]
            )
            raise ManifestError(
                "release-wide files differ across native candidates: "
                + ", ".join(drifted)
            )
        for relative in sorted(file_hashes):
            digest, size = file_hashes[relative]
            entries.append(
                {
                    "format": "file",
                    "kind": "sorafs-cli-release-candidate",
                    "path": f"{directory_name}/{relative}",
                    "profile": target,
                    "sha256": digest,
                    "size": size,
                    "target": target,
                }
            )
            if len(entries) > MAX_TOTAL_FILES:
                raise ManifestError(
                    f"release inventory exceeds the {MAX_TOTAL_FILES}-file limit"
                )

    return {
        "artifact_count": len(entries),
        "artifacts": entries,
        "commit": commit,
        "ref": ref,
        "repository": repository,
        "schema": SCHEMA,
        "targets": list(TARGETS),
        "version": version,
    }


def canonical_payload(manifest: dict[str, object]) -> bytes:
    payload = (
        json.dumps(manifest, indent=2, sort_keys=True, allow_nan=False) + "\n"
    ).encode("utf-8")
    if len(payload) > MAX_RELEASE_MANIFEST_BYTES:
        raise ManifestError(
            f"release manifest exceeds the {MAX_RELEASE_MANIFEST_BYTES}-byte limit"
        )
    return payload


def _write_new(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, 0o644)
    except OSError as exc:
        raise ManifestError(f"cannot create output manifest {path}: {exc}") from exc
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("create", "check"))
    parser.add_argument("--artifacts-dir", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--ref", required=True)
    output = parser.add_mutually_exclusive_group(required=True)
    output.add_argument("--output")
    output.add_argument("--manifest")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    if args.command == "create" and args.output is None:
        print("error: create requires --output", file=sys.stderr)
        return 2
    if args.command == "check" and args.manifest is None:
        print("error: check requires --manifest", file=sys.stderr)
        return 2
    try:
        manifest = build_manifest(
            Path(args.artifacts_dir),
            version=args.version,
            commit=args.commit,
            repository=args.repository,
            ref=args.ref,
        )
        payload = canonical_payload(manifest)
        if args.command == "create":
            _write_new(Path(args.output), payload)
        else:
            committed = _read_bounded_regular(
                Path(args.manifest),
                "committed release manifest",
                MAX_RELEASE_MANIFEST_BYTES,
            )
            if committed != payload:
                raise ManifestError(
                    "committed release manifest does not match the exact candidate inventory"
                )
    except (ManifestError, OSError) as exc:
        print(f"error: invalid SoraFS CLI release manifest: {exc}", file=sys.stderr)
        return 1

    print(
        json.dumps(
            {
                "artifact_count": manifest["artifact_count"],
                "manifest_sha256": hashlib.sha256(payload).hexdigest(),
                "schema": SCHEMA,
                "status": "created" if args.command == "create" else "matched",
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
