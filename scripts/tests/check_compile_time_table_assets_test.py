"""Tests for the compile-time Rust static asset verifier."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts/check_compile_time_table_assets.py"
SPEC = importlib.util.spec_from_file_location(
    "check_compile_time_table_assets", MODULE_PATH
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_checked_in_compile_time_assets_and_preimages_are_exact() -> None:
    counts = MODULE.audit_repository(ROOT)
    assert counts == MODULE.AuditCounts(
        manifests=11,
        assets=85,
        bytes=272_803,
        source_preimages=93,
    )


def test_declaration_slice_uses_exact_name_line_count_and_final_lf() -> None:
    source = b"const FIRST: [u8; 1] = [1];\nconst SECOND: [u8; 2] = [\n    2,\n    3,\n];\n"
    assert MODULE.declaration_slice(source, "SECOND", 4) == (
        b"const SECOND: [u8; 2] = [\n    2,\n    3,\n];\n"
    )
    with pytest.raises(MODULE.AssetError, match="semicolon \\+ LF"):
        MODULE.declaration_slice(source, "SECOND", 3)
    with pytest.raises(MODULE.AssetError, match="exactly once"):
        MODULE.declaration_slice(source, "MISSING", 1)


def test_line_span_slice_pins_start_count_and_final_lf() -> None:
    source = b"first\nsecond\nthird\n"
    assert MODULE.line_span_slice(source, 2, 2) == b"second\nthird\n"
    with pytest.raises(MODULE.AssetError, match="exceeds source bounds"):
        MODULE.line_span_slice(source, 3, 2)
    with pytest.raises(MODULE.AssetError, match="does not end with LF"):
        MODULE.line_span_slice(b"first\nsecond", 2, 1)


def test_raw_string_payload_recovers_exact_literal_bytes() -> None:
    span = b'    let value = r#"first\nsecond\n"#;\n'
    assert MODULE.rust_raw_string_payload(span) == b"first\nsecond\n"
    with pytest.raises(MODULE.AssetError, match="no r# raw string"):
        MODULE.rust_raw_string_payload(b'let value = "ordinary";\n')
    with pytest.raises(MODULE.AssetError, match="no raw string terminator"):
        MODULE.rust_raw_string_payload(b'let value = r#"unterminated\n')


def test_soracloud_format_template_transform_is_exact_and_strict() -> None:
    span = b'''fn demo(service_name: &str, prelude: &str) -> String {
    format!(r#"{{"service": {service_name:?}, "literal": "{{literal}}"}}
{prelude}
"#)
}
'''
    assert MODULE.soracloud_format_template_payload(span) == (
        b'{"service": __SORACLOUD_SERVICE_NAME_DEBUG__, "literal": "{literal}"}\n'
        b"__SORACLOUD_SHELL_PRELUDE__\n"
    )
    with pytest.raises(MODULE.AssetError, match="unsupported Soracloud format field"):
        MODULE.soracloud_format_template_payload(
            b'fn demo() { format!(r#"{unknown}"#) }\n'
        )


def test_suffix_include_inventory_rejects_unmanifested_consumers(tmp_path: Path) -> None:
    root = tmp_path.resolve()
    manifest_path = root / "crate/src/assets/template_manifest.json"
    manifested = manifest_path.parent / "manifested.tmpl"
    extra = root / "crate/src/other/extra.tmpl"
    consumer = MODULE.IncludeConsumer(root / "crate/src/lib.rs", "include_str", None)

    MODULE._verify_suffix_include_inventory(
        root, manifest_path, {manifested}, {manifested: [consumer]}, ".tmpl"
    )
    with pytest.raises(MODULE.AssetError, match=r"extra=\['extra\.tmpl'\]"):
        MODULE._verify_suffix_include_inventory(
            root,
            manifest_path,
            {manifested},
            {manifested: [consumer], extra: [consumer]},
            ".tmpl",
        )
