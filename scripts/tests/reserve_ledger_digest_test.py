"""Adversarial tests for exact SoraFS reserve-ledger summaries."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts" / "telemetry"))

import reserve_ledger_digest as digest  # noqa: E402


def exact_ledger(**overrides: object) -> dict[str, object]:
    ledger: dict[str, object] = {
        "quote_path": "quote.json",
        "rent_due": "1.000000001",
        "reserve_shortfall": "340282366920938463463374607431768211456.000000001",
        "top_up_shortfall": "0.000000001",
        "instructions": [{"Transfer": {}}],
    }
    ledger.update(overrides)
    return ledger


def test_summary_preserves_submicro_and_wider_than_u128_values_exactly() -> None:
    summary = digest._summarise(
        exact_ledger(), "exact", generated_at="2026-07-13T00:00:00Z"
    )

    assert summary["rent_due_xor"] == "1.000000001"
    assert (
        summary["reserve_shortfall_xor"]
        == "340282366920938463463374607431768211456.000000001"
    )
    assert summary["top_up_shortfall_xor"] == "0.000000001"
    assert summary["instruction_count"] == 1
    assert summary["requires_top_up"] is True
    assert summary["meets_underwriting"] is False
    assert summary["transfers"] == [
        {"kind": "rent", "amount_xor": "1.000000001"},
        {
            "kind": "reserve_top_up",
            "amount_xor": "340282366920938463463374607431768211456.000000001",
        },
    ]


@pytest.mark.parametrize(
    "value",
    (
        1,
        1.5,
        True,
        "",
        " 1",
        "1 ",
        "+1",
        "-1",
        "01",
        "1.",
        ".1",
        "1.0",
        "1.230",
        "0.000000000",
        "0.0000000001",
        "1e3",
        "NaN",
        "Infinity",
    ),
)
def test_quantity_parser_rejects_numeric_and_noncanonical_inputs(value: object) -> None:
    with pytest.raises(SystemExit, match="canonical XOR decimal string"):
        digest._parse_xor_quantity(value, "rent_due")


def test_quantity_parser_enforces_the_bounded_512_bit_domain() -> None:
    maximum = str((1 << 511) - 1)
    assert digest._parse_xor_quantity(maximum, "rent_due") is not None
    with pytest.raises(SystemExit, match="bounded XOR quantity domain"):
        digest._parse_xor_quantity(str(1 << 511), "rent_due")


def test_summary_rejects_legacy_micro_fields_and_missing_exact_fields() -> None:
    ledger = exact_ledger()
    del ledger["rent_due"]
    ledger["rent_due_micro_xor"] = "1000000"

    with pytest.raises(SystemExit, match="missing required field `rent_due`"):
        digest._summarise(ledger, "legacy")


@pytest.mark.parametrize(
    "payload, message",
    (
        ('{"rent_due":"1","rent_due":"2"}', "duplicate JSON field"),
        ('{"rent_due":NaN}', "non-finite JSON number"),
        ("[]", "must be an object"),
    ),
)
def test_json_loader_rejects_ambiguous_or_nonobject_roots(
    tmp_path: Path, payload: str, message: str
) -> None:
    path = tmp_path / "ledger.json"
    path.write_text(payload, encoding="utf-8")

    with pytest.raises(SystemExit, match=message):
        digest._load_json(path)


def test_prometheus_rendering_keeps_exact_decimal_text_and_writer_is_callable(
    tmp_path: Path,
) -> None:
    summary = digest._summarise(
        exact_ledger(), "exact", generated_at="2026-07-13T00:00:00Z"
    )
    lines = digest._prometheus_lines(summary)
    assert any(
        line.endswith("340282366920938463463374607431768211456.000000001")
        for line in lines
    )
    assert any(line.endswith("0.000000001") for line in lines)

    output = tmp_path / "reserve.prom"
    digest._write_prometheus(output, [summary])
    assert output.read_text(encoding="utf-8") == "\n".join(lines) + "\n"
