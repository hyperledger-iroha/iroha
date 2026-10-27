//! Hardware autodetection wrappers from upstream AES 0.9.3.

use crate::backends;
use cipher::{
    AlgorithmName, BlockCipherDecClosure, BlockCipherDecrypt, BlockCipherEncClosure,
    BlockCipherEncrypt, BlockSizeUser, Key, KeyInit, KeySizeUser,
    consts::{U16, U24, U32},
};
use core::fmt;
use cpubits::cfg_if;

// Define token used for target feature detection
cfg_if! {
    if #[cfg(aes_backend = "soft")] {
        type Token = ();
    } else if #[cfg(any(target_arch = "x86_64", target_arch = "x86"))] {
        cpufeatures::new!(features_aes, "aes");
        cpufeatures::new!(features_vaes256, "vaes");
        cpufeatures::new!(features_vaes512, "avx512f", "vaes");

        #[derive(Clone, Copy)]
        struct Token {
            aes: features_aes::InitToken,
            vaes256: features_vaes256::InitToken,
            vaes512: features_vaes512::InitToken,
        }

        impl Default for Token {
            fn default() -> Self {
                Token {
                    aes: features_aes::InitToken::init(),
                    vaes256: features_vaes256::InitToken::init(),
                    vaes512: features_vaes512::InitToken::init(),
                }
            }
        }

    } else if #[cfg(all(target_arch = "aarch64", not(miri)))] {
        cpufeatures::new!(features_aes, "aes");

        #[derive(Clone, Copy)]
        struct Token {
            aes: features_aes::InitToken,
        }

        impl Default for Token {
            fn default() -> Self {
                Token {
                    aes: features_aes::InitToken::init(),
                }
            }
        }
    } else {
        type Token = ();
    }
}

/// Returns `true` if this crate can use AES hardware acceleration on the current machine.
///
/// This is a runtime check performed on the machine where the code is executed.
///
/// ```
/// if aes::hardware_accelerated() {
///     println!("AES hardware acceleration is available");
/// } else {
///     println!("WARNING: using software fallback for AES");
/// }
/// ```
pub fn hardware_accelerated() -> bool {
    cfg_if! {
        if #[cfg(aes_backend = "soft")] {
            false
        } else if #[cfg(any(target_arch = "x86_64", target_arch = "x86"))] {
            features_aes::get()
        } else if #[cfg(all(target_arch = "aarch64", not(miri)))] {
            features_aes::get()
        } else {
            false
        }
    }
}

macro_rules! impl_key_init {
    ($name:ident, $soft_name:ident, $key_size:ty, $inner:path) => {
        impl KeySizeUser for $name {
            type KeySize = $key_size;
        }

        impl KeyInit for $name {
            #[inline]
            fn new(key: &Key<Self>) -> Self {
                type Inner = $inner;
                let token = Token::default();
                let key = &key.0;

                #[cfg(not(aes_backend = "soft"))]
                cfg_if! {
                    if #[cfg(any(target_arch = "x86_64", target_arch = "x86"))] {
                        if token.aes.get() {
                            // SAFETY: we confirmed that the required target features are available
                            let aes = unsafe { backends::x86_aes::$name::new(key) };
                            let inner = Inner { aes };
                            return Self { inner, token };
                        }
                    } else if #[cfg(all(target_arch = "aarch64", not(miri)))] {
                        if token.aes.get() {
                            // SAFETY: we confirmed that the required target features are available
                            let aes = unsafe { backends::aarch64_aes::$name::new(key) };
                            let inner = Inner { aes };
                            return Self { inner, token };
                        }
                    }
                }

                let soft = backends::soft::$soft_name::new(key);
                let inner = Inner { soft };
                Self { inner, token }
            }
        }
    };
}

macro_rules! impl_encrypt {
    ($ty_name:ident, $name:ident) => {
        impl BlockCipherEncrypt for $ty_name {
            #[inline]
            fn encrypt_with_backend(&self, f: impl BlockCipherEncClosure<BlockSize = U16>) {
                #[cfg(not(aes_backend = "soft"))]
                cfg_if! {
                    if #[cfg(any(target_arch = "x86_64", target_arch = "x86"))] {
                        if self.token.vaes512.get() {
                            // SAFETY: we access correct union variant
                            let enc_rk = unsafe { &self.inner.aes.enc_rk };
                            // SAFETY: we confirmed that the required target features are available
                            unsafe { backends::x86_vaes512::$name::encrypt(enc_rk, f) };
                            return;
                        }

                        if self.token.vaes256.get() {
                            // SAFETY: we access correct union variant
                            let enc_rk = unsafe { &self.inner.aes.enc_rk };
                            // SAFETY: we confirmed that the required target features are available
                            unsafe { backends::x86_vaes256::$name::encrypt(enc_rk, f) };
                            return;
                        }

                        if self.token.aes.get() {
                            // SAFETY: we access correct union variant
                            let aes = unsafe { &self.inner.aes };
                            // SAFETY: we confirmed that the required target features are available
                            unsafe { aes.encrypt(f) };
                            return;
                        }
                    } else if #[cfg(all(target_arch = "aarch64", not(miri)))] {
                        if self.token.aes.get() {
                            // SAFETY: we access correct union variant
                            let aes = unsafe { &self.inner.aes };
                            // SAFETY: we confirmed that the required target features are available
                            unsafe { aes.encrypt(f) };
                            return;
                        }
                    }
                }

                // SAFETY: we access correct union variant
                let backend = unsafe { &self.inner.soft };
                f.call(backend);
            }
        }
    };
}

macro_rules! impl_decrypt {
    ($name:ident, $alg_name:ident) => {
        impl BlockCipherDecrypt for $name {
            #[inline]
            fn decrypt_with_backend(&self, f: impl BlockCipherDecClosure<BlockSize = U16>) {
                #[cfg(not(aes_backend = "soft"))]
                cfg_if! {
                    if #[cfg(any(target_arch = "x86_64", target_arch = "x86"))] {
                        if self.token.vaes512.get() {
                            // SAFETY: we access correct union variant
                            let dec_rk = unsafe { &self.inner.aes.dec_rk };
                            // SAFETY: we confirmed that the required target features are available
                            unsafe { backends::x86_vaes512::$alg_name::decrypt(dec_rk, f) };
                            return;
                        }

                        if self.token.vaes256.get() {
                            // SAFETY: we access correct union variant
                            let dec_rk = unsafe { &self.inner.aes.dec_rk };
                            // SAFETY: we confirmed that the required target features are available
                            unsafe { backends::x86_vaes256::$alg_name::decrypt(dec_rk, f) };
                            return;
                        }

                        if self.token.aes.get() {
                            // SAFETY: we access correct union variant
                            let backend = unsafe { &self.inner.aes };
                            // SAFETY: we confirmed that the required target features are available
                            unsafe { backend.decrypt(f) };
                            return;
                        }
                    } else if #[cfg(all(target_arch = "aarch64", not(miri)))] {
                        if self.token.aes.get() {
                            // SAFETY: we access correct union variant
                            let backend = unsafe { &self.inner.aes };
                            // SAFETY: we confirmed that the required target features are available
                            unsafe { backend.decrypt(f) };
                            return;
                        }
                    }
                }

                // SAFETY: we access correct union variant
                let backend = unsafe { &self.inner.soft };
                f.call(backend);
            }
        }
    };
}

macro_rules! impl_from_enc {
    ($name:ident, $name_enc:ident, $inner:path, $into_fn:ident) => {
        impl From<&$name_enc> for $name {
            #[inline]
            fn from(enc: &$name_enc) -> $name {
                type Inner = $inner;

                let token = enc.token;

                #[cfg(not(aes_backend = "soft"))]
                cfg_if! {
                    if #[cfg(any(target_arch = "x86_64", target_arch = "x86"))] {
                        if token.aes.get() {
                            // SAFETY: we access correct union variant
                            let aes_enc = unsafe { &enc.inner.aes };
                            // SAFETY: we confirmed that the required target features are available
                            let aes = unsafe { aes_enc.$into_fn() };
                            let inner = Inner { aes };
                            return Self { inner, token };
                        }
                    } else if #[cfg(all(target_arch = "aarch64", not(miri)))] {
                        if token.aes.get() {
                            // SAFETY: we access correct union variant
                            let aes_enc = unsafe { &enc.inner.aes };
                            // SAFETY: we confirmed that the required target features are available
                            let aes = unsafe { aes_enc.$into_fn() };
                            let inner = Inner { aes };
                            return Self { inner, token };
                        }
                    }
                }

                // SAFETY: we access correct union variant
                let soft = unsafe { enc.inner.soft };
                let inner = Inner { soft };
                Self { inner, token }
            }
        }

        impl From<$name_enc> for $name {
            #[inline]
            fn from(enc: $name_enc) -> $name {
                Self::from(&enc)
            }
        }
    };
}

macro_rules! common_impls {
    ($name:ident) => {
        impl BlockSizeUser for $name {
            type BlockSize = U16;
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
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
                unsafe {
                    zeroize::zeroize_flat_type(self);
                }
            }
        }

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name {}
    };
}

macro_rules! define_aes_impl {
    (
        name = $name:ident,
        name_enc = $name_enc:ident,
        name_dec = $name_dec:ident,
        module = $module:tt,
        key_size = $key_size:ident,
        doc = $doc:expr,
    ) => {
        mod $module {
            use crate::backends;

            #[derive(Copy, Clone)]
            pub(super) union Inner {
                #[cfg(all(
                    any(target_arch = "x86_64", target_arch = "x86"),
                    not(aes_backend = "soft"),
                ))]
                pub(super) aes: backends::x86_aes::$name,
                #[cfg(all(target_arch = "aarch64", not(miri), not(aes_backend = "soft")))]
                pub(super) aes: backends::aarch64_aes::$name,
                pub(super) soft: backends::soft::$name,
            }

            #[derive(Copy, Clone)]
            pub(super) union InnerEnc {
                #[cfg(all(
                    any(target_arch = "x86_64", target_arch = "x86"),
                    not(aes_backend = "soft"),
                ))]
                pub(super) aes: backends::x86_aes::$name_enc,
                #[cfg(all(target_arch = "aarch64", not(miri), not(aes_backend = "soft")))]
                pub(super) aes: backends::aarch64_aes::$name_enc,
                pub(super) soft: backends::soft::$name,
            }

            #[derive(Copy, Clone)]
            pub(super) union InnerDec {
                #[cfg(all(
                    any(target_arch = "x86_64", target_arch = "x86"),
                    not(aes_backend = "soft"),
                ))]
                pub(super) aes: backends::x86_aes::$name_dec,
                #[cfg(all(target_arch = "aarch64", not(miri), not(aes_backend = "soft")))]
                pub(super) aes: backends::aarch64_aes::$name_dec,
                pub(super) soft: backends::soft::$name,
            }
        }

        #[doc=$doc]
        #[doc = "block cipher"]
        #[derive(Clone)]
        pub struct $name {
            inner: $module::Inner,
            #[allow(dead_code, reason = "this field is not used on software-only targets")]
            token: Token,
        }

        common_impls!($name);
        impl_key_init!($name, $name, $key_size, $module::Inner);
        impl_encrypt!($name, $name);
        impl_decrypt!($name, $name);
        impl_from_enc!($name, $name_enc, $module::Inner, as_encdec);

        #[doc=$doc]
        #[doc = "block cipher (encrypt-only)"]
        #[derive(Clone)]
        pub struct $name_enc {
            inner: $module::InnerEnc,
            #[allow(dead_code, reason = "this field is not used on software-only targets")]
            token: Token,
        }

        common_impls!($name_enc);
        impl_key_init!($name_enc, $name, $key_size, $module::InnerEnc);
        impl_encrypt!($name_enc, $name);

        #[doc=$doc]
        #[doc = "block cipher (decrypt-only)"]
        #[derive(Clone)]
        pub struct $name_dec {
            inner: $module::InnerDec,
            #[allow(dead_code, reason = "this field is not used on software-only targets")]
            token: Token,
        }

        common_impls!($name_dec);
        impl_key_init!($name_dec, $name, $key_size, $module::InnerDec);
        impl_decrypt!($name_dec, $name);
        impl_from_enc!($name_dec, $name_enc, $module::InnerDec, as_dec);
    };
}

define_aes_impl!(
    name = Aes128,
    name_enc = Aes128Enc,
    name_dec = Aes128Dec,
    module = aes128,
    key_size = U16,
    doc = "AES-128",
);
define_aes_impl!(
    name = Aes192,
    name_enc = Aes192Enc,
    name_dec = Aes192Dec,
    module = aes192,
    key_size = U24,
    doc = "AES-192",
);
define_aes_impl!(
    name = Aes256,
    name_enc = Aes256Enc,
    name_dec = Aes256Dec,
    module = aes256,
    key_size = U32,
    doc = "AES-256",
);
