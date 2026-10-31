//! Public portable AES ciphers backed by upstream's constant-time fixslicing.
//!
//! These types never dispatch to hardware, even when another dependency enables
//! the `hardware` feature. Cipher arithmetic and key expansion remain owned by
//! the upstream portable backend.

use crate::backends;
use cipher::{
    AlgorithmName, BlockCipherDecClosure, BlockCipherDecrypt, BlockCipherEncClosure,
    BlockCipherEncrypt, BlockSizeUser, Key, KeyInit, KeySizeUser,
    consts::{U16, U24, U32},
};
use core::fmt;

macro_rules! common {
    ($name:ident, $backend:ident, $key_size:ty, $description:literal) => {
        #[doc = $description]
        #[derive(Clone)]
        #[repr(transparent)]
        pub struct $name(backends::soft::$backend);

        impl KeySizeUser for $name {
            type KeySize = $key_size;
        }

        impl KeyInit for $name {
            #[inline]
            fn new(key: &Key<Self>) -> Self {
                Self(backends::soft::$backend::new(&key.0))
            }
        }

        impl BlockSizeUser for $name {
            type BlockSize = U16;
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), " { .. }"))
            }
        }

        impl AlgorithmName for $name {
            fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(stringify!($name))
            }
        }

        impl Drop for $name {
            #[inline]
            fn drop(&mut self) {
                #[cfg(feature = "zeroize")]
                // SAFETY: the upstream soft backend contains only integer key
                // schedule arrays. This transparent wrapper adds no pointers or
                // drop state, matching upstream's flat key-schedule erasure.
                unsafe {
                    zeroize::zeroize_flat_type(self);
                }
            }
        }

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name {}
    };
}

macro_rules! encrypt {
    ($name:ident) => {
        impl BlockCipherEncrypt for $name {
            #[inline]
            fn encrypt_with_backend(&self, f: impl BlockCipherEncClosure<BlockSize = U16>) {
                f.call(&self.0);
            }
        }
    };
}

macro_rules! decrypt {
    ($name:ident) => {
        impl BlockCipherDecrypt for $name {
            #[inline]
            fn decrypt_with_backend(&self, f: impl BlockCipherDecClosure<BlockSize = U16>) {
                f.call(&self.0);
            }
        }
    };
}

macro_rules! from_encryptor {
    ($name:ident, $enc:ident) => {
        impl From<&$enc> for $name {
            #[inline]
            fn from(cipher: &$enc) -> Self {
                Self(cipher.0)
            }
        }

        impl From<$enc> for $name {
            #[inline]
            fn from(cipher: $enc) -> Self {
                Self::from(&cipher)
            }
        }
    };
}

macro_rules! portable {
    ($name:ident, $enc:ident, $dec:ident, $size:ty) => {
        common!($name, $name, $size, "Portable AES block cipher.");
        common!(
            $enc,
            $name,
            $size,
            "Portable AES block cipher (encrypt-only)."
        );
        common!(
            $dec,
            $name,
            $size,
            "Portable AES block cipher (decrypt-only)."
        );
        encrypt!($name);
        encrypt!($enc);
        decrypt!($name);
        decrypt!($dec);
        from_encryptor!($name, $enc);
        from_encryptor!($dec, $enc);
    };
}

portable!(Aes128, Aes128Enc, Aes128Dec, U16);
portable!(Aes192, Aes192Enc, Aes192Dec, U24);
portable!(Aes256, Aes256Enc, Aes256Dec, U32);
