#!/usr/bin/env python3
"""Build a deterministic, hash-bound SCCP V1 public release bundle."""

from __future__ import annotations

import argparse
import hashlib
import os
import stat
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
    require_verified_validator_build_time,
    verify_evidence_artifacts,
    verify_production_semantic_artifacts,
    verify_rust_lane_evidence,
    verify_rust_release_signatures,
    verify_rust_semantic_proofs,
    verify_validator_build_release,
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


def _copy_verified_relative_output(
    output_descriptor: int,
    source_descriptor: int,
    relative: str,
    entry: dict[str, object],
) -> None:
    """Copy one already-indexed artifact with constant memory and rehash it."""

    parts = relative.split("/")
    source = os.dup(source_descriptor)
    destination = os.dup(output_descriptor)
    source_file = None
    destination_file = None
    try:
        for part in parts[:-1]:
            source_child = open_directory_at(source, part, label="bundle source parent")
            os.close(source)
            source = source_child
            try:
                os.mkdir(part, mode=0o755, dir_fd=destination)
            except FileExistsError:
                pass
            destination_child = open_directory_at(
                destination, part, label="bundle output parent"
            )
            os.close(destination)
            destination = destination_child
        read_flags = (
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
        )
        write_flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        source_file = os.open(parts[-1], read_flags, dir_fd=source)
        before = os.fstat(source_file)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_size != entry["size_bytes"]
        ):
            raise SccpReleaseError("bundle source artifact changed before publication")
        destination_file = os.open(parts[-1], write_flags, 0o644, dir_fd=destination)
        digest = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(source_file, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if total > entry["size_bytes"]:
                raise SccpReleaseError(
                    "bundle source artifact exceeded its signed size"
                )
            digest.update(chunk)
            view = memoryview(chunk)
            while view:
                written = os.write(destination_file, view)
                if written <= 0:
                    raise SccpReleaseError("bundle artifact copy made no progress")
                view = view[written:]
        os.fsync(destination_file)
        after = os.fstat(source_file)
        if (
            total != entry["size_bytes"]
            or digest.hexdigest() != entry["sha256_hex"]
            or (
                before.st_dev,
                before.st_ino,
                before.st_size,
                before.st_mtime_ns,
                before.st_ctime_ns,
            )
            != (
                after.st_dev,
                after.st_ino,
                after.st_size,
                after.st_mtime_ns,
                after.st_ctime_ns,
            )
            or os.fstat(destination_file).st_size != total
        ):
            raise SccpReleaseError(
                "bundle artifact changed during streaming publication"
            )
        os.fsync(destination)
    finally:
        if source_file is not None:
            os.close(source_file)
        if destination_file is not None:
            os.close(destination_file)
        os.close(source)
        os.close(destination)


def build_bundle(
    evidence_path: Path,
    artifact_root: Path,
    output_dir: Path,
    trust_policy_path: Path,
    trust_policy: dict[str, object],
    trust_policy_bytes: bytes,
    rust_validator: Path | None,
    *,
    validator_build_release: Path | None = None,
    trusted_validator_builder_policy_sha256: str | None = None,
) -> dict[str, object]:
    """Validate inputs and publish one new fail-closed, never-overwritten bundle."""

    # Reject an unsafe or already-existing destination before reading evidence
    # or invoking the authenticated validator. Output-path safety is an
    # independent precondition and must not be masked by a later input error.
    parent = ensure_new_output_parent(output_dir)
    verified_validator_executable_hash: str | None = None
    verified_validator_built_at_unix_ms: int | None = None
    if trust_policy["environment"] == "production":
        if rust_validator is not None:
            raise SccpReleaseError(
                "production bundle creation cannot accept an unauthenticated validator path"
            )
        if (
            validator_build_release is None
            or trusted_validator_builder_policy_sha256 is None
        ):
            raise SccpReleaseError(
                "production bundle creation requires a verified validator build"
            )
        (
            rust_validator,
            validator_build_hashes,
            verified_validator_built_at_unix_ms,
        ) = verify_validator_build_release(
            validator_build_release,
            trust_policy,
            trusted_policy_sha256=trusted_validator_builder_policy_sha256,
        )
        verified_validator_executable_hash = validator_build_hashes[
            "validator_executable_sha256_hex"
        ]
    elif rust_validator is None:
        raise SccpReleaseError(
            "test-fixture bundle creation requires its fixture validator"
        )
    evidence, evidence_bytes = load_evidence_file(evidence_path, trust_policy)
    if verified_validator_built_at_unix_ms is not None:
        require_verified_validator_build_time(
            evidence,
            verified_validator_built_at_unix_ms,
        )
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
    if (
        verified_validator_executable_hash is not None
        and executable_hash != verified_validator_executable_hash
    ):
        raise SccpReleaseError(
            "executed Rust validator differs from its verified build release"
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
    if (
        verified_validator_executable_hash is not None
        and executable_hash != verified_validator_executable_hash
    ):
        raise SccpReleaseError(
            "executed Rust validator differs from its verified build release"
        )
    index = make_bundle_index(
        evidence,
        evidence_bytes,
        trust_policy,
        trust_policy_bytes,
        executable_hash,
    )
    parent_descriptor = open_direct_directory(parent, label="output parent")
    artifact_root_descriptor = open_direct_directory(
        artifact_root, label="artifact root"
    )
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
                _copy_verified_relative_output(
                    output_descriptor,
                    artifact_root_descriptor,
                    relative,
                    entry,
                )
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
        os.close(artifact_root_descriptor)
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
        "--validator-build-release",
        type=Path,
        required=True,
        help="Authenticated two-party validator-build release directory.",
    )
    parser.add_argument(
        "--trusted-validator-builder-policy-sha256",
        required=True,
        help="Exact lowercase SHA-256 of the trusted validator-builder policy.",
    )
    parser.add_argument(
        "--artifact-root",
        type=Path,
        help="Direct artifact root. Defaults to the evidence file's parent.",
    )
    parser.add_argument(
        "--output-dir", type=Path, required=True, help="New bundle directory."
    )
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
            None,
            validator_build_release=args.validator_build_release,
            trusted_validator_builder_policy_sha256=(
                args.trusted_validator_builder_policy_sha256
            ),
        )
        evidence, _ = load_evidence_file(args.evidence, trust_policy)
        readiness = readiness_summary(
            evidence, bundle_root_hash=index["bundle_root_hash_hex"]
        )
        summary = {
            "schema": "sccp-release-bundle-build-final-v1",
            "release_id": index["release_id"],
            "bundle_root_hash_hex": index["bundle_root_hash_hex"],
            "output_dir": args.output_dir.name,
            "ready": readiness["ready"],
        }
        sys.stdout.buffer.write(canonical_json_file_bytes(summary))
        return 0
    except (OSError, SccpReleaseError, ValueError) as error:
        print(
            f"SCCP release bundle creation failed: {public_error(error)}",
            file=sys.stderr,
        )
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
