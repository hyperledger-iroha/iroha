#!/usr/bin/env python3
"""Validate, assemble, and sign physical-iOS Kagemusha candidate evidence."""

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
    parser.add_argument(
        "--artifact-root",
        required=True,
        help="Exact owner-private raw evidence tree.",
    )
    parser.add_argument("--private-key", required=True, help="Ed25519 private PEM.")
    parser.add_argument("--public-key", required=True, help="Ed25519 public PEM.")
    parser.add_argument(
        "--signer-key-id",
        required=True,
        help="Stable signer key identifier embedded in the evidence.",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Signed JSON output outside --artifact-root.",
    )
    args = parser.parse_args(argv)
    artifact_root = Path(args.artifact_root)
    output = Path(args.output)
    try:
        try:
            artifact_root_resolved = artifact_root.resolve(strict=True)
            output_resolved = output.resolve(strict=False)
        except OSError as error:
            raise ios_evidence.EvidenceError(
                "artifact root or signed output path could not be resolved"
            ) from error
        try:
            output_resolved.relative_to(artifact_root_resolved)
        except ValueError:
            pass
        else:
            raise ios_evidence.EvidenceError(
                "signed evidence output must stay outside artifact root"
            )
        evidence = ios_evidence.build_signed_evidence(
            artifact_root,
            Path(args.private_key),
            Path(args.public_key),
            args.signer_key_id,
        )
        ios_evidence.write_private_json(output, evidence)
    except ios_evidence.EvidenceError as error:
        print(f"[kagemusha-ios-evidence] ERROR: {error}", file=sys.stderr)
        return 1
    print(f"[kagemusha-ios-evidence] signed evidence: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
