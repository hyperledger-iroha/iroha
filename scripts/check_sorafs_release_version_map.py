#!/usr/bin/env python3
"""Validate the SoraFS cross-package first-release version map."""

from __future__ import annotations

import argparse
import json
import os
import re
import stat
import sys
import xml.etree.ElementTree as ET
from pathlib import Path, PurePosixPath
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier use the pinned backport.
    import tomli as tomllib


SCHEMA = "sorafs.release.version_map.v1"
SUPPORTED_ECOSYSTEMS = frozenset(
    {"cargo", "gradle-property", "msbuild", "npm", "plain-semver", "python"}
)
SEMVER_RE = re.compile(
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?\Z"
)


def _json_object_without_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Build one JSON object while rejecting ambiguous duplicate members."""

    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("package metadata JSON contains a duplicate key")
        result[key] = value
    return result


def _read_bytes_no_follow(path: Path) -> bytes:
    """Read a regular file without following a final-component symlink."""

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode):
            raise ValueError("version source must be a regular file")
        chunks: list[bytes] = []
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                return b"".join(chunks)
            chunks.append(chunk)
    finally:
        os.close(descriptor)


def _require_regular_repo_file(root: Path, relative: object) -> Path:
    """Return a canonical repository file after rejecting path aliases."""

    if not isinstance(relative, str) or not relative:
        raise ValueError("package path must be a non-empty string")
    if "\\" in relative or "://" in relative or "%" in relative:
        raise ValueError("package path contains a forbidden separator or encoding")
    logical = PurePosixPath(relative)
    if logical.is_absolute() or any(part in {"", ".", ".."} for part in logical.parts):
        raise ValueError("package path must be a canonical repository-relative path")

    current = root
    for index, component in enumerate(logical.parts):
        current = current / component
        try:
            metadata = current.lstat()
        except FileNotFoundError as error:
            raise ValueError("package version source is missing") from error
        if stat.S_ISLNK(metadata.st_mode):
            raise ValueError("package path must not contain symlinks")
        if index + 1 < len(logical.parts):
            if not stat.S_ISDIR(metadata.st_mode):
                raise ValueError("package path parent must be a directory")
        elif not stat.S_ISREG(metadata.st_mode):
            raise ValueError("package version source must be a regular file")
    return current


def _read_declared_version(
    path: Path,
    ecosystem: str,
    version_key: str | None = None,
) -> str:
    """Extract a package version using the package ecosystem's native format."""

    payload = _read_bytes_no_follow(path)
    if ecosystem == "npm":
        document = json.loads(
            payload,
            object_pairs_hook=_json_object_without_duplicate_keys,
        )
        version = document.get("version") if isinstance(document, dict) else None
    elif ecosystem == "cargo":
        document = tomllib.loads(payload.decode("utf-8"))
        package = document.get("package")
        version = package.get("version") if isinstance(package, dict) else None
    elif ecosystem == "python":
        document = tomllib.loads(payload.decode("utf-8"))
        project = document.get("project")
        version = project.get("version") if isinstance(project, dict) else None
    elif ecosystem == "msbuild":
        root = ET.fromstring(payload)
        versions = [
            node.text.strip()
            for node in root.iter()
            if node.tag.rsplit("}", 1)[-1] == "Version" and node.text
        ]
        if len(versions) != 1:
            raise ValueError("msbuild package Version must occur exactly once")
        version = versions[0]
    elif ecosystem == "gradle-property":
        if version_key is None or re.fullmatch(r"[A-Za-z][A-Za-z0-9.]{0,63}", version_key) is None:
            raise ValueError("gradle-property package requires a canonical version_key")
        matches: list[str] = []
        for raw_line in payload.decode("utf-8").splitlines():
            line = raw_line.strip()
            if not line or line.startswith(("#", "!")) or "=" not in line:
                continue
            key, candidate = line.split("=", 1)
            if key.strip() == version_key:
                matches.append(candidate.strip())
        if len(matches) != 1:
            raise ValueError("gradle-property version_key must occur exactly once")
        version = matches[0]
    elif ecosystem == "plain-semver":
        text = payload.decode("ascii")
        candidate = text.removesuffix("\n")
        version = candidate if text in {candidate, f"{candidate}\n"} else None
    else:
        raise ValueError(f"unsupported package ecosystem: {ecosystem}")
    if not isinstance(version, str) or not SEMVER_RE.fullmatch(version):
        raise ValueError(f"{ecosystem} package declares an invalid or missing version")
    return version


def validate_version_map(root: Path, map_relative: str = "release/version-map.toml") -> dict[str, Any]:
    """Validate the version map and return its schema-closed summary."""

    map_path = _require_regular_repo_file(root, map_relative)
    document = tomllib.loads(_read_bytes_no_follow(map_path).decode("utf-8"))
    if set(document) != {"schema_version", "release_version", "packages"}:
        raise ValueError("version map must use the schema-closed top-level contract")
    if document["schema_version"] != 1 or isinstance(document["schema_version"], bool):
        raise ValueError("version map schema_version must equal integer 1")

    release_version = document["release_version"]
    if not isinstance(release_version, str) or not SEMVER_RE.fullmatch(release_version):
        raise ValueError("release_version must be canonical SemVer")
    packages = document["packages"]
    if not isinstance(packages, list) or not packages:
        raise ValueError("version map must declare at least one package")

    normalized: list[dict[str, str]] = []
    identifiers: set[str] = set()
    source_paths: set[str] = set()
    for index, package in enumerate(packages):
        if not isinstance(package, dict):
            raise ValueError(f"package row {index} must use the schema-closed contract")
        ecosystem = package.get("ecosystem")
        expected_fields = {"id", "ecosystem", "path", "version"}
        if ecosystem == "gradle-property":
            expected_fields.add("version_key")
        if set(package) != expected_fields:
            raise ValueError(f"package row {index} must use the schema-closed contract")
        identifier = package["id"]
        relative = package["path"]
        expected_version = package["version"]
        version_key = package.get("version_key")
        if not isinstance(identifier, str) or not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", identifier):
            raise ValueError(f"package row {index} has an invalid id")
        if identifier in identifiers:
            raise ValueError(f"package row {index} duplicates an id")
        identifiers.add(identifier)
        if not isinstance(ecosystem, str) or ecosystem not in SUPPORTED_ECOSYSTEMS:
            raise ValueError(f"package row {index} has an unsupported ecosystem")
        if ecosystem == "gradle-property" and (
            not isinstance(version_key, str)
            or re.fullmatch(r"[A-Za-z][A-Za-z0-9.]{0,63}", version_key) is None
        ):
            raise ValueError(f"package row {index} has an invalid version_key")
        if not isinstance(expected_version, str) or not SEMVER_RE.fullmatch(expected_version):
            raise ValueError(f"package row {index} has an invalid version")
        if not isinstance(relative, str):
            raise ValueError(f"package row {index} has an invalid path")
        source = _require_regular_repo_file(root, relative)
        if relative in source_paths:
            raise ValueError(f"package row {index} duplicates a version source")
        source_paths.add(relative)
        actual_version = _read_declared_version(source, ecosystem, version_key)
        if actual_version != expected_version:
            raise ValueError(f"package row {index} version does not match its source")
        normalized_row = {
            "id": identifier,
            "ecosystem": ecosystem,
            "path": relative,
            "version": expected_version,
        }
        if ecosystem == "gradle-property":
            normalized_row["version_key"] = version_key
        normalized.append(normalized_row)

    if [row["id"] for row in normalized] != sorted(identifiers):
        raise ValueError("package rows must be sorted by id")
    return {
        "schema": SCHEMA,
        "release_version": release_version,
        "package_count": len(normalized),
        "packages": normalized,
    }


def main(argv: list[str] | None = None) -> int:
    """Run the version-map validator."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--map", default="release/version-map.toml", help="repository-relative map path")
    arguments = parser.parse_args(argv)
    repository = Path(__file__).resolve().parent.parent
    try:
        summary = validate_version_map(repository, arguments.map)
    except (OSError, UnicodeError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError, ET.ParseError) as error:
        print(f"error: invalid SoraFS release version map: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
