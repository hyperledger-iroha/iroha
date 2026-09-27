"""Non-Cargo source consistency tests for the blocked zk-X509 proof layout."""

from pathlib import Path
from runpy import run_path
from itertools import combinations

import pytest


SCREEN = run_path(
    str(Path(__file__).resolve().parents[1] / "check_zk_x509_proof_geometry.py")
)


def _sources() -> tuple[str, str, str, str, str]:
    return tuple(path.read_text() for path in (
        SCREEN["PROFILE"],
        SCREEN["STARK"],
        SCREEN["CREDENTIAL"],
        SCREEN["ACCUMULATOR"],
        SCREEN["NATIVE_TEST"],
    ))


def test_current_source_complete_relation_exact_wire_bound() -> None:
    result = SCREEN["screen"](*_sources())
    assert result["proof_cap_bytes"] == 9_437_184
    assert result["combined_current_max_bytes"] == 9_204_362
    assert result["headroom_bytes"] == 232_822
    assert result["current_trace_columns"] == 5_623
    assert result["current_trace_opening_bytes"] == 6_117_824
    assert result["complete_deep_opening_bytes"] == 412_864
    assert result["main_section_cap_bytes"] == 7_936_966
    assert result["logical_main_groups"] == 6
    assert result["physical_main_base_roots"] == 1
    assert result["p256_signature_count"] == 5
    assert result["p256_all_group_trace_columns"] == 4_000
    assert result["remaining_non_p256_trace_columns"] == 1_623
    assert result["production_qualified"] is False
    assert result["implemented_paired_fri_saving_bytes"] == 867_456
    assert result["current_main_inner_max_bytes"] == 7_692_192
    assert result["current_ca_inner_max_bytes"] == 1_498_816


def test_closed_frontier_bound_matches_exhaustive_small_trees() -> None:
    def actual_frontier(leaves: tuple[int, ...], count: int) -> int:
        nodes = set(leaves)
        total = 0
        while count > 1:
            total += sum((index ^ 1) not in nodes for index in nodes)
            nodes = {index // 2 for index in nodes}
            count //= 2
        return total

    for count in (1, 2, 4, 8, 16):
        maximum = 0
        for opened in range(1, count + 1):
            maximum = max(maximum, max(actual_frontier(leaves, count)
                for leaves in combinations(range(count), opened)))
            assert SCREEN["_maximum_frontier"](count, opened) == maximum


def test_candidate_cannot_assume_unimplemented_pair_commitments() -> None:
    profile, stark, credential, accumulator, native_test = _sources()
    changed = accumulator.replace("AggregateFriCommitmentLayoutV1::Paired", "AggregateFriCommitmentLayoutV1::Scalar")
    assert changed != accumulator
    with pytest.raises(SCREEN["GeometryError"], match="paired FRI commitment layout is missing"):
        SCREEN["screen"](profile, stark, credential, changed, native_test)


def test_source_drift_cannot_silently_reuse_old_component_bound() -> None:
    profile, stark, credential, accumulator, native_test = _sources()
    changed = profile.replace(
        "ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1: u32 = 9_204_362;",
        "ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1: u32 = 9_204_363;",
        1,
    )
    assert changed != profile
    with pytest.raises(SCREEN["GeometryError"], match="component sizes disagree"):
        SCREEN["screen"](changed, stark, credential, accumulator, native_test)


def test_opening_width_must_match_independent_rust_budget_assertion() -> None:
    profile, stark, credential, accumulator, native_test = _sources()
    changed = profile.replace(
        "ZK_X509_SHARED_STARK_WIDE_MAIN_TRACE_OPENING_BYTES_V1: u32 = 6_117_824;",
        "ZK_X509_SHARED_STARK_WIDE_MAIN_TRACE_OPENING_BYTES_V1: u32 = 6_118_912;",
        1,
    )
    assert changed != profile
    with pytest.raises(SCREEN["GeometryError"], match="trace opening width disagrees"):
        SCREEN["screen"](changed, stark, credential, accumulator, native_test)


def test_fixed_proof_ceiling_cannot_be_increased_in_screen() -> None:
    profile, stark, credential, accumulator, native_test = _sources()
    changed = profile.replace(
        "ZK_X509_MAX_PROOF_BYTES_V1: u32 = 9 * 1024 * 1024;",
        "ZK_X509_MAX_PROOF_BYTES_V1: u32 = 10 * 1024 * 1024;",
        1,
    )
    assert changed != profile
    with pytest.raises(SCREEN["GeometryError"], match="9 MiB contract changed"):
        SCREEN["screen"](changed, stark, credential, accumulator, native_test)


def test_all_p256_width_must_match_independent_rust_assertion() -> None:
    profile, stark, credential, accumulator, native_test = _sources()
    changed = stark.replace(
        "assert!(P256_MAIN_LOG16_AUX_WIDTH_V1 == 375);",
        "assert!(P256_MAIN_LOG16_AUX_WIDTH_V1 == 374);",
        1,
    )
    assert changed != stark
    with pytest.raises(SCREEN["GeometryError"], match="all-group width changed"):
        SCREEN["screen"](profile, changed, credential, accumulator, native_test)


def test_current_rows_require_the_implemented_reduced_layout() -> None:
    profile, stark, credential, accumulator, native_test = _sources()
    changed = accumulator.replace("AggregateTraceLayoutV1::GroupedCurrent", "AggregateTraceLayoutV1::GroupedCurrentNext")
    with pytest.raises(SCREEN["GeometryError"], match="current-only commitment layout is missing"):
        SCREEN["screen"](profile, stark, credential, changed, native_test)
