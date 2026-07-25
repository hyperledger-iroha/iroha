"""Focused tests for canonical Kotodama syntax/editor/SDK regeneration."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil
import sys

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "regenerate_kotodama_syntax.py"
SPEC = importlib.util.spec_from_file_location("regenerate_kotodama_syntax", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def _copy_generator_inputs(destination: Path) -> None:
    source = MODULE.REPOSITORY_ROOT
    paths = {
        MODULE.DEFAULT_POLICY,
        *MODULE.GENERATED_TARGETS,
        Path("crates/ivm_abi/src/pointer_abi.rs"),
        Path("crates/iroha_primitives/src/numeric_abi.rs"),
    }
    policy = MODULE.load_policy(source)
    paths.add(policy.lexical_grammar)
    for relative in paths:
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / relative, target)


def _generated_body(text: str, prefix: str = "//") -> str:
    start = f"{prefix} BEGIN GENERATED: {MODULE.MARKER_NAME}"
    end = f"{prefix} END GENERATED: {MODULE.MARKER_NAME}"
    return text.split(start, 1)[1].split(end, 1)[0].strip()


def test_policy_fixture_has_no_clock_or_provenance_fields() -> None:
    raw = json.loads(
        (MODULE.REPOSITORY_ROOT / MODULE.DEFAULT_POLICY).read_text(encoding="utf-8")
    )

    def keys(value: object) -> set[str]:
        if isinstance(value, dict):
            return set(value).union(*(keys(item) for item in value.values()))
        if isinstance(value, list):
            return set().union(*(keys(item) for item in value))
        return set()

    assert keys(raw).isdisjoint(
        {"created_at", "generated_at", "timestamp", "updated_at", "provenance"}
    )


def test_final_policy_pins_quantity_without_an_abi_tombstone() -> None:
    policy = MODULE.load_policy(MODULE.REPOSITORY_ROOT)
    schemas = {schema.source_type: schema for schema in policy.numeric_schemas}

    assert schemas["quantity"].rust_type == "Quantity"
    assert schemas["quantity"].pointer_id == 0x0010
    assert schemas["int"].pointer_id == 0x0011
    assert schemas["decimal"].pointer_id == 0x0012
    assert policy.unassigned_pointers == (0x0013,)
    assert "quantity" in policy.active_source_types
    assert "Quantity" not in policy.active_source_types
    assert "Amount" in policy.retired_numeric_type_spellings
    assert "amount" in policy.ordinary_value_identifiers
    assert policy.numeric_suffix_fixits == ("amt", "qty")
    assert {schema.source_type for schema in policy.numeric_schemas} == {
        "int",
        "decimal",
        "quantity",
    }


def test_repository_generated_outputs_are_current_and_complete() -> None:
    policy = MODULE.load_policy(MODULE.REPOSITORY_ROOT)
    outputs = MODULE.render_outputs(MODULE.REPOSITORY_ROOT, policy)

    assert tuple(outputs) == MODULE.GENERATED_TARGETS
    assert len(outputs) == 18
    assert MODULE.apply_outputs(MODULE.REPOSITORY_ROOT, outputs, check=True) == 0


def test_javascript_runtime_and_declaration_policy_are_generated_from_one_mapping() -> (
    None
):
    root = MODULE.REPOSITORY_ROOT
    source_body = _generated_body(
        (root / MODULE.JAVASCRIPT_PATHS[0]).read_text(encoding="utf-8")
    )
    dist_body = _generated_body(
        (root / MODULE.JAVASCRIPT_PATHS[1]).read_text(encoding="utf-8")
    )
    declarations = _generated_body(
        (root / MODULE.TYPESCRIPT_PATH).read_text(encoding="utf-8")
    )

    assert source_body == dist_body
    assert "pointerType: 0x0010" in source_body
    assert "readonly pointerType: 0x0010" in declarations
    assert "0x0013" not in source_body
    assert "Amount" not in source_body


def test_textmate_policy_marks_only_attached_amt_and_qty_suffixes() -> None:
    grammar = json.loads(
        (MODULE.REPOSITORY_ROOT / MODULE.TEXTMATE_PATH).read_text(encoding="utf-8")
    )
    pattern = grammar["repository"]["retiredNumericSuffixes"]["patterns"][0]
    includes = [
        item.get("include") for item in grammar["patterns"] if isinstance(item, dict)
    ]

    assert includes.count("#retiredNumericSuffixes") == 1
    assert pattern["name"] == "invalid.deprecated.numeric.suffix.kotodama"
    assert "amt|qty" in pattern["match"]
    assert "amount" not in pattern["match"]
    assert "Amount" not in grammar["repository"]["types"]["patterns"][0]["match"]


def test_check_detects_drift_write_repairs_it_and_second_check_is_clean(
    tmp_path: Path,
) -> None:
    _copy_generator_inputs(tmp_path)
    docs = tmp_path / MODULE.GRAMMAR_DOC_PATH
    docs.write_text(
        docs.read_text(encoding="utf-8").replace(
            "| `authorize` | `Authorize` |",
            "| `authorize` | `Broken` |",
            1,
        ),
        encoding="utf-8",
    )

    policy = MODULE.load_policy(tmp_path)
    outputs = MODULE.render_outputs(tmp_path, policy)
    assert MODULE.apply_outputs(tmp_path, outputs, check=True) == 1
    assert MODULE.apply_outputs(tmp_path, outputs, check=False) == 1

    second = MODULE.render_outputs(tmp_path, policy)
    assert MODULE.apply_outputs(tmp_path, second, check=True) == 0


def test_missing_or_duplicate_markers_fail_closed() -> None:
    with pytest.raises(MODULE.GenerationError, match="exactly one marker"):
        MODULE._replace_generated(
            "plain text",
            "BEGIN",
            "END",
            "body",
            path=Path("missing.txt"),
        )
    with pytest.raises(MODULE.GenerationError, match="exactly one marker"):
        MODULE._replace_generated(
            "BEGIN\nold\nEND\nBEGIN\nold\nEND\n",
            "BEGIN",
            "END",
            "body",
            path=Path("duplicate.txt"),
        )
    with pytest.raises(MODULE.GenerationError, match="precedes"):
        MODULE._replace_generated(
            "END\nold\nBEGIN\n",
            "BEGIN",
            "END",
            "body",
            path=Path("reversed.txt"),
        )


def test_sdk_validator_policy_is_contextual_and_descriptor_owned() -> None:
    root = MODULE.REPOSITORY_ROOT
    policy = MODULE.load_policy(root)
    assert "amount" in policy.retired_numeric_type_spellings
    assert "amount" in policy.ordinary_value_identifiers

    javascript = (root / MODULE.JAVASCRIPT_IDENTIFIER_PATHS[0]).read_text()
    assert "KOTODAMA_V1_RETIRED_TYPE_NAMES" in javascript
    assert "typeDeclaration = false" in javascript
    assert "NUMERIC_TYPE_KEYWORD_SET" not in javascript

    python = (root / MODULE.PYTHON_MANIFEST_PATH).read_text()
    for spelling in policy.retired_numeric_type_spellings:
        assert f'"{spelling}",' in python
    assert "type_declaration: bool = False" in python

    for path in (
        MODULE.KOTLIN_MANIFEST_PATH,
        MODULE.JAVA_MANIFEST_PATH,
        MODULE.SWIFT_MANIFEST_PATH,
        MODULE.CSHARP_MANIFEST_PATH,
    ):
        text = (root / path).read_text()
        assert "retiredNumericTypeNames" in text or "RETIRED_NUMERIC_TYPE_NAMES" in text or "RetiredNumericTypeNames" in text
