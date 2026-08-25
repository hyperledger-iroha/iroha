"""Unit tests for the source file line-budget guard."""

from __future__ import annotations

import importlib.util
import json
import os
import shutil
import subprocess
import sys
from dataclasses import replace
from pathlib import Path
from typing import Any, Callable

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


def init_git_repository(root: Path) -> None:
    """Initialize an unsigned local Git repository for source-budget tests."""
    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "source-budget@example.invalid"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Source Budget Test"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "config", "commit.gpgsign", "false"],
        cwd=root,
        check=True,
    )


def commit_all(root: Path, message: str) -> str:
    """Commit the complete fixture tree and return its exact commit id."""
    subprocess.run(["git", "add", "."], cwd=root, check=True)
    subprocess.run(
        ["git", "commit", "-q", "-m", message],
        cwd=root,
        check=True,
    )
    return subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=root, text=True
    ).strip()


def write_provenance_anchor(root: Path, commit: object) -> None:
    """Write the minimal provenance shape consumed by ``validate_accepted_ref``."""
    path = root / "ci/build_efficiency_provenance.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps({"lineage": {"signed_lock_anchor": {"commit": commit}}}),
        encoding="utf-8",
    )


def allow_minimal_provenance(
    monkeypatch: pytest.MonkeyPatch,
) -> list[tuple[Path, object, object, str | None]]:
    """Replace full provenance validation while retaining strict I/O and Git."""
    calls: list[tuple[Path, object, object, str | None]] = []

    def validate(
        root: Path,
        payload: object,
        store: object,
        *,
        head_commit: str | None = None,
    ) -> dict[str, int]:
        calls.append((root, payload, store, head_commit))
        return {}

    monkeypatch.setattr(MODULE.provenance, "validate_provenance", validate)
    return calls


def write_budget(
    root: Path,
    *,
    production_limit: int = 5_000,
    baseline: int = 100,
    ceiling: int = 80,
) -> None:
    """Write a compact mandatory source-budget policy fixture."""
    (root / "budget.json").write_text(
        json.dumps(
            {
                "schema_version": 1,
                "limits": {"production": production_limit, "test": 3_000},
                "excluded_prefixes": [],
                "exceptions": {},
                "aggregate_rust": {
                    "baseline": baseline,
                    "ceiling": ceiling,
                    "ratchet_ceiling": baseline,
                    "working_target": ceiling,
                },
            }
        ),
        encoding="utf-8",
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
    assert parsed.base_ref is None
    assert parsed.accepted_ref is None


def test_parse_args_exposes_base_comparison_mode(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        sys,
        "argv",
        [str(MODULE_PATH), "--base-ref", "base-commit"],
    )

    parsed = MODULE.parse_args()

    assert parsed.base_ref == "base-commit"
    assert parsed.accepted_ref is None
    assert parsed.require_objective is False
    assert parsed.write_baseline is False


@pytest.mark.parametrize("shadow_name", ["sitecustomize.py", "argparse.py"])
def test_isolated_cli_ignores_hostile_python_import_paths(
    tmp_path: Path,
    shadow_name: str,
) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    checker = scripts / MODULE_PATH.name
    shutil.copy2(MODULE_PATH, checker)
    shutil.copy2(
        MODULE_PATH.with_name("check_build_efficiency_provenance.py"),
        scripts / "check_build_efficiency_provenance.py",
    )
    marker = tmp_path / "shadow-imported"
    (scripts / shadow_name).write_text(
        "from pathlib import Path\n"
        f"Path({str(marker)!r}).write_text('executed', encoding='utf-8')\n"
        "raise SystemExit(0)\n",
        encoding="utf-8",
    )
    environment = os.environ.copy()
    environment["PYTHONPATH"] = str(scripts)

    result = subprocess.run(
        [sys.executable, "-I", "-S", str(checker), "--help"],
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 0
    assert "--accepted-ref" in result.stdout
    assert not marker.exists()


@pytest.mark.parametrize(
    ("path", "expected"),
    [
        ("crates/core/src/lib.rs", False),
        ("crates/core/tests/network.rs", True),
        ("scripts/tests/check_guard_test.py", True),
        ("javascript/client.test.js", True),
        ("crates/core/examples/query.rs", True),
        ("IrohaSwift/Tests/IrohaSwiftTests/Client.swift", True),
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


def test_evaluate_counts_case_insensitive_rust_suffixes() -> None:
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

    findings = MODULE.evaluate(
        {"crates/a/src/lib.RS": 951},
        configured,
    )

    assert [(item.path, item.message) for item in findings] == [
        (
            "<aggregate Rust>",
            "951 lines exceeds the aggregate ratchet 950",
        )
    ]


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


def test_evaluate_against_base_reports_new_and_worsened_findings() -> None:
    aggregate = MODULE.AggregateRustBudget(
        baseline=10_000,
        ceiling=9_000,
        ratchet_ceiling=9_500,
        working_target=8_500,
    )
    configured = MODULE.Budget(
        production_limit=5_000,
        test_limit=3_000,
        excluded_prefixes=(),
        exceptions={},
        aggregate_rust=aggregate,
    )
    base_counts = {
        "crates/core/src/legacy.rs": 6_000,
        "crates/core/src/support.rs": 3_501,
    }
    current_counts = {
        "crates/core/src/legacy.rs": 7_000,
        "crates/core/src/support.rs": 3_501,
        "crates/core/tests/new.rs": 3_001,
    }

    candidate_only, inherited = MODULE.evaluate_against_base(
        current_counts,
        base_counts,
        configured,
    )

    assert {finding.path for finding in candidate_only} == {
        "<aggregate Rust>",
        "crates/core/src/legacy.rs",
        "crates/core/tests/new.rs",
    }
    assert inherited == []


def test_evaluate_against_base_allows_unchanged_or_reduced_debt() -> None:
    aggregate = MODULE.AggregateRustBudget(
        baseline=10_000,
        ceiling=9_000,
        ratchet_ceiling=9_500,
        working_target=8_500,
    )
    configured = MODULE.Budget(
        production_limit=5_000,
        test_limit=3_000,
        excluded_prefixes=(),
        exceptions={},
        aggregate_rust=aggregate,
    )

    candidate_only, inherited = MODULE.evaluate_against_base(
        {"crates/core/src/legacy.rs": 5_999, "crates/core/src/other.rs": 3_001},
        {"crates/core/src/legacy.rs": 6_000, "crates/core/src/other.rs": 3_001},
        configured,
    )

    assert candidate_only == []
    assert [finding.path for finding in inherited] == [
        "crates/core/src/legacy.rs"
    ]


def test_evaluate_against_base_accepts_selected_repair_floor() -> None:
    configured = budget()
    candidate_only, inherited = MODULE.evaluate_against_base(
        {"crates/core/src/legacy.rs": 6_500},
        {"crates/core/src/legacy.rs": 7_000},
        configured,
    )

    assert candidate_only == []
    assert [finding.path for finding in inherited] == [
        "crates/core/src/legacy.rs"
    ]


def test_selected_newer_floor_is_not_componentwise_maxed() -> None:
    configured = budget()
    candidate_only, inherited = MODULE.evaluate_against_base(
        {"crates/core/src/legacy.rs": 6_500},
        {"crates/core/src/legacy.rs": 6_000},
        configured,
    )

    assert [finding.path for finding in candidate_only] == [
        "crates/core/src/legacy.rs"
    ]
    assert inherited == []


def test_current_exception_exactness_cannot_be_inherited() -> None:
    source = "crates/core/src/legacy.rs"
    configured = budget(**{source: 7_000})

    candidate_only, inherited = MODULE.evaluate_against_base(
        {source: 6_500},
        {source: 6_500},
        configured,
    )

    assert [finding.path for finding in candidate_only] == [source]
    assert "shrunk from baseline 7000 to 6500" in candidate_only[0].message
    assert inherited == []


def test_tightened_policy_cannot_manufacture_inherited_debt() -> None:
    floor = budget()
    candidate = replace(floor, production_limit=4_000)

    candidate_only, inherited = MODULE.evaluate_against_base(
        {"crates/core/src/lib.rs": 4_500},
        {"crates/core/src/lib.rs": 4_500},
        candidate,
        comparison_budget=floor,
    )

    assert [finding.path for finding in candidate_only] == [
        "crates/core/src/lib.rs"
    ]
    assert inherited == []


def source_budget_with_aggregate() -> Any:
    """Build the complete policy fixture used by weakening tests."""
    return MODULE.Budget(
        production_limit=5_000,
        test_limit=3_000,
        excluded_prefixes=("target/", "vendor/"),
        exceptions={"crates/core/src/legacy.rs": 7_000},
        aggregate_rust=MODULE.AggregateRustBudget(
            baseline=10_000,
            ceiling=9_000,
            ratchet_ceiling=9_500,
            working_target=8_500,
        ),
    )


@pytest.mark.parametrize(
    ("candidate", "message"),
    [
        pytest.param(
            lambda floor: replace(floor, production_limit=5_001),
            "production limit",
            id="production-limit",
        ),
        pytest.param(
            lambda floor: replace(floor, test_limit=3_001),
            "test limit",
            id="test-limit",
        ),
        pytest.param(
            lambda floor: replace(
                floor,
                excluded_prefixes=(*floor.excluded_prefixes, "generated/"),
            ),
            "expands excluded prefixes",
            id="exclusions",
        ),
        pytest.param(
            lambda floor: replace(
                floor,
                exceptions={**floor.exceptions, "crates/new.rs": 6_000},
            ),
            "adds exceptions",
            id="added-exception",
        ),
        pytest.param(
            lambda floor: replace(
                floor,
                exceptions={"crates/core/src/legacy.rs": 7_001},
            ),
            "raises exceptions",
            id="raised-exception",
        ),
        pytest.param(
            lambda floor: replace(
                floor,
                aggregate_rust=replace(floor.aggregate_rust, baseline=10_001),
            ),
            "baseline changed",
            id="aggregate-baseline",
        ),
        pytest.param(
            lambda floor: replace(
                floor,
                aggregate_rust=replace(floor.aggregate_rust, ceiling=8_999),
            ),
            "ceiling changed",
            id="aggregate-ceiling",
        ),
        pytest.param(
            lambda floor: replace(
                floor,
                aggregate_rust=replace(
                    floor.aggregate_rust,
                    ratchet_ceiling=9_501,
                ),
            ),
            "ratchet ceiling increased",
            id="aggregate-ratchet",
        ),
        pytest.param(
            lambda floor: replace(
                floor,
                aggregate_rust=replace(
                    floor.aggregate_rust,
                    working_target=None,
                ),
            ),
            "working target increased",
            id="removed-working-target",
        ),
    ],
)
def test_candidate_budget_policy_rejects_weakening(
    candidate: Callable[[Any], Any],
    message: str,
) -> None:
    floor = source_budget_with_aggregate()

    with pytest.raises(ValueError, match=message):
        MODULE.validate_candidate_budget_policy(candidate(floor), floor)


def test_candidate_budget_policy_allows_ratchets() -> None:
    floor = source_budget_with_aggregate()
    candidate = replace(
        floor,
        production_limit=4_900,
        test_limit=2_900,
        excluded_prefixes=("vendor/generated/",),
        exceptions={"crates/core/src/legacy.rs": 6_900},
        aggregate_rust=replace(
            floor.aggregate_rust,
            ratchet_ceiling=9_400,
            working_target=8_400,
        ),
    )

    MODULE.validate_candidate_budget_policy(candidate, floor)


def test_validate_accepted_ref_accepts_matching_ancestor(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "source.rs").write_text("//! anchor\n", encoding="utf-8")
    anchor = commit_all(tmp_path, "accepted source budget anchor")
    (tmp_path / "source.rs").write_text("//! candidate\n", encoding="utf-8")
    commit_all(tmp_path, "source budget candidate")
    write_provenance_anchor(tmp_path, anchor)
    calls = allow_minimal_provenance(monkeypatch)

    assert MODULE.validate_accepted_ref(tmp_path, anchor) == anchor
    assert len(calls) == 1
    assert calls[0][0] == tmp_path
    assert calls[0][3] == subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=tmp_path, text=True
    ).strip()


def test_validate_accepted_ref_uses_supplied_candidate_snapshot(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    source.write_text("//! anchor\n", encoding="utf-8")
    anchor = commit_all(tmp_path, "accepted source budget anchor")
    source.write_text("//! candidate\n", encoding="utf-8")
    candidate = commit_all(tmp_path, "source budget candidate")
    write_provenance_anchor(tmp_path, anchor)
    calls = allow_minimal_provenance(monkeypatch)
    delegate = MODULE.provenance.GitObjectStore(tmp_path)

    class MovingHeadStore:
        def __init__(self) -> None:
            self.head_calls = 0

        def __getattr__(self, name: str) -> Any:
            return getattr(delegate, name)

        def head(self) -> str:
            self.head_calls += 1
            return anchor

    store = MovingHeadStore()

    assert (
        MODULE.validate_accepted_ref(
            tmp_path,
            anchor,
            candidate_commit=candidate,
            store=store,
        )
        == anchor
    )
    assert store.head_calls == 0
    assert calls[0][3] == candidate


def test_validate_accepted_ref_rejects_candidate_head(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "source.rs").write_text("//! head\n", encoding="utf-8")
    head = commit_all(tmp_path, "source budget head")
    write_provenance_anchor(tmp_path, head)
    allow_minimal_provenance(monkeypatch)

    with pytest.raises(ValueError, match="strict ancestor of HEAD"):
        MODULE.validate_accepted_ref(tmp_path, "HEAD")


def test_validate_accepted_ref_rejects_ref_manifest_mismatch(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    source.write_text("//! anchor\n", encoding="utf-8")
    anchor = commit_all(tmp_path, "accepted source budget anchor")
    source.write_text("//! descendant\n", encoding="utf-8")
    descendant = commit_all(tmp_path, "source budget descendant")
    assert descendant != anchor
    write_provenance_anchor(tmp_path, anchor)
    allow_minimal_provenance(monkeypatch)

    with pytest.raises(ValueError, match="does not match the signed lock anchor"):
        MODULE.validate_accepted_ref(tmp_path, "HEAD")


def test_validate_accepted_ref_rejects_matching_nonancestor(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "source.rs").write_text("//! head\n", encoding="utf-8")
    head = commit_all(tmp_path, "source budget head")
    tree = subprocess.check_output(
        ["git", "rev-parse", f"{head}^{{tree}}"],
        cwd=tmp_path,
        text=True,
    ).strip()
    nonancestor = subprocess.check_output(
        ["git", "commit-tree", tree, "-p", head],
        cwd=tmp_path,
        input="unattached descendant\n",
        text=True,
    ).strip()
    write_provenance_anchor(tmp_path, nonancestor)
    allow_minimal_provenance(monkeypatch)

    with pytest.raises(ValueError, match="strict ancestor of HEAD"):
        MODULE.validate_accepted_ref(tmp_path, nonancestor)


def test_validate_accepted_ref_requires_full_pinned_provenance(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "source.rs").write_text("//! head\n", encoding="utf-8")
    anchor = commit_all(tmp_path, "head")
    path = tmp_path / "ci/build_efficiency_provenance.json"
    path.parent.mkdir(parents=True)
    path.write_text(
        json.dumps({"lineage": {"signed_lock_anchor": {"commit": anchor}}}),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="provenance manifest has invalid keys"):
        MODULE.validate_accepted_ref(tmp_path, "HEAD")


def test_validate_accepted_ref_uses_strict_json(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "source.rs").write_text("//! head\n", encoding="utf-8")
    commit_all(tmp_path, "head")
    path = tmp_path / "ci/build_efficiency_provenance.json"
    path.parent.mkdir(parents=True)
    path.write_text('{"lineage": {}, "lineage": {}}', encoding="utf-8")

    with pytest.raises(ValueError, match="duplicate JSON key: 'lineage'"):
        MODULE.validate_accepted_ref(tmp_path, "HEAD")


def test_comparison_topology_selects_accepted_after_base(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    source.write_text("//! base\n", encoding="utf-8")
    base = commit_all(tmp_path, "base")
    source.write_text("//! accepted\n", encoding="utf-8")
    accepted = commit_all(tmp_path, "accepted repair")
    source.write_text("//! candidate\n", encoding="utf-8")
    commit_all(tmp_path, "candidate")
    store = MODULE.provenance.GitObjectStore(tmp_path)

    assert MODULE.validate_comparison_topology(store, base, accepted) == accepted


def test_comparison_topology_uses_supplied_candidate_snapshot() -> None:
    base = "1" * 40
    candidate = "2" * 40
    moved_head = "3" * 40

    class MovingHeadStore:
        def __init__(self) -> None:
            self.head_calls = 0

        def head(self) -> str:
            self.head_calls += 1
            return moved_head

        def is_ancestor(self, ancestor: str, descendant: str) -> bool:
            return (ancestor, descendant) == (base, moved_head)

    store = MovingHeadStore()

    with pytest.raises(ValueError, match="base must be a strict ancestor"):
        MODULE.validate_comparison_topology(
            store,
            base,
            candidate_commit=candidate,
        )
    assert store.head_calls == 0


def test_comparison_topology_selects_equal_base_and_accepted(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    source.write_text("//! accepted base\n", encoding="utf-8")
    base = commit_all(tmp_path, "accepted base")
    source.write_text("//! candidate\n", encoding="utf-8")
    commit_all(tmp_path, "candidate")
    store = MODULE.provenance.GitObjectStore(tmp_path)

    assert MODULE.validate_comparison_topology(store, base, base) == base


def test_comparison_topology_selects_base_after_accepted(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    source.write_text("//! accepted\n", encoding="utf-8")
    accepted = commit_all(tmp_path, "accepted repair")
    source.write_text("//! newer base\n", encoding="utf-8")
    base = commit_all(tmp_path, "newer base")
    source.write_text("//! candidate\n", encoding="utf-8")
    commit_all(tmp_path, "candidate")
    store = MODULE.provenance.GitObjectStore(tmp_path)

    assert MODULE.validate_comparison_topology(store, base, accepted) == base


def test_comparison_topology_requires_strict_base_ancestor(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "source.rs").write_text("//! head\n", encoding="utf-8")
    head = commit_all(tmp_path, "head")
    store = MODULE.provenance.GitObjectStore(tmp_path)

    with pytest.raises(ValueError, match="strict ancestor of HEAD"):
        MODULE.validate_comparison_topology(store, head)


def test_comparison_topology_rejects_accepted_candidate_head(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    source.write_text("//! base\n", encoding="utf-8")
    base = commit_all(tmp_path, "base")
    source.write_text("//! accepted head\n", encoding="utf-8")
    accepted = commit_all(tmp_path, "accepted head")
    store = MODULE.provenance.GitObjectStore(tmp_path)

    with pytest.raises(ValueError, match="strict ancestor of HEAD"):
        MODULE.validate_comparison_topology(store, base, accepted)


def test_comparison_topology_rejects_divergent_floor_ancestors(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    source.write_text("//! root\n", encoding="utf-8")
    root = commit_all(tmp_path, "root")
    source.write_text("//! base\n", encoding="utf-8")
    base = commit_all(tmp_path, "base")
    tree = subprocess.check_output(
        ["git", "rev-parse", f"{root}^{{tree}}"],
        cwd=tmp_path,
        text=True,
    ).strip()
    unrelated_accepted = subprocess.check_output(
        ["git", "commit-tree", tree, "-p", root],
        cwd=tmp_path,
        input="unrelated accepted\n",
        text=True,
    ).strip()
    merge_tree = subprocess.check_output(
        ["git", "rev-parse", f"{base}^{{tree}}"],
        cwd=tmp_path,
        text=True,
    ).strip()
    candidate = subprocess.check_output(
        ["git", "commit-tree", merge_tree, "-p", base, "-p", unrelated_accepted],
        cwd=tmp_path,
        input="candidate merge\n",
        text=True,
    ).strip()
    subprocess.run(
        ["git", "update-ref", "HEAD", candidate],
        cwd=tmp_path,
        check=True,
    )
    store = MODULE.provenance.GitObjectStore(tmp_path)

    with pytest.raises(ValueError, match="comparable ancestors"):
        MODULE.validate_comparison_topology(store, base, unrelated_accepted)


def test_base_ref_reads_exact_commit_and_reports_worsened_debt(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "src").mkdir()
    (tmp_path / "excluded").mkdir()
    source = tmp_path / "src/lib.rs"
    source.write_text("//! base\n" * 91, encoding="utf-8")
    (tmp_path / "excluded/ignored.rs").write_text(
        "//! excluded\n" * 500,
        encoding="utf-8",
    )
    (tmp_path / "budget.json").write_text(
        json.dumps(
            {
                "schema_version": 1,
                "limits": {"production": 5_000, "test": 3_000},
                "excluded_prefixes": ["excluded"],
                "exceptions": {},
                "aggregate_rust": {
                    "baseline": 100,
                    "ceiling": 90,
                    "ratchet_ceiling": 100,
                    "working_target": 80,
                },
            }
        ),
        encoding="utf-8",
    )
    base = commit_all(tmp_path, "source budget base")
    source.write_text("//! candidate\n" * 92, encoding="utf-8")
    commit_all(tmp_path, "source budget candidate")
    report_path = tmp_path / "report.json"

    result = subprocess.run(
        [
            sys.executable,
            str(MODULE_PATH),
            "--root",
            str(tmp_path),
            "--baseline",
            "budget.json",
            "--base-ref",
            base,
            "--json-out",
            str(report_path),
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 1
    assert "ERROR: <aggregate Rust>" in result.stdout
    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert report["checked_files"] == 1
    assert report["base_comparison"] == {
        "accepted_commit": None,
        "commit": base,
        "floor_commit": base,
        "inherited_finding_paths": [],
    }
    assert [finding["path"] for finding in report["findings"]] == [
        "<aggregate Rust>"
    ]


def test_base_ref_reads_candidate_sources_from_head_not_worktree(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "src").mkdir()
    source = tmp_path / "src/lib.rs"
    write_budget(tmp_path)
    source.write_text("//! base\n" * 90, encoding="utf-8")
    base = commit_all(tmp_path, "source budget base")
    source.write_text("//! candidate\n" * 91, encoding="utf-8")
    commit_all(tmp_path, "source budget candidate")
    source.write_text("//! hidden worktree truncation\n", encoding="utf-8")
    report_path = tmp_path / "report.json"

    result = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(MODULE_PATH),
            "--root",
            str(tmp_path),
            "--baseline",
            "budget.json",
            "--base-ref",
            base,
            "--json-out",
            str(report_path),
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 1
    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert report["rust_lines"] == 91
    assert [finding["path"] for finding in report["findings"]] == [
        "<aggregate Rust>"
    ]


def test_base_ref_reads_candidate_budget_from_head_not_worktree(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    source.write_text("//! source\n", encoding="utf-8")
    write_budget(tmp_path)
    base = commit_all(tmp_path, "source budget base")
    write_budget(tmp_path, production_limit=6_000)
    commit_all(tmp_path, "weaken candidate source budget")
    write_budget(tmp_path)

    result = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(MODULE_PATH),
            "--root",
            str(tmp_path),
            "--baseline",
            "budget.json",
            "--base-ref",
            base,
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 2
    assert "candidate production limit exceeds" in result.stderr


def test_authenticated_main_rejects_head_movement(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    write_budget(tmp_path)
    source.write_text("//! base\n" * 70, encoding="utf-8")
    base = commit_all(tmp_path, "source budget base")
    source.write_text("//! candidate\n" * 71, encoding="utf-8")
    commit_all(tmp_path, "source budget candidate")
    delegate = MODULE.provenance.GitObjectStore(tmp_path)

    class MovingHeadStore:
        def __init__(self, _root: Path) -> None:
            self.head_calls = 0

        def __getattr__(self, name: str) -> Any:
            return getattr(delegate, name)

        def head(self) -> str:
            self.head_calls += 1
            return base

    moving_store = MovingHeadStore(tmp_path)
    monkeypatch.setattr(
        MODULE.provenance,
        "GitObjectStore",
        lambda _root: moving_store,
    )
    monkeypatch.setattr(
        MODULE,
        "parse_args",
        lambda: MODULE.argparse.Namespace(
            root=tmp_path,
            baseline=Path("budget.json"),
            write_baseline=False,
            require_objective=False,
            base_ref=base,
            accepted_ref=None,
            json_out=None,
        ),
    )

    assert MODULE.main() == 2
    assert moving_store.head_calls == 1
    assert "HEAD changed during source-budget validation" in capsys.readouterr().err


def test_standalone_mode_still_counts_untracked_sources(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    write_budget(tmp_path, baseline=10_000, ceiling=9_000)
    (tmp_path / "untracked.RS").write_text(
        "//! untracked source\n" * 5_001,
        encoding="utf-8",
    )

    result = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(MODULE_PATH),
            "--root",
            str(tmp_path),
            "--baseline",
            "budget.json",
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 1
    assert "rust_lines=5001" in result.stdout
    assert "untracked.RS: 5001 lines exceeds" in result.stdout


def test_base_ref_ignores_post_commit_untracked_sources(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    write_budget(tmp_path)
    source.write_text("//! base\n" * 70, encoding="utf-8")
    base = commit_all(tmp_path, "source budget base")
    source.write_text("//! candidate\n" * 71, encoding="utf-8")
    commit_all(tmp_path, "source budget candidate")
    (tmp_path / "generated.rs").write_text(
        "//! post-commit generated source\n" * 5_001,
        encoding="utf-8",
    )

    result = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(MODULE_PATH),
            "--root",
            str(tmp_path),
            "--baseline",
            "budget.json",
            "--base-ref",
            base,
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 0
    assert "rust_lines=71" in result.stdout


def test_base_after_accepted_is_the_floor_and_reopened_debt_fails(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    init_git_repository(tmp_path)
    source = tmp_path / "source.rs"
    write_budget(tmp_path, ceiling=70)
    source.write_text("//! accepted debt\n" * 90, encoding="utf-8")
    accepted = commit_all(tmp_path, "accepted repair")
    source.write_text("//! repaired base\n" * 80, encoding="utf-8")
    base = commit_all(tmp_path, "newer repaired base")
    source.write_text("//! reopened candidate debt\n" * 85, encoding="utf-8")
    commit_all(tmp_path, "candidate reopens debt")
    monkeypatch.setattr(
        MODULE,
        "parse_args",
        lambda: MODULE.argparse.Namespace(
            root=tmp_path,
            baseline=Path("budget.json"),
            write_baseline=False,
            require_objective=False,
            base_ref=base,
            accepted_ref=accepted,
            json_out=None,
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "validate_accepted_ref",
        lambda _root, _ref, *, candidate_commit, store: accepted,
    )

    assert MODULE.main() == 1
    output = capsys.readouterr().out
    assert f"floor={base}" in output
    assert "85 lines exceeds the aggregate objective ceiling 70" in output


def test_base_after_accepted_supplies_the_policy_floor(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "source.rs").write_text("//! source\n", encoding="utf-8")
    write_budget(tmp_path, production_limit=6_000)
    accepted = commit_all(tmp_path, "accepted policy")
    write_budget(tmp_path, production_limit=5_000)
    base = commit_all(tmp_path, "tightened base policy")
    write_budget(tmp_path, production_limit=6_000)
    commit_all(tmp_path, "candidate reopens policy")
    monkeypatch.setattr(
        MODULE,
        "parse_args",
        lambda: MODULE.argparse.Namespace(
            root=tmp_path,
            baseline=Path("budget.json"),
            write_baseline=False,
            require_objective=False,
            base_ref=base,
            accepted_ref=accepted,
            json_out=None,
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "validate_accepted_ref",
        lambda _root, _ref, *, candidate_commit, store: accepted,
    )

    assert MODULE.main() == 2
    assert "candidate production limit exceeds" in capsys.readouterr().err


def test_base_comparison_uses_floor_exclusions_before_candidate_tightening(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "src").mkdir()
    (tmp_path / "generated").mkdir()
    (tmp_path / "src/lib.rs").write_text("//! governed\n" * 80, encoding="utf-8")
    (tmp_path / "generated/table.rs").write_text(
        "//! newly governed\n" * 20,
        encoding="utf-8",
    )
    budget_path = tmp_path / "budget.json"
    payload = {
        "schema_version": 1,
        "limits": {"production": 5_000, "test": 3_000},
        "excluded_prefixes": ["generated"],
        "exceptions": {},
        "aggregate_rust": {
            "baseline": 100,
            "ceiling": 90,
            "ratchet_ceiling": 100,
            "working_target": 80,
        },
    }
    budget_path.write_text(json.dumps(payload), encoding="utf-8")
    base = commit_all(tmp_path, "excluded source base")
    payload["excluded_prefixes"] = []
    budget_path.write_text(json.dumps(payload), encoding="utf-8")
    commit_all(tmp_path, "tighten source exclusions")

    result = subprocess.run(
        [
            sys.executable,
            str(MODULE_PATH),
            "--root",
            str(tmp_path),
            "--baseline",
            "budget.json",
            "--base-ref",
            base,
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 1
    assert "100 lines exceeds the aggregate objective ceiling 90" in result.stdout


def test_base_comparison_rejects_candidate_exception_addition(
    tmp_path: Path,
) -> None:
    init_git_repository(tmp_path)
    (tmp_path / "src").mkdir()
    source = "src/legacy.rs"
    (tmp_path / source).write_text("//! legacy\n" * 6_001, encoding="utf-8")
    budget_path = tmp_path / "budget.json"
    payload = {
        "schema_version": 1,
        "limits": {"production": 5_000, "test": 3_000},
        "excluded_prefixes": [],
        "exceptions": {},
        "aggregate_rust": {
            "baseline": 7_000,
            "ceiling": 6_300,
            "ratchet_ceiling": 7_000,
            "working_target": 6_000,
        },
    }
    budget_path.write_text(json.dumps(payload), encoding="utf-8")
    base = commit_all(tmp_path, "unexcepted source base")
    payload["exceptions"] = {source: 6_001}
    budget_path.write_text(json.dumps(payload), encoding="utf-8")
    commit_all(tmp_path, "attempt exception addition")

    result = subprocess.run(
        [
            sys.executable,
            str(MODULE_PATH),
            "--root",
            str(tmp_path),
            "--baseline",
            "budget.json",
            "--base-ref",
            base,
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 2
    assert "candidate source budget adds exceptions" in result.stderr


def test_evaluate_against_base_enforces_objective_after_base_is_clean() -> None:
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

    candidate_only, inherited = MODULE.evaluate_against_base(
        {"crates/core/src/lib.rs": 901},
        {"crates/core/src/lib.rs": 900},
        configured,
    )

    assert [finding.path for finding in candidate_only] == ["<aggregate Rust>"]
    assert inherited == []


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
            base_ref=None,
            accepted_ref=None,
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


@pytest.mark.parametrize("write_baseline", [False, True])
def test_main_rejects_accepted_ref_without_base_ref_before_other_modes(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    write_baseline: bool,
) -> None:
    monkeypatch.setattr(
        MODULE,
        "parse_args",
        lambda: MODULE.argparse.Namespace(
            root=tmp_path,
            baseline=Path("budget.json"),
            write_baseline=write_baseline,
            require_objective=False,
            base_ref=None,
            accepted_ref="accepted-anchor",
            json_out=None,
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "load_budget",
        lambda _path: pytest.fail("accepted/base validation must run first"),
    )
    monkeypatch.setattr(MODULE, "tracked_paths", lambda _root: [])
    monkeypatch.setattr(
        MODULE,
        "collect_counts",
        lambda _root, _paths, _excluded: {},
    )

    assert MODULE.main() == 2
    assert "--accepted-ref requires --base-ref" in capsys.readouterr().err


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


def test_load_budget_rejects_duplicate_json_keys(tmp_path: Path) -> None:
    path = tmp_path / "budget.json"
    path.write_text(
        '{"schema_version": 1, "schema_version": 1}',
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="duplicate JSON key: 'schema_version'"):
        MODULE.load_budget(path)


def test_parse_budget_rejects_colliding_normalized_exceptions() -> None:
    payload = {
        "schema_version": 1,
        "limits": {"production": 5_000, "test": 3_000},
        "excluded_prefixes": [],
        "exceptions": {"crates\\legacy.rs": 6_000, "crates/legacy.rs": 6_000},
        "aggregate_rust": {
            "baseline": 10_000,
            "ceiling": 9_000,
            "ratchet_ceiling": 10_000,
        },
    }

    with pytest.raises(ValueError, match="repeats normalized exception path"):
        MODULE.parse_budget(payload)


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
