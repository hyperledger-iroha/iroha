#!/usr/bin/env python3
"""Exercise the pinned, non-production SCCP typed release fixture.

This is the only entrypoint allowed to consume the deliberately incompatible
test trust-policy schema. It cannot accept caller-selected evidence or policy
paths and therefore cannot turn fixture keys into a production trust root.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from sccp_release_bundle import build_bundle
from sccp_release_common import (
    SccpReleaseError,
    canonical_json_file_bytes,
    load_evidence_file,
    load_trust_policy,
    public_error,
    readiness_summary,
    verify_evidence_artifacts,
    verify_rust_lane_evidence,
    verify_rust_release_signatures,
)
from sccp_verify_release_bundle import verify_bundle


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "fixtures" / "sccp" / "release_evidence_v1"
FIXTURE_EVIDENCE = FIXTURE_ROOT / "evidence.json"
FIXTURE_POLICY = FIXTURE_ROOT / "test-trust-policy.json"
FIXTURE_RELEASE_ID = "sccp-v1-typed-fixture-20260710"
FIXTURE_POLICY_ID = "sccp-v1-fixture-policy-20260710"


def _load_fixture() -> tuple[dict[str, object], bytes, dict[str, object], bytes]:
    policy, policy_bytes = load_trust_policy(FIXTURE_POLICY, allow_test_policy=True)
    if policy["policy_id"] != FIXTURE_POLICY_ID:
        raise SccpReleaseError("pinned fixture trust policy has the wrong policy_id")
    evidence, evidence_bytes = load_evidence_file(FIXTURE_EVIDENCE, policy)
    if evidence["release_id"] != FIXTURE_RELEASE_ID:
        raise SccpReleaseError("pinned fixture evidence has the wrong release_id")
    return policy, policy_bytes, evidence, evidence_bytes


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate, bundle, or verify the pinned non-production SCCP fixture."
    )
    parser.add_argument(
        "--rust-validator",
        type=Path,
        required=True,
        help="Canonical production-built sccp_release_evidence executable.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("validate", help="Validate the pinned fixture and artifacts.")
    build = subparsers.add_parser("build", help="Build a new bundle from the pinned fixture.")
    build.add_argument("--output-dir", type=Path, required=True)
    verify = subparsers.add_parser("verify", help="Verify a bundle using the pinned fixture policy.")
    verify.add_argument("bundle_dir", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        policy, policy_bytes, fixture_evidence, fixture_evidence_bytes = _load_fixture()
        if args.command == "validate":
            verify_rust_release_signatures(
                trust_policy_path=FIXTURE_POLICY,
                trust_policy=policy,
                trust_policy_bytes=policy_bytes,
                evidence_path=FIXTURE_EVIDENCE,
                evidence=fixture_evidence,
                evidence_bytes=fixture_evidence_bytes,
                validator_path=args.rust_validator,
                environment="test-fixture",
            )
            verify_evidence_artifacts(fixture_evidence, FIXTURE_ROOT)
            verify_rust_lane_evidence(
                fixture_evidence,
                FIXTURE_ROOT,
                args.rust_validator,
                policy,
                trust_policy_path=FIXTURE_POLICY,
                evidence_path=FIXTURE_EVIDENCE,
                environment="test-fixture",
            )
            evidence = fixture_evidence
            root_hash = None
        elif args.command == "build":
            index = build_bundle(
                FIXTURE_EVIDENCE,
                FIXTURE_ROOT,
                args.output_dir,
                FIXTURE_POLICY,
                policy,
                policy_bytes,
                args.rust_validator,
            )
            evidence = fixture_evidence
            root_hash = index["bundle_root_hash_hex"]
        else:
            evidence, index = verify_bundle(
                args.bundle_dir,
                FIXTURE_POLICY,
                policy,
                policy_bytes,
                args.rust_validator,
            )
            root_hash = index["bundle_root_hash_hex"]
        summary = readiness_summary(evidence, bundle_root_hash=root_hash)
        summary["fixture_only"] = True
        sys.stdout.buffer.write(canonical_json_file_bytes(summary))
        return 0
    except (OSError, SccpReleaseError, ValueError) as error:
        print(f"SCCP release fixture failed: {public_error(error)}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
