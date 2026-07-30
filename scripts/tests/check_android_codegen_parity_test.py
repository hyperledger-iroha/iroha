"""Tests for scripts/check_android_codegen_parity.py."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

MODULE_PATH = (Path(__file__).resolve().parents[1] / "check_android_codegen_parity.py")
SPEC = importlib.util.spec_from_file_location("check_android_codegen_parity", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
SPEC.loader.exec_module(MODULE)


def _write_manifest(root: Path, filename: str, key: str, entries: list[dict]) -> Path:
    path = root / filename
    path.write_text(json.dumps({key: entries}, indent=2), encoding="utf-8")
    return path


def _manifest_sha(payload: dict) -> str:
    canonical = dict(payload)
    canonical["generated_at"] = canonical.get("generated_at", "")
    canonical["instructions"] = sorted(canonical.get("instructions", []), key=lambda entry: entry.get("discriminant", ""))
    return MODULE._canonical_sha256(canonical)  # type: ignore[attr-defined]


def _builder_sha(payload: dict) -> str:
    canonical = dict(payload)
    canonical["generated_at"] = canonical.get("generated_at", "")
    canonical["builders"] = sorted(canonical.get("builders", []), key=lambda entry: entry.get("discriminant", ""))
    return MODULE._canonical_sha256(canonical)  # type: ignore[attr-defined]


def _write_js_type_map(
    path: Path,
    *,
    alias_type: str = "iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition",
    derive_hashes: bool = True,
) -> Path:
    schema_expression = (
        "Object.fromEntries(Object.entries(INNER_TYPE_NAME_BY_WIRE_ID)"
        ".map(([wireId, typeName]) => [wireId, schemaHashForTypeName(typeName)]))"
        if derive_hashes
        else '{"zk::ScheduleConfidentialPolicyTransition": Buffer.from("00".repeat(16), "hex")}'
    )
    path.write_text(
        'const RECORD_SCCP_MESSAGE_WIRE_ID = '
        '"iroha_data_model::isi::bridge::RecordSccpMessage";\n'
        "const INNER_TYPE_NAME_BY_WIRE_ID = Object.freeze({\n"
        '  "zk::ScheduleConfidentialPolicyTransition": '
        f'"{alias_type}",\n'
        "  [RECORD_SCCP_MESSAGE_WIRE_ID]: RECORD_SCCP_MESSAGE_WIRE_ID,\n"
        "});\n"
        "const INNER_SCHEMA_HASH_BY_WIRE_ID = Object.freeze(\n"
        f"  {schema_expression},\n"
        ");\n"
        "const INNER_HEADER_PADDING_BY_WIRE_ID = Object.freeze({});\n",
        encoding="utf-8",
    )
    return path


def test_parity_success(tmp_path: Path) -> None:
    manifest = _write_manifest(tmp_path, "manifest.json", "instructions", [{"discriminant": "alpha"}])
    builder_index = _write_manifest(tmp_path, "builders.json", "builders", [{"builder": "alpha"}])

    manifest_payload = json.loads(manifest.read_text(encoding="utf-8"))
    builder_payload = json.loads(builder_index.read_text(encoding="utf-8"))
    metadata = {
        "instruction_manifest": {
            "sha256": _manifest_sha(manifest_payload),
            "entry_count": 1,
        },
        "builder_index": {
            "sha256": _builder_sha(builder_payload),
            "entry_count": 1,
        },
    }
    metadata_path = tmp_path / "metadata.json"
    metadata_path.write_text(json.dumps(metadata, indent=2), encoding="utf-8")

    summary_path = tmp_path / "summary.json"
    exit_code = MODULE.main(
        [
            "--manifest",
            str(manifest),
            "--builder-index",
            str(builder_index),
            "--metadata",
            str(metadata_path),
            "--json-out",
            str(summary_path),
            "--quiet",
        ]
    )
    assert exit_code == 0
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["status"] == "ok"
    assert summary["errors"] == []


def test_parity_failure(tmp_path: Path) -> None:
    manifest = _write_manifest(tmp_path, "manifest.json", "instructions", [{"discriminant": "alpha"}])
    builder_index = _write_manifest(tmp_path, "builders.json", "builders", [{"builder": "alpha"}])

    metadata = {
        "instruction_manifest": {
            "sha256": "deadbeef",
            "entry_count": 2,
        },
        "builder_index": {"sha256": "deadbeef", "entry_count": 1},
    }
    metadata_path = tmp_path / "metadata.json"
    metadata_path.write_text(json.dumps(metadata, indent=2), encoding="utf-8")

    exit_code = MODULE.main(
        [
            "--manifest",
            str(manifest),
            "--builder-index",
            str(builder_index),
            "--metadata",
            str(metadata_path),
            "--quiet",
        ]
    )
    assert exit_code == 1


def test_codegen_metadata_is_ordered_and_timestamp_independent() -> None:
    manifest = {
        "version": 1,
        "generated_at": "2026-07-26T10:00:00Z",
        "instructions": [
            {"discriminant": "zeta", "schema_hash": "02"},
            {"discriminant": "alpha", "schema_hash": "01"},
        ],
    }
    builders = {
        "generated_at": "2026-07-26T10:00:00Z",
        "builders": [
            {"discriminant": "zeta", "builder_name": "ZetaBuilder"},
            {"discriminant": "alpha", "builder_name": "AlphaBuilder"},
        ],
    }
    first = MODULE.build_codegen_metadata(manifest, builders)

    manifest["generated_at"] = "2030-01-01T00:00:00Z"
    manifest["instructions"].reverse()
    builders["generated_at"] = "2030-01-01T00:00:00Z"
    builders["builders"].reverse()
    second = MODULE.build_codegen_metadata(manifest, builders)

    assert first == second
    assert first["instruction_manifest"]["entry_count"] == 2
    assert first["builder_index"]["entry_count"] == 2
    assert set(first["instruction_manifest"]) == {"sha256", "entry_count"}
    assert set(first["builder_index"]) == {"sha256", "entry_count"}


def test_js_instruction_type_map_matches_manifest_aliases(tmp_path: Path) -> None:
    manifest_path = _write_manifest(
        tmp_path,
        "manifest.json",
        "instructions",
        [
            {
                "discriminant": "zk::ScheduleConfidentialPolicyTransition",
                "type_name": (
                    "iroha_data_model::isi::zk::"
                    "ScheduleConfidentialPolicyTransition"
                ),
            }
        ],
    )
    source = _write_js_type_map(tmp_path / "src.js")
    errors: list[str] = []

    summary = MODULE._check_js_instruction_type_maps(  # type: ignore[attr-defined]
        manifest_path,
        source,
        errors,
    )

    assert errors == []
    assert summary["entry_count"] == 2
    assert summary["manifest_matched_entry_count"] == 1


def test_js_instruction_type_map_rejects_hardcoded_hashes(
    tmp_path: Path,
) -> None:
    source = _write_js_type_map(tmp_path / "src.js", derive_hashes=False)

    with pytest.raises(ValueError, match="hardcodes an instruction schema hash"):
        MODULE._extract_js_instruction_type_map(source)  # type: ignore[attr-defined]
