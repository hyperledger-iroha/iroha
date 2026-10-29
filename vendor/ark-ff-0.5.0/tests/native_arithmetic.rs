//! Exact native arithmetic vectors after retirement of alternate build targets.

use ark_ff::{
    BigInt, Fp128, MontBackend, MontConfig, PrimeField, biginteger::arithmetic::widening_mul,
};

#[derive(MontConfig)]
#[modulus = "170141183460469231731687303715884105727"]
#[generator = "43"]
struct NativeFieldConfig;

type NativeField = Fp128<MontBackend<NativeFieldConfig, 2>>;

#[test]
fn widening_multiplication_preserves_both_limbs() {
    for (a, b, high, low) in [
        (0, u64::MAX, 0, 0),
        (1, u64::MAX, 0, u64::MAX),
        (1 << 32, 1 << 32, 1, 0),
        (u64::MAX, 2, 1, u64::MAX - 1),
        (u64::MAX, u64::MAX, u64::MAX - 1, 1),
        (0x8000_0000_0000_0001, 3, 1, 0x8000_0000_0000_0003),
    ] {
        let expected = (u128::from(high) << 64) | u128::from(low);
        assert_eq!(widening_mul(a, b), expected);
        assert_eq!(widening_mul(b, a), expected);
    }
}

#[test]
fn montgomery_reduction_retains_canonical_native_limbs() {
    let modulus_minus_one = BigInt([u64::MAX - 1, 0x7fff_ffff_ffff_ffff]);
    let negative_one = NativeField::from_bigint(modulus_minus_one).unwrap();
    assert_eq!(negative_one.into_bigint(), modulus_minus_one);
    assert_eq!((negative_one * negative_one).into_bigint(), BigInt([1, 0]));

    let high_limb = NativeField::from_bigint(BigInt([0, 1])).unwrap();
    assert_eq!(high_limb.into_bigint(), BigInt([0, 1]));
    // 2^128 = 2 modulo the fixed Mersenne prime 2^127 - 1.
    assert_eq!((high_limb * high_limb).into_bigint(), BigInt([2, 0]));
    assert!(NativeField::from_bigint(BigInt([u64::MAX, 0x7fff_ffff_ffff_ffff])).is_none());
}
