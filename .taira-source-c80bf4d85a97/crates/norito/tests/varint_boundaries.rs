//! Validate default compact length-prefix sizing.

#[test]
fn len_prefix_len_defaults_to_compact_width() {
    norito::core::reset_decode_state();
    assert_eq!(
        norito::core::default_encode_flags(),
        norito::core::header_flags::COMPACT_LEN
    );
    for (value, expected) in [
        (0usize, 1usize),
        (127, 1),
        (128, 2),
        (16383, 2),
        (16384, 3),
        (1 << 21, 4),
    ] {
        assert_eq!(norito::core::len_prefix_len(value), expected);
    }
    norito::core::reset_decode_state();
}
