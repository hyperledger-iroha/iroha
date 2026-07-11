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
    canonical_json_file_bytes,
    enumerate_direct_files,
    load_trust_policy,
    parse_json_bytes,
    public_error,
    read_relative_file,
    readiness_summary,
    reject_secret_material,
    require_canonical_json_file,
    sha256_hex,
    validate_bundle_index,
    validate_evidence,
    verify_production_semantic_artifacts,
    verify_rust_lane_evidence,
    verify_rust_release_signatures,
    verify_rust_semantic_proofs,
)


def _entry_limit(kind: str) -> int:
    if kind == "release-evidence":
        return MAX_EVIDENCE_BYTES
    return artifact_limit(kind)


def verify_bundle(
    bundle_dir: Path,
    trust_policy_path: Path,
    trust_policy: dict[str, Any],
    trust_policy_bytes: bytes,
    rust_validator: Path,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Verify index, inventory, hashes, evidence, provenance, and exact file set."""

    index_bytes = read_relative_file(
        bundle_dir, "bundle.json", label="bundle index", maximum=MAX_INDEX_BYTES
    )
    index_value = parse_json_bytes(index_bytes, label="bundle index", maximum=MAX_INDEX_BYTES)
    require_canonical_json_file(index_bytes, index_value, label="bundle index")
    index = validate_bundle_index(index_value)
    if (
        index["trust_policy_id"] != trust_policy["policy_id"]
        or index["trust_policy_sha256_hex"] != sha256_hex(trust_policy_bytes)
    ):
        raise SccpReleaseError(
            "bundle trust-policy commitment does not match the external production policy"
        )

    actual_paths = enumerate_direct_files(bundle_dir)
    expected_paths = tuple(sorted(("bundle.json", *(entry["path"] for entry in index["entries"]))))
    if actual_paths != expected_paths:
        raise SccpReleaseError("bundle file inventory does not exactly match bundle.json")

    entry_bytes: dict[str, bytes] = {}
    for entry in index["entries"]:
        data = read_relative_file(
            bundle_dir,
            entry["path"],
            label=f"bundle entry {entry['path']}",
            maximum=_entry_limit(entry["kind"]),
        )
        if len(data) != entry["size_bytes"] or sha256_hex(data) != entry["sha256_hex"]:
            raise SccpReleaseError(
                f"bundle entry {entry['path']} does not match its indexed size and SHA-256"
            )
        reject_secret_material(data, label=f"bundle entry {entry['path']}")
        entry_bytes[entry["path"]] = data

    evidence_bytes = entry_bytes["evidence.json"]
    evidence_value = parse_json_bytes(
        evidence_bytes, label="bundled release evidence", maximum=MAX_EVIDENCE_BYTES
    )
    require_canonical_json_file(evidence_bytes, evidence_value, label="bundled release evidence")
    evidence = validate_evidence(evidence_value, trust_policy)
    if evidence["release_id"] != index["release_id"]:
        raise SccpReleaseError("bundle release_id does not match signed release evidence")
    if evidence["validator"] != index["validator"]:
        raise SccpReleaseError("bundle validator identity does not match signed release evidence")
    if (
        index["validator_executable_sha256_hex"]
        != evidence["validator"]["executable_sha256_hex"]
    ):
        raise SccpReleaseError(
            "bundle executable commitment does not match signed release evidence"
        )

    indexed_artifacts = {
        entry["path"]: entry for entry in index["entries"] if entry["kind"] != "release-evidence"
    }
    signed_artifacts = {entry["path"]: entry for entry in evidence["artifacts"]}
    if indexed_artifacts != signed_artifacts:
        raise SccpReleaseError("bundle artifact inventory does not equal the signed evidence inventory")
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
        "--rust-validator",
        type=Path,
        required=True,
        help="Canonical sccp_release_evidence Rust validator executable.",
    )
    parser.add_argument("--quiet", action="store_true", help="Verify without a JSON summary.")
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
            args.rust_validator,
        )
        if not args.quiet:
            summary = readiness_summary(
                evidence, bundle_root_hash=index["bundle_root_hash_hex"]
            )
            sys.stdout.buffer.write(canonical_json_file_bytes(summary))
        return 0
    except (OSError, SccpReleaseError, ValueError) as error:
        print(f"SCCP release bundle verification failed: {public_error(error)}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
