#[cfg(feature = "std")]
use crate::serde::SerdeObject;
#[cfg(any(feature = "std", feature = "derive_serde"))]
use ff::Field;
use ff::{FromUniformBytes, PrimeField};
use rand_core::RngCore;

// Tests to_repr/ from_repr
pub(crate) fn from_to_repr_test<F: PrimeField>(mut rng: impl RngCore, n: usize) {
    // n = 1M
    for _ in 0..n {
        let a = F::random(&mut rng);
        let bytes = a.to_repr();
        let b = F::from_repr(bytes).unwrap();
        assert_eq!(a, b);
    }
}

// Tests to_raw_bytes / from_raw_bytes + read_raw /write_raw
#[cfg(feature = "std")]
pub(crate) fn from_to_raw_bytes_test<F: Field + SerdeObject>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
        let a = F::random(&mut rng);
        let bytes = a.to_raw_bytes();
        let b = F::from_raw_bytes(&bytes).unwrap();
        assert_eq!(a, b);

        let mut buf = Vec::new();
        a.write_raw(&mut buf).unwrap();
        let b = F::read_raw(&mut &buf[..]).unwrap();
        assert_eq!(a, b);
    }
}

// Tests derive_serde
#[cfg(feature = "derive_serde")]
pub(crate) fn derive_serde_test<F>(mut rng: impl RngCore, n: usize)
where
    for<'de> F: Field + serde::Serialize + serde::Deserialize<'de>,
{
    for _ in 0..n {
        let a = F::random(&mut rng);

        // Byte serialization
        let bytes = bincode::serialize(&a).unwrap();

        #[cfg(feature = "std")]
        {
            // std version uses Cursor
            let reader = std::io::Cursor::new(bytes);
            let b = bincode::deserialize_from(reader).unwrap();
            assert_eq!(a, b);
        }
        #[cfg(not(feature = "std"))]
        {
            // no_std version uses slice directly
            let b = bincode::deserialize(&bytes).unwrap();
            assert_eq!(a, b);
        }

        // JSON serialization
        let json = serde_json::to_string(&a).unwrap();
        #[cfg(feature = "std")]
        {
            let reader = std::io::Cursor::new(json);
            let b: F = serde_json::from_reader(reader).unwrap();
            assert_eq!(a, b);
        }
        #[cfg(not(feature = "std"))]
        {
            let b: F = serde_json::from_str(&json).unwrap();
            assert_eq!(a, b);
        }
    }
}

#[cfg(feature = "bits")]
pub(crate) fn test_bits<F: ff::PrimeFieldBits>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
        let a = F::random(&mut rng);
        let bytes = a.to_repr();
        let bits = a.to_le_bits();
        for idx in 0..bits.len() {
            assert_eq!(bits[idx], ((bytes.as_ref()[idx / 8] >> (idx % 8)) & 1) == 1);
        }
    }
}

#[macro_export]
macro_rules! serde_test {
    ($field:ident) => {
        test!(serde, $field, from_to_repr_test, 100_000);
        #[cfg(feature = "std")]
        test!(serde, $field, from_to_raw_bytes_test, 100_000);
        #[cfg(feature = "derive_serde")]
        test!(serde, $field, derive_serde_test, 100_000);
    };

    ($field:ident PrimeFieldBits) => {
        serde_test!($field);
        #[cfg(feature = "bits")]
        test!(serde, $field, test_bits, 100_000);
    };
}

// Out of serde_tests macro, since it needs to be tested for several generic L.
// Tests from_uniform_bytes **for prime fields only**.
pub(crate) fn from_uniform_bytes_test<F: PrimeField, const L: usize>(
    mut rng: impl RngCore,
    n: usize,
) where
    F: FromUniformBytes<L>,
{
    use num_bigint::BigUint;

    let uniform_bytes = [0u8; L];
    assert_eq!(F::from_uniform_bytes(&uniform_bytes), F::ZERO);

    let mut uniform_bytes = [u8::MAX; L];

    for _ in 0..n {
        let e0 = BigUint::from_bytes_le(&uniform_bytes);
        let e0: F = crate::tests::big_to_fe(&e0);

        let e1 = F::from_uniform_bytes(&uniform_bytes);
        assert_eq!(e0, e1);

        rng.fill_bytes(&mut uniform_bytes[..]);
    }
}

#[macro_export]
macro_rules! from_uniform_bytes_test {
    ($field:ident, $size:expr, L $L: expr ) => {
        paste::paste! {
        #[test]
        fn [< from_uniform_bytes_test_ $L>]() {
            use rand_core::SeedableRng;
            use rand_xorshift::XorShiftRng;
            let mut rng = XorShiftRng::from_seed($crate::tests::SEED);
            $crate::tests::field::serde::from_uniform_bytes_test::<$field, $L>(&mut rng, $size);
        }

        }
    };

    ($field:ident,$size:expr, L $L:expr, $(L $rest:expr),+ ) => {
        from_uniform_bytes_test!( $field, $size, L $L );
        from_uniform_bytes_test! { $field, $size, $(L $rest),+ }
    };
}
