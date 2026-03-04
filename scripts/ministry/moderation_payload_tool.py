#!/usr/bin/env python3
"""Red-team payload helper for bundling fixtures and denylist patches."""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import shutil
from pathlib import Path
from typing import Iterable, Sequence

from . import red_team_common

DEFAULT_SAMPLE_PATH = Path("docs/source/sorafs_gateway_denylist_sample.json")


def _hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def bundle_payloads(
    *,
    scenario_id: str,
    sources: Sequence[Path],
    artifacts_root: Path = red_team_common.DEFAULT_ARTIFACTS_DIR,
    overwrite: bool = False,
) -> Path:
    """Copy payload fixtures into the scenario payload directory with a manifest."""

    if not sources:
        raise ValueError("at least one --source path must be supplied")
    scenario_paths = red_team_common.resolve_paths(
        scenario_id,
        artifacts_root=artifacts_root,
    )
    payload_dir = scenario_paths.ensure_subdir("payloads")
    entries: list[dict[str, str]] = []
    for source in sources:
        if not source.is_file():
            raise FileNotFoundError(f"payload source not found: {source}")
        destination = payload_dir / source.name
        if destination.exists() and not overwrite:
            raise FileExistsError(
                f"{destination} already exists; pass --overwrite to replace it"
            )
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        entries.append(
            {
                "file": destination.name,
                "sha256": _hash_file(destination),
                "size_bytes": destination.stat().st_size,
            }
        )
    manifest = {
        "scenario_id": scenario_id,
        "generated_at": _dt.datetime.utcnow().isoformat(timespec="seconds") + "Z",
        "files": entries,
    }
    manifest_path = payload_dir / "payload_manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return manifest_path


def prepare_denylist_patch(
    *,
    scenario_id: str,
    sample_path: Path = DEFAULT_SAMPLE_PATH,
    entry_json: Iterable[str] = (),
    artifacts_root: Path = red_team_common.DEFAULT_ARTIFACTS_DIR,
) -> Path:
    """Generate a denylist patch file for the scenario."""

    if not sample_path.is_file():
        raise FileNotFoundError(f"sample denylist file missing: {sample_path}")
    entries = json.loads(sample_path.read_text(encoding="utf-8"))
    for raw in entry_json:
        additions = json.loads(raw)
        if isinstance(additions, list):
            entries.extend(additions)
        else:
            entries.append(additions)

    scenario_paths = red_team_common.resolve_paths(
        scenario_id,
        artifacts_root=artifacts_root,
    )
    manifest_dir = scenario_paths.ensure_subdir("manifests")
    patch = {
        "scenario_id": scenario_id,
        "generated_at": _dt.datetime.utcnow().isoformat(timespec="seconds") + "Z",
        "entries": entries,
    }
    patch_path = manifest_dir / "denylist_patch.json"
    patch_path.write_text(json.dumps(patch, indent=2), encoding="utf-8")
    return patch_path


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    payload_parser = subparsers.add_parser(
        "bundle-payloads", help="Copy payload fixtures into the scenario bundle."
    )
    payload_parser.add_argument("--scenario-id", required=True)
    payload_parser.add_argument(
        "--source",
        action="append",
        dest="sources",
        type=Path,
        help="Path to a payload fixture. Repeat for multiple files.",
    )
    payload_parser.add_argument(
        "--artifacts-dir",
        type=Path,
        default=red_team_common.DEFAULT_ARTIFACTS_DIR,
        help="Custom artifacts directory (default: artifacts/ministry/red-team).",
    )
    payload_parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Overwrite existing payload files if present.",
    )

    denylist_parser = subparsers.add_parser(
        "prepare-denylist",
        help="Stage a denylist patch JSON for the scenario.",
    )
    denylist_parser.add_argument("--scenario-id", required=True)
    denylist_parser.add_argument(
        "--sample",
        type=Path,
        default=DEFAULT_SAMPLE_PATH,
        help="Base denylist sample to copy (default: docs/source/sorafs_gateway_denylist_sample.json).",
    )
    denylist_parser.add_argument(
        "--entry-json",
        action="append",
        default=[],
        help="Additional JSON object/array appended to the denylist patch. Repeat as needed.",
    )
    denylist_parser.add_argument(
        "--artifacts-dir",
        type=Path,
        default=red_team_common.DEFAULT_ARTIFACTS_DIR,
        help="Custom artifacts directory (default: artifacts/ministry/red-team).",
    )

    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    if args.command == "bundle-payloads":
        manifest_path = bundle_payloads(
            scenario_id=args.scenario_id,
            sources=args.sources or [],
            artifacts_root=args.artifacts_dir,
            overwrite=args.overwrite,
        )
        print(f"Payload manifest written to {manifest_path}")
    elif args.command == "prepare-denylist":
        patch_path = prepare_denylist_patch(
            scenario_id=args.scenario_id,
            sample_path=args.sample,
            entry_json=args.entry_json,
            artifacts_root=args.artifacts_dir,
        )
        print(f"Denylist patch written to {patch_path}")
    else:
        parser.error(f"unknown command {args.command}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
