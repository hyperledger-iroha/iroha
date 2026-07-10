#!/usr/bin/env python3
"""Render readiness from verified SCCP V1 evidence or a verified bundle."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

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


def _markdown(summary: dict[str, object]) -> str:
    lines = [
        "# SCCP V1 release readiness",
        "",
        f"- Release: `{summary['release_id']}`",
        f"- Ready: **{'yes' if summary['ready'] else 'no'}**",
    ]
    root = summary["bundle_root_hash_hex"]
    if root is not None:
        lines.append(f"- Bundle root: `{root}`")
    lines.extend(("", "## Exact lane status", ""))
    for lane in summary["lanes"]:
        lines.append(
            f"- `{lane['counterparty_profile']}`: inbound `{lane['inbound_status']}`, "
            f"outbound `{lane['outbound_status']}` (required: inbound "
            f"`{lane['required_inbound_status']}`, outbound "
            f"`{lane['required_outbound_status']}`)"
        )
    if summary["blocking_capabilities"]:
        lines.extend(("", "## Blocking capabilities", ""))
        lines.extend(f"- `{blocker}`" for blocker in summary["blocking_capabilities"])
    lines.extend(("", "Solana and TON are outside SCCP V1 and are rejected.", ""))
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Render SCCP readiness from verified public inputs.")
    parser.add_argument("source", type=Path, help="Canonical evidence JSON or bundle directory.")
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
        help="Artifact root for a standalone evidence file. Defaults to its parent.",
    )
    parser.add_argument("--format", choices=("json", "markdown"), default="json")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        trust_policy, trust_policy_bytes = load_trust_policy(args.trust_policy)
        if args.source.is_dir() and not args.source.is_symlink():
            evidence, index = verify_bundle(
                args.source,
                args.trust_policy,
                trust_policy,
                trust_policy_bytes,
                args.rust_validator,
            )
            root_hash = index["bundle_root_hash_hex"]
        else:
            evidence, evidence_bytes = load_evidence_file(args.source, trust_policy)
            verify_rust_release_signatures(
                trust_policy_path=args.trust_policy,
                trust_policy=trust_policy,
                trust_policy_bytes=trust_policy_bytes,
                evidence_path=args.source,
                evidence=evidence,
                evidence_bytes=evidence_bytes,
                validator_path=args.rust_validator,
                environment="production",
            )
            artifact_root = args.artifact_root or args.source.parent
            verify_evidence_artifacts(evidence, artifact_root)
            verify_rust_lane_evidence(
                evidence,
                artifact_root,
                args.rust_validator,
                trust_policy,
                trust_policy_path=args.trust_policy,
                evidence_path=args.source,
                environment="production",
            )
            root_hash = None
        summary = readiness_summary(evidence, bundle_root_hash=root_hash)
        if args.format == "json":
            sys.stdout.buffer.write(canonical_json_file_bytes(summary))
        else:
            sys.stdout.write(_markdown(summary))
        return 0
    except (OSError, SccpReleaseError, ValueError) as error:
        print(f"SCCP release readiness failed: {public_error(error)}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
