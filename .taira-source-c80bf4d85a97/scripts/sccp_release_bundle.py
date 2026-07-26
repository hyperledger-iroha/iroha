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
    create_new_directory_at,
    ensure_new_output_parent,
    load_evidence_file,
    load_trust_policy,
    make_bundle_index,
    open_direct_directory,
    open_directory_at,
    public_error,
    readiness_summary,
    verify_evidence_artifacts,
    verify_production_semantic_artifacts,
    verify_rust_lane_evidence,
    verify_rust_release_signatures,
    verify_rust_semantic_proofs,
    write_new_file_at,
)


def _write_relative_output(root_descriptor: int, relative: str, data: bytes) -> None:
    """Write one already-validated path beneath a stable output-directory fd."""

    parts = relative.split("/")
    current = os.dup(root_descriptor)
    try:
        for part in parts[:-1]:
            try:
                os.mkdir(part, mode=0o755, dir_fd=current)
            except FileExistsError:
                pass
            except (OSError, TypeError, NotImplementedError) as error:
                raise SccpReleaseError(
                    "bundle output parent could not be created safely"
                ) from error
            child = open_directory_at(
                current,
                part,
                label="bundle output parent",
            )
            os.close(current)
            current = child
        write_new_file_at(
            current,
            parts[-1],
            data,
            label=f"bundle output {relative}",
        )
        os.fsync(current)
    finally:
        os.close(current)


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

    # Reject an unsafe or already-existing destination before reading evidence
    # or invoking the authenticated validator. Output-path safety is an
    # independent precondition and must not be masked by a later input error.
    parent = ensure_new_output_parent(output_dir)
    evidence, evidence_bytes = load_evidence_file(evidence_path, trust_policy)
    artifacts = verify_evidence_artifacts(evidence, artifact_root)
    semantic_records = verify_production_semantic_artifacts(
        evidence, artifacts, trust_policy
    )
    _, executable_hash = verify_rust_release_signatures(
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
    verify_rust_semantic_proofs(
        evidence=evidence,
        evidence_bytes=evidence_bytes,
        artifact_root=artifact_root,
        semantic_records=semantic_records,
        trust_policy=trust_policy,
        trust_policy_bytes=trust_policy_bytes,
        trust_policy_path=trust_policy_path,
        evidence_path=evidence_path,
        validator_path=rust_validator,
        expected_executable_hash=executable_hash,
    )
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
    parent_descriptor = open_direct_directory(parent, label="output parent")
    try:
        output_descriptor = create_new_directory_at(
            parent_descriptor,
            output_dir.name,
            label="output directory",
        )
        try:
            _write_relative_output(output_descriptor, "evidence.json", evidence_bytes)
            for entry in evidence["artifacts"]:
                relative = entry["path"]
                _write_relative_output(output_descriptor, relative, artifacts[relative])
            # The index is the completion record and is deliberately written
            # last. A crash leaves a reserved but unverifiable directory.
            _write_relative_output(
                output_descriptor,
                "bundle.json",
                canonical_json_file_bytes(index),
            )
            os.fsync(output_descriptor)
            # Detect a parent-directory rename/swap before reporting success.
            reopened = open_directory_at(
                parent_descriptor,
                output_dir.name,
                label="completed output directory",
            )
            try:
                expected = os.fstat(output_descriptor)
                actual = os.fstat(reopened)
                if (expected.st_dev, expected.st_ino) != (actual.st_dev, actual.st_ino):
                    raise SccpReleaseError(
                        "output directory changed while publishing the SCCP bundle"
                    )
            finally:
                os.close(reopened)
        finally:
            os.close(output_descriptor)
        os.fsync(parent_descriptor)
    finally:
        os.close(parent_descriptor)
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
