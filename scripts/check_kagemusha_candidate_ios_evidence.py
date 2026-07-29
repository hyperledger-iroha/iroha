#!/usr/bin/env python3
"""Verify signed physical-iOS Kagemusha candidate evidence."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys
from typing import Optional

SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import kagemusha_candidate_ios_evidence as ios_evidence


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, help="Signed evidence JSON.")
    parser.add_argument(
        "--artifact-root",
        required=True,
        help="Owner-private raw evidence root; the signed file must be outside it.",
    )
    parser.add_argument(
        "--trusted-key-id",
        required=True,
        help="Exact trusted signer key identifier.",
    )
    parser.add_argument(
        "--trusted-public-key",
        required=True,
        help="Trusted OpenSSL Ed25519 public PEM.",
    )
    args = parser.parse_args(argv)
    errors = ios_evidence.validate_signed_evidence(
        Path(args.evidence),
        Path(args.artifact_root),
        args.trusted_key_id,
        Path(args.trusted_public_key),
    )
    if errors:
        for error in errors:
            print(f"[kagemusha-ios-evidence] ERROR: {error}", file=sys.stderr)
        return 1
    print("[kagemusha-ios-evidence] signed physical-iOS evidence is valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
