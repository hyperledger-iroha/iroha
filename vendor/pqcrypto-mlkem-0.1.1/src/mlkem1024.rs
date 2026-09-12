//! ml-kem-1024
//!
//! These bindings use the clean version from [PQClean][pqc]
//!
//! # Example
//! ```
//! // if using pqcrypto-mlkem
//! use pqcrypto_mlkem::mlkem1024::*;
//! // or if using the pqcrypto crate:
//! // use pqcrypto::kem::mlkem1024::*;
//! let (pk, sk) = keypair();
//! let (ss1, ct) = encapsulate(&pk);
//! let ss2 = decapsulate(&ct, &sk);
//! assert!(ss1 == ss2);
//! ```
//!
//! [pqc]: https://github.com/pqclean/pqclean/

// This file is generated.

#[cfg(feature = "serialization")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serialization")]
use serde_big_array::BigArray;

use crate::ffi;
use pqcrypto_traits::kem as primitive;
use pqcrypto_traits::{Error, Result};

#[cfg(feature = "std")]
use std::fmt;

macro_rules! simple_struct {
    ($type: ident, $size: expr) => {
        #[derive(Clone, Copy)]
        #[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
        pub struct $type(
            #[cfg_attr(feature = "serialization", serde(with = "BigArray"))] [u8; $size],
        );

        impl $type {
            /// Generates an uninitialized object
            ///
            /// Used to pass to ``ffi`` interfaces.
            ///
            /// Internal use only!
            fn new() -> Self {
                $type([0u8; $size])
            }
        }

        impl primitive::$type for $type {
            /// Get this object as a byte slice
            #[inline]
            fn as_bytes(&self) -> &[u8] {
                &self.0
            }

            /// Construct this object from a byte slice
            fn from_bytes(bytes: &[u8]) -> Result<Self> {
                if bytes.len() != $size {
                    Err(Error::BadLength {
                        name: stringify!($type),
                        actual: bytes.len(),
                        expected: $size,
                    })
                } else {
                    let mut array = [0u8; $size];
                    array.copy_from_slice(bytes);
                    Ok($type(array))
                }
            }
        }

        impl PartialEq for $type {
            /// By no means constant time comparison
            fn eq(&self, other: &Self) -> bool {
                self.0
                    .iter()
                    .zip(other.0.iter())
                    .try_for_each(|(a, b)| if a == b { Ok(()) } else { Err(()) })
                    .is_ok()
            }
        }

        #[cfg(feature = "std")]
        impl fmt::Debug for $type {
            /// Add a debug implementation that won't leak private values
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{} ({} bytes)", stringify!($type), self.0.len())
            }
        }
    };
}

simple_struct!(
    PublicKey,
    ffi::PQCLEAN_MLKEM1024_CLEAN_CRYPTO_PUBLICKEYBYTES
);
simple_struct!(
    SecretKey,
    ffi::PQCLEAN_MLKEM1024_CLEAN_CRYPTO_SECRETKEYBYTES
);
simple_struct!(
    Ciphertext,
    ffi::PQCLEAN_MLKEM1024_CLEAN_CRYPTO_CIPHERTEXTBYTES
);
simple_struct!(SharedSecret, ffi::PQCLEAN_MLKEM1024_CLEAN_CRYPTO_BYTES);

/// Get the number of bytes for a public key
pub const fn public_key_bytes() -> usize {
    ffi::PQCLEAN_MLKEM1024_CLEAN_CRYPTO_PUBLICKEYBYTES
}

/// Get the number of bytes for a secret key
pub const fn secret_key_bytes() -> usize {
    ffi::PQCLEAN_MLKEM1024_CLEAN_CRYPTO_SECRETKEYBYTES
}

/// Get the number of bytes for the encapsulated ciphertext
pub const fn ciphertext_bytes() -> usize {
    ffi::PQCLEAN_MLKEM1024_CLEAN_CRYPTO_CIPHERTEXTBYTES
}

/// Get the number of bytes for the shared secret
pub const fn shared_secret_bytes() -> usize {
    ffi::PQCLEAN_MLKEM1024_CLEAN_CRYPTO_BYTES
}

macro_rules! gen_keypair {
    ($variant:ident) => {{
        let mut pk = PublicKey::new();
        let mut sk = SecretKey::new();
        assert_eq!(
            unsafe { ffi::$variant(pk.0.as_mut_ptr(), sk.0.as_mut_ptr()) },
            0
        );
        (pk, sk)
    }};
}

/// Generate a ml-kem-1024 keypair
pub fn keypair() -> (PublicKey, SecretKey) {
    #[cfg(all(enable_x86_avx2, feature = "avx2"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            return gen_keypair!(PQCLEAN_MLKEM1024_AVX2_crypto_kem_keypair);
        }
    }
    #[cfg(all(enable_aarch64_neon, feature = "neon"))]
    {
        // always use AArch64 code, when target is detected as all AArch64 targets have NEON
        // support, and std::is_aarch64_feature_detected!("neon") works only with Rust nightly at
        // the moment
        if true {
            return gen_keypair!(PQCLEAN_MLKEM1024_AARCH64_crypto_kem_keypair);
        }
    }
    gen_keypair!(PQCLEAN_MLKEM1024_CLEAN_crypto_kem_keypair)
}

macro_rules! encap {
    ($variant:ident, $pk:ident) => {{
        let mut ss = SharedSecret::new();
        let mut ct = Ciphertext::new();
        assert_eq!(
            unsafe { ffi::$variant(ct.0.as_mut_ptr(), ss.0.as_mut_ptr(), $pk.0.as_ptr()) },
            0,
        );
        (ss, ct)
    }};
}

/// Encapsulate to a ml-kem-1024 public key
pub fn encapsulate(pk: &PublicKey) -> (SharedSecret, Ciphertext) {
    #[cfg(all(enable_x86_avx2, feature = "avx2"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            return encap!(PQCLEAN_MLKEM1024_AVX2_crypto_kem_enc, pk);
        }
    }
    #[cfg(all(enable_aarch64_neon, feature = "neon"))]
    {
        if true {
            return encap!(PQCLEAN_MLKEM1024_AARCH64_crypto_kem_enc, pk);
        }
    }
    encap!(PQCLEAN_MLKEM1024_CLEAN_crypto_kem_enc, pk)
}

macro_rules! decap {
    ($variant:ident, $ct:ident, $sk:ident) => {{
        let mut ss = SharedSecret::new();
        assert_eq!(
            unsafe { ffi::$variant(ss.0.as_mut_ptr(), $ct.0.as_ptr(), $sk.0.as_ptr(),) },
            0
        );
        ss
    }};
}

/// Decapsulate the received ml-kem-1024 ciphertext
pub fn decapsulate(ct: &Ciphertext, sk: &SecretKey) -> SharedSecret {
    #[cfg(all(enable_x86_avx2, feature = "avx2"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            return decap!(PQCLEAN_MLKEM1024_AVX2_crypto_kem_dec, ct, sk);
        }
    }
    #[cfg(all(enable_aarch64_neon, feature = "neon"))]
    {
        if true {
            return decap!(PQCLEAN_MLKEM1024_AARCH64_crypto_kem_dec, ct, sk);
        }
    }
    decap!(PQCLEAN_MLKEM1024_CLEAN_crypto_kem_dec, ct, sk)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_kem() {
        let (pk, sk) = keypair();
        let (ss1, ct) = encapsulate(&pk);
        let ss2 = decapsulate(&ct, &sk);
        assert_eq!(&ss1.0[..], &ss2.0[..], "Difference in shared secrets!");
    }
}
