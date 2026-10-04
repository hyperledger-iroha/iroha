//! Exact native AES round routines; entry requires the CPU owner capability gate.

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
pub(super) unsafe fn aesenc_armv8(state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    use core::arch::aarch64::*;
    let s = unsafe { vld1q_u8(state.as_ptr()) };
    let k = unsafe { vld1q_u8(rk.as_ptr()) };
    // AESE consumes its round key before SubBytes/ShiftRows, whereas our
    // AESENC contract adds the key after MixColumns. Keep the order exact.
    let r = vaeseq_u8(s, vdupq_n_u8(0));
    let r = vaesmcq_u8(r);
    let r = veorq_u8(r, k);
    let mut out = [0u8; 16];
    unsafe { vst1q_u8(out.as_mut_ptr(), r) };
    out
}
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
pub(super) unsafe fn aesdec_armv8(state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    use core::arch::aarch64::*;
    let s = unsafe { vld1q_u8(state.as_ptr()) };
    let k = unsafe { vld1q_u8(rk.as_ptr()) };
    // `aesdec` takes the raw round key and must match:
    // AddRoundKey -> InvMixColumns -> InvShiftRows -> InvSubBytes.
    // ARM's AESD round does not accept that contract directly, so we apply the
    // XOR and InvMixColumns first, then finish with an inverse "last" round.
    let r = veorq_u8(s, k);
    let r = vaesimcq_u8(r);
    let r = vaesdq_u8(r, vdupq_n_u8(0));
    let mut out = [0u8; 16];
    unsafe { vst1q_u8(out.as_mut_ptr(), r) };
    out
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "aes")]
pub(super) unsafe fn aesenc_aesni(state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;
    unsafe {
        let s = _mm_loadu_si128(state.as_ptr() as *const __m128i);
        let k = _mm_loadu_si128(rk.as_ptr() as *const __m128i);
        let r = _mm_aesenc_si128(s, k);
        let mut out = core::mem::MaybeUninit::<[u8; 16]>::uninit();
        _mm_storeu_si128(out.as_mut_ptr() as *mut __m128i, r);
        out.assume_init()
    }
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "aes")]
pub(super) unsafe fn aesdec_aesni(state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;
    unsafe {
        let s = _mm_loadu_si128(state.as_ptr() as *const __m128i);
        let k = _mm_loadu_si128(rk.as_ptr() as *const __m128i);
        // `_mm_aesdec_si128` expects the equivalent-inverse-cipher key
        // schedule, but the public `aesdec` API takes the raw round key. Match
        // the scalar inverse round by applying AddRoundKey and InvMixColumns
        // explicitly, then finish with an inverse "last" round.
        let r = _mm_xor_si128(s, k);
        let r = _mm_aesimc_si128(r);
        let r = _mm_aesdeclast_si128(r, _mm_setzero_si128());
        let mut out = core::mem::MaybeUninit::<[u8; 16]>::uninit();
        _mm_storeu_si128(out.as_mut_ptr() as *mut __m128i, r);
        out.assume_init()
    }
}
