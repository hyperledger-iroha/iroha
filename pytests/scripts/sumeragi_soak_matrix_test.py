"""Regression tests for the revision-4 Sumeragi soak-matrix geometry."""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "run_sumeragi_soak_matrix.py"
SPEC = importlib.util.spec_from_file_location("run_sumeragi_soak_matrix", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MATRIX = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MATRIX)

STRESS_SCRIPT = ROOT_DIR / "scripts" / "run_sumeragi_stress.py"
STRESS_SPEC = importlib.util.spec_from_file_location("run_sumeragi_stress", STRESS_SCRIPT)
assert STRESS_SPEC is not None and STRESS_SPEC.loader is not None
STRESS = importlib.util.module_from_spec(STRESS_SPEC)
STRESS_SPEC.loader.exec_module(STRESS)


def test_default_matrix_uses_only_bounded_three_f_plus_one_committees() -> None:
    """Every shipped scenario must pass production committee admission."""

    assert [scenario["peers"] for scenario in MATRIX.DEFAULT_MATRIX] == [4, 7, 10]
    assert all(
        MATRIX.is_revision4_committee_size(scenario["peers"])
        for scenario in MATRIX.DEFAULT_MATRIX
    )


@pytest.mark.parametrize("peers", [4, 7, 10, 13, 16, 19, 22, 25, 28, 31])
def test_custom_revision4_committee_sizes_are_admitted(peers: int) -> None:
    """All bounded revision-4 committee sizes remain selectable."""

    scenario = MATRIX.parse_scenario(f"name=peers{peers},peers={peers}")
    assert scenario["peers"] == peers


@pytest.mark.parametrize("peers", [0, 1, 2, 3, 5, 6, 8, 30, 32, 34])
def test_non_revision4_committee_sizes_fail_before_execution(peers: int) -> None:
    """Invalid matrix rows must not reach the Cargo stress launcher."""

    with pytest.raises(MATRIX.ScenarioParseError, match=r"exact revision-4 3f\+1"):
        MATRIX.parse_scenario(f"name=invalid,peers={peers}")


def test_retired_collector_knobs_are_not_accepted_as_matrix_dimensions() -> None:
    """Revision 4 derives Set A/B and proxy-tail routing from the roster."""

    with pytest.raises(MATRIX.ScenarioParseError, match="unsupported revision-4"):
        MATRIX.parse_scenario("name=legacy,peers=4,collectors_k=2")


def test_default_stress_tests_exist_in_the_revision4_harness() -> None:
    """The matrix must never launch names removed with collector/pacemaker V1."""

    source = (
        ROOT_DIR / "integration_tests" / "tests" / "sumeragi_npos_performance.rs"
    ).read_text(encoding="utf-8")
    assert STRESS.DEFAULT_TESTS == (
        "npos_baseline_1s_captures_metrics",
        "npos_queue_backpressure_triggers_metrics",
        "npos_rbc_store_backpressure_records_metrics",
        "npos_rbc_chunk_loss_fault_reports_backlog",
    )
    assert all(f"fn {name}(" in source for name in STRESS.DEFAULT_TESTS)
