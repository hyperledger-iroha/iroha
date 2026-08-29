#!/usr/bin/env python3
"""Independently verify a deterministic SCCP V1 public release bundle."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

from sccp_release_common import (
    MAX_EVIDENCE_BYTES,
    MAX_INDEX_BYTES,
    SccpReleaseError,
    artifact_limit,
    artifact_stream_limit,
    canonical_json_file_bytes,
    enumerate_direct_files,
    load_trust_policy,
    parse_json_bytes,
    public_error,
    read_relative_file,
    readiness_summary,
    require_canonical_json_file,
    require_verified_validator_build_time,
    sha256_hex,
    validate_bundle_index,
    validate_bundle_index_against_evidence,
    validate_evidence,
    verify_production_semantic_artifacts,
    verify_relative_file_stream,
    verify_rust_lane_evidence,
    verify_rust_release_signatures,
    verify_rust_semantic_proofs,
    verify_validator_build_release,
)


def _entry_limit(entry: dict[str, Any], *, production: bool) -> int:
    if entry["kind"] == "release-evidence":
        return MAX_EVIDENCE_BYTES
    if production:
        return artifact_stream_limit(entry)
    return artifact_limit(entry["kind"])


def verify_bundle(
    bundle_dir: Path,
    trust_policy_path: Path,
    trust_policy: dict[str, Any],
    trust_policy_bytes: bytes,
    rust_validator: Path | None,
    *,
    validator_build_release: Path | None = None,
    trusted_validator_builder_policy_sha256: str | None = None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Verify index, inventory, hashes, evidence, provenance, and exact file set."""

    verified_validator_executable_hash: str | None = None
    verified_validator_built_at_unix_ms: int | None = None
    if trust_policy["environment"] == "production":
        if rust_validator is not None:
            raise SccpReleaseError(
                "production bundle verification cannot accept an unauthenticated validator path"
            )
        if (
            validator_build_release is None
            or trusted_validator_builder_policy_sha256 is None
        ):
            raise SccpReleaseError(
                "production bundle verification requires a verified validator build"
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
            "test-fixture bundle verification requires its fixture validator"
        )

    index_bytes = read_relative_file(
        bundle_dir, "bundle.json", label="bundle index", maximum=MAX_INDEX_BYTES
    )
    index_value = parse_json_bytes(
        index_bytes, label="bundle index", maximum=MAX_INDEX_BYTES
    )
    require_canonical_json_file(index_bytes, index_value, label="bundle index")
    index = validate_bundle_index(index_value)
    if index["trust_policy_id"] != trust_policy["policy_id"] or index[
        "trust_policy_sha256_hex"
    ] != sha256_hex(trust_policy_bytes):
        raise SccpReleaseError(
            "bundle trust-policy commitment does not match the external production policy"
        )

    actual_paths = enumerate_direct_files(bundle_dir)
    expected_paths = tuple(
        sorted(("bundle.json", *(entry["path"] for entry in index["entries"])))
    )
    if actual_paths != expected_paths:
        raise SccpReleaseError(
            "bundle file inventory does not exactly match bundle.json"
        )

    entry_bytes: dict[str, bytes] = {}
    for entry in index["entries"]:
        data = verify_relative_file_stream(
            bundle_dir,
            entry["path"],
            label=f"bundle entry {entry['path']}",
            maximum=_entry_limit(
                entry, production=index["environment"] == "production"
            ),
            expected_size=entry["size_bytes"],
            expected_sha256_hex=entry["sha256_hex"],
            capture_maximum=MAX_EVIDENCE_BYTES
            if entry["kind"] == "release-evidence"
            else 16 * 1024 * 1024,
        )
        entry_bytes[entry["path"]] = data

    evidence_bytes = entry_bytes["evidence.json"]
    evidence_value = parse_json_bytes(
        evidence_bytes, label="bundled release evidence", maximum=MAX_EVIDENCE_BYTES
    )
    require_canonical_json_file(
        evidence_bytes, evidence_value, label="bundled release evidence"
    )
    evidence = validate_evidence(evidence_value, trust_policy)
    if verified_validator_built_at_unix_ms is not None:
        require_verified_validator_build_time(
            evidence,
            verified_validator_built_at_unix_ms,
        )
    validate_bundle_index_against_evidence(index, evidence, evidence_bytes)
    semantic_records = verify_production_semantic_artifacts(
        evidence, entry_bytes, trust_policy
    )
    _, executable_hash = verify_rust_release_signatures(
        trust_policy_path=trust_policy_path,
        trust_policy=trust_policy,
        trust_policy_bytes=trust_policy_bytes,
        evidence_path=bundle_dir / "evidence.json",
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
        artifact_root=bundle_dir,
        semantic_records=semantic_records,
        trust_policy=trust_policy,
        trust_policy_bytes=trust_policy_bytes,
        trust_policy_path=trust_policy_path,
        evidence_path=bundle_dir / "evidence.json",
        validator_path=rust_validator,
        expected_executable_hash=executable_hash,
    )
    _, executable_hash = verify_rust_lane_evidence(
        evidence,
        bundle_dir,
        rust_validator,
        trust_policy,
        trust_policy_path=trust_policy_path,
        evidence_path=bundle_dir / "evidence.json",
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
    if executable_hash != index["validator_executable_sha256_hex"]:
        raise SccpReleaseError(
            "Rust validator executable does not match the bundle commitment"
        )
    return evidence, index


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify one SCCP release bundle without trusting the bundle builder."
    )
    parser.add_argument("bundle_dir", type=Path, help="Direct bundle directory.")
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
        "--quiet", action="store_true", help="Verify without a JSON summary."
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        trust_policy, trust_policy_bytes = load_trust_policy(args.trust_policy)
        evidence, index = verify_bundle(
            args.bundle_dir,
            args.trust_policy,
            trust_policy,
            trust_policy_bytes,
            None,
            validator_build_release=args.validator_build_release,
            trusted_validator_builder_policy_sha256=(
                args.trusted_validator_builder_policy_sha256
            ),
        )
        if not args.quiet:
            summary = readiness_summary(
                evidence, bundle_root_hash=index["bundle_root_hash_hex"]
            )
            sys.stdout.buffer.write(canonical_json_file_bytes(summary))
        return 0
    except (OSError, SccpReleaseError, ValueError) as error:
        print(
            f"SCCP release bundle verification failed: {public_error(error)}",
            file=sys.stderr,
        )
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
