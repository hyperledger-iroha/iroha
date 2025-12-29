#!/usr/bin/env python3
"""Utilities for extracting Swift Norito fixture archives.

This helper is invoked from ``scripts/swift_fixture_regen.sh`` when the Swift
SDK consumes the canonical Norito fixtures from a Rust-generated archive
instead of copying them directly from the Android resources tree.
"""

from __future__ import annotations

import argparse
import json
import tarfile
import zipfile
from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path
from typing import Iterable, Tuple


class ArchiveError(RuntimeError):
    """Raised when an archive cannot be unpacked or validated."""


@dataclass(frozen=True)
class ExtractResult:
    """Metadata returned after extracting a fixture archive."""

    root: Path
    sha256: str
    kind: str


def _compute_sha256(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def _ensure_clean_directory(path: Path) -> None:
    if path.exists():
        if not path.is_dir():
            raise ArchiveError(f"extraction destination {path} exists and is not a directory")
        try:
            next(path.iterdir())
        except StopIteration:
            return
        raise ArchiveError(f"extraction destination {path} must be empty")
    path.mkdir(parents=True, exist_ok=True)


def _identify_archive(path: Path) -> str:
    if zipfile.is_zipfile(path):
        return "zip"
    if tarfile.is_tarfile(path):
        return "tar"
    raise ArchiveError(f"unsupported archive format for {path}")


def _extract_zip(path: Path, out_dir: Path) -> None:
    with zipfile.ZipFile(path) as archive:
        archive.extractall(out_dir)


def _extract_tar(path: Path, out_dir: Path) -> None:
    with tarfile.open(path, mode="r:*") as archive:
        archive.extractall(out_dir)


def _candidate_score(base: Path, candidate: Path) -> Tuple[int, int, str]:
    norito_count = sum(1 for _ in candidate.glob("*.norito"))
    depth = len(candidate.relative_to(base).parts) if candidate != base else 0
    return (-norito_count, depth, str(candidate))


def _find_fixture_root(out_dir: Path) -> Path:
    manifests = {path.parent for path in out_dir.rglob("transaction_fixtures.manifest.json")}
    candidates = manifests or {path.parent for path in out_dir.rglob("transaction_payloads.json")}
    if not candidates:
        raise ArchiveError("archive does not contain Norito fixture manifests or payloads")
    return min(candidates, key=lambda item: _candidate_score(out_dir, item))


def extract_archive(archive_path: Path, out_dir: Path) -> ExtractResult:
    """Extract ``archive_path`` into ``out_dir`` and locate the fixture root."""
    archive_path = archive_path.resolve()
    if not archive_path.exists():
        raise ArchiveError(f"archive {archive_path} does not exist")
    if not archive_path.is_file():
        raise ArchiveError(f"archive {archive_path} is not a file")

    _ensure_clean_directory(out_dir)
    kind = _identify_archive(archive_path)
    if kind == "zip":
        _extract_zip(archive_path, out_dir)
    else:
        _extract_tar(archive_path, out_dir)

    root = _find_fixture_root(out_dir).resolve()
    digest = _compute_sha256(archive_path)
    return ExtractResult(root=root, sha256=digest, kind=kind)


def _write_metadata(path: Path, result: ExtractResult) -> None:
    payload = {
        "root": str(result.root),
        "sha256": result.sha256,
        "kind": result.kind,
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Extract Swift Norito fixture archives produced by the Rust exporter."
    )
    parser.add_argument("--archive", required=True, type=Path, help="Archive containing Norito fixtures.")
    parser.add_argument(
        "--out-dir", required=True, type=Path, help="Directory where the archive will be extracted."
    )
    parser.add_argument(
        "--meta-out",
        type=Path,
        help="Optional path to write JSON metadata {root, sha256, kind}.",
    )
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    try:
        result = extract_archive(args.archive, args.out_dir)
    except ArchiveError as exc:
        parser.error(str(exc))
    metadata = {"root": str(result.root), "sha256": result.sha256, "kind": result.kind}
    if args.meta_out:
        _write_metadata(args.meta_out, result)
    print(json.dumps(metadata))
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
