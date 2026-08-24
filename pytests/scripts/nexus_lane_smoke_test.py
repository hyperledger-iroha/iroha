"""Tests for strict Nexus lane lifecycle smoke validation."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/nexus_lane_smoke.py"
SPEC = importlib.util.spec_from_file_location("nexus_lane_smoke", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
SMOKE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = SMOKE
SPEC.loader.exec_module(SMOKE)


def test_require_hash_accepts_canonical_norito_literal() -> None:
    literal = "hash:" + "A1" * 32 + "#30FA"
    assert SMOKE._require_hash(literal, "catalog_hash") == literal


@pytest.mark.parametrize(
    "literal",
    [
        "hash:" + "00" * 32,
        "hash:" + "00" * 32 + "#D52F",
        "hash:" + "aa" * 32 + "#0000",
        "hash:" + "00" * 31 + "#D52F",
        "hash:" + "00" * 32 + "#0000",
        "hash:" + "00" * 32 + "#d52f",
    ],
)
def test_require_hash_rejects_noncanonical_literals(literal: str) -> None:
    with pytest.raises(SMOKE.SmokeError, match="invalid `catalog_hash` commitment"):
        SMOKE._require_hash(literal, "catalog_hash")


def test_parse_args_accepts_only_canonical_source_flags() -> None:
    args = SMOKE.parse_args(
        [
            "--lifecycle-file",
            "lifecycle.json",
            "--telemetry-file",
            "telemetry.ndjson",
            "--lane-alias",
            "core",
        ]
    )

    assert args.lifecycle_file == "lifecycle.json"
    assert args.telemetry_file == "telemetry.ndjson"


@pytest.mark.parametrize(
    "retired_args",
    [
        ["--status-url", "https://example.invalid/v1/nexus/lifecycle"],
        ["--status-file", "lifecycle.json"],
        ["--lifecycle-file", "lifecycle.json", "--from-telemetry", "events.ndjson"],
    ],
)
def test_parse_args_rejects_retired_source_flags(retired_args: list[str]) -> None:
    with pytest.raises(SystemExit):
        SMOKE.parse_args([*retired_args, "--lane-alias", "core"])
