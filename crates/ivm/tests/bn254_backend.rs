//! Tests for BN254 field arithmetic backends.

#![cfg(feature = "ivm_zk_tests")]
use halo2curves::{bn256::Fr, ff::Field};
use ivm::{
    bn254_vec::{FieldElem, MODULUS, add, add_scalar, mul, mul_scalar, sub, sub_scalar},
    field_dispatch::{self, FieldArithmetic},
};
use rand_core::OsRng;

fn lt_modulus(a: &FieldElem) -> bool {
    for i in (0..4).rev() {
        if a.0[i] < MODULUS[i] {
            return true;
        }
        if a.0[i] > MODULUS[i] {
            return false;
        }
    }
    false
}

fn check_backend(name: &str, backend: &'static dyn FieldArithmetic) {
    let _guard = field_dispatch::field_impl_test_lock();
    field_dispatch::set_field_impl_for_tests(backend);

    // random correctness and cross-consistency with scalar reference
    for _ in 0..64 {
        let a_fr = Fr::random(&mut OsRng);
        let b_fr = Fr::random(&mut OsRng);
        let a = FieldElem::from_fr(a_fr);
        let b = FieldElem::from_fr(b_fr);

        let add_ref = add_scalar(a, b);
        let sub_ref = sub_scalar(a, b);
        let mul_ref = mul_scalar(a, b);

        let add_res = add(a, b);
        let sub_res = sub(a, b);
        let mul_res = mul(a, b);

        assert_eq!(add_res, add_ref, "{} add mismatch", name);
        assert_eq!(sub_res, sub_ref, "{} sub mismatch", name);
        assert_eq!(mul_res, mul_ref, "{} mul mismatch", name);
        assert!(lt_modulus(&add_res));
        assert!(lt_modulus(&sub_res));
        assert!(lt_modulus(&mul_res));
    }

    // edge cases
    let zero = FieldElem([0u64; 4]);
    let one = FieldElem::from_fr(Fr::one());
    let pm1 = FieldElem::from_fr(-Fr::one());

    assert_eq!(add(zero, zero).to_fr(), Fr::zero(), "{} add 0+0", name);
    assert_eq!(add(one, zero).to_fr(), Fr::one(), "{} add x+0", name);
    assert_eq!(add(pm1, one).to_fr(), Fr::zero(), "{} add p-1+1", name);
    assert_eq!(sub(one, one).to_fr(), Fr::zero(), "{} sub x-x", name);
    assert_eq!(sub(zero, one).to_fr(), -Fr::one(), "{} sub 0-1", name);
    assert_eq!(mul(zero, pm1).to_fr(), Fr::zero(), "{} mul 0*x", name);
    assert_eq!(mul(one, pm1).to_fr(), -Fr::one(), "{} mul 1*(p-1)", name);
    assert_eq!(mul(pm1, pm1).to_fr(), Fr::one(), "{} mul (p-1)^2", name);

    field_dispatch::clear_field_impl_for_tests();
}

#[test]
fn test_field_backends_consistency() {
    let mut backends: Vec<(&'static dyn FieldArithmetic, &str)> = Vec::new();
    backends.push((&field_dispatch::ScalarField, "scalar"));

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        backends.push((&field_dispatch::Sse2Field, "sse2"));
        if std::is_x86_feature_detected!("avx2") {
            backends.push((&field_dispatch::Avx2Field, "avx2"));
            backends.push((&field_dispatch::Avx512Field, "avx512"));
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            backends.push((&field_dispatch::NeonField, "neon"));
        }
    }

    for (backend, name) in backends {
        check_backend(name, backend);
    }
}
