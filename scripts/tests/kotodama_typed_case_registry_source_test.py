#!/usr/bin/env python3
"""Fail closed on the typed Kotodama test-registry compaction contract."""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
IVM_SOURCE = Path("crates/ivm/tests/kotodama.rs")
IR_SOURCE = Path("crates/kotodama_lang/src/ir.rs")
FIXTURE_MANIFEST = Path("crates/kotodama_lang/kotodama_fixtures_v1.manifest.json")

IVM_REGION_SHA256 = "7ba66a05d520056adea3d1098a2298bc198fb6d9c900ce1e845a70c1a8083ff1"
IR_REGION_SHA256 = "5687f1fc7303948c2e9d5aba10c51c509cc3cf1e21b7eef9f730ae9a8323edfe"
IVM_CASE_IDS_SHA256 = "54426fcf612986f0b7eceee6233ee3ad9b60a08165f511191be6680eaa2eff79"

IVM_MACROS = (
    ("compile_cases", 8),
    ("compile_rejection_cases", 32),
    ("semantic_rejection_cases", 27),
    ("vm_result_cases", 8),
    ("parse_rejection_cases", 6),
    ("semantic_success_cases", 7),
)
IVM_REGISTRY_TESTS = (
    "compile_case_registry",
    "compile_rejection_case_registry",
    "semantic_rejection_case_registry",
    "vm_result_case_registry",
    "parse_rejection_case_registry",
    "semantic_success_case_registry",
)
IR_TEST_NAMES = (
    "lower_resolve_account_alias_builtin",
    "lower_resolve_account_alias_builtin_uses_string_literal",
    "lower_resolve_account_alias_invalid_literal_uses_string_literal",
    "lower_resolve_account_alias_domain_qualified_builtin_uses_string_literal",
    "lower_resolve_account_alias_invalid_domain_qualified_literal_uses_string_literal",
    "lower_account_id_alias_literal_to_resolve_account_alias",
    "lower_account_id_domain_qualified_alias_literal_to_resolve_account_alias",
    "lower_account_id_invalid_non_alias_literal_keeps_static_account_dataref",
    "lower_account_id_canonical_literal_to_static_account_dataref",
    "lower_account_id_invalid_alias_shaped_literal_to_resolve_account_alias",
    "lower_account_id_invalid_domain_qualified_alias_literal_to_resolve_account_alias",
)


def _read_source(relative: Path) -> str:
    path = ROOT / relative
    if path.is_symlink() or not path.is_file():
        raise AssertionError(f"missing or non-regular source: {relative}")
    path.resolve(strict=True).relative_to(ROOT)
    return path.read_text(encoding="utf-8")


def _region(source: str, start: str, end: str) -> str:
    start_index = source.index(start)
    end_index = source.index(end, start_index)
    return source[start_index:end_index]


def _sha256(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


class KotodamaTypedCaseRegistrySourceTest(unittest.TestCase):
    """Authenticate the data-only registries and their charged line ceiling."""

    def test_typed_case_registry_contract(self) -> None:
        ivm_source = _read_source(IVM_SOURCE)
        ir_source = _read_source(IR_SOURCE)
        ivm_region = _region(
            ivm_source,
            "#[derive(Clone, Copy)]\nenum CaseSource",
            "#[test]\nfn assert_builtin_obeys_truthiness",
        )
        ir_region = _region(
            ir_source,
            "    #[derive(Clone, Copy)]\n    enum AliasSource",
            "    #[test]\n    fn lower_get_quantity_builtin",
        )

        self.assertEqual(_sha256(ivm_region), IVM_REGION_SHA256)
        self.assertEqual(_sha256(ir_region), IR_REGION_SHA256)
        self.assertLessEqual(len(ivm_region.splitlines()), 500)
        self.assertLessEqual(len(ir_region.splitlines()), 217)
        self.assertGreaterEqual(
            1_217 - len(ivm_region.splitlines()) - len(ir_region.splitlines()),
            500,
        )

        forbidden = (
            "rustfmt::skip",
            ":tt",
            "$body",
            "$action",
            "$step",
            "dyn Fn",
            "impl Fn",
            "kotodama_integration_v1",
        )
        for token in forbidden:
            self.assertNotIn(token, ivm_region)
            self.assertNotIn(token, ir_region)

        case_ids: list[str] = []
        for macro_name, expected_count in IVM_MACROS:
            self.assertEqual(ivm_region.count(f"macro_rules! {macro_name}"), 1)
            invocation_start = ivm_region.index(f"{macro_name}! {{")
            invocation_end = ivm_region.index("\n}", invocation_start)
            invocation = ivm_region[invocation_start:invocation_end]
            ids = re.findall(r'^ {4}"([^"]+)"', invocation, re.MULTILINE)
            self.assertEqual(len(ids), expected_count, macro_name)
            case_ids.extend(ids)

        self.assertEqual(len(case_ids), 88)
        self.assertEqual(len(set(case_ids)), 88)
        self.assertEqual(len({case_id.split("/", 1)[0] for case_id in case_ids}), 74)
        self.assertEqual(
            hashlib.sha256(
                json.dumps(case_ids, separators=(",", ":")).encode()
            ).hexdigest(),
            IVM_CASE_IDS_SHA256,
        )
        for test_name in IVM_REGISTRY_TESTS:
            self.assertIn(f"#[test]\nfn {test_name}()", ivm_region)
        self.assertEqual(ivm_region.count("#[test]"), len(IVM_REGISTRY_TESTS))

        emitted_names = tuple(
            re.findall(r"(?m)^    alias_lowering_case!\(\n        ([a-z0-9_]+),", ir_region)
        )
        self.assertEqual(emitted_names, IR_TEST_NAMES)
        self.assertEqual(len(set(emitted_names)), len(IR_TEST_NAMES))
        self.assertEqual(ir_region.count("macro_rules! alias_lowering_case"), 1)
        self.assertEqual(ir_region.count("#[test]"), 1)

        manifest = json.loads(_read_source(FIXTURE_MANIFEST))
        source_entry = next(
            entry
            for entry in manifest["source_files"]
            if entry["path"] == IR_SOURCE.as_posix()
        )
        manifest_names = tuple(source_entry["test_names"])
        first = manifest_names.index(IR_TEST_NAMES[0])
        self.assertEqual(manifest_names[first : first + len(IR_TEST_NAMES)], IR_TEST_NAMES)


if __name__ == "__main__":
    unittest.main()
