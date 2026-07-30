"""Unit tests for the source file line-budget guard."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_source_file_budget.py"
SPEC = importlib.util.spec_from_file_location("check_source_file_budget", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def budget(**exceptions: int):
    """Build a compact budget fixture."""
    return MODULE.Budget(
        production_limit=5_000,
        test_limit=3_000,
        excluded_prefixes=("vendor/",),
        exceptions=exceptions,
    )


@pytest.mark.parametrize(
    ("path", "expected"),
    [
        ("crates/core/src/lib.rs", False),
        ("crates/core/tests/network.rs", True),
        ("scripts/tests/check_guard_test.py", True),
        ("javascript/client.test.js", True),
        ("crates/core/examples/query.rs", True),
    ],
)
def test_test_path_classification(path: str, expected: bool) -> None:
    assert MODULE.is_test_path(path) is expected


def test_evaluate_enforces_new_file_limits() -> None:
    findings = MODULE.evaluate(
        {
            "crates/core/src/small.rs": 5_000,
            "crates/core/src/large.rs": 5_001,
            "crates/core/tests/large.rs": 3_001,
        },
        budget(),
    )
    assert [(finding.path, finding.message) for finding in findings] == [
        (
            "crates/core/src/large.rs",
            "5001 lines exceeds the 5000-line production limit",
        ),
        (
            "crates/core/tests/large.rs",
            "3001 lines exceeds the 3000-line test limit",
        ),
    ]


def test_evaluate_requires_exact_ratcheting_baselines() -> None:
    source = "crates/core/src/state.rs"
    baseline = budget(**{source: 12_000})

    assert MODULE.evaluate({source: 12_000}, baseline) == []
    assert "grew from baseline 12000 to 12001" in MODULE.evaluate(
        {source: 12_001}, baseline
    )[0].message
    assert "refresh the baseline to ratchet it down" in MODULE.evaluate(
        {source: 11_999}, baseline
    )[0].message


def test_evaluate_rejects_stale_and_missing_exceptions() -> None:
    findings = MODULE.evaluate(
        {"crates/core/src/stale.rs": 100},
        budget(
            **{
                "crates/core/src/stale.rs": 4_000,
                "crates/core/src/missing.rs": 7_000,
            }
        ),
    )
    assert [finding.path for finding in findings] == [
        "crates/core/src/stale.rs",
        "crates/core/src/missing.rs",
    ]


def test_baseline_payload_only_records_oversized_sources() -> None:
    payload = MODULE.baseline_payload(
        {
            "crates/core/src/lib.rs": 5_001,
            "crates/core/src/small.rs": 50,
            "crates/core/tests/large.rs": 3_001,
        },
        production_limit=5_000,
        test_limit=3_000,
        excluded_prefixes=("vendor/",),
    )
    assert payload["exceptions"] == {
        "crates/core/src/lib.rs": 5_001,
        "crates/core/tests/large.rs": 3_001,
    }


def test_load_budget_validates_and_normalizes(tmp_path: Path) -> None:
    path = tmp_path / "budget.json"
    path.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "limits": {"production": 5_000, "test": 3_000},
                "excluded_prefixes": ["vendor", "target/"],
                "exceptions": {"crates/core/src/lib.rs": 6_000},
            }
        ),
        encoding="utf-8",
    )
    parsed = MODULE.load_budget(path)
    assert parsed.excluded_prefixes == ("target/", "vendor/")
    assert parsed.exceptions == {"crates/core/src/lib.rs": 6_000}


def test_source_line_count_uses_logical_lines_and_rejects_symlinks(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source.rs"
    source.write_text("one\ntwo\n", encoding="utf-8")
    assert MODULE.source_line_count(tmp_path, "source.rs") == 2

    link = tmp_path / "link.rs"
    link.symlink_to(source)
    with pytest.raises(ValueError, match="not a regular file"):
        MODULE.source_line_count(tmp_path, "link.rs")


def test_tracked_paths_uses_the_candidate_tree(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "present.rs").write_text("//! Present.\n", encoding="utf-8")
    monkeypatch.setattr(
        MODULE.subprocess,
        "check_output",
        lambda *_args, **_kwargs: b"missing.rs\0present.rs\0",
    )

    assert MODULE.tracked_paths(tmp_path) == ["present.rs"]
