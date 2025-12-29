#![cfg(test)]

use norito::core::heuristics;

#[test]
fn select_layout_flags_with_custom_heuristics() {
    // Force small thresholds to toggle bits for small sizes
    let h = heuristics::Heuristics {
        enable_compact_seq_len_up_to: 64,
        enable_varint_offsets_up_to: 64,
        ..Default::default()
    };

    let small = heuristics::select_layout_flags_for_size_with(&h, 16);
    assert_eq!(small, 0, "sequential layout must not enable adaptive flags");

    let big = heuristics::select_layout_flags_for_size_with(&h, 128);
    assert_eq!(big, 0, "sequential layout must not enable adaptive flags");
}
