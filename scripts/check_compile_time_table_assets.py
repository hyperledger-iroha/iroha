#!/usr/bin/env python3
"""Verify versioned binary assets decoded into Rust constants at compile time."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


MANIFEST_PATHS = (
    Path(
        "crates/iroha_core/src/privacy_engines/bootle_lantern/"
        "falcon512/assets/manifest.json"
    ),
    Path("crates/iroha_core/src/privacy_engines/zk_x509/assets/manifest.json"),
    Path("crates/iroha_sccp/src/assets/manifest.json"),
    Path("crates/ivm/src/assets/manifest.json"),
    Path("crates/fastpq_isi/src/assets/manifest.json"),
)
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
CONST_DECLARATION_RE = re.compile(
    r"\bconst\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*:"
)
INCLUDE_BYTES_RE = re.compile(
    r"(?:pub(?:\([^)]*\))?\s+)?const\s+"
    r"[A-Za-z_][A-Za-z0-9_]*\s*:\s*&\s*\[\s*u8\s*;\s*"
    r"(?P<length>[0-9_]+)\s*\]\s*=\s*include_bytes!\(\s*"
    r'"(?P<path>[^"\r\n]+)"\s*\)',
    re.MULTILINE,
)
MANIFEST_FIELDS = frozenset(
    {
        "format_version",
        "source_commit",
        "source_slice_hash_scope",
        "canonical_ron",
        "assets",
    }
)
ASSET_FIELDS = frozenset(
    {"path", "byte_length", "sha256", "layout", "source_preimages"}
)
PREIMAGE_FIELDS = frozenset(
    {"path", "constant", "physical_lines", "sha256"}
)


class AssetError(ValueError):
    """One compile-time table asset failed its sealed contract."""


@dataclass(frozen=True)
class AuditCounts:
    """Counts emitted by a successful repository audit."""

    manifests: int
    assets: int
    bytes: int
    source_preimages: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root (default: inferred from this script)",
    )
    return parser.parse_args()


def _object(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise AssetError(f"{context} must be a JSON object")
    return value


def _exact_fields(value: dict[str, Any], expected: frozenset[str], context: str) -> None:
    unknown = sorted(set(value) - expected)
    if unknown:
        raise AssetError(f"{context} has unknown fields: {', '.join(unknown)}")


def _positive_int(value: Any, context: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise AssetError(f"{context} must be a positive integer")
    return value


def _string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value:
        raise AssetError(f"{context} must be a non-empty string")
    return value


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _safe_path(root: Path, base: Path, raw: str, context: str) -> Path:
    candidate = Path(raw)
    if candidate.is_absolute():
        raise AssetError(f"{context} must be repository-relative")
    unresolved = base / candidate
    if unresolved.is_symlink():
        raise AssetError(f"{context} must not be a symlink: {raw}")
    resolved = unresolved.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise AssetError(f"{context} escapes the repository: {raw}") from error
    return resolved


def declaration_slice(source: bytes, constant: str, physical_lines: int) -> bytes:
    """Return one line-count-pinned constant declaration from a historical blob."""

    try:
        text = source.decode("utf-8")
    except UnicodeDecodeError as error:
        raise AssetError("historical Rust source is not UTF-8") from error
    lines = text.splitlines(keepends=True)
    starts = []
    for index, line in enumerate(lines):
        match = CONST_DECLARATION_RE.search(line)
        if match is not None and match.group("name") == constant:
            starts.append(index)
    if len(starts) != 1:
        raise AssetError(
            f"historical Rust source must declare const {constant} exactly once; "
            f"found {len(starts)}"
        )
    start = starts[0]
    end = start + physical_lines
    if end > len(lines):
        raise AssetError(
            f"const {constant} declares {physical_lines} lines beyond source EOF"
        )
    declaration = "".join(lines[start:end]).encode("utf-8")
    if not declaration.endswith(b";\n"):
        raise AssetError(
            f"line-count-pinned const {constant} does not end with semicolon + LF"
        )
    return declaration


def _git_blob(root: Path, commit: str, relative: str) -> bytes:
    try:
        return subprocess.check_output(
            ["git", "show", f"{commit}:{relative}"],
            cwd=root,
            stderr=subprocess.PIPE,
        )
    except subprocess.CalledProcessError as error:
        diagnostic = error.stderr.decode("utf-8", errors="replace").strip()
        raise AssetError(
            f"cannot read historical source {commit}:{relative}: {diagnostic}"
        ) from error


def _crate_root(path: Path, repository_root: Path) -> Path:
    candidate = path.parent
    while candidate != repository_root:
        if (candidate / "Cargo.toml").is_file():
            return candidate
        candidate = candidate.parent
    raise AssetError(f"asset manifest is not inside a Cargo package: {path}")


def _include_consumers(crate_root: Path) -> dict[Path, list[tuple[Path, int]]]:
    consumers: dict[Path, list[tuple[Path, int]]] = {}
    for source in sorted((crate_root / "src").rglob("*.rs")):
        if source.is_symlink() or not source.is_file():
            continue
        text = source.read_text(encoding="utf-8")
        for match in INCLUDE_BYTES_RE.finditer(text):
            included = (source.parent / match.group("path")).resolve()
            length = int(match.group("length").replace("_", ""))
            consumers.setdefault(included, []).append((source, length))
    return consumers


def _verify_canonical_file(
    root: Path, manifest_dir: Path, raw: Any, context: str
) -> None:
    canonical = _object(raw, context)
    _exact_fields(canonical, frozenset({"path", "byte_length", "sha256"}), context)
    path = _safe_path(
        root,
        manifest_dir,
        _string(canonical.get("path"), f"{context}.path"),
        f"{context}.path",
    )
    if not path.is_file():
        raise AssetError(f"{context}.path is missing: {path.relative_to(root)}")
    data = path.read_bytes()
    expected_length = _positive_int(canonical.get("byte_length"), f"{context}.byte_length")
    expected_sha = _string(canonical.get("sha256"), f"{context}.sha256")
    if SHA256_RE.fullmatch(expected_sha) is None:
        raise AssetError(f"{context}.sha256 is not canonical lowercase SHA-256")
    if len(data) != expected_length or _sha256(data) != expected_sha:
        raise AssetError(f"{context} length or SHA-256 does not match its manifest")


def audit_repository(root: Path) -> AuditCounts:
    """Verify every checked-in compile-time table manifest and consumer."""

    root = root.resolve()
    assets_seen: set[Path] = set()
    total_bytes = 0
    total_preimages = 0

    for relative_manifest in MANIFEST_PATHS:
        manifest_path = root / relative_manifest
        if not manifest_path.is_file() or manifest_path.is_symlink():
            raise AssetError(f"required asset manifest is missing: {relative_manifest}")
        manifest = _object(
            json.loads(manifest_path.read_text(encoding="utf-8")),
            str(relative_manifest),
        )
        _exact_fields(manifest, MANIFEST_FIELDS, str(relative_manifest))
        if manifest.get("format_version") != 1:
            raise AssetError(f"{relative_manifest}: format_version must be 1")
        commit = _string(
            manifest.get("source_commit"), f"{relative_manifest}.source_commit"
        )
        if COMMIT_RE.fullmatch(commit) is None:
            raise AssetError(f"{relative_manifest}: source_commit is not a full Git id")
        scope = _string(
            manifest.get("source_slice_hash_scope"),
            f"{relative_manifest}.source_slice_hash_scope",
        )
        if scope != "complete Rust constant declaration including trailing LF":
            raise AssetError(f"{relative_manifest}: unsupported source hash scope")
        if "canonical_ron" in manifest:
            _verify_canonical_file(
                root,
                manifest_path.parent,
                manifest["canonical_ron"],
                f"{relative_manifest}.canonical_ron",
            )

        rows = manifest.get("assets")
        if not isinstance(rows, list) or not rows:
            raise AssetError(f"{relative_manifest}.assets must be a non-empty array")
        crate_root = _crate_root(manifest_path, root)
        consumers = _include_consumers(crate_root)
        manifest_assets: set[Path] = set()
        for index, raw_row in enumerate(rows):
            context = f"{relative_manifest}.assets[{index}]"
            row = _object(raw_row, context)
            _exact_fields(row, ASSET_FIELDS, context)
            asset = _safe_path(
                root,
                manifest_path.parent,
                _string(row.get("path"), f"{context}.path"),
                f"{context}.path",
            )
            if asset.parent != manifest_path.parent or asset.suffix != ".bin":
                raise AssetError(f"{context}.path must name a sibling .bin file")
            if asset in assets_seen or asset in manifest_assets:
                raise AssetError(f"duplicate asset path: {asset.relative_to(root)}")
            if not asset.is_file() or asset.is_symlink():
                raise AssetError(f"asset is missing or not a regular file: {asset}")
            data = asset.read_bytes()
            expected_length = _positive_int(
                row.get("byte_length"), f"{context}.byte_length"
            )
            expected_sha = _string(row.get("sha256"), f"{context}.sha256")
            if SHA256_RE.fullmatch(expected_sha) is None:
                raise AssetError(f"{context}.sha256 is not canonical lowercase SHA-256")
            if len(data) != expected_length:
                raise AssetError(
                    f"{asset.relative_to(root)} has {len(data)} bytes, "
                    f"expected {expected_length}"
                )
            observed_sha = _sha256(data)
            if observed_sha != expected_sha:
                raise AssetError(
                    f"{asset.relative_to(root)} SHA-256 {observed_sha} != {expected_sha}"
                )
            _string(row.get("layout"), f"{context}.layout")
            asset_consumers = consumers.get(asset, [])
            if len(asset_consumers) != 1:
                raise AssetError(
                    f"{asset.relative_to(root)} must have exactly one fixed-size "
                    f"include_bytes! consumer; found {len(asset_consumers)}"
                )
            consumer_path, consumer_length = asset_consumers[0]
            if consumer_length != expected_length:
                raise AssetError(
                    f"{consumer_path.relative_to(root)} declares {consumer_length} bytes "
                    f"for {asset.name}, expected {expected_length}"
                )

            preimages = row.get("source_preimages")
            if not isinstance(preimages, list) or not preimages:
                raise AssetError(f"{context}.source_preimages must be non-empty")
            for preimage_index, raw_preimage in enumerate(preimages):
                preimage_context = f"{context}.source_preimages[{preimage_index}]"
                preimage = _object(raw_preimage, preimage_context)
                _exact_fields(preimage, PREIMAGE_FIELDS, preimage_context)
                historical_path = _safe_path(
                    root,
                    manifest_path.parent,
                    _string(preimage.get("path"), f"{preimage_context}.path"),
                    f"{preimage_context}.path",
                )
                relative_source = historical_path.relative_to(root).as_posix()
                constant = _string(
                    preimage.get("constant"), f"{preimage_context}.constant"
                )
                physical_lines = _positive_int(
                    preimage.get("physical_lines"),
                    f"{preimage_context}.physical_lines",
                )
                expected_preimage_sha = _string(
                    preimage.get("sha256"), f"{preimage_context}.sha256"
                )
                if SHA256_RE.fullmatch(expected_preimage_sha) is None:
                    raise AssetError(
                        f"{preimage_context}.sha256 is not canonical lowercase SHA-256"
                    )
                historical = _git_blob(root, commit, relative_source)
                declaration = declaration_slice(
                    historical, constant, physical_lines
                )
                observed_preimage_sha = _sha256(declaration)
                if observed_preimage_sha != expected_preimage_sha:
                    raise AssetError(
                        f"historical {relative_source}::{constant} SHA-256 "
                        f"{observed_preimage_sha} != {expected_preimage_sha}"
                    )
                total_preimages += 1

            manifest_assets.add(asset)
            assets_seen.add(asset)
            total_bytes += len(data)

        actual_assets: set[Path] = set()
        for path in manifest_path.parent.glob("*.bin"):
            if path.is_symlink() or not path.is_file():
                raise AssetError(
                    f"{path.relative_to(root)} must be a regular non-symlink asset"
                )
            actual_assets.add(path.resolve())
        if actual_assets != manifest_assets:
            missing = sorted(path.name for path in manifest_assets - actual_assets)
            extra = sorted(path.name for path in actual_assets - manifest_assets)
            raise AssetError(
                f"{relative_manifest}: binary inventory mismatch; "
                f"missing={missing}, extra={extra}"
            )

    return AuditCounts(
        manifests=len(MANIFEST_PATHS),
        assets=len(assets_seen),
        bytes=total_bytes,
        source_preimages=total_preimages,
    )


def main() -> int:
    args = parse_args()
    try:
        counts = audit_repository(args.root)
    except (AssetError, OSError, json.JSONDecodeError) as error:
        print(f"compile-time table asset check failed: {error}", file=sys.stderr)
        return 1
    print(
        "compile-time table assets: "
        f"manifests={counts.manifests} assets={counts.assets} "
        f"bytes={counts.bytes} source_preimages={counts.source_preimages}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
