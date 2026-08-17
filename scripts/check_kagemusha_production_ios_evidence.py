#!/usr/bin/env python3
"""Verify fail-closed production iOS Kagemusha evidence."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys
from typing import Optional


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import kagemusha_candidate_ios_evidence as candidate_evidence
import kagemusha_production_ios_evidence as production_evidence


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, help="Production signed evidence JSON.")
    parser.add_argument("--artifact-root", required=True, help="Exact owner-private raw lab tree.")
    parser.add_argument("--production-policy", required=True, help="Canonical production iOS policy JSON.")
    parser.add_argument("--trusted-key-id", required=True, help="Exact trusted lab signer key id.")
    parser.add_argument("--trusted-public-key", required=True, help="Trusted Ed25519 public PEM.")
    args = parser.parse_args(argv)
    errors = production_evidence.validate_production_signed_evidence(
        Path(args.evidence),
        Path(args.artifact_root),
        args.trusted_key_id,
        Path(args.trusted_public_key),
        Path(args.production_policy),
        candidate_evidence,
    )
    if errors:
        for error in errors:
            print(f"[kagemusha-production-ios-evidence] ERROR: {error}", file=sys.stderr)
        return 1
    print("[kagemusha-production-ios-evidence] production evidence is valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
