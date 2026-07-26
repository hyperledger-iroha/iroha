//! Runtime selection of BN254 field arithmetic backends.
//!
//! `FieldArithmetic` is implemented for scalar and SIMD variants.
//! [`field_impl`] chooses between them based on [`vector::simd_choice`],
//! enabling SSE2/AVX2/AVX-512 or NEON acceleration transparently.
use std::{any::Any, sync::OnceLock};

use crate::vector::{SimdChoice, simd_choice};

pub trait FieldArithmetic: Any + Sync {
    fn add(
        &self,
        a: crate::bn254_vec::FieldElem,
        b: crate::bn254_vec::FieldElem,
    ) -> crate::bn254_vec::FieldElem;
    fn sub(
        &self,
        a: crate::bn254_vec::FieldElem,
        b: crate::bn254_vec::FieldElem,
    ) -> crate::bn254_vec::FieldElem;
    fn mul(
        &self,
        a: crate::bn254_vec::FieldElem,
        b: crate::bn254_vec::FieldElem,
    ) -> crate::bn254_vec::FieldElem;
}

#[derive(Clone, Copy, Debug)]
pub struct ScalarField;
#[derive(Clone, Copy, Debug)]
pub struct Sse2Field;
#[derive(Clone, Copy, Debug)]
pub struct Avx2Field;
#[derive(Clone, Copy, Debug)]
pub struct Avx512Field;
#[derive(Clone, Copy, Debug)]
pub struct NeonField;

static FIELD_IMPL: OnceLock<&'static dyn FieldArithmetic> = OnceLock::new();

use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicU8, Ordering},
};

#[doc(hidden)]
static FIELD_IMPL_TEST_LOCK: Mutex<()> = Mutex::new(());

#[doc(hidden)]
pub fn field_impl_test_lock() -> MutexGuard<'static, ()> {
    FIELD_IMPL_TEST_LOCK
        .lock()
        .expect("field_impl test lock poisoned")
}

#[doc(hidden)]
pub static TEST_CHOICE: AtomicU8 = AtomicU8::new(0);

/// Return the field arithmetic backend selected for this platform.
///
/// The implementation is chosen on first use based on the SIMD
/// capabilities detected by [`vector::simd_choice`]. The selected
/// instance is cached for all subsequent calls.
pub fn field_impl() -> &'static dyn FieldArithmetic {
    match TEST_CHOICE.load(Ordering::SeqCst) {
        1 => return &ScalarField,
        2 => return &Sse2Field,
        3 => return &Avx2Field,
        4 => return &Avx512Field,
        #[cfg(target_arch = "aarch64")]
        5 => return &NeonField,
        _ => {}
    }
    *FIELD_IMPL.get_or_init(|| backend_from_choice(simd_choice()))
}

fn backend_from_choice(choice: SimdChoice) -> &'static dyn FieldArithmetic {
    match choice {
        #[cfg(target_arch = "x86_64")]
        SimdChoice::Avx512 => &Avx512Field,
        #[cfg(target_arch = "x86_64")]
        SimdChoice::Avx2 => &Avx2Field,
        #[cfg(target_arch = "x86_64")]
        SimdChoice::Sse2 => &Sse2Field,
        #[cfg(target_arch = "aarch64")]
        SimdChoice::Neon => &NeonField,
        _ => &ScalarField,
    }
}

pub fn set_field_impl_for_tests(backend: &'static dyn FieldArithmetic) {
    let id = if core::ptr::eq(backend, &ScalarField as &dyn FieldArithmetic) {
        1
    } else if core::ptr::eq(backend, &Sse2Field as &dyn FieldArithmetic) {
        2
    } else if core::ptr::eq(backend, &Avx2Field as &dyn FieldArithmetic) {
        3
    } else if core::ptr::eq(backend, &Avx512Field as &dyn FieldArithmetic) {
        4
    } else {
        #[cfg(target_arch = "aarch64")]
        {
            if core::ptr::eq(backend, &NeonField as &dyn FieldArithmetic) {
                5
            } else {
                1
            }
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            1
        }
    };
    TEST_CHOICE.store(id, Ordering::SeqCst);
}

pub fn clear_field_impl_for_tests() {
    TEST_CHOICE.store(0, Ordering::SeqCst);
}
