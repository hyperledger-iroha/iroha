#!/usr/bin/env python3
"""Validate one exact, signed SCCP V1 release-evidence document."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from sccp_release_common import (
    SccpReleaseError,
    canonical_json_file_bytes,
    evidence_signing_payload,
    load_evidence_file,
    load_trust_policy,
    public_error,
    readiness_summary,
    sha256_hex,
    verify_evidence_artifacts,
    verify_production_semantic_artifacts,
    verify_rust_lane_evidence,
    verify_rust_release_signatures,
    verify_rust_semantic_proofs,
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate strict SCCP V1 release evidence and all signed public artifacts."
    )
    parser.add_argument("evidence", type=Path, help="Canonical sccp-release-evidence-v1 JSON file.")
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
    parser.add_argument(
        "--quiet", action="store_true", help="Validate without writing a summary to stdout."
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        trust_policy, trust_policy_bytes = load_trust_policy(args.trust_policy)
        evidence, evidence_bytes = load_evidence_file(args.evidence, trust_policy)
        artifact_root = args.artifact_root or args.evidence.parent
        artifacts = verify_evidence_artifacts(evidence, artifact_root)
        semantic_records = verify_production_semantic_artifacts(
            evidence, artifacts, trust_policy
        )
        _, signature_validator_hash = verify_rust_release_signatures(
            trust_policy_path=args.trust_policy,
            trust_policy=trust_policy,
            trust_policy_bytes=trust_policy_bytes,
            evidence_path=args.evidence,
            evidence=evidence,
            evidence_bytes=evidence_bytes,
            validator_path=args.rust_validator,
            environment="production",
        )
        verify_rust_semantic_proofs(
            evidence=evidence,
            evidence_bytes=evidence_bytes,
            artifact_root=artifact_root,
            semantic_records=semantic_records,
            trust_policy=trust_policy,
            trust_policy_bytes=trust_policy_bytes,
            trust_policy_path=args.trust_policy,
            evidence_path=args.evidence,
            validator_path=args.rust_validator,
            expected_executable_hash=signature_validator_hash,
        )
        receipts, executable_hash = verify_rust_lane_evidence(
            evidence,
            artifact_root,
            args.rust_validator,
            trust_policy,
            trust_policy_path=args.trust_policy,
            evidence_path=args.evidence,
            environment="production",
        )
        summary = readiness_summary(evidence, bundle_root_hash=None)
        summary["signing_payload_sha256_hex"] = sha256_hex(evidence_signing_payload(evidence))
        summary["validator_executable_sha256_hex"] = executable_hash
        summary["validated_lane_receipts"] = len(receipts)
        if not args.quiet:
            sys.stdout.buffer.write(canonical_json_file_bytes(summary))
        return 0
    except (OSError, SccpReleaseError, ValueError) as error:
        print(f"SCCP release evidence validation failed: {public_error(error)}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
