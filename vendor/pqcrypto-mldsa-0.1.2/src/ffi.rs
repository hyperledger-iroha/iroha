//! Foreign function interfaces
//!
//! This module defines the foreign function interface for the following
//! crypto implementations from PQClean:
//!
//!  * ml-dsa-44
//!  * ml-dsa-65
//!  * ml-dsa-87
// This file has been generated from PQClean.
// Find the templates in pqcrypto-template
use core::ffi::c_int;

// ensures we link correctly
#[allow(unused_imports)]
use pqcrypto_internals::*;

pub const PQCLEAN_MLDSA44_CLEAN_CRYPTO_SECRETKEYBYTES: usize = 2560;
pub const PQCLEAN_MLDSA44_CLEAN_CRYPTO_PUBLICKEYBYTES: usize = 1312;
pub const PQCLEAN_MLDSA44_CLEAN_CRYPTO_BYTES: usize = 2420;

#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLDSA44_AVX2_CRYPTO_SECRETKEYBYTES: usize = 2560;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLDSA44_AVX2_CRYPTO_PUBLICKEYBYTES: usize = 1312;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLDSA44_AVX2_CRYPTO_BYTES: usize = 2420;

#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLDSA44_AARCH64_CRYPTO_SECRETKEYBYTES: usize = 2560;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLDSA44_AARCH64_CRYPTO_PUBLICKEYBYTES: usize = 1312;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLDSA44_AARCH64_CRYPTO_BYTES: usize = 2420;

pub const PQCLEAN_MLDSA65_CLEAN_CRYPTO_SECRETKEYBYTES: usize = 4032;
pub const PQCLEAN_MLDSA65_CLEAN_CRYPTO_PUBLICKEYBYTES: usize = 1952;
pub const PQCLEAN_MLDSA65_CLEAN_CRYPTO_BYTES: usize = 3309;

#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLDSA65_AVX2_CRYPTO_SECRETKEYBYTES: usize = 4032;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLDSA65_AVX2_CRYPTO_PUBLICKEYBYTES: usize = 1952;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLDSA65_AVX2_CRYPTO_BYTES: usize = 3309;

#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLDSA65_AARCH64_CRYPTO_SECRETKEYBYTES: usize = 4032;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLDSA65_AARCH64_CRYPTO_PUBLICKEYBYTES: usize = 1952;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLDSA65_AARCH64_CRYPTO_BYTES: usize = 3309;

pub const PQCLEAN_MLDSA87_CLEAN_CRYPTO_SECRETKEYBYTES: usize = 4896;
pub const PQCLEAN_MLDSA87_CLEAN_CRYPTO_PUBLICKEYBYTES: usize = 2592;
pub const PQCLEAN_MLDSA87_CLEAN_CRYPTO_BYTES: usize = 4627;

#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLDSA87_AVX2_CRYPTO_SECRETKEYBYTES: usize = 4896;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLDSA87_AVX2_CRYPTO_PUBLICKEYBYTES: usize = 2592;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLDSA87_AVX2_CRYPTO_BYTES: usize = 4627;

#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLDSA87_AARCH64_CRYPTO_SECRETKEYBYTES: usize = 4896;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLDSA87_AARCH64_CRYPTO_PUBLICKEYBYTES: usize = 2592;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLDSA87_AARCH64_CRYPTO_BYTES: usize = 4627;

#[link(name = "ml-dsa-44_clean")]
extern "C" {
    pub fn PQCLEAN_MLDSA44_CLEAN_crypto_sign_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    pub fn PQCLEAN_MLDSA44_CLEAN_crypto_sign(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA44_CLEAN_crypto_sign_ctx(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA44_CLEAN_crypto_sign_open(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        pk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA44_CLEAN_crypto_sign_open_ctx(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA44_CLEAN_crypto_sign_signature(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA44_CLEAN_crypto_sign_signature_ctx(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA44_CLEAN_crypto_sign_verify(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        pk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA44_CLEAN_crypto_sign_verify_ctx(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;

}

#[cfg(enable_x86_avx2)]
#[link(name = "ml-dsa-44_avx2")]
extern "C" {
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA44_AVX2_crypto_sign_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA44_AVX2_crypto_sign(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA44_AVX2_crypto_sign_ctx(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA44_AVX2_crypto_sign_open(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA44_AVX2_crypto_sign_open_ctx(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA44_AVX2_crypto_sign_signature(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA44_AVX2_crypto_sign_signature_ctx(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA44_AVX2_crypto_sign_verify(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA44_AVX2_crypto_sign_verify_ctx(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;

}

#[cfg(enable_aarch64_neon)]
#[link(name = "ml-dsa-44_aarch64")]
extern "C" {
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA44_AARCH64_crypto_sign_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA44_AARCH64_crypto_sign(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA44_AARCH64_crypto_sign_ctx(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA44_AARCH64_crypto_sign_open(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA44_AARCH64_crypto_sign_open_ctx(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA44_AARCH64_crypto_sign_signature(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA44_AARCH64_crypto_sign_signature_ctx(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA44_AARCH64_crypto_sign_verify(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA44_AARCH64_crypto_sign_verify_ctx(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;

}

#[link(name = "ml-dsa-65_clean")]
extern "C" {
    pub fn PQCLEAN_MLDSA65_CLEAN_crypto_sign_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    pub fn PQCLEAN_MLDSA65_CLEAN_crypto_sign(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA65_CLEAN_crypto_sign_ctx(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA65_CLEAN_crypto_sign_open(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        pk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA65_CLEAN_crypto_sign_open_ctx(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA65_CLEAN_crypto_sign_signature(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA65_CLEAN_crypto_sign_signature_ctx(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA65_CLEAN_crypto_sign_verify(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        pk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA65_CLEAN_crypto_sign_verify_ctx(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;

}

#[cfg(enable_x86_avx2)]
#[link(name = "ml-dsa-65_avx2")]
extern "C" {
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA65_AVX2_crypto_sign_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA65_AVX2_crypto_sign(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA65_AVX2_crypto_sign_ctx(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA65_AVX2_crypto_sign_open(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA65_AVX2_crypto_sign_open_ctx(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA65_AVX2_crypto_sign_signature(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA65_AVX2_crypto_sign_signature_ctx(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA65_AVX2_crypto_sign_verify(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA65_AVX2_crypto_sign_verify_ctx(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;

}

#[cfg(enable_aarch64_neon)]
#[link(name = "ml-dsa-65_aarch64")]
extern "C" {
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA65_AARCH64_crypto_sign_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA65_AARCH64_crypto_sign(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA65_AARCH64_crypto_sign_ctx(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA65_AARCH64_crypto_sign_open(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA65_AARCH64_crypto_sign_open_ctx(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA65_AARCH64_crypto_sign_signature(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA65_AARCH64_crypto_sign_signature_ctx(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA65_AARCH64_crypto_sign_verify(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA65_AARCH64_crypto_sign_verify_ctx(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;

}

#[link(name = "ml-dsa-87_clean")]
extern "C" {
    pub fn PQCLEAN_MLDSA87_CLEAN_crypto_sign_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    pub fn PQCLEAN_MLDSA87_CLEAN_crypto_sign(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA87_CLEAN_crypto_sign_ctx(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA87_CLEAN_crypto_sign_open(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        pk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA87_CLEAN_crypto_sign_open_ctx(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA87_CLEAN_crypto_sign_signature(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA87_CLEAN_crypto_sign_signature_ctx(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA87_CLEAN_crypto_sign_verify(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        pk: *const u8,
    ) -> c_int;
    pub fn PQCLEAN_MLDSA87_CLEAN_crypto_sign_verify_ctx(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;

}

#[cfg(enable_x86_avx2)]
#[link(name = "ml-dsa-87_avx2")]
extern "C" {
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA87_AVX2_crypto_sign_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA87_AVX2_crypto_sign(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA87_AVX2_crypto_sign_ctx(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA87_AVX2_crypto_sign_open(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA87_AVX2_crypto_sign_open_ctx(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA87_AVX2_crypto_sign_signature(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA87_AVX2_crypto_sign_signature_ctx(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA87_AVX2_crypto_sign_verify(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLDSA87_AVX2_crypto_sign_verify_ctx(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;

}

#[cfg(enable_aarch64_neon)]
#[link(name = "ml-dsa-87_aarch64")]
extern "C" {
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA87_AARCH64_crypto_sign_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA87_AARCH64_crypto_sign(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA87_AARCH64_crypto_sign_ctx(
        sm: *mut u8,
        smlen: *mut usize,
        msg: *const u8,
        len: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA87_AARCH64_crypto_sign_open(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA87_AARCH64_crypto_sign_open_ctx(
        m: *mut u8,
        mlen: *mut usize,
        sm: *const u8,
        smlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA87_AARCH64_crypto_sign_signature(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA87_AARCH64_crypto_sign_signature_ctx(
        sig: *mut u8,
        siglen: *mut usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        sk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA87_AARCH64_crypto_sign_verify(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLDSA87_AARCH64_crypto_sign_verify_ctx(
        sig: *const u8,
        siglen: usize,
        m: *const u8,
        mlen: usize,
        ctx: *const u8,
        ctxlen: usize,
        pk: *const u8,
    ) -> c_int;

}

#[cfg(test)]
mod test_mldsa44_clean {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use rand::prelude::*;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut rng = rand::rng();
            let mut mlen: usize = rng.random::<u16>() as usize;
            let msg: Vec<u8> = (0..mlen).map(|_| rng.random()).collect();

            let mut pk = vec![0u8; PQCLEAN_MLDSA44_CLEAN_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLDSA44_CLEAN_CRYPTO_SECRETKEYBYTES];
            let mut pk_alt = vec![0u8; PQCLEAN_MLDSA44_CLEAN_CRYPTO_PUBLICKEYBYTES];
            let mut sk_alt = vec![0u8; PQCLEAN_MLDSA44_CLEAN_CRYPTO_SECRETKEYBYTES];
            let mut detached_sig = vec![0u8; PQCLEAN_MLDSA44_CLEAN_CRYPTO_BYTES];
            let mut sm = Vec::with_capacity(mlen + PQCLEAN_MLDSA44_CLEAN_CRYPTO_BYTES);
            let mut smlen = 0;

            assert_eq!(
                0,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign"
            );
            sm.set_len(smlen);

            let mut unpacked_m = Vec::with_capacity(mlen + PQCLEAN_MLDSA44_CLEAN_CRYPTO_BYTES);
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr(),
                ),
                "sign_open"
            );
            unpacked_m.set_len(mlen);
            assert_eq!(unpacked_m, msg);

            // check verification fails with wrong pk
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_keypair(pk_alt.as_mut_ptr(), sk_alt.as_mut_ptr()),
                "keypair"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_open"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_signature_ctx(
                    detached_sig.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign_signature"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA44_CLEAN_CRYPTO_BYTES,
                "Signed message length should be ≤ CRYPTO_BYTES"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_verify alt pk"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len() - 1,
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify wrong length"
            );

            let ctx = vec![1u8; 10];
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    sk.as_ptr()
                ),
                "Sign ctx call"
            );
            sm.set_len(smlen);
            assert!(smlen >= PQCLEAN_MLDSA44_CLEAN_CRYPTO_BYTES, "too small sig");
            assert!(
                smlen <= PQCLEAN_MLDSA44_CLEAN_CRYPTO_BYTES + msg.len(),
                "too big sig"
            );

            let mut mlen: usize = 0;
            unpacked_m.clear();
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx"
            );
            unpacked_m.set_len(mlen);
            assert!(unpacked_m == msg, "unequal messages");

            let ctx_alt = vec![0u8; 10];
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx_alt.as_ptr(),
                    ctx_alt.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx with alt context"
            );
        }
    }
}

#[cfg(all(test, enable_x86_avx2, feature = "avx2"))]
mod test_mldsa44_avx2 {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use rand::prelude::*;
    use std::is_x86_feature_detected;

    #[test]
    fn test_ffi() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        unsafe {
            let mut rng = rand::rng();
            let mut mlen: usize = rng.random::<u16>() as usize;
            let msg: Vec<u8> = (0..mlen).map(|_| rng.random()).collect();

            let mut pk = vec![0u8; PQCLEAN_MLDSA44_AVX2_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLDSA44_AVX2_CRYPTO_SECRETKEYBYTES];
            let mut pk_alt = vec![0u8; PQCLEAN_MLDSA44_AVX2_CRYPTO_PUBLICKEYBYTES];
            let mut sk_alt = vec![0u8; PQCLEAN_MLDSA44_AVX2_CRYPTO_SECRETKEYBYTES];
            let mut detached_sig = vec![0u8; PQCLEAN_MLDSA44_AVX2_CRYPTO_BYTES];
            let mut sm = Vec::with_capacity(mlen + PQCLEAN_MLDSA44_AVX2_CRYPTO_BYTES);
            let mut smlen = 0;

            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign"
            );
            sm.set_len(smlen);

            let mut unpacked_m = Vec::with_capacity(mlen + PQCLEAN_MLDSA44_AVX2_CRYPTO_BYTES);
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr(),
                ),
                "sign_open"
            );
            unpacked_m.set_len(mlen);
            assert_eq!(unpacked_m, msg);

            // check verification fails with wrong pk
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_keypair(pk_alt.as_mut_ptr(), sk_alt.as_mut_ptr()),
                "keypair"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_open"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_signature_ctx(
                    detached_sig.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign_signature"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA44_AVX2_CRYPTO_BYTES,
                "Signed message length should be ≤ CRYPTO_BYTES"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_verify alt pk"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len() - 1,
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify wrong length"
            );

            let ctx = vec![1u8; 10];
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    sk.as_ptr()
                ),
                "Sign ctx call"
            );
            sm.set_len(smlen);
            assert!(smlen >= PQCLEAN_MLDSA44_AVX2_CRYPTO_BYTES, "too small sig");
            assert!(
                smlen <= PQCLEAN_MLDSA44_AVX2_CRYPTO_BYTES + msg.len(),
                "too big sig"
            );

            let mut mlen: usize = 0;
            unpacked_m.clear();
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx"
            );
            unpacked_m.set_len(mlen);
            assert!(unpacked_m == msg, "unequal messages");

            let ctx_alt = vec![0u8; 10];
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx_alt.as_ptr(),
                    ctx_alt.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx with alt context"
            );
        }
    }
}

#[cfg(all(test, enable_aarch64_neon, feature = "neon"))]
mod test_mldsa44_aarch64 {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use rand::prelude::*;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut rng = rand::rng();
            let mut mlen: usize = rng.random::<u16>() as usize;
            let msg: Vec<u8> = (0..mlen).map(|_| rng.random()).collect();

            let mut pk = vec![0u8; PQCLEAN_MLDSA44_AARCH64_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLDSA44_AARCH64_CRYPTO_SECRETKEYBYTES];
            let mut pk_alt = vec![0u8; PQCLEAN_MLDSA44_AARCH64_CRYPTO_PUBLICKEYBYTES];
            let mut sk_alt = vec![0u8; PQCLEAN_MLDSA44_AARCH64_CRYPTO_SECRETKEYBYTES];
            let mut detached_sig = vec![0u8; PQCLEAN_MLDSA44_AARCH64_CRYPTO_BYTES];
            let mut sm = Vec::with_capacity(mlen + PQCLEAN_MLDSA44_AARCH64_CRYPTO_BYTES);
            let mut smlen = 0;

            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign"
            );
            sm.set_len(smlen);

            let mut unpacked_m = Vec::with_capacity(mlen + PQCLEAN_MLDSA44_AARCH64_CRYPTO_BYTES);
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr(),
                ),
                "sign_open"
            );
            unpacked_m.set_len(mlen);
            assert_eq!(unpacked_m, msg);

            // check verification fails with wrong pk
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_keypair(
                    pk_alt.as_mut_ptr(),
                    sk_alt.as_mut_ptr()
                ),
                "keypair"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_open"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_signature_ctx(
                    detached_sig.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign_signature"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA44_AARCH64_CRYPTO_BYTES,
                "Signed message length should be ≤ CRYPTO_BYTES"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_verify alt pk"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len() - 1,
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify wrong length"
            );

            let ctx = vec![1u8; 10];
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    sk.as_ptr()
                ),
                "Sign ctx call"
            );
            sm.set_len(smlen);
            assert!(
                smlen >= PQCLEAN_MLDSA44_AARCH64_CRYPTO_BYTES,
                "too small sig"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA44_AARCH64_CRYPTO_BYTES + msg.len(),
                "too big sig"
            );

            let mut mlen: usize = 0;
            unpacked_m.clear();
            assert_eq!(
                0,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx"
            );
            unpacked_m.set_len(mlen);
            assert!(unpacked_m == msg, "unequal messages");

            let ctx_alt = vec![0u8; 10];
            assert_eq!(
                -1,
                PQCLEAN_MLDSA44_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx_alt.as_ptr(),
                    ctx_alt.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx with alt context"
            );
        }
    }
}

#[cfg(test)]
mod test_mldsa65_clean {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use rand::prelude::*;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut rng = rand::rng();
            let mut mlen: usize = rng.random::<u16>() as usize;
            let msg: Vec<u8> = (0..mlen).map(|_| rng.random()).collect();

            let mut pk = vec![0u8; PQCLEAN_MLDSA65_CLEAN_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLDSA65_CLEAN_CRYPTO_SECRETKEYBYTES];
            let mut pk_alt = vec![0u8; PQCLEAN_MLDSA65_CLEAN_CRYPTO_PUBLICKEYBYTES];
            let mut sk_alt = vec![0u8; PQCLEAN_MLDSA65_CLEAN_CRYPTO_SECRETKEYBYTES];
            let mut detached_sig = vec![0u8; PQCLEAN_MLDSA65_CLEAN_CRYPTO_BYTES];
            let mut sm = Vec::with_capacity(mlen + PQCLEAN_MLDSA65_CLEAN_CRYPTO_BYTES);
            let mut smlen = 0;

            assert_eq!(
                0,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign"
            );
            sm.set_len(smlen);

            let mut unpacked_m = Vec::with_capacity(mlen + PQCLEAN_MLDSA65_CLEAN_CRYPTO_BYTES);
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr(),
                ),
                "sign_open"
            );
            unpacked_m.set_len(mlen);
            assert_eq!(unpacked_m, msg);

            // check verification fails with wrong pk
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_keypair(pk_alt.as_mut_ptr(), sk_alt.as_mut_ptr()),
                "keypair"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_open"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_signature_ctx(
                    detached_sig.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign_signature"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA65_CLEAN_CRYPTO_BYTES,
                "Signed message length should be ≤ CRYPTO_BYTES"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_verify alt pk"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len() - 1,
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify wrong length"
            );

            let ctx = vec![1u8; 10];
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    sk.as_ptr()
                ),
                "Sign ctx call"
            );
            sm.set_len(smlen);
            assert!(smlen >= PQCLEAN_MLDSA65_CLEAN_CRYPTO_BYTES, "too small sig");
            assert!(
                smlen <= PQCLEAN_MLDSA65_CLEAN_CRYPTO_BYTES + msg.len(),
                "too big sig"
            );

            let mut mlen: usize = 0;
            unpacked_m.clear();
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx"
            );
            unpacked_m.set_len(mlen);
            assert!(unpacked_m == msg, "unequal messages");

            let ctx_alt = vec![0u8; 10];
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx_alt.as_ptr(),
                    ctx_alt.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx with alt context"
            );
        }
    }
}

#[cfg(all(test, enable_x86_avx2, feature = "avx2"))]
mod test_mldsa65_avx2 {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use rand::prelude::*;
    use std::is_x86_feature_detected;

    #[test]
    fn test_ffi() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        unsafe {
            let mut rng = rand::rng();
            let mut mlen: usize = rng.random::<u16>() as usize;
            let msg: Vec<u8> = (0..mlen).map(|_| rng.random()).collect();

            let mut pk = vec![0u8; PQCLEAN_MLDSA65_AVX2_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLDSA65_AVX2_CRYPTO_SECRETKEYBYTES];
            let mut pk_alt = vec![0u8; PQCLEAN_MLDSA65_AVX2_CRYPTO_PUBLICKEYBYTES];
            let mut sk_alt = vec![0u8; PQCLEAN_MLDSA65_AVX2_CRYPTO_SECRETKEYBYTES];
            let mut detached_sig = vec![0u8; PQCLEAN_MLDSA65_AVX2_CRYPTO_BYTES];
            let mut sm = Vec::with_capacity(mlen + PQCLEAN_MLDSA65_AVX2_CRYPTO_BYTES);
            let mut smlen = 0;

            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign"
            );
            sm.set_len(smlen);

            let mut unpacked_m = Vec::with_capacity(mlen + PQCLEAN_MLDSA65_AVX2_CRYPTO_BYTES);
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr(),
                ),
                "sign_open"
            );
            unpacked_m.set_len(mlen);
            assert_eq!(unpacked_m, msg);

            // check verification fails with wrong pk
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_keypair(pk_alt.as_mut_ptr(), sk_alt.as_mut_ptr()),
                "keypair"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_open"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_signature_ctx(
                    detached_sig.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign_signature"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA65_AVX2_CRYPTO_BYTES,
                "Signed message length should be ≤ CRYPTO_BYTES"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_verify alt pk"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len() - 1,
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify wrong length"
            );

            let ctx = vec![1u8; 10];
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    sk.as_ptr()
                ),
                "Sign ctx call"
            );
            sm.set_len(smlen);
            assert!(smlen >= PQCLEAN_MLDSA65_AVX2_CRYPTO_BYTES, "too small sig");
            assert!(
                smlen <= PQCLEAN_MLDSA65_AVX2_CRYPTO_BYTES + msg.len(),
                "too big sig"
            );

            let mut mlen: usize = 0;
            unpacked_m.clear();
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx"
            );
            unpacked_m.set_len(mlen);
            assert!(unpacked_m == msg, "unequal messages");

            let ctx_alt = vec![0u8; 10];
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx_alt.as_ptr(),
                    ctx_alt.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx with alt context"
            );
        }
    }
}

#[cfg(all(test, enable_aarch64_neon, feature = "neon"))]
mod test_mldsa65_aarch64 {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use rand::prelude::*;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut rng = rand::rng();
            let mut mlen: usize = rng.random::<u16>() as usize;
            let msg: Vec<u8> = (0..mlen).map(|_| rng.random()).collect();

            let mut pk = vec![0u8; PQCLEAN_MLDSA65_AARCH64_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLDSA65_AARCH64_CRYPTO_SECRETKEYBYTES];
            let mut pk_alt = vec![0u8; PQCLEAN_MLDSA65_AARCH64_CRYPTO_PUBLICKEYBYTES];
            let mut sk_alt = vec![0u8; PQCLEAN_MLDSA65_AARCH64_CRYPTO_SECRETKEYBYTES];
            let mut detached_sig = vec![0u8; PQCLEAN_MLDSA65_AARCH64_CRYPTO_BYTES];
            let mut sm = Vec::with_capacity(mlen + PQCLEAN_MLDSA65_AARCH64_CRYPTO_BYTES);
            let mut smlen = 0;

            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign"
            );
            sm.set_len(smlen);

            let mut unpacked_m = Vec::with_capacity(mlen + PQCLEAN_MLDSA65_AARCH64_CRYPTO_BYTES);
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr(),
                ),
                "sign_open"
            );
            unpacked_m.set_len(mlen);
            assert_eq!(unpacked_m, msg);

            // check verification fails with wrong pk
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_keypair(
                    pk_alt.as_mut_ptr(),
                    sk_alt.as_mut_ptr()
                ),
                "keypair"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_open"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_signature_ctx(
                    detached_sig.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign_signature"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA65_AARCH64_CRYPTO_BYTES,
                "Signed message length should be ≤ CRYPTO_BYTES"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_verify alt pk"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len() - 1,
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify wrong length"
            );

            let ctx = vec![1u8; 10];
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    sk.as_ptr()
                ),
                "Sign ctx call"
            );
            sm.set_len(smlen);
            assert!(
                smlen >= PQCLEAN_MLDSA65_AARCH64_CRYPTO_BYTES,
                "too small sig"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA65_AARCH64_CRYPTO_BYTES + msg.len(),
                "too big sig"
            );

            let mut mlen: usize = 0;
            unpacked_m.clear();
            assert_eq!(
                0,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx"
            );
            unpacked_m.set_len(mlen);
            assert!(unpacked_m == msg, "unequal messages");

            let ctx_alt = vec![0u8; 10];
            assert_eq!(
                -1,
                PQCLEAN_MLDSA65_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx_alt.as_ptr(),
                    ctx_alt.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx with alt context"
            );
        }
    }
}

#[cfg(test)]
mod test_mldsa87_clean {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use rand::prelude::*;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut rng = rand::rng();
            let mut mlen: usize = rng.random::<u16>() as usize;
            let msg: Vec<u8> = (0..mlen).map(|_| rng.random()).collect();

            let mut pk = vec![0u8; PQCLEAN_MLDSA87_CLEAN_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLDSA87_CLEAN_CRYPTO_SECRETKEYBYTES];
            let mut pk_alt = vec![0u8; PQCLEAN_MLDSA87_CLEAN_CRYPTO_PUBLICKEYBYTES];
            let mut sk_alt = vec![0u8; PQCLEAN_MLDSA87_CLEAN_CRYPTO_SECRETKEYBYTES];
            let mut detached_sig = vec![0u8; PQCLEAN_MLDSA87_CLEAN_CRYPTO_BYTES];
            let mut sm = Vec::with_capacity(mlen + PQCLEAN_MLDSA87_CLEAN_CRYPTO_BYTES);
            let mut smlen = 0;

            assert_eq!(
                0,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign"
            );
            sm.set_len(smlen);

            let mut unpacked_m = Vec::with_capacity(mlen + PQCLEAN_MLDSA87_CLEAN_CRYPTO_BYTES);
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr(),
                ),
                "sign_open"
            );
            unpacked_m.set_len(mlen);
            assert_eq!(unpacked_m, msg);

            // check verification fails with wrong pk
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_keypair(pk_alt.as_mut_ptr(), sk_alt.as_mut_ptr()),
                "keypair"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_open"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_signature_ctx(
                    detached_sig.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign_signature"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA87_CLEAN_CRYPTO_BYTES,
                "Signed message length should be ≤ CRYPTO_BYTES"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_verify alt pk"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len() - 1,
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify wrong length"
            );

            let ctx = vec![1u8; 10];
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    sk.as_ptr()
                ),
                "Sign ctx call"
            );
            sm.set_len(smlen);
            assert!(smlen >= PQCLEAN_MLDSA87_CLEAN_CRYPTO_BYTES, "too small sig");
            assert!(
                smlen <= PQCLEAN_MLDSA87_CLEAN_CRYPTO_BYTES + msg.len(),
                "too big sig"
            );

            let mut mlen: usize = 0;
            unpacked_m.clear();
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx"
            );
            unpacked_m.set_len(mlen);
            assert!(unpacked_m == msg, "unequal messages");

            let ctx_alt = vec![0u8; 10];
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_CLEAN_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx_alt.as_ptr(),
                    ctx_alt.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx with alt context"
            );
        }
    }
}

#[cfg(all(test, enable_x86_avx2, feature = "avx2"))]
mod test_mldsa87_avx2 {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use rand::prelude::*;
    use std::is_x86_feature_detected;

    #[test]
    fn test_ffi() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        unsafe {
            let mut rng = rand::rng();
            let mut mlen: usize = rng.random::<u16>() as usize;
            let msg: Vec<u8> = (0..mlen).map(|_| rng.random()).collect();

            let mut pk = vec![0u8; PQCLEAN_MLDSA87_AVX2_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLDSA87_AVX2_CRYPTO_SECRETKEYBYTES];
            let mut pk_alt = vec![0u8; PQCLEAN_MLDSA87_AVX2_CRYPTO_PUBLICKEYBYTES];
            let mut sk_alt = vec![0u8; PQCLEAN_MLDSA87_AVX2_CRYPTO_SECRETKEYBYTES];
            let mut detached_sig = vec![0u8; PQCLEAN_MLDSA87_AVX2_CRYPTO_BYTES];
            let mut sm = Vec::with_capacity(mlen + PQCLEAN_MLDSA87_AVX2_CRYPTO_BYTES);
            let mut smlen = 0;

            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign"
            );
            sm.set_len(smlen);

            let mut unpacked_m = Vec::with_capacity(mlen + PQCLEAN_MLDSA87_AVX2_CRYPTO_BYTES);
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr(),
                ),
                "sign_open"
            );
            unpacked_m.set_len(mlen);
            assert_eq!(unpacked_m, msg);

            // check verification fails with wrong pk
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_keypair(pk_alt.as_mut_ptr(), sk_alt.as_mut_ptr()),
                "keypair"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_open"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_signature_ctx(
                    detached_sig.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign_signature"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA87_AVX2_CRYPTO_BYTES,
                "Signed message length should be ≤ CRYPTO_BYTES"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_verify alt pk"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len() - 1,
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify wrong length"
            );

            let ctx = vec![1u8; 10];
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    sk.as_ptr()
                ),
                "Sign ctx call"
            );
            sm.set_len(smlen);
            assert!(smlen >= PQCLEAN_MLDSA87_AVX2_CRYPTO_BYTES, "too small sig");
            assert!(
                smlen <= PQCLEAN_MLDSA87_AVX2_CRYPTO_BYTES + msg.len(),
                "too big sig"
            );

            let mut mlen: usize = 0;
            unpacked_m.clear();
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx"
            );
            unpacked_m.set_len(mlen);
            assert!(unpacked_m == msg, "unequal messages");

            let ctx_alt = vec![0u8; 10];
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_AVX2_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx_alt.as_ptr(),
                    ctx_alt.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx with alt context"
            );
        }
    }
}

#[cfg(all(test, enable_aarch64_neon, feature = "neon"))]
mod test_mldsa87_aarch64 {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use rand::prelude::*;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut rng = rand::rng();
            let mut mlen: usize = rng.random::<u16>() as usize;
            let msg: Vec<u8> = (0..mlen).map(|_| rng.random()).collect();

            let mut pk = vec![0u8; PQCLEAN_MLDSA87_AARCH64_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLDSA87_AARCH64_CRYPTO_SECRETKEYBYTES];
            let mut pk_alt = vec![0u8; PQCLEAN_MLDSA87_AARCH64_CRYPTO_PUBLICKEYBYTES];
            let mut sk_alt = vec![0u8; PQCLEAN_MLDSA87_AARCH64_CRYPTO_SECRETKEYBYTES];
            let mut detached_sig = vec![0u8; PQCLEAN_MLDSA87_AARCH64_CRYPTO_BYTES];
            let mut sm = Vec::with_capacity(mlen + PQCLEAN_MLDSA87_AARCH64_CRYPTO_BYTES);
            let mut smlen = 0;

            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign"
            );
            sm.set_len(smlen);

            let mut unpacked_m = Vec::with_capacity(mlen + PQCLEAN_MLDSA87_AARCH64_CRYPTO_BYTES);
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr(),
                ),
                "sign_open"
            );
            unpacked_m.set_len(mlen);
            assert_eq!(unpacked_m, msg);

            // check verification fails with wrong pk
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_keypair(
                    pk_alt.as_mut_ptr(),
                    sk_alt.as_mut_ptr()
                ),
                "keypair"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_open"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_signature_ctx(
                    detached_sig.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    sk.as_ptr()
                ),
                "sign_signature"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA87_AARCH64_CRYPTO_BYTES,
                "Signed message length should be ≤ CRYPTO_BYTES"
            );
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len(),
                    core::ptr::null(),
                    0,
                    pk_alt.as_ptr()
                ),
                "sign_verify alt pk"
            );
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_verify_ctx(
                    detached_sig.as_ptr(),
                    smlen,
                    msg.as_ptr(),
                    msg.len() - 1,
                    core::ptr::null(),
                    0,
                    pk.as_ptr()
                ),
                "sign_verify wrong length"
            );

            let ctx = vec![1u8; 10];
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_ctx(
                    sm.as_mut_ptr(),
                    &mut smlen as *mut usize,
                    msg.as_ptr(),
                    msg.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    sk.as_ptr()
                ),
                "Sign ctx call"
            );
            sm.set_len(smlen);
            assert!(
                smlen >= PQCLEAN_MLDSA87_AARCH64_CRYPTO_BYTES,
                "too small sig"
            );
            assert!(
                smlen <= PQCLEAN_MLDSA87_AARCH64_CRYPTO_BYTES + msg.len(),
                "too big sig"
            );

            let mut mlen: usize = 0;
            unpacked_m.clear();
            assert_eq!(
                0,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx.as_ptr(),
                    ctx.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx"
            );
            unpacked_m.set_len(mlen);
            assert!(unpacked_m == msg, "unequal messages");

            let ctx_alt = vec![0u8; 10];
            assert_eq!(
                -1,
                PQCLEAN_MLDSA87_AARCH64_crypto_sign_open_ctx(
                    unpacked_m.as_mut_ptr(),
                    &mut mlen as *mut usize,
                    sm.as_ptr(),
                    sm.len(),
                    ctx_alt.as_ptr(),
                    ctx_alt.len(),
                    pk.as_ptr()
                ),
                "sign_open_ctx with alt context"
            );
        }
    }
}
