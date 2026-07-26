#![cfg(test)]

use norito::core::heuristics;

#[test]
fn select_layout_flags_with_custom_heuristics() {
    // Tweaking compression heuristics must not affect layout flags.
    let h = heuristics::Heuristics {
        min_compress_bytes_cpu: 1,
        ..Default::default()
    };

    let small = heuristics::select_layout_flags_for_size_with(&h, 16);
    assert_eq!(
        small,
        norito::core::default_encode_flags(),
        "heuristics must not alter layout flags"
    );

    let big = heuristics::select_layout_flags_for_size_with(&h, 128);
    assert_eq!(
        big,
        norito::core::default_encode_flags(),
        "heuristics must not alter layout flags"
    );
}
