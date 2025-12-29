#!/usr/bin/env python3
"""
Package AND7 enablement assets into a reproducible manifest.

Roadmap item AND7 requires that every telemetry enablement run ships a single
bundle with deterministic hashes for the slide deck, lab reports, quiz
summaries, and screenshot/recording manifests. This helper walks the supplied
files or directories, computes SHA-256 digests, optionally copies the assets
into a bundle directory, and writes both JSON and Markdown manifests so
governance packets can reference a stable evidence set.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, List, Sequence


@dataclass(frozen=True)
class AssetSpec:
    """User-supplied asset (file or directory) with a logical label."""

    label: str
    path: Path


@dataclass(frozen=True)
class AssetEntry:
    """Concrete file captured in the bundle manifest."""

    label: str
    source: Path
    relative_path: str
    size_bytes: int
    sha256: str
    bundled_path: str | None


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _iter_files(path: Path) -> Iterable[tuple[Path, str]]:
    if path.is_file():
        yield path, path.name
    elif path.is_dir():
        for file_path in sorted(p for p in path.rglob("*") if p.is_file()):
            yield file_path, file_path.relative_to(path).as_posix()
    else:
        raise FileNotFoundError(f"asset path {path} does not exist")


def _copy_into_bundle(
    source: Path, label: str, relative: str, bundle_root: Path | None, out_dir: Path
) -> str | None:
    if bundle_root is None:
        return None
    destination = bundle_root / label / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    return destination.relative_to(out_dir).as_posix()


def collect_asset_entries(
    specs: Sequence[AssetSpec], bundle_root: Path | None, out_dir: Path
) -> List[AssetEntry]:
    entries: List[AssetEntry] = []
    for spec in specs:
        for file_path, relative in _iter_files(spec.path):
            bundled = _copy_into_bundle(
                file_path, spec.label, relative, bundle_root, out_dir
            )
            entries.append(
                AssetEntry(
                    label=spec.label,
                    source=file_path,
                    relative_path=relative,
                    size_bytes=file_path.stat().st_size,
                    sha256=_sha256(file_path),
                    bundled_path=bundled,
                )
            )
    entries.sort(key=lambda entry: (entry.label, entry.relative_path))
    return entries


def build_manifest(
    entries: Sequence[AssetEntry],
    stamp: str | None,
    note: str | None,
) -> dict:
    generated_at = datetime.now(timezone.utc).isoformat()
    resolved_stamp = stamp or generated_at.replace("-", "")[:8]
    return {
        "stamp": resolved_stamp,
        "generated_at": generated_at,
        "asset_count": len(entries),
        "note": note or "",
        "assets": [
            {
                "label": entry.label,
                "source": entry.source.as_posix(),
                "relative_path": entry.relative_path,
                "size_bytes": entry.size_bytes,
                "sha256": entry.sha256,
                "bundled_path": entry.bundled_path,
            }
            for entry in entries
        ],
    }


def render_markdown(manifest: dict) -> str:
    lines = [
        "# AND7 Enablement Bundle",
        f"Stamp: `{manifest['stamp']}`",
        f"Generated at: `{manifest['generated_at']}`",
        f"Asset count: **{manifest['asset_count']}**",
    ]
    note = manifest.get("note") or ""
    if note:
        lines.append(f"Note: {note}")
    lines.append("")
    lines.append("| Label | Path | SHA-256 | Size (bytes) | Bundled path |")
    lines.append("|-------|------|---------|--------------|--------------|")
    for asset in manifest["assets"]:
        lines.append(
            "| {label} | {path} | `{sha}` | {size} | {bundled} |".format(
                label=asset["label"],
                path=asset["relative_path"],
                sha=asset["sha256"],
                size=asset["size_bytes"],
                bundled=asset.get("bundled_path") or "—",
            )
        )
    return "\n".join(lines)


def _parse_asset_arg(arg: str) -> AssetSpec:
    if "=" not in arg:
        raise ValueError(f"asset must use LABEL=PATH syntax: {arg}")
    label, raw_path = arg.split("=", 1)
    label = label.strip()
    if not label:
        raise ValueError("asset label may not be empty")
    path = Path(raw_path).expanduser()
    return AssetSpec(label=label, path=path)


def _write_outputs(manifest: dict, out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True), encoding="utf-8"
    )
    (out_dir / "manifest.md").write_text(
        render_markdown(manifest), encoding="utf-8"
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Package Android AND7 enablement assets into a manifest."
    )
    parser.add_argument(
        "--asset",
        action="append",
        required=True,
        help="Asset entry in LABEL=PATH form. Files or directories allowed.",
    )
    parser.add_argument(
        "--out-dir",
        required=True,
        type=Path,
        help="Output directory for the manifest (and optional bundled assets).",
    )
    parser.add_argument(
        "--stamp",
        help="Bundle stamp (e.g., 2026-02-18). Defaults to YYYYMMDD derived from now.",
    )
    parser.add_argument(
        "--note",
        help="Optional note to embed in the manifest (e.g., session name or incident id).",
    )
    parser.add_argument(
        "--bundle-assets",
        action="store_true",
        help="Copy assets into <out-dir>/assets/<label>/… alongside the manifest.",
    )
    args = parser.parse_args(argv)

    specs = [_parse_asset_arg(raw) for raw in args.asset]
    bundle_root = args.out_dir / "assets" if args.bundle_assets else None
    entries = collect_asset_entries(specs, bundle_root, args.out_dir)
    manifest = build_manifest(entries, stamp=args.stamp, note=args.note)
    _write_outputs(manifest, args.out_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
