"""Source-drift checks for the current and test-only FASTPQ proof geometry."""

import pytest

from scripts.fastpq.check_compact_source_budget import budget, load_sources


def test_source_owned_current_floor_and_test_only_deep_boundary() -> None:
    """The present complete-row proof cannot satisfy either fixed payload cap."""
    result = budget(load_sources())
    current = result["current"]
    assert current["mandatory_row_bytes"] == 1_026_000
    assert current["mandatory_mixed_quotient_bytes"] == 24_000
    assert current["mandatory_raw_floor"] == 1_050_000
    assert current["framed_maximum"] == 4_017_376
    assert current["framed_maximum_over_segment_cap"] == 3_493_088
    assert result["limits"] == {"segment": 524_288, "axt_inner": 1_048_576}
    assert current["fri_value_bytes_at_independent_maxima"] == 272_512
    assert current["fri_frontier_bytes_at_independent_maxima"] == 875_760
    assert result["deep_test_only_dto"] == {"max_frame_bytes": 506_351, "headroom": 17_937}
    assert result["production_qualified"] is False


@pytest.mark.parametrize(
    ("name", "old", "new"),
    [
        ("row", "struct RowValues([u64; 342]);", "struct RowValues([u64; 341]);"),
        ("shared", "proof.rows.len() < query_count", "proof.rows.len() < 1"),
        ("shared", "mixed: GoldilocksFp4V1,", "mixed: u64,"),
        ("backend", 'mod deep_engine;', 'mod admitted_deep_engine;'),
        ("deep", "caller_max_bytes.min(MAX_FRAME_BYTES)", "caller_max_bytes"),
        ("producer", "SHARED_FRAME_BOUND: usize = 4_017_376", "SHARED_FRAME_BOUND: usize = 4_017_377"),
        ("deep", "PROOF_BYTE_TARGET: usize = 512 * 1024", "PROOF_BYTE_TARGET: usize = 513 * 1024"),
    ],
)
def test_source_drift_fails_closed(name: str, old: str, new: str) -> None:
    """A changed layout or admission boundary invalidates this size model."""
    sources = load_sources()
    assert old in sources[name]
    sources[name] = sources[name].replace(old, new, 1)
    with pytest.raises(ValueError):
        budget(sources)
