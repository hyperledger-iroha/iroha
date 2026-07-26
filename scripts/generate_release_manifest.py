#!/usr/bin/env python3
"""Generate the closed canonical aggregate release manifest."""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from release_artifact_contract import (
    RELEASE_MANIFEST_SCHEMA,
    RELEASE_MANIFEST_SCHEMA_VERSION,
    ReleaseArtifactError,
    canonical_json_bytes,
    exclusive_write_bytes,
    format_source_date_epoch,
    parse_artifact_spec,
    parse_sha256sums,
    parse_source_date_epoch,
    scan_inventory_paths,
    stable_hash_relative,
    validate_release_manifest,
)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifacts-dir",
        required=True,
        help="Closed artifact root containing canonical SHA256SUMS",
    )
    parser.add_argument("--version", required=True)
    parser.add_argument(
        "--commit",
        required=True,
        help="Exact lowercase hexadecimal release commit",
    )
    parser.add_argument(
        "--source-date-epoch",
        required=True,
        help="Canonical nonnegative decimal release epoch",
    )
    parser.add_argument("--os-tag", required=True)
    parser.add_argument("--arch", required=True)
    parser.add_argument(
        "--artifact",
        action="append",
        required=True,
        help=(
            "Expected artifact as profile:target:kind:format:relative-path. "
            "Repeat once for every non-SHA256SUMS file under --artifacts-dir."
        ),
    )
    parser.add_argument("--output", required=True)
    return parser.parse_args(argv)


def build_release_manifest(args: argparse.Namespace) -> dict[str, object]:
    artifact_root = Path(args.artifacts_dir)
    output = Path(args.output)
    artifact_root_absolute = Path(os.path.abspath(artifact_root))
    output_absolute = Path(os.path.abspath(output))
    try:
        output_absolute.relative_to(artifact_root_absolute)
    except ValueError:
        pass
    else:
        raise ReleaseArtifactError(
            "release manifest output must be outside the closed artifact root"
        )
    for ancestor in (output_absolute.parent, *output_absolute.parent.parents):
        try:
            if ancestor.samefile(artifact_root_absolute):
                raise ReleaseArtifactError(
                    "release manifest output directory aliases the artifact root"
                )
        except FileNotFoundError:
            continue
        except OSError as exc:
            raise ReleaseArtifactError(
                f"failed to compare release output and artifact root: {exc}"
            ) from exc
    descriptors = [parse_artifact_spec(raw) for raw in args.artifact]
    expected_paths = [str(descriptor["path"]) for descriptor in descriptors]
    if len(set(expected_paths)) != len(expected_paths):
        raise ReleaseArtifactError("release inventory contains duplicate artifact paths")

    scanned_before = scan_inventory_paths(
        artifact_root,
        ignored={"SHA256SUMS"},
    )
    if set(scanned_before) != set(expected_paths):
        missing = sorted(set(expected_paths) - set(scanned_before))
        extra = sorted(set(scanned_before) - set(expected_paths))
        raise ReleaseArtifactError(
            f"closed release inventory mismatch: missing={missing}, extra={extra}"
        )

    checksums = parse_sha256sums(artifact_root)
    if set(checksums) != set(expected_paths):
        missing = sorted(set(expected_paths) - set(checksums))
        extra = sorted(set(checksums) - set(expected_paths))
        raise ReleaseArtifactError(
            f"canonical SHA256SUMS inventory mismatch: missing={missing}, extra={extra}"
        )

    rows: list[dict[str, object]] = []
    captured_files: dict[str, object] = {}
    for descriptor in sorted(
        descriptors,
        key=lambda row: (
            str(row["path"]),
            str(row["profile"]),
            str(row["target"]),
            str(row["kind"]),
            str(row["format"]),
        ),
    ):
        path = str(descriptor["path"])
        file_info = stable_hash_relative(artifact_root, path)
        if checksums[path] != file_info.sha256:
            raise ReleaseArtifactError(
                f"SHA256SUMS digest mismatch for {path}: "
                f"listed={checksums[path]} computed={file_info.sha256}"
            )
        captured_files[path] = file_info
        rows.append(
            {
                **descriptor,
                "sha256": file_info.sha256,
                "size": file_info.size,
            }
        )

    scanned_after = scan_inventory_paths(
        artifact_root,
        ignored={"SHA256SUMS"},
    )
    if scanned_after != scanned_before:
        raise ReleaseArtifactError(
            "release artifact inventory changed while the manifest was generated"
        )
    if parse_sha256sums(artifact_root) != checksums:
        raise ReleaseArtifactError(
            "canonical SHA256SUMS changed while the manifest was generated"
        )
    for path, before in captured_files.items():
        if stable_hash_relative(artifact_root, path) != before:
            raise ReleaseArtifactError(
                f"release artifact {path!r} changed while the manifest was generated"
            )

    epoch = parse_source_date_epoch(args.source_date_epoch)
    return validate_release_manifest(
        {
            "schema": RELEASE_MANIFEST_SCHEMA,
            "schema_version": RELEASE_MANIFEST_SCHEMA_VERSION,
            "version": args.version,
            "commit": args.commit,
            "source_date_epoch": epoch,
            "built_at": format_source_date_epoch(epoch),
            "os": args.os_tag,
            "arch": args.arch,
            "artifacts": rows,
        }
    )


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        manifest = build_release_manifest(args)
        exclusive_write_bytes(Path(args.output), canonical_json_bytes(manifest))
    except ReleaseArtifactError as exc:
        print(f"release manifest error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
