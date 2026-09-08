#!/usr/bin/env python3
"""Reject explicit Cargo targets inside an ancestor or neighbouring source tree.

Cargo dep-info can contain relative source paths. Sharing a checkout's target
directory with a frozen checkout can therefore make an older artifact appear
fresh. This read-only path check protects explicit wrapper/Cargo/environment
target selections. It does not qualify external stable lanes or release inputs.
"""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


def workspace_root(manifest: Path, working_directory: Path) -> Path:
    # Let Cargo resolve membership, including explicit workspace pointers and
    # nested independent workspaces. This command does not build or resolve deps.
    result = subprocess.run(
        ["cargo", "locate-project", "--workspace", "--message-format", "plain",
         "--manifest-path", str(manifest)],
        cwd=working_directory, capture_output=True, text=True, check=True, timeout=30,
    )
    located = Path(result.stdout.strip())
    if not located.is_absolute() or not located.is_file():
        raise ValueError("cargo locate-project did not return an existing absolute manifest")
    # Cargo resolves source paths from the manifest's directory even when the
    # manifest file is a symlink. Resolving the file would alias distinct inputs.
    return located.parent.resolve()


def check_target_owner(working_directory: Path, target: Path, manifest: Path) -> None:
    working_directory = working_directory.resolve()
    manifest = working_directory / manifest
    source_root = workspace_root(manifest.parent.resolve() / manifest.name, working_directory)
    target = (working_directory / target).resolve()
    for candidate in (target, *target.parents):
        candidate_manifest = candidate / "Cargo.toml"
        if not candidate_manifest.is_file():
            continue
        owner = workspace_root(candidate_manifest, working_directory)
        if owner != source_root:
            raise ValueError(
                f"Cargo target {target} belongs to another source tree ({owner}); "
                f"use the warm target for {source_root} or a dedicated external build lane"
            )
        return


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--manifest-path", type=Path, default=Path("Cargo.toml"))
    args = parser.parse_args()
    try:
        check_target_owner(args.source_root, args.target_dir, args.manifest_path)
    except subprocess.CalledProcessError as error:
        parser.exit(1, f"error: cannot determine Cargo workspace: {error.stderr.strip()}\n")
    except subprocess.TimeoutExpired:
        parser.exit(1, "error: timed out determining Cargo workspace\n")
    except (OSError, RuntimeError, ValueError) as error:
        parser.exit(1, f"error: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
