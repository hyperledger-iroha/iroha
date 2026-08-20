#!/usr/bin/env python3
"""Inspect the exact broker-free four-file input set for a local Taira reset."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    from . import prepare_taira_empty_reset_bundle as reset_bundle
except ImportError:
    import prepare_taira_empty_reset_bundle as reset_bundle


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--reviewed-input-dir", type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--dpn-validator-release-commit", required=True)
    parser.add_argument("--cargo-lock-sha256", required=True)
    parser.add_argument("--workspace-source-manifest-sha256", required=True)
    parser.add_argument("--local-testnet-source-closure-sha256", required=True)
    parser.add_argument("--local-testnet-python-sha256", required=True)
    parser.add_argument("--digest-only", action="store_true")
    args = parser.parse_args(argv)
    reset_bundle.require_local_testnet_source_runtime(
        args.local_testnet_source_closure_sha256,
        args.local_testnet_python_sha256,
        entrypoint="scripts/inspect_taira_local_reviewed_inputs.py",
    )
    source_commit = reset_bundle.require_source_commit(args.source_commit)
    dpn_commit = reset_bundle.require_source_commit(
        args.dpn_validator_release_commit
    )
    cargo_lock_sha256 = reset_bundle.require_sha256(
        args.cargo_lock_sha256, "Cargo.lock SHA-256"
    )
    workspace_sha256 = reset_bundle.require_sha256(
        args.workspace_source_manifest_sha256,
        "workspace source manifest SHA-256",
    )
    _, manifest, digest = reset_bundle._inspect_local_testnet_reviewed_inputs(
        args.reviewed_input_dir,
        source_commit=source_commit,
        dpn_validator_release_commit=dpn_commit,
        cargo_lock_sha256=cargo_lock_sha256,
        workspace_source_manifest_sha256=workspace_sha256,
    )
    if args.digest_only:
        print(digest)
    else:
        sys.stdout.buffer.write(
            reset_bundle.canonical_json_bytes({**manifest, "sha256": digest})
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, reset_bundle.ReleaseArtifactError, RuntimeError) as error:
        print(f"local Taira reviewed inputs refused: {error}", file=sys.stderr)
        raise SystemExit(1)
