use ivm::{
    SimdChoice, clear_forced_simd, clear_thread_forced_simd, forced_simd_test_lock,
    set_thread_forced_simd, simd_backend, simd_choice, vector_supported,
};

#[cfg(target_arch = "x86_64")]
#[test]
fn test_vector_detection_x86() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    let _simd_guard = forced_simd_test_lock();
    clear_forced_simd();
    clear_thread_forced_simd();
    let has_avx512_path =
        std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx2");
    let has_avx2 = std::is_x86_feature_detected!("avx2");
    let has_sse2 = std::is_x86_feature_detected!("sse2");
    let expected = has_avx512_path || has_avx2 || has_sse2;
    assert_eq!(vector_supported(), expected);
}

#[cfg(target_arch = "aarch64")]
#[test]
fn test_vector_detection_aarch64() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    let _simd_guard = forced_simd_test_lock();
    clear_forced_simd();
    clear_thread_forced_simd();
    let expected = std::arch::is_aarch64_feature_detected!("neon");
    assert_eq!(vector_supported(), expected);
}

#[test]
fn test_backend_matches_support() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    let _simd_guard = forced_simd_test_lock();
    clear_forced_simd();
    clear_thread_forced_simd();
    let supported = vector_supported();
    let backend = simd_backend();
    assert_eq!(supported, backend != "scalar");
}

#[test]
fn test_simd_choice_consistency() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    let _simd_guard = forced_simd_test_lock();
    clear_forced_simd();
    clear_thread_forced_simd();
    let choice = simd_choice();
    #[cfg(target_arch = "x86_64")]
    let expected =
        if std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx2") {
            SimdChoice::Avx512
        } else if std::is_x86_feature_detected!("avx2") {
            SimdChoice::Avx2
        } else if std::is_x86_feature_detected!("sse2") {
            SimdChoice::Sse2
        } else {
            SimdChoice::Scalar
        };
    #[cfg(target_arch = "aarch64")]
    let expected = if std::arch::is_aarch64_feature_detected!("neon") {
        SimdChoice::Neon
    } else {
        SimdChoice::Scalar
    };
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    let expected = SimdChoice::Scalar;
    assert_eq!(choice, expected);
}

#[test]
fn test_force_simd_override_scalar() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    let _simd_guard = forced_simd_test_lock();
    let prev = set_thread_forced_simd(Some(SimdChoice::Scalar));
    assert_eq!(simd_choice(), SimdChoice::Scalar);
    set_thread_forced_simd(prev);
}

#[cfg(target_arch = "x86_64")]
#[test]
fn test_force_simd_override_avx2() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    let _simd_guard = forced_simd_test_lock();
    let prev = set_thread_forced_simd(Some(SimdChoice::Avx2));
    let expected = if std::is_x86_feature_detected!("avx2") {
        SimdChoice::Avx2
    } else {
        SimdChoice::Scalar
    };
    assert_eq!(simd_choice(), expected);
    set_thread_forced_simd(prev);
}

#[cfg(target_arch = "x86_64")]
#[test]
fn test_force_simd_override_avx512() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    let _simd_guard = forced_simd_test_lock();
    let prev = set_thread_forced_simd(Some(SimdChoice::Avx512));
    let expected =
        if std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx2") {
            SimdChoice::Avx512
        } else {
            SimdChoice::Scalar
        };
    assert_eq!(simd_choice(), expected);
    set_thread_forced_simd(prev);
}

#[cfg(target_arch = "aarch64")]
#[test]
fn test_force_simd_override_neon() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    let _simd_guard = forced_simd_test_lock();
    let prev = set_thread_forced_simd(Some(SimdChoice::Neon));
    let expected = if std::arch::is_aarch64_feature_detected!("neon") {
        SimdChoice::Neon
    } else {
        SimdChoice::Scalar
    };
    assert_eq!(simd_choice(), expected);
    set_thread_forced_simd(prev);
}

#[test]
fn test_clear_forced_simd_restore_detection() {
    let _guard = ivm::field_dispatch::field_impl_test_lock();
    let _simd_guard = forced_simd_test_lock();
    let prev = set_thread_forced_simd(None);
    let expected = simd_choice();
    set_thread_forced_simd(Some(SimdChoice::Scalar));
    assert_eq!(simd_choice(), SimdChoice::Scalar);
    clear_forced_simd();
    clear_thread_forced_simd();
    assert_eq!(simd_choice(), expected);
    set_thread_forced_simd(prev);
}
