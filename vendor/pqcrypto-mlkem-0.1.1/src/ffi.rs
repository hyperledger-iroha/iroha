//! Foreign function interfaces
//!
//! This module defines the foreign function interface for the following
//! crypto implementations from PQClean:
//!
//!  * ml-kem-512
//!  * ml-kem-768
//!  * ml-kem-1024
// This file has been generated from PQClean.
// Find the templates in pqcrypto-template
use core::ffi::c_int;

// ensures we link correctly
#[allow(unused_imports)]
use pqcrypto_internals::*;

pub const PQCLEAN_MLKEM512_CLEAN_CRYPTO_SECRETKEYBYTES: usize = 1632;
pub const PQCLEAN_MLKEM512_CLEAN_CRYPTO_PUBLICKEYBYTES: usize = 800;
pub const PQCLEAN_MLKEM512_CLEAN_CRYPTO_CIPHERTEXTBYTES: usize = 768;
pub const PQCLEAN_MLKEM512_CLEAN_CRYPTO_BYTES: usize = 32;

#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM512_AVX2_CRYPTO_SECRETKEYBYTES: usize = 1632;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM512_AVX2_CRYPTO_PUBLICKEYBYTES: usize = 800;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM512_AVX2_CRYPTO_CIPHERTEXTBYTES: usize = 768;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM512_AVX2_CRYPTO_BYTES: usize = 32;

#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM512_AARCH64_CRYPTO_SECRETKEYBYTES: usize = 1632;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM512_AARCH64_CRYPTO_PUBLICKEYBYTES: usize = 800;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM512_AARCH64_CRYPTO_CIPHERTEXTBYTES: usize = 768;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM512_AARCH64_CRYPTO_BYTES: usize = 32;

pub const PQCLEAN_MLKEM768_CLEAN_CRYPTO_SECRETKEYBYTES: usize = 2400;
pub const PQCLEAN_MLKEM768_CLEAN_CRYPTO_PUBLICKEYBYTES: usize = 1184;
pub const PQCLEAN_MLKEM768_CLEAN_CRYPTO_CIPHERTEXTBYTES: usize = 1088;
pub const PQCLEAN_MLKEM768_CLEAN_CRYPTO_BYTES: usize = 32;

#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM768_AVX2_CRYPTO_SECRETKEYBYTES: usize = 2400;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM768_AVX2_CRYPTO_PUBLICKEYBYTES: usize = 1184;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM768_AVX2_CRYPTO_CIPHERTEXTBYTES: usize = 1088;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM768_AVX2_CRYPTO_BYTES: usize = 32;

#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM768_AARCH64_CRYPTO_SECRETKEYBYTES: usize = 2400;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM768_AARCH64_CRYPTO_PUBLICKEYBYTES: usize = 1184;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM768_AARCH64_CRYPTO_CIPHERTEXTBYTES: usize = 1088;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM768_AARCH64_CRYPTO_BYTES: usize = 32;

pub const PQCLEAN_MLKEM1024_CLEAN_CRYPTO_SECRETKEYBYTES: usize = 3168;
pub const PQCLEAN_MLKEM1024_CLEAN_CRYPTO_PUBLICKEYBYTES: usize = 1568;
pub const PQCLEAN_MLKEM1024_CLEAN_CRYPTO_CIPHERTEXTBYTES: usize = 1568;
pub const PQCLEAN_MLKEM1024_CLEAN_CRYPTO_BYTES: usize = 32;

#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM1024_AVX2_CRYPTO_SECRETKEYBYTES: usize = 3168;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM1024_AVX2_CRYPTO_PUBLICKEYBYTES: usize = 1568;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM1024_AVX2_CRYPTO_CIPHERTEXTBYTES: usize = 1568;
#[cfg(enable_x86_avx2)]
pub const PQCLEAN_MLKEM1024_AVX2_CRYPTO_BYTES: usize = 32;

#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM1024_AARCH64_CRYPTO_SECRETKEYBYTES: usize = 3168;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM1024_AARCH64_CRYPTO_PUBLICKEYBYTES: usize = 1568;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM1024_AARCH64_CRYPTO_CIPHERTEXTBYTES: usize = 1568;
#[cfg(enable_aarch64_neon)]
pub const PQCLEAN_MLKEM1024_AARCH64_CRYPTO_BYTES: usize = 32;

#[link(name = "ml-kem-512_clean")]
extern "C" {
    pub fn PQCLEAN_MLKEM512_CLEAN_crypto_kem_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    pub fn PQCLEAN_MLKEM512_CLEAN_crypto_kem_enc(ct: *mut u8, ss: *mut u8, pk: *const u8) -> c_int;
    pub fn PQCLEAN_MLKEM512_CLEAN_crypto_kem_dec(
        ss: *mut u8,
        ct: *const u8,
        sk: *const u8,
    ) -> c_int;
}

#[cfg(enable_x86_avx2)]
#[link(name = "ml-kem-512_avx2")]
extern "C" {
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLKEM512_AVX2_crypto_kem_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLKEM512_AVX2_crypto_kem_enc(ct: *mut u8, ss: *mut u8, pk: *const u8) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLKEM512_AVX2_crypto_kem_dec(ss: *mut u8, ct: *const u8, sk: *const u8)
        -> c_int;
}

#[cfg(enable_aarch64_neon)]
#[link(name = "ml-kem-512_aarch64")]
extern "C" {
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLKEM512_AARCH64_crypto_kem_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLKEM512_AARCH64_crypto_kem_enc(
        ct: *mut u8,
        ss: *mut u8,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLKEM512_AARCH64_crypto_kem_dec(
        ss: *mut u8,
        ct: *const u8,
        sk: *const u8,
    ) -> c_int;
}

#[link(name = "ml-kem-768_clean")]
extern "C" {
    pub fn PQCLEAN_MLKEM768_CLEAN_crypto_kem_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    pub fn PQCLEAN_MLKEM768_CLEAN_crypto_kem_enc(ct: *mut u8, ss: *mut u8, pk: *const u8) -> c_int;
    pub fn PQCLEAN_MLKEM768_CLEAN_crypto_kem_dec(
        ss: *mut u8,
        ct: *const u8,
        sk: *const u8,
    ) -> c_int;
}

#[cfg(enable_x86_avx2)]
#[link(name = "ml-kem-768_avx2")]
extern "C" {
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLKEM768_AVX2_crypto_kem_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLKEM768_AVX2_crypto_kem_enc(ct: *mut u8, ss: *mut u8, pk: *const u8) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLKEM768_AVX2_crypto_kem_dec(ss: *mut u8, ct: *const u8, sk: *const u8)
        -> c_int;
}

#[cfg(enable_aarch64_neon)]
#[link(name = "ml-kem-768_aarch64")]
extern "C" {
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLKEM768_AARCH64_crypto_kem_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLKEM768_AARCH64_crypto_kem_enc(
        ct: *mut u8,
        ss: *mut u8,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLKEM768_AARCH64_crypto_kem_dec(
        ss: *mut u8,
        ct: *const u8,
        sk: *const u8,
    ) -> c_int;
}

#[link(name = "ml-kem-1024_clean")]
extern "C" {
    pub fn PQCLEAN_MLKEM1024_CLEAN_crypto_kem_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    pub fn PQCLEAN_MLKEM1024_CLEAN_crypto_kem_enc(ct: *mut u8, ss: *mut u8, pk: *const u8)
        -> c_int;
    pub fn PQCLEAN_MLKEM1024_CLEAN_crypto_kem_dec(
        ss: *mut u8,
        ct: *const u8,
        sk: *const u8,
    ) -> c_int;
}

#[cfg(enable_x86_avx2)]
#[link(name = "ml-kem-1024_avx2")]
extern "C" {
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLKEM1024_AVX2_crypto_kem_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLKEM1024_AVX2_crypto_kem_enc(ct: *mut u8, ss: *mut u8, pk: *const u8) -> c_int;
    #[cfg(enable_x86_avx2)]
    pub fn PQCLEAN_MLKEM1024_AVX2_crypto_kem_dec(
        ss: *mut u8,
        ct: *const u8,
        sk: *const u8,
    ) -> c_int;
}

#[cfg(enable_aarch64_neon)]
#[link(name = "ml-kem-1024_aarch64")]
extern "C" {
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLKEM1024_AARCH64_crypto_kem_keypair(pk: *mut u8, sk: *mut u8) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLKEM1024_AARCH64_crypto_kem_enc(
        ct: *mut u8,
        ss: *mut u8,
        pk: *const u8,
    ) -> c_int;
    #[cfg(enable_aarch64_neon)]
    pub fn PQCLEAN_MLKEM1024_AARCH64_crypto_kem_dec(
        ss: *mut u8,
        ct: *const u8,
        sk: *const u8,
    ) -> c_int;
}

#[cfg(test)]
mod test_mlkem512_clean {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut pk = vec![0u8; PQCLEAN_MLKEM512_CLEAN_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLKEM512_CLEAN_CRYPTO_SECRETKEYBYTES];
            let mut ct = vec![0u8; PQCLEAN_MLKEM512_CLEAN_CRYPTO_CIPHERTEXTBYTES];
            let mut ss1 = vec![0u8; PQCLEAN_MLKEM512_CLEAN_CRYPTO_BYTES];
            let mut ss2 = vec![0u8; PQCLEAN_MLKEM512_CLEAN_CRYPTO_BYTES];

            assert_eq!(
                0,
                PQCLEAN_MLKEM512_CLEAN_crypto_kem_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM512_CLEAN_crypto_kem_enc(
                    ct.as_mut_ptr(),
                    ss1.as_mut_ptr(),
                    pk.as_ptr()
                )
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM512_CLEAN_crypto_kem_dec(ss2.as_mut_ptr(), ct.as_ptr(), sk.as_ptr())
            );
            assert_eq!(&ss1[..], &ss2[..], "Shared secrets should be equal");
        }
    }
}

#[cfg(all(test, enable_x86_avx2, feature = "avx2"))]
mod test_mlkem512_avx2 {
    use super::*;
    use alloc::vec;
    use std::is_x86_feature_detected;

    #[test]
    fn test_ffi() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        unsafe {
            let mut pk = vec![0u8; PQCLEAN_MLKEM512_AVX2_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLKEM512_AVX2_CRYPTO_SECRETKEYBYTES];
            let mut ct = vec![0u8; PQCLEAN_MLKEM512_AVX2_CRYPTO_CIPHERTEXTBYTES];
            let mut ss1 = vec![0u8; PQCLEAN_MLKEM512_AVX2_CRYPTO_BYTES];
            let mut ss2 = vec![0u8; PQCLEAN_MLKEM512_AVX2_CRYPTO_BYTES];

            assert_eq!(
                0,
                PQCLEAN_MLKEM512_AVX2_crypto_kem_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM512_AVX2_crypto_kem_enc(
                    ct.as_mut_ptr(),
                    ss1.as_mut_ptr(),
                    pk.as_ptr()
                )
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM512_AVX2_crypto_kem_dec(ss2.as_mut_ptr(), ct.as_ptr(), sk.as_ptr())
            );
            assert_eq!(&ss1[..], &ss2[..], "Shared secrets should be equal");
        }
    }
}

#[cfg(all(test, enable_aarch64_neon, feature = "neon"))]
mod test_mlkem512_aarch64 {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut pk = vec![0u8; PQCLEAN_MLKEM512_AARCH64_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLKEM512_AARCH64_CRYPTO_SECRETKEYBYTES];
            let mut ct = vec![0u8; PQCLEAN_MLKEM512_AARCH64_CRYPTO_CIPHERTEXTBYTES];
            let mut ss1 = vec![0u8; PQCLEAN_MLKEM512_AARCH64_CRYPTO_BYTES];
            let mut ss2 = vec![0u8; PQCLEAN_MLKEM512_AARCH64_CRYPTO_BYTES];

            assert_eq!(
                0,
                PQCLEAN_MLKEM512_AARCH64_crypto_kem_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM512_AARCH64_crypto_kem_enc(
                    ct.as_mut_ptr(),
                    ss1.as_mut_ptr(),
                    pk.as_ptr()
                )
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM512_AARCH64_crypto_kem_dec(ss2.as_mut_ptr(), ct.as_ptr(), sk.as_ptr())
            );
            assert_eq!(&ss1[..], &ss2[..], "Shared secrets should be equal");
        }
    }
}

#[cfg(test)]
mod test_mlkem768_clean {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut pk = vec![0u8; PQCLEAN_MLKEM768_CLEAN_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLKEM768_CLEAN_CRYPTO_SECRETKEYBYTES];
            let mut ct = vec![0u8; PQCLEAN_MLKEM768_CLEAN_CRYPTO_CIPHERTEXTBYTES];
            let mut ss1 = vec![0u8; PQCLEAN_MLKEM768_CLEAN_CRYPTO_BYTES];
            let mut ss2 = vec![0u8; PQCLEAN_MLKEM768_CLEAN_CRYPTO_BYTES];

            assert_eq!(
                0,
                PQCLEAN_MLKEM768_CLEAN_crypto_kem_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM768_CLEAN_crypto_kem_enc(
                    ct.as_mut_ptr(),
                    ss1.as_mut_ptr(),
                    pk.as_ptr()
                )
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM768_CLEAN_crypto_kem_dec(ss2.as_mut_ptr(), ct.as_ptr(), sk.as_ptr())
            );
            assert_eq!(&ss1[..], &ss2[..], "Shared secrets should be equal");
        }
    }
}

#[cfg(all(test, enable_x86_avx2, feature = "avx2"))]
mod test_mlkem768_avx2 {
    use super::*;
    use alloc::vec;
    use std::is_x86_feature_detected;

    #[test]
    fn test_ffi() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        unsafe {
            let mut pk = vec![0u8; PQCLEAN_MLKEM768_AVX2_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLKEM768_AVX2_CRYPTO_SECRETKEYBYTES];
            let mut ct = vec![0u8; PQCLEAN_MLKEM768_AVX2_CRYPTO_CIPHERTEXTBYTES];
            let mut ss1 = vec![0u8; PQCLEAN_MLKEM768_AVX2_CRYPTO_BYTES];
            let mut ss2 = vec![0u8; PQCLEAN_MLKEM768_AVX2_CRYPTO_BYTES];

            assert_eq!(
                0,
                PQCLEAN_MLKEM768_AVX2_crypto_kem_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM768_AVX2_crypto_kem_enc(
                    ct.as_mut_ptr(),
                    ss1.as_mut_ptr(),
                    pk.as_ptr()
                )
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM768_AVX2_crypto_kem_dec(ss2.as_mut_ptr(), ct.as_ptr(), sk.as_ptr())
            );
            assert_eq!(&ss1[..], &ss2[..], "Shared secrets should be equal");
        }
    }
}

#[cfg(all(test, enable_aarch64_neon, feature = "neon"))]
mod test_mlkem768_aarch64 {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut pk = vec![0u8; PQCLEAN_MLKEM768_AARCH64_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLKEM768_AARCH64_CRYPTO_SECRETKEYBYTES];
            let mut ct = vec![0u8; PQCLEAN_MLKEM768_AARCH64_CRYPTO_CIPHERTEXTBYTES];
            let mut ss1 = vec![0u8; PQCLEAN_MLKEM768_AARCH64_CRYPTO_BYTES];
            let mut ss2 = vec![0u8; PQCLEAN_MLKEM768_AARCH64_CRYPTO_BYTES];

            assert_eq!(
                0,
                PQCLEAN_MLKEM768_AARCH64_crypto_kem_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM768_AARCH64_crypto_kem_enc(
                    ct.as_mut_ptr(),
                    ss1.as_mut_ptr(),
                    pk.as_ptr()
                )
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM768_AARCH64_crypto_kem_dec(ss2.as_mut_ptr(), ct.as_ptr(), sk.as_ptr())
            );
            assert_eq!(&ss1[..], &ss2[..], "Shared secrets should be equal");
        }
    }
}

#[cfg(test)]
mod test_mlkem1024_clean {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut pk = vec![0u8; PQCLEAN_MLKEM1024_CLEAN_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLKEM1024_CLEAN_CRYPTO_SECRETKEYBYTES];
            let mut ct = vec![0u8; PQCLEAN_MLKEM1024_CLEAN_CRYPTO_CIPHERTEXTBYTES];
            let mut ss1 = vec![0u8; PQCLEAN_MLKEM1024_CLEAN_CRYPTO_BYTES];
            let mut ss2 = vec![0u8; PQCLEAN_MLKEM1024_CLEAN_CRYPTO_BYTES];

            assert_eq!(
                0,
                PQCLEAN_MLKEM1024_CLEAN_crypto_kem_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM1024_CLEAN_crypto_kem_enc(
                    ct.as_mut_ptr(),
                    ss1.as_mut_ptr(),
                    pk.as_ptr()
                )
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM1024_CLEAN_crypto_kem_dec(ss2.as_mut_ptr(), ct.as_ptr(), sk.as_ptr())
            );
            assert_eq!(&ss1[..], &ss2[..], "Shared secrets should be equal");
        }
    }
}

#[cfg(all(test, enable_x86_avx2, feature = "avx2"))]
mod test_mlkem1024_avx2 {
    use super::*;
    use alloc::vec;
    use std::is_x86_feature_detected;

    #[test]
    fn test_ffi() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        unsafe {
            let mut pk = vec![0u8; PQCLEAN_MLKEM1024_AVX2_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLKEM1024_AVX2_CRYPTO_SECRETKEYBYTES];
            let mut ct = vec![0u8; PQCLEAN_MLKEM1024_AVX2_CRYPTO_CIPHERTEXTBYTES];
            let mut ss1 = vec![0u8; PQCLEAN_MLKEM1024_AVX2_CRYPTO_BYTES];
            let mut ss2 = vec![0u8; PQCLEAN_MLKEM1024_AVX2_CRYPTO_BYTES];

            assert_eq!(
                0,
                PQCLEAN_MLKEM1024_AVX2_crypto_kem_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM1024_AVX2_crypto_kem_enc(
                    ct.as_mut_ptr(),
                    ss1.as_mut_ptr(),
                    pk.as_ptr()
                )
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM1024_AVX2_crypto_kem_dec(ss2.as_mut_ptr(), ct.as_ptr(), sk.as_ptr())
            );
            assert_eq!(&ss1[..], &ss2[..], "Shared secrets should be equal");
        }
    }
}

#[cfg(all(test, enable_aarch64_neon, feature = "neon"))]
mod test_mlkem1024_aarch64 {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_ffi() {
        unsafe {
            let mut pk = vec![0u8; PQCLEAN_MLKEM1024_AARCH64_CRYPTO_PUBLICKEYBYTES];
            let mut sk = vec![0u8; PQCLEAN_MLKEM1024_AARCH64_CRYPTO_SECRETKEYBYTES];
            let mut ct = vec![0u8; PQCLEAN_MLKEM1024_AARCH64_CRYPTO_CIPHERTEXTBYTES];
            let mut ss1 = vec![0u8; PQCLEAN_MLKEM1024_AARCH64_CRYPTO_BYTES];
            let mut ss2 = vec![0u8; PQCLEAN_MLKEM1024_AARCH64_CRYPTO_BYTES];

            assert_eq!(
                0,
                PQCLEAN_MLKEM1024_AARCH64_crypto_kem_keypair(pk.as_mut_ptr(), sk.as_mut_ptr())
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM1024_AARCH64_crypto_kem_enc(
                    ct.as_mut_ptr(),
                    ss1.as_mut_ptr(),
                    pk.as_ptr()
                )
            );
            assert_eq!(
                0,
                PQCLEAN_MLKEM1024_AARCH64_crypto_kem_dec(
                    ss2.as_mut_ptr(),
                    ct.as_ptr(),
                    sk.as_ptr()
                )
            );
            assert_eq!(&ss1[..], &ss2[..], "Shared secrets should be equal");
        }
    }
}
