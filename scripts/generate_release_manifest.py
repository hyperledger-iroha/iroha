#!/usr/bin/env python3
"""
Produce a deterministic JSON manifest describing release artifacts for
Iroha dual-profile builds.

The script expects the aggregated `SHA256SUMS` file created by the
packaging pipeline and records hashes alongside artifact metadata so
downstream publishing jobs can sign and upload bundles consistently.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
from typing import Dict, List


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts-dir", required=True, help="Directory containing artifacts and SHA256SUMS")
    parser.add_argument("--version", required=True, help="Semantic version for this release")
    parser.add_argument("--commit", required=True, help="Short commit hash used for the build")
    parser.add_argument("--built-at", required=True, help="UTC timestamp when artifacts were produced")
    parser.add_argument("--os-tag", required=True, help="Operating system tag (linux, mac, win, ...)")
    parser.add_argument("--arch", required=True, help="Architecture (e.g., x86_64, arm64)")
    parser.add_argument("--output", required=True, help="Path to write the JSON manifest")
    return parser.parse_args()


def read_sha256sums(path: Path) -> Dict[str, str]:
    hashes: Dict[str, str] = {}
    with path.open("r", encoding="utf-8") as fh:
        for line in fh:
            parts = line.strip().split()
            if len(parts) != 2:
                continue
            sha, filename = parts
            hashes[filename] = sha
    return hashes


def build_entries(
    artifacts_dir: Path,
    hashes: Dict[str, str],
    version: str,
    os_tag: str,
    arch: str,
) -> List[Dict[str, str]]:
    entries: List[Dict[str, str]] = []
    for profile in ("iroha2", "iroha3"):
        bundle_name = f"{profile}-{version}-{os_tag}.tar.zst"
        appimage_name = f"{profile}-{version}-{arch}.AppImage"

        bundle_path = artifacts_dir / bundle_name
        if bundle_path.exists():
            sha = hashes.get(bundle_name)
            if not sha:
                raise SystemExit(f"Missing SHA256 entry for {bundle_name}")
            entries.append(
                {
                    "profile": profile,
                    "kind": "bundle",
                    "format": "tar.zst",
                    "path": os.path.normpath(bundle_name),
                    "sha256": sha,
                }
            )

        appimage_path = artifacts_dir / appimage_name
        if appimage_path.exists():
            sha = hashes.get(appimage_name)
            if not sha:
                raise SystemExit(f"Missing SHA256 entry for {appimage_name}")
            entries.append(
                {
                    "profile": profile,
                    "kind": "appimage",
                    "format": "AppImage",
                    "path": os.path.normpath(appimage_name),
                    "sha256": sha,
                }
            )
    return entries


def main() -> int:
    args = parse_args()
    artifacts_dir = Path(args.artifacts_dir)
    sha_path = artifacts_dir / "SHA256SUMS"
    hashes = read_sha256sums(sha_path)

    manifest = {
        "version": args.version,
        "commit": args.commit,
        "built_at": args.built_at,
        "os": args.os_tag,
        "arch": args.arch,
        "artifacts": build_entries(artifacts_dir, hashes, args.version, args.os_tag, args.arch),
    }

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as fh:
        json.dump(manifest, fh, indent=2)
        fh.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
