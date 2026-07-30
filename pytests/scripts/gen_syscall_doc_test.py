"""Regression tests for the canonical ABI-v1 syscall Markdown generator."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "gen_syscall_doc.py"
SPEC = importlib.util.spec_from_file_location("gen_syscall_doc", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
generator = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = generator
SPEC.loader.exec_module(generator)


def test_repository_rows_include_numeric_v1_and_omit_non_public_constants() -> None:
    rows = generator.build_rows()
    rendered = "\n".join(rows)

    assert "| SYSCALL_INT_FROM_I64 | 0x10100 |" in rendered
    assert "| SYSCALL_DECIMAL_DIV_ROUND | 0x10126 |" in rendered
    assert "| SYSCALL_QUANTITY_RATIO_ROUND | 0x10149 |" in rendered
    assert "SYSCALL_KOTO_TEST_" not in rendered
    assert "SYSCALL_NUMERIC_" not in rendered
    assert "SYSCALL_AMOUNT_" not in rendered


def test_localized_table_replacement_preserves_surrounding_content() -> None:
    lines = [
        "---",
        "lang: ja",
        "---",
        "# localized title",
        "",
        "| Name | Value (hex) | Note |",
        "|------|-------------|------|",
        "| stale | 0x69 | stale |",
        "",
        "localized note",
    ]

    updated = generator.replace_localized_table(lines, ["| current | 0x10100 | |"])

    assert updated[:7] == lines[:7]
    assert updated[7] == "| current | 0x10100 | |"
    assert updated[8:] == lines[8:]


@pytest.mark.parametrize(
    "lines, message",
    [
        (
            ["no table"],
            "expected exactly one syscall table separator, found 0",
        ),
        (["| Name | Value |", "|---|---|", ""], "no syscall rows found"),
    ],
)
def test_localized_table_replacement_rejects_malformed_tables(
    lines: list[str], message: str
) -> None:
    with pytest.raises(RuntimeError, match=message):
        generator.replace_localized_table(lines, ["| row | 0x1 | |"])
