"""Source-drift checks for offline DEEP and retained FASTPQ diagnostics."""

import pytest

from scripts.fastpq.check_compact_source_budget import budget, load_sources


def test_source_owned_offline_deep_bound_and_retained_diagnostic_floor() -> None:
    """The sole offline profile fits the byte cap without qualifying admission."""
    result = budget(load_sources())
    current = result["shared_diagnostic"]
    assert current["mandatory_row_bytes"] == 1_026_000
    assert current["mandatory_mixed_quotient_bytes"] == 24_000
    assert current["mandatory_raw_floor"] == 1_050_000
    assert current["framed_maximum"] == 4_017_376
    assert current["framed_maximum_over_segment_cap"] == 3_493_088
    assert result["limits"] == {"segment": 524_288, "axt_inner": 1_048_576}
    assert current["fri_value_bytes_at_independent_maxima"] == 272_512
    assert current["fri_frontier_bytes_at_independent_maxima"] == 875_760
    assert result["offline_deep"] == {"max_frame_bytes": 500_783, "headroom": 23_505}
    assert set(result["retained_test_metadata"]) == {"ordinary-single", "axt-single"}
    assert result["production_qualified"] is False


@pytest.mark.parametrize(
    ("name", "old", "new"),
    [
        ("row", "struct RowValues([u64; 342]);", "struct RowValues([u64; 341]);"),
        ("shared", "proof.rows.len() < query_count", "proof.rows.len() < 1"),
        ("shared", "mixed: GoldilocksFp4V1,", "mixed: u64,"),
        ("backend", 'mod deep_engine;', 'mod admitted_deep_engine;'),
        ("deep", "caller_max_bytes.min(MAX_FRAME_BYTES)", "caller_max_bytes"),
        ("producer", "SHARED_FRAME_BOUND: usize = MAX_FRAME_BYTES", "SHARED_FRAME_BOUND: usize = 4_017_376"),
        ("deep", "PROOF_BYTE_TARGET: usize = 512 * 1024", "PROOF_BYTE_TARGET: usize = 513 * 1024"),
        ("deep", "MAX_FRAME_BYTES: usize = 500_783", "MAX_FRAME_BYTES: usize = 500_784"),
        ("deep", "values: FriValues,", "values: Vec<Fp4>,"),
        ("fiber", "1 + arity * Fp4::BYTES", "8 + arity * Fp4::BYTES"),
        ("geometry", "FRI_ARITIES: [usize; 5] = [16, 16, 8, 8, 4]", "FRI_ARITIES: [usize; 5] = [16, 16, 8, 4, 8]"),
        ("artifact", "quantity_diagnostic_profile_id()\n}", "FastpqCompactProfileIdV1([0; 32])\n}"),
        ("public_columns", "PUBLIC_COLUMN_COUNT: usize = 41", "PUBLIC_COLUMN_COUNT: usize = 40"),
        ("backend", '#[path = "backend/deep_engine.rs"]', '#[cfg(test)]\n#[path = "backend/deep_engine.rs"]'),
        ("backend", '#[cfg(test)]\n#[path = "backend/compact_quantity_diagnostic.rs"]', '#[path = "backend/compact_quantity_diagnostic.rs"]'),
    ],
)
def test_source_drift_fails_closed(name: str, old: str, new: str) -> None:
    """A changed layout or admission boundary invalidates this size model."""
    sources = load_sources()
    assert old in sources[name]
    sources[name] = sources[name].replace(old, new, 1)
    with pytest.raises(ValueError):
        budget(sources)
