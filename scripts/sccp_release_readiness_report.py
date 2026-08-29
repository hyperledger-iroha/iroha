#!/usr/bin/env python3
"""Render readiness from verified SCCP V1 evidence or a verified bundle."""

from __future__ import annotations

import argparse
import concurrent.futures
import http.client
import secrets
import ssl
import sys
import urllib.parse
from pathlib import Path

from sccp_release_common import (
    MAX_FRESHNESS_HEAD_BYTES,
    SccpReleaseError,
    canonical_json_file_bytes,
    freshness_request,
    live_readiness_summary,
    load_evidence_file,
    load_trust_policy,
    parse_json_bytes,
    public_error,
    readiness_summary,
    require_canonical_json_file,
    require_verified_validator_build_time,
    select_valid_freshness_quorum,
    verify_evidence_artifacts,
    verify_production_semantic_artifacts,
    verify_rust_lane_evidence,
    verify_rust_release_signatures,
    verify_rust_semantic_proofs,
    verify_validator_build_release,
)
from sccp_verify_release_bundle import verify_bundle


def _markdown(summary: dict[str, object]) -> str:
    lines = [
        "# SCCP V1 release readiness",
        "",
        f"- Release: `{summary['release_id']}`",
        f"- Ready: **{'yes' if summary['ready'] else 'no'}**",
        f"- Validation mode: `{summary['mode']}`",
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
    lines.extend(
        (
            "",
            "Final V1 admits exactly Ethereum mainnet, BSC mainnet, TRON mainnet, and "
            "TON mainnet as external SCCP lanes; every retired or non-mainnet profile "
            "is unrepresentable.",
            "",
        )
    )
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Render SCCP readiness from verified public inputs."
    )
    parser.add_argument(
        "source", type=Path, help="Canonical evidence JSON or bundle directory."
    )
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
        help="Artifact root for a standalone evidence file. Defaults to its parent.",
    )
    parser.add_argument("--format", choices=("json", "markdown"), default="json")
    parser.add_argument(
        "--historical",
        action="store_true",
        help="Validate final-V1 integrity without contacting freshness authorities; never emits ready.",
    )
    return parser


def _fetch_freshness_head(
    authority: dict[str, object], payload: bytes
) -> dict[str, object]:
    """POST one nonce-bound request without redirects, credentials, or caching."""

    parsed = urllib.parse.urlsplit(str(authority["https_endpoint"]))
    connection = http.client.HTTPSConnection(
        parsed.hostname,
        parsed.port or 443,
        timeout=10,
        context=ssl.create_default_context(),
    )
    target = parsed.path
    try:
        connection.request(
            "POST",
            target,
            body=payload,
            headers={
                "Accept": "application/json",
                "Cache-Control": "no-store",
                "Content-Type": "application/json",
            },
        )
        response = connection.getresponse()
        content_type = (
            response.getheader("Content-Type", "").split(";", 1)[0].strip().lower()
        )
        if response.status != 200 or content_type != "application/json":
            raise SccpReleaseError(
                "freshness authority returned a non-canonical response"
            )
        body = response.read(MAX_FRESHNESS_HEAD_BYTES + 1)
        if len(body) > MAX_FRESHNESS_HEAD_BYTES or response.read(1):
            raise SccpReleaseError(
                "freshness authority response exceeds its byte bound"
            )
    finally:
        connection.close()
    value = parse_json_bytes(
        body, label="freshness authority response", maximum=MAX_FRESHNESS_HEAD_BYTES
    )
    require_canonical_json_file(body, value, label="freshness authority response")
    if type(value) is not dict:
        raise SccpReleaseError("freshness authority response must be an object")
    return value


def _live_freshness_state(
    policy: dict[str, object], bundle_root_hash: str
) -> dict[str, object]:
    nonce = secrets.token_bytes(32)
    request = freshness_request(
        nonce=nonce,
        policy_root_sha256_hex=str(policy["policy_root_sha256_hex"]),
        bundle_root_hash_hex=bundle_root_hash,
    )
    payload = canonical_json_file_bytes(request)
    heads: list[dict[str, object]] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as executor:
        futures = [
            executor.submit(_fetch_freshness_head, authority, payload)
            for authority in policy["freshness_authorities"]
        ]
        for future in futures:
            try:
                heads.append(future.result())
            except (OSError, SccpReleaseError, ValueError, http.client.HTTPException):
                continue
    if len(heads) < 2:
        raise SccpReleaseError(
            "fewer than two independent freshness authorities responded"
        )
    return select_valid_freshness_quorum(heads, policy=policy, request=request)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        trust_policy, trust_policy_bytes = load_trust_policy(args.trust_policy)
        source_is_bundle = args.source.is_dir() and not args.source.is_symlink()
        if source_is_bundle:
            evidence, index = verify_bundle(
                args.source,
                args.trust_policy,
                trust_policy,
                trust_policy_bytes,
                None,
                validator_build_release=args.validator_build_release,
                trusted_validator_builder_policy_sha256=(
                    args.trusted_validator_builder_policy_sha256
                ),
            )
            root_hash = index["bundle_root_hash_hex"]
        else:
            (
                rust_validator,
                validator_build_hashes,
                validator_built_at_unix_ms,
            ) = verify_validator_build_release(
                args.validator_build_release,
                trust_policy,
                trusted_policy_sha256=(args.trusted_validator_builder_policy_sha256),
            )
            verified_validator_executable_hash = validator_build_hashes[
                "validator_executable_sha256_hex"
            ]
            evidence, evidence_bytes = load_evidence_file(args.source, trust_policy)
            require_verified_validator_build_time(
                evidence,
                validator_built_at_unix_ms,
            )
            artifact_root = args.artifact_root or args.source.parent
            artifacts = verify_evidence_artifacts(evidence, artifact_root)
            semantic_records = verify_production_semantic_artifacts(
                evidence, artifacts, trust_policy
            )
            _, executable_hash = verify_rust_release_signatures(
                trust_policy_path=args.trust_policy,
                trust_policy=trust_policy,
                trust_policy_bytes=trust_policy_bytes,
                evidence_path=args.source,
                evidence=evidence,
                evidence_bytes=evidence_bytes,
                validator_path=rust_validator,
                environment="production",
            )
            if executable_hash != verified_validator_executable_hash:
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
                trust_policy_path=args.trust_policy,
                evidence_path=args.source,
                validator_path=rust_validator,
                expected_executable_hash=executable_hash,
            )
            _, executable_hash = verify_rust_lane_evidence(
                evidence,
                artifact_root,
                rust_validator,
                trust_policy,
                trust_policy_path=args.trust_policy,
                evidence_path=args.source,
                environment="production",
            )
            if executable_hash != verified_validator_executable_hash:
                raise SccpReleaseError(
                    "executed Rust validator differs from its verified build release"
                )
            root_hash = None
        if args.historical:
            summary = readiness_summary(evidence, bundle_root_hash=root_hash)
        else:
            if not source_is_bundle or root_hash is None:
                raise SccpReleaseError(
                    "live readiness requires a verified bundle root; use --historical for standalone evidence"
                )
            freshness_state = _live_freshness_state(trust_policy, root_hash)
            summary = live_readiness_summary(
                evidence,
                bundle_root_hash=root_hash,
                policy=trust_policy,
                freshness_state=freshness_state,
            )
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
