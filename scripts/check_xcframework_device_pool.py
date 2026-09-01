#!/usr/bin/env python3
"""
Validate the XCFramework smoke harness against the canonical device defaults.

- Ensures the matrix JSON contains the required software/simulator destinations.
- Confirms the smoke harness pulls defaults from the matrix file instead of hard-coding.
- Optionally verifies a dedicated hardware lane and its DeviceKit tag.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Dict, List


def load_matrix(path: Path, *, require_hardware_lane: bool = False) -> Dict[str, str]:
    data = json.loads(path.read_text())
    required = ["iphone_sim", "ipad_sim", "mac_fallback"]
    if require_hardware_lane:
        required.append("strongbox")
    missing: List[str] = [
        key for key in required if not str(data.get(key, "")).strip()
    ]
    if missing:
        raise SystemExit(f"matrix missing required keys: {', '.join(missing)}")
    included = required + (
        ["strongbox"] if "strongbox" in data and not require_hardware_lane else []
    )
    return {key: data[key] for key in included}


def inspect_harness(path: Path, matrix_path: Path) -> Dict[str, object]:
    text = path.read_text()
    matrix_tokens = {
        matrix_path.name,
        matrix_path.as_posix(),
        Path("configs/swift/xcframework_device_matrix.json").as_posix(),
    }
    uses_matrix_path = any(token in text for token in matrix_tokens)
    has_matrix_env = "IOS6_SMOKE_MATRIX_PATH" in text or "MATRIX_PATH" in text
    uses_reader = "read_matrix_value" in text
    required_default_vars = all(
        key in text
        for key in [
            "IOS6_SMOKE_DEST_IPHONE_SIM",
            "IOS6_SMOKE_DEST_IPAD_SIM",
            "IOS6_SMOKE_DEST_MAC_FALLBACK",
        ]
    )
    return {
        "uses_matrix_file": uses_matrix_path and has_matrix_env,
        "uses_reader": uses_reader,
        "has_required_dest_defaults": required_default_vars,
        "has_optional_hardware_dest": "IOS6_SMOKE_DEST_STRONGBOX" in text,
    }


def inspect_pipeline(path: Path) -> Dict[str, object]:
    text = path.read_text()
    match = re.search(r"devicekit:\s*\"?([^\s#\"]+)\"?", text)
    devicekit = match.group(1) if match else None
    return {"devicekit": devicekit}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--matrix",
        default="configs/swift/xcframework_device_matrix.json",
        type=Path,
        help="Path to the XCFramework device matrix JSON file.",
    )
    parser.add_argument(
        "--harness",
        default="scripts/ci/run_xcframework_smoke.sh",
        type=Path,
        help="Path to the XCFramework smoke harness script.",
    )
    parser.add_argument(
        "--pipeline",
        default=".buildkite/xcframework-smoke.yml",
        type=Path,
        help="Path to the Buildkite pipeline definition for the smoke harness.",
    )
    parser.add_argument(
        "--expected-devicekit",
        default="ios14p-strongbox",
        help="Expected DeviceKit label when --require-hardware-lane is selected.",
    )
    parser.add_argument(
        "--require-hardware-lane",
        action="store_true",
        help="Validate the optional physical-device lane and its pipeline binding.",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Optional path to write a JSON summary.",
    )
    args = parser.parse_args()

    if not args.matrix.is_file():
        raise SystemExit(f"matrix file not found: {args.matrix}")
    if not args.harness.is_file():
        raise SystemExit(f"harness file not found: {args.harness}")
    if args.require_hardware_lane and not args.pipeline.is_file():
        raise SystemExit(f"hardware-lane pipeline file not found: {args.pipeline}")

    matrix = load_matrix(args.matrix, require_hardware_lane=args.require_hardware_lane)
    harness = inspect_harness(args.harness, args.matrix)
    pipeline = (
        inspect_pipeline(args.pipeline)
        if args.pipeline.is_file()
        else {"devicekit": None}
    )

    summary = {
        "matrix_path": str(args.matrix),
        "harness_path": str(args.harness),
        "pipeline_path": str(args.pipeline),
        "matrix": matrix,
        "harness": harness,
        "pipeline": pipeline,
        "hardware_lane_required": args.require_hardware_lane,
    }

    if args.json_out:
        args.json_out.write_text(json.dumps(summary, indent=2) + "\n")

    if not harness["uses_matrix_file"] or not harness["uses_reader"]:
        raise SystemExit("smoke harness does not use the matrix file defaults")
    if not harness["has_required_dest_defaults"]:
        raise SystemExit("smoke harness missing required destination default variables")

    devicekit = pipeline.get("devicekit")
    if args.require_hardware_lane and not harness["has_optional_hardware_dest"]:
        raise SystemExit("smoke harness missing optional hardware destination variable")
    if args.require_hardware_lane and devicekit != args.expected_devicekit:
        raise SystemExit(
            f"expected devicekit tag {args.expected_devicekit!r} but found {devicekit!r}"
        )


if __name__ == "__main__":
    main()
