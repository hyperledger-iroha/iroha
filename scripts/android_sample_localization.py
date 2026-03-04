#!/usr/bin/env python3
"""
Annotate Android sample manifests with localized screenshot metadata.

This helper scans the docs screenshot archive for each sample/locale,
records present assets (with hashes) and surfaces missing coverage so
`scripts/check_android_samples.sh` can flag localization gaps.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import OrderedDict
from pathlib import Path
from typing import Iterable, Sequence

SUPPORTED_IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".webp"}


def _compute_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def _collect_locale_files(
    sample: str,
    screenshots_root: Path,
    locale: str,
    repo_root: Path,
) -> dict:
    """Return metadata for the files belonging to a given locale."""
    locale_dir = screenshots_root / sample / locale
    files = []

    if locale_dir.exists():
        for file_path in sorted(locale_dir.rglob("*")):
            if not file_path.is_file():
                continue
            if file_path.suffix.lower() not in SUPPORTED_IMAGE_EXTENSIONS:
                continue
            rel_path = file_path.relative_to(repo_root)
            files.append(
                OrderedDict(
                    [
                        ("path", str(rel_path)),
                        ("sha256", _compute_sha256(file_path)),
                        ("size_bytes", file_path.stat().st_size),
                    ]
                )
            )

    if not locale_dir.exists():
        status = "missing_dir"
    elif not files:
        # Directory exists but no usable screenshots yet.
        status = "missing_assets"
    else:
        status = "present"

    return {
        "locale": locale,
        "status": status,
        "count": len(files),
        "files": files,
    }


def annotate_manifest_with_localization(
    manifest_path: Path,
    sample: str,
    screenshots_root: Path,
    locales: Sequence[str],
    repo_root: Path,
) -> OrderedDict:
    """Update the manifest JSON with localized screenshot metadata."""
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    data = json.loads(manifest_path.read_text(encoding="utf-8"), object_pairs_hook=OrderedDict)

    localized = OrderedDict()
    missing = []
    for locale in locales:
        entry = _collect_locale_files(sample, screenshots_root, locale, repo_root)
        localized[locale] = entry
        if entry["status"] != "present":
            missing.append(locale)

    metadata = data.get("metadata")
    if metadata is None:
        metadata = OrderedDict()
        data["metadata"] = metadata

    metadata["localized_screenshots"] = OrderedDict(
        locales=localized,
        missing=missing,
        required_locales=list(locales),
    )

    manifest_path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    return data


def _parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True, help="Path to sample_manifest.json")
    parser.add_argument("--sample", required=True, help="Sample slug (operator-console, retail-wallet, ...)")
    parser.add_argument("--screenshots-root", type=Path, required=True, help="Root of docs sample screenshots")
    parser.add_argument("--locales", nargs="+", required=True, help="Locales to track (e.g., en ja he)")
    parser.add_argument("--repo-root", type=Path, required=True, help="Repository root for relative paths")
    return parser.parse_args(argv)


def main(argv: Iterable[str] | None = None) -> None:
    args = _parse_args(argv)
    annotate_manifest_with_localization(
        manifest_path=args.manifest,
        sample=args.sample,
        screenshots_root=args.screenshots_root,
        locales=args.locales,
        repo_root=args.repo_root,
    )


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    main()
