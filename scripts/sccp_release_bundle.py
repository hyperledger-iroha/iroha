#!/usr/bin/env python3
"""Build a deterministic, hash-bound SCCP V1 public release bundle."""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from sccp_release_common import (
    SccpReleaseError,
    canonical_json_file_bytes,
    ensure_new_output_parent,
    load_evidence_file,
    load_trust_policy,
    make_bundle_index,
    public_error,
    readiness_summary,
    verify_evidence_artifacts,
    verify_rust_lane_evidence,
    verify_rust_release_signatures,
    write_new_file,
)


def _create_parent_directories(root: Path, relative: str) -> None:
    current = root
    for part in relative.split("/")[:-1]:
        current = current / part
        if current.exists():
            if current.is_symlink() or not current.is_dir():
                raise SccpReleaseError("bundle output path contains a non-directory component")
            continue
        current.mkdir(mode=0o755)


def build_bundle(
    evidence_path: Path,
    artifact_root: Path,
    output_dir: Path,
    trust_policy_path: Path,
    trust_policy: dict[str, object],
    trust_policy_bytes: bytes,
    rust_validator: Path,
) -> dict[str, object]:
    """Validate inputs and publish one new fail-closed, never-overwritten bundle."""

    evidence, evidence_bytes = load_evidence_file(evidence_path, trust_policy)
    verify_rust_release_signatures(
        trust_policy_path=trust_policy_path,
        trust_policy=trust_policy,
        trust_policy_bytes=trust_policy_bytes,
        evidence_path=evidence_path,
        evidence=evidence,
        evidence_bytes=evidence_bytes,
        validator_path=rust_validator,
        environment=(
            "test-fixture"
            if trust_policy["environment"] == "test-fixture"
            else "production"
        ),
    )
    artifacts = verify_evidence_artifacts(evidence, artifact_root)
    _, executable_hash = verify_rust_lane_evidence(
        evidence,
        artifact_root,
        rust_validator,
        trust_policy,
        trust_policy_path=trust_policy_path,
        evidence_path=evidence_path,
        environment=(
            "test-fixture"
            if trust_policy["environment"] == "test-fixture"
            else "production"
        ),
    )
    index = make_bundle_index(
        evidence,
        evidence_bytes,
        trust_policy,
        trust_policy_bytes,
        executable_hash,
    )
    parent = ensure_new_output_parent(output_dir)
    try:
        output_dir.mkdir(mode=0o755)
    except FileExistsError:
        raise SccpReleaseError(
            "output directory already exists; SCCP bundle creation never overwrites"
        ) from None
    except OSError as error:
        raise SccpReleaseError("bundle output directory could not be reserved safely") from error
    write_new_file(output_dir / "evidence.json", evidence_bytes)
    for entry in evidence["artifacts"]:
        relative = entry["path"]
        _create_parent_directories(output_dir, relative)
        write_new_file(output_dir.joinpath(*relative.split("/")), artifacts[relative])
    # The index is the completion record and is deliberately written last. A
    # crash leaves an incomplete reserved directory for explicit operator
    # inspection; a verifier cannot mistake it for a bundle, a later build
    # cannot overwrite it, and cleanup never risks following a swapped path.
    write_new_file(output_dir / "bundle.json", canonical_json_file_bytes(index))
    directory_fd = os.open(output_dir, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
    parent_fd = os.open(parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(parent_fd)
    finally:
        os.close(parent_fd)
    return index


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Build a new SCCP release bundle from externally signed public evidence."
    )
    parser.add_argument("evidence", type=Path, help="Canonical signed evidence JSON.")
    parser.add_argument(
        "--trust-policy",
        type=Path,
        required=True,
        help="Canonical external production release trust policy.",
    )
    parser.add_argument(
        "--rust-validator",
        type=Path,
        required=True,
        help="Canonical sccp_release_evidence Rust validator executable.",
    )
    parser.add_argument(
        "--artifact-root",
        type=Path,
        help="Direct artifact root. Defaults to the evidence file's parent.",
    )
    parser.add_argument("--output-dir", type=Path, required=True, help="New bundle directory.")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        trust_policy, trust_policy_bytes = load_trust_policy(args.trust_policy)
        index = build_bundle(
            args.evidence,
            args.artifact_root or args.evidence.parent,
            args.output_dir,
            args.trust_policy,
            trust_policy,
            trust_policy_bytes,
            args.rust_validator,
        )
        evidence, _ = load_evidence_file(args.evidence, trust_policy)
        readiness = readiness_summary(
            evidence, bundle_root_hash=index["bundle_root_hash_hex"]
        )
        summary = {
            "schema": "sccp-release-bundle-build-v1",
            "release_id": index["release_id"],
            "bundle_root_hash_hex": index["bundle_root_hash_hex"],
            "output_dir": args.output_dir.name,
            "ready": readiness["ready"],
        }
        sys.stdout.buffer.write(canonical_json_file_bytes(summary))
        return 0
    except (OSError, SccpReleaseError, ValueError) as error:
        print(f"SCCP release bundle creation failed: {public_error(error)}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
