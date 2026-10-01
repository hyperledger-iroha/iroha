"""Source-drift checks for offline DEEP and retained FASTPQ diagnostics."""

import pytest

from scripts.fastpq.check_compact_source_budget import budget, load_sources


def test_source_owned_q77_bound_and_separate_hypothetical_q375_floor() -> None:
    """The sole offline profile fits the byte cap without qualifying admission."""
    result = budget(load_sources())
    current = result["hypothetical_full_group_q375"]
    assert current["mandatory_row_bytes"] == 1_026_000
    assert current["mandatory_mixed_quotient_bytes"] == 24_000
    assert current["mandatory_raw_floor"] == 1_050_000
    assert current["raw_floor_over_segment_cap"] == 525_712
    assert current["raw_floor_over_axt_cap"] == 1_424
    assert current["framed_maximum"] == 4_017_376
    assert current["framed_maximum_over_segment_cap"] == 3_493_088
    assert result["limits"] == {"segment": 524_288, "axt_inner": 1_048_576}
    assert current["fri_value_bytes_at_independent_maxima"] == 272_512
    assert current["fri_frontier_bytes_at_independent_maxima"] == 875_760
    assert result["offline_deep"] == {"max_frame_bytes": 500_084, "headroom": 24_204}
    assert set(result["retained_test_labels"]) == {"ordinary-single", "axt-single"}
    assert result["current_geometry"] == {
        "queries": 77, "trace_rows": 65_536, "lde_rows": 8_388_608,
        "constraints": 923, "columns": 342, "retained_columns": 301,
        "fp4_bytes": 32, "digest_bytes": 32,
    }
    assert result["fixture_evidence"] == "not checked by this source screen"
    assert result["production_qualified"] is False


@pytest.mark.parametrize(
    ("name", "old", "new"),
    [
        ("deep_row", "struct RowValues([u64; COMMITTED_COLUMN_COUNT]);", "struct RowValues([u64; 300]);"),
        ("deep", "proof.rows.len() != QUERY_COUNT", "proof.rows.len() < 1"),
        ("deep", "low: Fp4,", "low: u64,"),
        ("backend", 'mod deep_engine;', 'mod admitted_deep_engine;'),
        ("deep", "caller_max_bytes.min(MAX_FRAME_BYTES)", "caller_max_bytes"),
        ("deep", "composition_mask: Fp4,", "composition_mask: u64,"),
        ("deep", "MAX_FRAME_BYTES: usize = 500_084", "MAX_FRAME_BYTES: usize = 500_085"),
        ("deep_tests", "assert_eq!(bytes.len(), 500_084);", "assert_eq!(bytes.len(), 500_083);"),
        ("deep_row", "COMMITTED_COLUMN_COUNT * size_of::<u64>()", "COMMITTED_COLUMN_COUNT * 4"),
        ("deep_fri", "1 + (arity - 1) * Fp4::BYTES", "8 + (arity - 1) * Fp4::BYTES"),
        ("deep_fri", "Self::Eight(_) => 8,", "Self::Eight(_) => 4,"),
        ("deep_fri", "writer.write_all(&[self.arity_byte()])", "writer.write_all(&[self.arity_byte(), 0])"),
        ("deep_geometry", "[16, 16, 8, 8, 4]", "[16, 16, 8, 8, 8]"),
        ("resources", "QUANTITY_SHARED_FRAME_BOUND: usize = deep_proof::MAX_FRAME_BYTES", "QUANTITY_SHARED_FRAME_BOUND: usize = 4_017_376"),
        ("resources", "QUANTITY_QUERY_COUNT: usize = deep_geometry::QUERY_COUNT", "QUANTITY_QUERY_COUNT: usize = 375"),
        ("resources", "maximum_segment_frame_bytes: QUANTITY_SHARED_FRAME_BOUND", "maximum_segment_frame_bytes: 0"),
        ("producer", ".check_proving_limits(proving, verification)?", ".check_proving_limits(proving, verification).ok()"),
        ("producer", "QUANTITY_SHARED_FRAME_BOUND as SHARED_FRAME_BOUND", "QUANTITY_SHARED_FRAME_BOUND as RETIRED_FRAME_BOUND"),
        ("deep", "PROOF_BYTE_TARGET: usize = 512 * 1024", "PROOF_BYTE_TARGET: usize = 513 * 1024"),
        ("deep", "values: FriValues,", "values: Vec<Fp4>,"),
        ("deep", "values: RowValues,", "values: Vec<u64>,"),
        ("deep_geometry", "LDE_ROWS: usize = 8_388_608", "LDE_ROWS: usize = 4_194_304"),
        ("artifact", "quantity_diagnostic_profile_id()\n}", "FastpqCompactProfileIdV1([0; 32])\n}"),
        ("public_columns", "PUBLIC_COLUMN_COUNT: usize = 41", "PUBLIC_COLUMN_COUNT: usize = 40"),
        ("backend", '#[path = "backend/deep_engine.rs"]', '#[cfg(test)]\n#[path = "backend/deep_engine.rs"]'),
        ("backend", '#[cfg(test)]\n#[path = "backend/compact_quantity_diagnostic.rs"]', '#[path = "backend/compact_quantity_diagnostic.rs"]'),
        ("challenge", "QUERY_COUNT: usize = 77;", "QUERY_COUNT: usize = 76;"),
        ("challenge", "TRACE_MASK_COEFFICIENTS: usize = 2 * QUERY_COUNT + 8", "TRACE_MASK_COEFFICIENTS: usize = 2 * QUERY_COUNT + 7"),
        ("deep_geometry", "QUERY_COUNT: usize = fastpq_isi::compact_challenge::QUERY_COUNT", "QUERY_COUNT: usize = 77"),
        ("digest", "BYTES: usize = 32", "BYTES: usize = 48"),
        ("digest", "writer.write_all(&self.0)", "writer.write_all(&self.0[..31])"),
        ("deep_fri", "Sixteen([Fp4; 15])", "Sixteen([Fp4; 16])"),
        ("deep", "ARITIES[round] - 1,", "ARITIES[round],"),
        ("deep", "exact_indices(proof.quotients.iter().map(|row| row.index), queries)", "exact_indices(proof.quotients.iter().map(|row| row.index), &[])"),
        ("deep", "row_root: Digest,\n    pub(super) quotient_root: Digest,", "quotient_root: Digest,\n    pub(super) row_root: Digest,"),
        ("deep", "pub(super) quotient: Vec<Fp4>,", "pub(super) quotient: Vec<u64>,"),
        ("engine", "limits.max_proof_bytes.min(deep_proof::MAX_FRAME_BYTES)", "limits.max_proof_bytes"),
        ("retained", "assert_eq!(hex::encode(Sha256::digest(&bytes)), pin.sha256)", "assert_eq!(pin.sha256, pin.sha256)"),
    ],
)
def test_source_drift_fails_closed(name: str, old: str, new: str) -> None:
    """A changed layout or admission boundary invalidates this size model."""
    sources = load_sources()
    assert old in sources[name]
    sources[name] = sources[name].replace(old, new, 1)
    with pytest.raises(ValueError):
        budget(sources)
