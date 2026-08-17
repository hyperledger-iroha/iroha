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
        aggregate_rust=None,
    )


def test_parse_args_exposes_strict_objective_mode(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        sys,
        "argv",
        [str(MODULE_PATH), "--require-objective"],
    )

    parsed = MODULE.parse_args()

    assert parsed.require_objective is True
    assert parsed.write_baseline is False


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


def test_evaluate_enforces_the_default_aggregate_rust_ratchet() -> None:
    aggregate = MODULE.AggregateRustBudget(
        baseline=1_000,
        ceiling=900,
        ratchet_ceiling=950,
        working_target=850,
    )
    configured = MODULE.Budget(
        production_limit=5_000,
        test_limit=3_000,
        excluded_prefixes=("vendor/",),
        exceptions={},
        aggregate_rust=aggregate,
    )
    counts = {
        "crates/a/src/lib.rs": 600,
        "crates/b/tests/cases.rs": 301,
        "scripts/helper.py": 10,
    }
    assert MODULE.evaluate(counts, configured) == []
    finding = MODULE.evaluate(
        {
            "crates/a/src/lib.rs": 650,
            "crates/b/tests/cases.rs": 301,
            "scripts/helper.py": 10,
        },
        configured,
    )
    assert [(item.path, item.message) for item in finding] == [
        (
            "<aggregate Rust>",
            "951 lines exceeds the aggregate ratchet 950",
        )
    ]
    assert MODULE.evaluate(
        {"crates/a/src/lib.rs": 599, "crates/b/tests/cases.rs": 301},
        configured,
    ) == []


def test_evaluate_can_require_the_aggregate_rust_objective() -> None:
    aggregate = MODULE.AggregateRustBudget(
        baseline=1_000,
        ceiling=900,
        ratchet_ceiling=950,
        working_target=850,
    )
    configured = MODULE.Budget(
        production_limit=5_000,
        test_limit=3_000,
        excluded_prefixes=("vendor/",),
        exceptions={},
        aggregate_rust=aggregate,
    )

    assert MODULE.evaluate(
        {"crates/a/src/lib.rs": 599, "crates/b/tests/cases.rs": 301},
        configured,
        require_objective=True,
    ) == []
    findings = MODULE.evaluate(
        {"crates/a/src/lib.rs": 600, "crates/b/tests/cases.rs": 301},
        configured,
        require_objective=True,
    )
    assert [(item.path, item.message) for item in findings] == [
        (
            "<aggregate Rust>",
            "901 lines exceeds the aggregate objective ceiling 900",
        )
    ]


@pytest.mark.parametrize(
    ("require_objective", "expected_exit_code"),
    [(False, 0), (True, 1)],
)
def test_main_applies_requested_aggregate_policy(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    require_objective: bool,
    expected_exit_code: int,
) -> None:
    aggregate = MODULE.AggregateRustBudget(
        baseline=1_000,
        ceiling=900,
        ratchet_ceiling=950,
        working_target=850,
    )
    configured = MODULE.Budget(
        production_limit=5_000,
        test_limit=3_000,
        excluded_prefixes=(),
        exceptions={},
        aggregate_rust=aggregate,
    )
    monkeypatch.setattr(
        MODULE,
        "parse_args",
        lambda: MODULE.argparse.Namespace(
            root=tmp_path,
            baseline=Path("budget.json"),
            write_baseline=False,
            require_objective=require_objective,
            json_out=None,
        ),
    )
    monkeypatch.setattr(MODULE, "load_budget", lambda _path: configured)
    monkeypatch.setattr(MODULE, "tracked_paths", lambda _root: ["lib.rs"])
    monkeypatch.setattr(
        MODULE,
        "collect_counts",
        lambda _root, _paths, _excluded: {"lib.rs": 901},
    )

    assert MODULE.main() == expected_exit_code
    output = capsys.readouterr().out
    assert ("exceeds the aggregate objective ceiling 900" in output) is (
        require_objective
    )


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
                "aggregate_rust": {
                    "baseline": 10_000,
                    "ceiling": 9_000,
                    "ratchet_ceiling": 10_250,
                    "working_target": 8_500,
                },
            }
        ),
        encoding="utf-8",
    )
    parsed = MODULE.load_budget(path)
    assert parsed.excluded_prefixes == ("target/", "vendor/")
    assert parsed.exceptions == {"crates/core/src/lib.rs": 6_000}
    assert parsed.aggregate_rust == MODULE.AggregateRustBudget(
        baseline=10_000,
        ceiling=9_000,
        ratchet_ceiling=10_250,
        working_target=8_500,
    )


def test_load_budget_requires_the_aggregate_rust_contract(tmp_path: Path) -> None:
    path = tmp_path / "budget.json"
    path.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "limits": {"production": 5_000, "test": 3_000},
                "excluded_prefixes": [],
                "exceptions": {},
            }
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="aggregate_rust.*mandatory"):
        MODULE.load_budget(path)


def test_baseline_payload_preserves_aggregate_targets() -> None:
    aggregate = MODULE.AggregateRustBudget(
        baseline=10_000,
        ceiling=9_000,
        ratchet_ceiling=10_250,
        working_target=8_500,
    )
    payload = MODULE.baseline_payload(
        {"crates/core/src/lib.rs": 5_001},
        production_limit=5_000,
        test_limit=3_000,
        excluded_prefixes=("vendor/",),
        aggregate_rust=aggregate,
    )
    assert payload["aggregate_rust"] == {
        "baseline": 10_000,
        "ceiling": 9_000,
        "ratchet_ceiling": 10_250,
        "working_target": 8_500,
    }


@pytest.mark.parametrize(
    ("aggregate", "message"),
    [
        (
            {
                "baseline": 10_000,
                "ceiling": 9_001,
                "ratchet_ceiling": 10_250,
            },
            "at least a 10% reduction",
        ),
        (
            {
                "baseline": 10_000,
                "ceiling": 9_000,
                "ratchet_ceiling": 8_999,
            },
            "ratchet_ceiling must not be below",
        ),
    ],
)
def test_load_budget_rejects_invalid_aggregate_contract(
    tmp_path: Path, aggregate: dict[str, int], message: str
) -> None:
    path = tmp_path / "budget.json"
    path.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "limits": {"production": 5_000, "test": 3_000},
                "excluded_prefixes": [],
                "exceptions": {},
                "aggregate_rust": aggregate,
            }
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match=message):
        MODULE.load_budget(path)


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


def test_tracked_paths_uses_the_complete_nonignored_candidate_tree(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "present.rs").write_text("//! Present.\n", encoding="utf-8")
    observed: dict[str, object] = {}

    def check_output(arguments: list[str], *, cwd: Path) -> bytes:
        observed["arguments"] = arguments
        observed["cwd"] = cwd
        return b"missing.rs\0present.rs\0"

    monkeypatch.setattr(MODULE.subprocess, "check_output", check_output)

    assert MODULE.tracked_paths(tmp_path) == ["present.rs"]
    assert observed == {
        "arguments": [
            "git",
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        "cwd": tmp_path,
    }
