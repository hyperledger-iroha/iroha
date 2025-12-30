#![cfg(feature = "ivm_zk_tests")]
use halo2curves::bn256::Fr;
#[cfg(target_arch = "aarch64")]
use ivm::field_dispatch::NeonField;
#[cfg(target_arch = "x86_64")]
use ivm::field_dispatch::{Avx2Field, Avx512Field, Sse2Field};
use ivm::{
    SimdChoice,
    bn254_vec::FieldElem,
    field_dispatch::{
        FieldArithmetic, ScalarField, clear_field_impl_for_tests, field_impl,
        set_field_impl_for_tests,
    },
    simd_choice,
};

#[test]
fn test_scalar_add_sub_mul() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    let backend = ScalarField;
    let a = FieldElem::from_fr(Fr::from(5u64));
    let b = FieldElem::from_fr(Fr::from(7u64));
    let add = backend.add(a, b);
    assert_eq!(add.to_fr(), Fr::from(12u64));
    let sub = backend.sub(add, b);
    assert_eq!(sub.to_fr(), Fr::from(5u64));
    let mul = backend.mul(a, b);
    assert_eq!(mul.to_fr(), Fr::from(35u64));
}

#[test]
fn test_field_impl_cached() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    clear_field_impl_for_tests();
    let p1 = field_impl();
    let p2 = field_impl();
    assert_eq!(p1.type_id(), p2.type_id(), "backend should be cached");
}

#[test]
fn test_field_impl_ops() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    clear_field_impl_for_tests();
    let backend = field_impl();
    let a = FieldElem::from_fr(Fr::from(3u64));
    let b = FieldElem::from_fr(Fr::from(4u64));
    let res = backend.mul(a, b);
    assert_eq!(res.to_fr(), Fr::from(12u64));
}

#[test]
fn test_runtime_detection_matches_choice() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    clear_field_impl_for_tests();
    let backend = field_impl();
    let expected: &'static dyn FieldArithmetic = match simd_choice() {
        #[cfg(target_arch = "x86_64")]
        SimdChoice::Avx512 => &Avx512Field,
        #[cfg(target_arch = "x86_64")]
        SimdChoice::Avx2 => &Avx2Field,
        #[cfg(target_arch = "x86_64")]
        SimdChoice::Sse2 => &Sse2Field,
        #[cfg(target_arch = "aarch64")]
        SimdChoice::Neon => &NeonField,
        _ => &ScalarField,
    };
    assert_eq!(backend.type_id(), expected.type_id());
}

#[test]
fn test_forced_scalar_backend() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    set_field_impl_for_tests(&ScalarField);
    let backend = field_impl();
    assert_eq!(backend.type_id(), core::any::TypeId::of::<ScalarField>());
    let a = FieldElem::from_fr(Fr::from(2u64));
    let b = FieldElem::from_fr(Fr::from(3u64));
    let res = backend.mul(a, b);
    assert_eq!(res.to_fr(), Fr::from(6u64));
    clear_field_impl_for_tests();
}
