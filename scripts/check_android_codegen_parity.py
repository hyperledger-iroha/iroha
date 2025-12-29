#!/usr/bin/env python3
"""Verify Android codegen manifests match the recorded metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from copy import deepcopy
from pathlib import Path
from typing import Any, Dict, List, Optional


def _canonical_sha256(payload: Dict[str, Any]) -> str:
    normalized = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(normalized).hexdigest()


def _sorted_entries(entries: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    return sorted(entries, key=lambda entry: entry.get("discriminant", ""))


def _load_json(path: Path) -> Dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc


def _load_metadata(path: Path) -> Dict[str, Any]:
    metadata = _load_json(path)
    for key in ("instruction_manifest", "builder_index"):
        if key not in metadata:
            raise ValueError(f"metadata missing `{key}` block: {path}")
    return metadata


def _compare(actual: int | str, expected: Optional[int | str], label: str, errors: List[str]) -> None:
    if expected is None:
        return
    if actual != expected:
        errors.append(f"{label} mismatch: expected {expected}, got {actual}")


def _build_summary(
    manifest_path: Path,
    builder_path: Path,
    metadata: Dict[str, Any],
    errors: List[str],
) -> Dict[str, Any]:
    manifest_payload = _load_json(manifest_path)
    builder_payload = _load_json(builder_path)

    manifest_entries = manifest_payload.get("instructions")
    builder_entries = builder_payload.get("builders")
    if not isinstance(manifest_entries, list):
        raise ValueError(f"{manifest_path} missing `instructions` array")
    if not isinstance(builder_entries, list):
        raise ValueError(f"{builder_path} missing `builders` array")

    manifest_entry_count = len(manifest_entries)
    builder_entry_count = len(builder_entries)
    manifest_canonical = deepcopy(manifest_payload)
    builder_canonical = deepcopy(builder_payload)
    manifest_canonical["instructions"] = _sorted_entries(manifest_entries)
    builder_canonical["builders"] = _sorted_entries(builder_entries)
    manifest_canonical["generated_at"] = ""
    builder_canonical["generated_at"] = ""

    manifest_sha = _canonical_sha256(manifest_canonical)
    builder_sha = _canonical_sha256(builder_canonical)

    manifest_meta = metadata["instruction_manifest"]
    builder_meta = metadata["builder_index"]

    _compare(manifest_sha, manifest_meta.get("sha256"), "instruction_manifest sha256", errors)
    _compare(manifest_entry_count, manifest_meta.get("entry_count"), "instruction_manifest entry_count", errors)
    _compare(builder_sha, builder_meta.get("sha256"), "builder_index sha256", errors)
    _compare(builder_entry_count, builder_meta.get("entry_count"), "builder_index entry_count", errors)

    status = "ok" if not errors else "error"
    return {
        "status": status,
        "instruction_manifest": {
            "path": str(manifest_path),
            "sha256": manifest_sha,
            "entry_count": manifest_entry_count,
            "expected_sha256": manifest_meta.get("sha256"),
            "expected_entry_count": manifest_meta.get("entry_count"),
        },
        "builder_index": {
            "path": str(builder_path),
            "sha256": builder_sha,
            "entry_count": builder_entry_count,
            "expected_sha256": builder_meta.get("sha256"),
            "expected_entry_count": builder_meta.get("entry_count"),
        },
        "errors": errors,
    }


def _write_summary(path: Path, summary: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(summary, indent=2), encoding="utf-8")


def parse_args(argv: Optional[List[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check Android codegen manifests against recorded metadata"
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("target-codex/android_codegen/instruction_manifest.json"),
        help="Path to instruction_manifest.json (default: %(default)s)",
    )
    parser.add_argument(
        "--builder-index",
        type=Path,
        default=Path("target-codex/android_codegen/builder_index.json"),
        help="Path to builder_index.json (default: %(default)s)",
    )
    parser.add_argument(
        "--metadata",
        type=Path,
        default=Path("docs/source/sdk/android/generated/codegen_manifest_metadata.json"),
        help="Recorded metadata JSON (default: %(default)s)",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Write a parity summary JSON file.",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress success output.",
    )
    return parser.parse_args(argv)


def main(argv: Optional[List[str]] = None) -> int:
    args = parse_args(argv)

    missing_paths = [path for path in (args.manifest, args.builder_index, args.metadata) if not path.exists()]
    if missing_paths:
        for missing in missing_paths:
            print(f"[android-codegen] missing required file: {missing}", file=sys.stderr)
        return 2

    errors: List[str] = []
    try:
        metadata = _load_metadata(args.metadata)
        summary = _build_summary(args.manifest, args.builder_index, metadata, errors)
    except ValueError as exc:
        print(f"[android-codegen] {exc}", file=sys.stderr)
        return 2

    if args.json_out is not None:
        _write_summary(args.json_out, summary)

    if summary["status"] == "ok":
        if not args.quiet:
            print(
                "[android-codegen] manifests match recorded metadata "
                f"(sha={summary['instruction_manifest']['sha256'][:12]}..., "
                f"entries={summary['instruction_manifest']['entry_count']})"
            )
        return 0

    print("[android-codegen] parity check failed:", file=sys.stderr)
    for error in summary["errors"]:
        print(f"  - {error}", file=sys.stderr)
    print("Re-run `make android-codegen-docs` and update the metadata/docs before retrying.", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
