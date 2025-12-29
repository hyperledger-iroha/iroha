"""Tests for scripts/android_codegen_docs.py layout rendering."""

from __future__ import annotations

import importlib.util
from pathlib import Path

MODULE_PATH = (Path(__file__).resolve().parents[1] / "android_codegen_docs.py")
SPEC = importlib.util.spec_from_file_location("android_codegen_docs", MODULE_PATH)
assert SPEC and SPEC.loader  # pragma: no cover - import guard
docs_mod = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(docs_mod)  # type: ignore[misc]


def test_render_instructions_renders_tables(tmp_path: Path) -> None:
    out_path = tmp_path / "instructions.md"
    instructions = [
        {
            "discriminant": "iroha.custom",
            "documentation": "Custom instruction payload.",
            "schema_hash": "abcd",
            "type_name": "iroha_data_model::isi::transparent::CustomInstruction",
            "layout": {
                "kind": "struct",
                "fields": [
                    {"name": "payload", "type": "Json"},
                    {"name": "chain_id", "type": "String"},
                ],
            },
        },
        {
            "discriminant": "iroha.set_parameter",
            "documentation": "Schema summary: tuple fields: _0: Parameter.",
            "schema_hash": "efgh",
            "type_name": "iroha_data_model::isi::transparent::SetParameter",
            "layout": {"kind": "tuple", "fields": [{"name": "_0", "type": "Parameter"}]},
        },
        {
            "discriminant": "iroha.register",
            "documentation": "",
            "schema_hash": "ijkl",
            "type_name": "iroha_data_model::isi::register::RegisterBox",
            "layout": {
                "kind": "enum",
                "variants": [
                    {"tag": "Domain", "discriminant": 0, "payload": "Register<Domain>"},
                    {"tag": "Role", "discriminant": 1, "payload": None},
                ],
            },
        },
    ]

    docs_mod.render_instructions(instructions, out_path)
    content = out_path.read_text(encoding="utf-8")
    assert "> Custom instruction payload." in content
    assert "| Field | Type |" in content
    assert "| `payload` | `Json` |" in content
    assert "| `chain_id` | `String` |" in content
    assert "> Schema summary: tuple fields: _0: Parameter." in content
    # Tuple fields should drop the underscore prefix.
    assert "| `0` | `Parameter` |" in content
    assert "| Tag | Discriminant | Payload |" in content
    assert "| `Domain` | 0 | `Register<Domain>` |" in content
    assert "| `Role` | 1 | — |" in content


def test_manifest_tables_are_inlined(tmp_path: Path) -> None:
    out_path = tmp_path / "instructions.md"
    instructions = [
        {
            "discriminant": "iroha_data_model::isi::sorafs::RegisterPinManifest",
            "documentation": "Register manifest with pin registry.",
            "schema_hash": "deadbeef",
            "type_name": "iroha_data_model::isi::sorafs::RegisterPinManifest",
            "layout": {
                "kind": "struct",
                "fields": [
                    {"name": "digest", "type": "ManifestDigest"},
                    {"name": "policy", "type": "PinPolicy"},
                    {"name": "alias", "type": "Option<ManifestAliasBinding>"},
                ],
            },
        }
    ]

    docs_mod.render_instructions(instructions, out_path)
    content = out_path.read_text(encoding="utf-8")
    assert "### Manifest field details" in content
    assert "#### PinPolicy fields" in content
    assert "| `min_replicas` | `u16` | Minimum replica count required by governance." in content
    assert "#### StorageClass variants" in content
    assert "| `Hot` | Low-latency replicas for developer workflows." in content


def test_manifest_catalog_captures_builders(tmp_path: Path) -> None:
    out_path = tmp_path / "manifest_catalog.md"
    instructions = [
        {
            "discriminant": "iroha.burn",
            "documentation": "",
            "schema_hash": "abc",
            "type_name": "iroha::BurnBox",
            "layout": {"kind": "struct", "fields": []},
        },
        {
            "discriminant": "iroha.mint",
            "documentation": "",
            "schema_hash": "def",
            "type_name": "iroha::MintBox",
            "layout": {"kind": "struct", "fields": []},
        },
    ]
    builders = [
        {
            "discriminant": "iroha.burn",
            "builder_name": "BurnAssetBuilder",
            "package": "org.hyperledger.iroha.android.mint",
            "stability": "stable",
            "feature_gates": ["burn"],
            "notes": "handles asset + trigger variants",
        },
        {
            "discriminant": "iroha.burn",
            "builder_name": "BurnTriggerBuilder",
            "package": "org.hyperledger.iroha.android.trigger",
            "stability": "beta",
            "feature_gates": [],
            "notes": None,
        },
    ]

    docs_mod.render_manifest_catalog(instructions, builders, out_path)
    content = out_path.read_text(encoding="utf-8")

    assert "# Android Norito Manifest Catalog" in content
    assert "| `iroha.burn` | `iroha::BurnBox` | `abc`" in content
    assert "`BurnAssetBuilder` (`org.hyperledger.iroha.android.mint`; stable; features: burn; notes: handles asset + trigger variants)" in content
    # Multiple builders get combined using <br /> separators.
    assert "`BurnTriggerBuilder`" in content
    assert "<br />" in content
    # Entries without builders should render the fallback symbol.
    assert "| `iroha.mint` | `iroha::MintBox` | `def` | — |" in content
