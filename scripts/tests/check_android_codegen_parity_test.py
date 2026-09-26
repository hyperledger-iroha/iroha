"""Tests for scripts/check_android_codegen_parity.py."""

from __future__ import annotations

import hashlib
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
    projection = (
        "const INSTRUCTION_WIRE_SCHEMA_BINDINGS = /* @__PURE__ */ (() => Object.freeze(\n"
        "  Object.entries(INNER_TYPE_NAME_BY_WIRE_ID).map(\n"
        "    ([outerWireId, innerTypeName]) =>\n"
        "      Object.freeze({ outerWireId, innerTypeName }),\n"
        "  ),\n"
        "))();\n"
        if derive_hashes
        else 'const INSTRUCTION_WIRE_SCHEMA_BINDINGS = Buffer.from("00".repeat(16), "hex");\n'
    )
    path.write_text(
        'const TEXT_IROHA_INSTRUCTION_V1 = "iroha.instruction.v1::";\n'
        'const TEXT_IROHA_DATA_MODEL_ISI = "iroha_data_model::isi::";\n'
        'const RECORD_SCCP_MESSAGE_WIRE_ID =\n'
        '  (TEXT_IROHA_INSTRUCTION_V1 + "bridge::RecordSccpMessage");\n'
        "const INNER_TYPE_NAME_BY_WIRE_ID = Object.freeze({\n"
        '  "zk::ScheduleConfidentialPolicyTransition": '
        f'"{alias_type}",\n'
        '  [RECORD_SCCP_MESSAGE_WIRE_ID]: '
        '`${TEXT_IROHA_DATA_MODEL_ISI}bridge::RecordSccpMessage`,\n'
        "});\n"
        + projection
        + "const facade = { _instructionWireSchemaBindings: () => "
        "INSTRUCTION_WIRE_SCHEMA_BINDINGS };\n",
        encoding="utf-8",
    )
    return path


def _schema_hash(type_name: str) -> str:
    return hashlib.sha256(
        b"norito:v1:type-name\0" + type_name.encode("utf-8")
    ).digest()[:16].hex()


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
    js_source = _write_js_type_map(tmp_path / "src.js")
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
            "--js-source",
            str(js_source),
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

    js_source = _write_js_type_map(tmp_path / "src.js")
    exit_code = MODULE.main(
        [
            "--manifest",
            str(manifest),
            "--builder-index",
            str(builder_index),
            "--metadata",
            str(metadata_path),
            "--js-source",
            str(js_source),
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
                "schema_hash": _schema_hash(
                    "iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition"
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

    with pytest.raises(ValueError, match="does not expose the source-derived instruction bindings"):
        MODULE._extract_js_instruction_type_map(source)  # type: ignore[attr-defined]


def _run_isolated_replay(root: Path) -> tuple[int, dict, Path, Path]:
    codegen_root = root / "stage" / "codegen"
    codegen_root.mkdir(parents=True)
    manifest = _write_manifest(
        codegen_root,
        "instruction_manifest.json",
        "instructions",
        [{"discriminant": "alpha"}],
    )
    builder_index = _write_manifest(
        codegen_root,
        "builder_index.json",
        "builders",
        [{"discriminant": "alpha"}],
    )
    metadata = {
        "instruction_manifest": {
            "sha256": _manifest_sha(json.loads(manifest.read_text(encoding="utf-8"))),
            "entry_count": 1,
        },
        "builder_index": {
            "sha256": _builder_sha(json.loads(builder_index.read_text(encoding="utf-8"))),
            "entry_count": 1,
        },
    }
    metadata_path = root / "stage" / "metadata.json"
    metadata_path.write_text(json.dumps(metadata), encoding="utf-8")
    source_root = root / "source"
    js_source = source_root / "javascript/iroha_js/src/norito.js"
    js_source.parent.mkdir(parents=True)
    _write_js_type_map(js_source)
    summary_path = root / "stage" / "summary.json"
    args = [
        "--manifest", str(manifest),
        "--builder-index", str(builder_index),
        "--metadata", str(metadata_path),
        "--json-out", str(summary_path),
        "--codegen-root", str(codegen_root),
        "--source-root", str(source_root),
        "--js-source", str(js_source),
        "--quiet",
    ]
    exit_code = MODULE.main(args)
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    return exit_code, summary, manifest, metadata_path


def test_isolated_replays_have_identical_logical_summaries(tmp_path: Path) -> None:
    first = _run_isolated_replay(tmp_path / "first")
    second = _run_isolated_replay(tmp_path / "second")

    assert first[0] == second[0] == 0
    assert first[1] == second[1]
    assert first[1]["instruction_manifest"]["path"] == (
        "target-codex/android_codegen/instruction_manifest.json"
    )
    assert first[1]["builder_index"]["path"] == (
        "target-codex/android_codegen/builder_index.json"
    )
    assert first[1]["javascript_instruction_schema_map"]["source_path"] == (
        "javascript/iroha_js/src/norito.js"
    )

    changed = json.loads(second[2].read_text(encoding="utf-8"))
    changed["instructions"].append({"discriminant": "beta"})
    second[2].write_text(json.dumps(changed), encoding="utf-8")
    status, changed_summary, _, _ = _run_recheck(second[2], second[3], tmp_path / "second")
    assert status == 1
    assert changed_summary["status"] == "error"
    assert changed_summary["instruction_manifest"]["sha256"] != (
        first[1]["instruction_manifest"]["sha256"]
    )


def _run_recheck(manifest: Path, metadata: Path, root: Path) -> tuple[int, dict, Path, Path]:
    summary_path = root / "stage" / "recheck.json"
    codegen_root = root / "stage" / "codegen"
    source_root = root / "source"
    exit_code = MODULE.main([
        "--manifest", str(manifest),
        "--builder-index", str(codegen_root / "builder_index.json"),
        "--metadata", str(metadata),
        "--json-out", str(summary_path),
        "--codegen-root", str(codegen_root),
        "--source-root", str(source_root),
        "--js-source", str(source_root / "javascript/iroha_js/src/norito.js"),
        "--quiet",
    ])
    return exit_code, json.loads(summary_path.read_text(encoding="utf-8")), manifest, metadata


def test_logical_path_requires_containment_and_preserves_relative_name(
    tmp_path: Path,
) -> None:
    codegen_root = tmp_path / "codegen"
    nested = codegen_root / "nested" / "manifest.json"
    nested.parent.mkdir(parents=True)
    nested.write_text("{}", encoding="utf-8")
    assert MODULE._logical_summary_path(  # type: ignore[attr-defined]
        nested, codegen_root, Path("target-codex/android_codegen")
    ) == "target-codex/android_codegen/nested/manifest.json"

    outside = tmp_path / "outside.json"
    outside.write_text("{}", encoding="utf-8")
    with pytest.raises(ValueError, match="outside summary root"):
        MODULE._logical_summary_path(  # type: ignore[attr-defined]
            outside, codegen_root, Path("target-codex/android_codegen")
        )


def _copy_current_js_inventory(tmp_path: Path) -> Path:
    source_root = Path(__file__).resolve().parents[2]
    source_dir = source_root / "javascript/iroha_js/src"
    target_dir = tmp_path / "javascript/iroha_js/src"
    target_dir.mkdir(parents=True)
    for filename in (
        "norito.js",
        "noritoNftMarketCodecs.js",
        "noritoGameRegistry.js",
    ):
        (target_dir / filename).write_bytes((source_dir / filename).read_bytes())
    return target_dir / "norito.js"


def test_current_js_computed_wire_id_and_imported_bindings_match_manifest(
    tmp_path: Path,
) -> None:
    js_source = _copy_current_js_inventory(tmp_path)
    source_map = MODULE._extract_js_instruction_type_map(js_source)  # type: ignore[attr-defined]
    assert len(source_map) >= 70
    assert source_map["iroha.instruction.v1::bridge::RecordSccpMessage"] == (
        "iroha_data_model::isi::bridge::RecordSccpMessage"
    )
    assert source_map["iroha.instruction.v1::nft_market::OfferNftV1"] == (
        "iroha_data_model::isi::nft_market::OfferNftV1"
    )
    assert source_map["iroha.instruction.v1::game::OpenGameSessionV1"] == (
        "iroha_data_model::isi::game::OpenGameSessionV1"
    )
    mint_type = source_map["iroha.mint"]
    zk_type = source_map["iroha.instruction.v1::zk::RegisterZkAsset"]
    manifest = _write_manifest(
        tmp_path,
        "manifest.json",
        "instructions",
        [
            {
                "discriminant": "iroha.mint",
                "type_name": mint_type,
                "schema_hash": _schema_hash(mint_type),
            },
            {
                "discriminant": zk_type,
                "type_name": zk_type,
                "schema_hash": _schema_hash(zk_type),
            },
        ],
    )
    errors: list[str] = []
    summary = MODULE._check_js_instruction_type_maps(  # type: ignore[attr-defined]
        manifest, js_source, errors
    )
    assert errors == []
    assert summary["entry_count"] == len(source_map)
    assert summary["manifest_matched_entry_count"] == 2
    assert summary["wire_binding_sha256"] == MODULE._canonical_sha256(source_map)  # type: ignore[attr-defined]

    payload = json.loads(manifest.read_text(encoding="utf-8"))
    payload["instructions"][0]["schema_hash"] = "00" * 16
    manifest.write_text(json.dumps(payload), encoding="utf-8")
    errors = []
    MODULE._check_js_instruction_type_maps(manifest, js_source, errors)  # type: ignore[attr-defined]
    assert any("schema-hash mismatch for iroha.mint" in error for error in errors)

    payload["instructions"][0]["schema_hash"] = _schema_hash(mint_type)
    payload["instructions"][0]["type_name"] = "iroha_data_model::isi::mint_burn::WrongBox"
    manifest.write_text(json.dumps(payload), encoding="utf-8")
    errors = []
    MODULE._check_js_instruction_type_maps(manifest, js_source, errors)  # type: ignore[attr-defined]
    assert any("type-name mismatch for iroha.mint" in error for error in errors)


def test_current_js_binding_parser_rejects_changed_wire_identity(tmp_path: Path) -> None:
    js_source = _copy_current_js_inventory(tmp_path)
    original = js_source.read_text(encoding="utf-8")
    changed = original.replace(
        'TEXT_IROHA_INSTRUCTION_V1 + "sorafs::IssueReplicationOrder"',
        'TEXT_IROHA_INSTRUCTION_V1 + "sorafs::WrongOrder"',
        1,
    )
    assert changed != original
    js_source.write_text(changed, encoding="utf-8")
    with pytest.raises(ValueError, match="changes canonical outer/inner identity"):
        MODULE._extract_js_instruction_type_map(js_source)  # type: ignore[attr-defined]


def test_current_js_binding_parser_rejects_changed_imported_wire_ids(
    tmp_path: Path,
) -> None:
    js_source = _copy_current_js_inventory(tmp_path)
    nft_module = js_source.with_name("noritoNftMarketCodecs.js")
    original = nft_module.read_text(encoding="utf-8")
    changed = original.replace(
        "iroha.instruction.v1::nft_market::${name}",
        "iroha.instruction.v1::nft_market_legacy::${name}",
        1,
    )
    assert changed != original
    nft_module.write_text(changed, encoding="utf-8")
    with pytest.raises(ValueError, match="does not derive NFT_MARKET_INSTRUCTION_WIRE_IDS_V1"):
        MODULE._extract_js_instruction_type_map(js_source)  # type: ignore[attr-defined]


def test_current_js_binding_parser_rejects_second_hash_map(tmp_path: Path) -> None:
    js_source = _copy_current_js_inventory(tmp_path)
    js_source.write_text(
        js_source.read_text(encoding="utf-8")
        + '\nconst INNER_SCHEMA_HASH_BY_WIRE_ID = { "iroha.mint": "00" };\n',
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="retains a separate instruction schema-hash map"):
        MODULE._extract_js_instruction_type_map(js_source)  # type: ignore[attr-defined]
