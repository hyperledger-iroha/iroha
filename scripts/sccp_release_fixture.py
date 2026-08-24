#!/usr/bin/env python3
"""Prove that the pinned, externally signed SCCP protocol-v3 fixture is retired.

The fixture remains checked in only as negative evidence. It cannot be validated,
bundled, verified, or resealed into a first-release SCCP artifact.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from sccp_release_common import (
    SccpReleaseError,
    canonical_json_file_bytes,
    load_trust_policy,
    public_error,
)


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "fixtures" / "sccp" / "release_evidence_v1"
FIXTURE_POLICY = FIXTURE_ROOT / "test-trust-policy.json"
FIXTURE_RELEASE_ID = "sccp-v1-typed-fixture-20260711"
FIXTURE_POLICY_ID = "sccp-v1-fixture-policy-20260711"


def _retired_protocol_versions() -> set[int]:
    """Return the protocol revisions recorded by the immutable negative fixture."""

    value = json.loads(FIXTURE_POLICY.read_text(encoding="utf-8"))
    return {
        proof["sora_finality_anchor"]["protocol_version"]
        for proof in value["proof_systems"]
    }


def reject_retired_fixture() -> dict[str, object]:
    """Require the current V1 policy loader to reject the pinned v3 fixture."""

    if _retired_protocol_versions() != {3}:
        raise SccpReleaseError("retired SCCP fixture no longer records exactly protocol v3")
    try:
        load_trust_policy(FIXTURE_POLICY, allow_test_policy=True)
    except SccpReleaseError as error:
        if "protocol_version" not in str(error):
            raise SccpReleaseError(
                "retired SCCP fixture failed before the protocol-v3 rejection boundary"
            ) from error
    else:
        raise SccpReleaseError("retired SCCP protocol-v3 fixture was accepted")
    return {
        "fixture_only": True,
        "policy_id": FIXTURE_POLICY_ID,
        "release_id": FIXTURE_RELEASE_ID,
        "rejected": True,
        "retired_protocol_version": 3,
        "schema": "sccp-retired-v3-fixture-rejection-v1",
    }


def build_parser() -> argparse.ArgumentParser:
    """Build the single-purpose negative-fixture command line."""

    parser = argparse.ArgumentParser(
        description="Assert that first-release SCCP rejects the pinned protocol-v3 fixture."
    )
    parser.add_argument(
        "command",
        choices=("reject",),
        help="Prove that the current policy loader rejects the retired fixture.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Run the explicit retired-fixture rejection check."""

    build_parser().parse_args(argv)
    try:
        result = reject_retired_fixture()
        sys.stdout.buffer.write(canonical_json_file_bytes(result))
        return 0
    except (OSError, SccpReleaseError, ValueError) as error:
        print(f"SCCP retired fixture rejection failed: {public_error(error)}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
