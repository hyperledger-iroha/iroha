#!/usr/bin/env python3
"""Check a standalone physical production App Attest capture."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys

import kagemusha_candidate_ios_evidence as candidate_evidence
import kagemusha_production_app_attest_capture as capture_evidence
import kagemusha_production_ios_evidence as production_evidence


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--capture", required=True)
    parser.add_argument("--request", required=True)
    parser.add_argument("--production-policy", required=True)
    parser.add_argument("--capture-app-code-sign-measurements")
    parser.add_argument("--platform-evidence-output")
    parser.add_argument("--summary-output")
    args = parser.parse_args()

    errors, platform_evidence, summary = capture_evidence.validate_capture(
        Path(args.capture),
        Path(args.request),
        Path(args.production_policy),
        candidate_evidence,
        production_evidence,
        capture_app_code_sign_measurements_path=(
            Path(args.capture_app_code_sign_measurements)
            if args.capture_app_code_sign_measurements is not None
            else None
        ),
    )
    if errors:
        for error in errors:
            print(f"[kagemusha-app-attest-capture] ERROR: {error}", file=sys.stderr)
        return 1
    assert platform_evidence is not None
    assert summary is not None
    for output_text, value in (
        (args.platform_evidence_output, platform_evidence),
        (args.summary_output, summary),
    ):
        if output_text is None:
            continue
        output = Path(output_text)
        if not output.is_absolute() or output.exists() or not output.parent.is_dir():
            print(
                "[kagemusha-app-attest-capture] ERROR: outputs must be new absolute files "
                "beneath existing directories",
                file=sys.stderr,
            )
            return 1
        candidate_evidence.write_private_json(output, value)
    print("[kagemusha-app-attest-capture] production App Attest capture is valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
