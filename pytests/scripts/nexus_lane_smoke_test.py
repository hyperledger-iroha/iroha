"""Tests for strict Nexus lane lifecycle smoke validation."""

from __future__ import annotations

import importlib.util
import json
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


def lifecycle_status() -> dict:
    literal = "hash:" + "A1" * 32 + "#30FA"
    return {
        "version": 1,
        "lane_count": 1,
        "lanes": [{"id": 0, "alias": "core", "dataspace_id": 0}],
        "catalog_hash": literal,
        "incarnations": [{"lane_id": 0, "incarnation": literal}],
        "incarnation_root": literal,
        "runtime_catalog_hash": None,
    }


@pytest.mark.parametrize("runtime_root", [None, "hash:" + "A1" * 32 + "#30FA"])
def test_lifecycle_accepts_explicit_nullable_runtime_root(runtime_root: str | None) -> None:
    status = lifecycle_status()
    status["runtime_catalog_hash"] = runtime_root
    assert SMOKE.validate_lane_lifecycle(status)["core"].lane_id == 0


def test_lifecycle_rejects_missing_runtime_root() -> None:
    status = lifecycle_status()
    del status["runtime_catalog_hash"]
    with pytest.raises(SMOKE.SmokeError, match="current V1 layout"):
        SMOKE.validate_lane_lifecycle(status)


@pytest.mark.parametrize("runtime_root", [False, 0, "", "hash:" + "00" * 31 + "01#C50E"])
def test_lifecycle_rejects_invalid_or_empty_runtime_root(runtime_root: object) -> None:
    status = lifecycle_status()
    status["runtime_catalog_hash"] = runtime_root
    with pytest.raises(SMOKE.SmokeError, match="runtime_catalog_hash"):
        SMOKE.validate_lane_lifecycle(status)


@pytest.mark.parametrize("first", [None, "hash:" + "A1" * 32 + "#30FA"])
@pytest.mark.parametrize("second", [None, "hash:" + "A1" * 32 + "#30FA"])
def test_lifecycle_sources_reject_duplicate_runtime_root(
    first: str | None, second: str | None, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    status = lifecycle_status()
    del status["runtime_catalog_hash"]
    payload = (
        json.dumps(status)[:-1]
        + ', "runtime_catalog_hash": '
        + json.dumps(first)
        + ', "runtime_catalog_hash": '
        + json.dumps(second)
        + "}"
    )
    source = tmp_path / "lifecycle.json"
    source.write_text(payload, encoding="utf-8")
    with pytest.raises(SMOKE.SmokeError, match="duplicate key `runtime_catalog_hash`"):
        SMOKE.read_json_file(str(source), label="lane lifecycle")
    monkeypatch.setattr(SMOKE, "fetch_text", lambda *args, **kwargs: payload)
    with pytest.raises(SMOKE.SmokeError, match="duplicate key `runtime_catalog_hash`"):
        SMOKE.fetch_json("https://example.invalid/v1/nexus/lifecycle", 1, False)


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
