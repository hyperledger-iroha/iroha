// Numeric selector and tuple-compression coverage for the DER STARK.
#[test]
fn bit_selectors_and_tuple_compression_are_numeric_and_total() {
    let bits = [F::ONE, F::ZERO, F::ONE];
    assert_eq!(pack_bits_v1(&bits), F(5));
    for value in 0..8 {
        assert_eq!(
            equality_selector_from_bits_v1(&bits, value),
            F(u64::from(value == 5))
        );
    }
    let challenge = core::array::from_fn(|index| F((index + 2) as u64));
    assert_ne!(compress_tuple_v1(&[F(7), F(11), F(13)], challenge), F::ZERO);
}
