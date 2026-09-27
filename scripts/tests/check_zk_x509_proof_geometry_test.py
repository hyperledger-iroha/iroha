"""Non-Cargo source consistency tests for the blocked zk-X509 proof layout."""

from pathlib import Path
from runpy import run_path

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


def test_current_source_exact_opening_and_partial_redesign_bound() -> None:
    result = SCREEN["screen"](*_sources())
    assert result["proof_cap_bytes"] == 9_437_184
    assert result["combined_current_max_bytes"] == 19_156_074
    assert result["combined_excess_bytes"] == 9_718_890
    assert result["current_trace_columns"] == 5_623
    assert result["current_trace_opening_bytes"] == 12_235_648
    assert result["main_section_cap_bytes"] == 6_740_870
    assert result["max_columns_if_non_trace_fixed"] == 1_156
    assert result["minimum_columns_to_remove_if_non_trace_fixed"] == 4_467
    assert result["log19_opening_bytes"] == 8_077_312
    assert result["remaining_non_log19_trace_columns"] == 1_911
    assert result["p256_signature_count"] == 5
    assert result["p256_log19_opening_bytes"] == 5_211_520
    assert result["combined_after_hypothetically_removing_log19_p256_openings_bytes"] == 13_944_554
    assert result["remaining_excess_after_log19_p256_removal_bytes"] == 4_507_370
    assert result["p256_all_group_trace_columns"] == 4_000
    assert result["p256_all_group_opening_bytes"] == 8_704_000
    assert result["remaining_non_p256_trace_columns"] == 1_623
    assert result["combined_after_hypothetically_removing_all_p256_trace_openings_bytes"] == 10_452_074
    assert result["remaining_excess_after_all_p256_trace_opening_removal_bytes"] == 1_014_890
    assert result["minimum_additional_non_p256_columns_to_replace_if_other_bytes_fixed"] == 467
    assert result["combined_after_hypothetically_removing_all_log19_openings_bytes"] == 11_078_762
    assert result["remaining_excess_after_log19_removal_bytes"] == 1_641_578
    assert result["minimum_additional_non_log19_columns_to_replace_if_other_bytes_fixed"] == 755
    assert result["production_qualified"] is False


def test_source_drift_cannot_silently_reuse_old_component_bound() -> None:
    profile, stark, credential, accumulator, native_test = _sources()
    changed = profile.replace(
        "ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1: u32 = 19_156_074;",
        "ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1: u32 = 19_156_075;",
        1,
    )
    assert changed != profile
    with pytest.raises(SCREEN["GeometryError"], match="component sizes disagree"):
        SCREEN["screen"](changed, stark, credential, accumulator, native_test)


def test_opening_width_must_match_independent_rust_budget_assertion() -> None:
    profile, stark, credential, accumulator, native_test = _sources()
    changed = profile.replace(
        "ZK_X509_SHARED_STARK_WIDE_MAIN_TRACE_OPENING_BYTES_V1: u32 = 12_235_648;",
        "ZK_X509_SHARED_STARK_WIDE_MAIN_TRACE_OPENING_BYTES_V1: u32 = 12_237_824;",
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
