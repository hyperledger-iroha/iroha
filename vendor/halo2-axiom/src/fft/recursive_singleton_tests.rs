//! Empty and singleton recursive FFTs are identities for fields and projective curves.

use super::*;
use group::Group;
use halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq};

fn identity<F: Field, G: FftGroup<F> + PartialEq + std::fmt::Debug>(value: G) {
    let data = FFTData::new(1, F::ONE, F::ONE);
    assert!(data.stages.is_empty());
    clear_scratch::<G>();
    for inverse in [false, true] {
        let mut empty = Vec::<G>::new();
        fft(&mut empty, F::ONE, 0, &data, inverse);
        recursive_fft(&data, &mut empty, inverse);
        assert!(empty.is_empty());
        let mut input = vec![value];
        let pointer = input.as_ptr();
        let capacity = input.capacity();
        fft(&mut input, F::ONE, 0, &data, inverse);
        assert_eq!(input, [value]);
        recursive_fft(&data, &mut input, inverse);
        assert_eq!(input, [value]);
        assert_eq!(input.as_ptr(), pointer);
        assert_eq!(input.capacity(), capacity);
        assert!(!FFT_SCRATCH_POOL.with(|pool| pool.borrow().contains_key(&TypeId::of::<G>())));
    }
}

#[test]
fn both_pasta_recursive_fft_empty_and_singleton_fields_preserve_values_without_scratch() {
    identity::<Fp, Fp>(Fp::from(29));
    identity::<Fq, Fq>(Fq::from(37));
}

fn curve<C: CurveAffine>() {
    identity::<C::Scalar, C::Curve>(C::Curve::generator());
    identity::<C::Scalar, C::Curve>(C::Curve::identity());
}

#[test]
fn both_pasta_recursive_fft_empty_and_singleton_groups_preserve_values_without_scratch() {
    curve::<EqAffine>();
    curve::<EpAffine>();
}
