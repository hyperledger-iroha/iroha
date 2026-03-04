//! Validate `len_prefix_len` under canonical (non-compact) builds.

#[test]
fn len_prefix_len_defaults_to_fixed_width() {
    norito::core::reset_decode_state();
    assert_eq!(norito::core::default_encode_flags(), 0);
    for value in [0usize, 127, 128, 16383, 16384, 1 << 21] {
        assert_eq!(norito::core::len_prefix_len(value), 8);
    }
    norito::core::reset_decode_state();
}
