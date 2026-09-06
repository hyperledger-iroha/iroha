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


def test_parse_args_rejects_retired_objective_flag(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(sys, "argv", [str(MODULE_PATH), "--require-objective"])
    with pytest.raises(SystemExit) as error:
        MODULE.parse_args()
    assert error.value.code == 2


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


@pytest.mark.parametrize("oversized", [False, True])
def test_main_reports_all_rust_lines_and_enforces_only_file_limits(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    oversized: bool,
) -> None:
    counts = {f"crates/core/src/file_{i}.rs": 5_000 for i in range(1_002)}
    counts["scripts/helper.py"] = 25
    if oversized:
        counts["crates/core/tests/network.rs"] = 3_001
    monkeypatch.setattr(
        MODULE, "parse_args", lambda: MODULE.argparse.Namespace(
            root=tmp_path, baseline=Path("budget.json"), write_baseline=False,
            json_out=Path("-"),
        ),
    )
    monkeypatch.setattr(MODULE, "load_budget", lambda _path: budget())
    monkeypatch.setattr(MODULE, "tracked_paths", lambda _root: list(counts))
    monkeypatch.setattr(MODULE, "collect_counts", lambda *_args: counts)

    assert MODULE.main() == int(oversized)
    output = capsys.readouterr()
    report = json.loads(output.out)
    assert report["schema_version"] == 2
    assert report["checked_files"] == len(counts)
    assert report["rust_lines"] == 5_010_000 + (3_001 if oversized else 0)
    assert report["production_limit"] == 5_000
    assert report["test_limit"] == 3_000
    assert len(report["findings"]) == int(oversized)
    assert "aggregate_rust" not in report
    assert "objective_met" not in report
    assert "rust_lines=" in output.err


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


def budget_payload() -> dict[str, object]:
    """Return a complete active file-budget fixture."""
    return {
        "schema_version": 2,
        "limits": {"production": 5_000, "test": 3_000},
        "excluded_prefixes": ["vendor", "target/"],
        "exceptions": {"crates/core/src/lib.rs": 6_000},
    }


def test_load_budget_validates_and_normalizes(tmp_path: Path) -> None:
    path = tmp_path / "budget.json"
    path.write_text(json.dumps(budget_payload()), encoding="utf-8")
    parsed = MODULE.load_budget(path)
    assert parsed.excluded_prefixes == ("target/", "vendor/")
    assert parsed.exceptions == {"crates/core/src/lib.rs": 6_000}
    assert (parsed.production_limit, parsed.test_limit) == (5_000, 3_000)


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        ("schema_version", 1, "schema_version must be 2"),
        ("aggregate_rust", {"ceiling": 4_540_000}, "keys must be exactly"),
        ("limits", {"production": True, "test": 3_000}, "non-negative integer"),
        ("limits", {"production": 0, "test": 3_000}, "greater than zero"),
        ("limits", {"production": 5_000, "test": 3_000, "total": 1}, "only production and test"),
        ("excluded_prefixes", ["../hidden"], "invalid excluded prefix"),
        ("exceptions", {"../hidden.rs": 6_000}, "invalid repository-relative path"),
    ],
)
def test_load_budget_rejects_invalid_or_retired_policy(
    tmp_path: Path, field: str, value: object, message: str,
) -> None:
    payload = budget_payload()
    payload[field] = value
    path = tmp_path / "budget.json"
    path.write_text(json.dumps(payload), encoding="utf-8")
    with pytest.raises(ValueError, match=message):
        MODULE.load_budget(path)


@pytest.mark.parametrize(
    ("counts", "expected_exit", "exceptions"),
    [
        ({"crates/core/src/lib.rs": 5_500}, 0, {"crates/core/src/lib.rs": 5_500}),
        ({"crates/core/src/lib.rs": 5_000}, 0, {}),
        ({}, 0, {}),
        ({"crates/core/src/lib.rs": 6_001}, 2, {"crates/core/src/lib.rs": 6_000}),
        ({"crates/core/src/new.rs": 5_001}, 2, {"crates/core/src/lib.rs": 6_000}),
    ],
)
def test_write_baseline_only_ratchets_existing_exceptions_down(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
    counts: dict[str, int], expected_exit: int, exceptions: dict[str, int],
) -> None:
    path = tmp_path / "budget.json"
    original = json.dumps(budget_payload())
    path.write_text(original, encoding="utf-8")
    monkeypatch.setattr(
        MODULE, "parse_args", lambda: MODULE.argparse.Namespace(
            root=tmp_path, baseline=Path("budget.json"), write_baseline=True,
            json_out=None,
        ),
    )
    monkeypatch.setattr(MODULE, "tracked_paths", lambda _root: list(counts))
    monkeypatch.setattr(MODULE, "collect_counts", lambda *_args: counts)
    assert MODULE.main() == expected_exit
    refreshed = json.loads(path.read_text(encoding="utf-8"))
    assert refreshed["exceptions"] == exceptions
    assert refreshed["limits"] == budget_payload()["limits"]
    assert "aggregate_rust" not in refreshed
    if expected_exit:
        assert path.read_text(encoding="utf-8") == original


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


def test_collect_counts_includes_unstaged_and_untracked_sources(tmp_path: Path) -> None:
    MODULE.subprocess.run(["git", "init", "-q", str(tmp_path)], check=True)
    (tmp_path / "tracked.rs").write_text("old\n", encoding="utf-8")
    MODULE.subprocess.run(["git", "add", "tracked.rs"], cwd=tmp_path, check=True)
    (tmp_path / "tracked.rs").write_text("changed\nsecond\n", encoding="utf-8")
    (tmp_path / "new.rs").write_text("new\n", encoding="utf-8")
    (tmp_path / ".gitignore").write_text("ignored.rs\n", encoding="utf-8")
    (tmp_path / "ignored.rs").write_text("ignored\n", encoding="utf-8")
    (tmp_path / "vendor").mkdir()
    (tmp_path / "vendor" / "external.rs").write_text("external\n", encoding="utf-8")
    assert MODULE.collect_counts(
        tmp_path, MODULE.tracked_paths(tmp_path), ("vendor/",)
    ) == {"new.rs": 1, "tracked.rs": 2}
