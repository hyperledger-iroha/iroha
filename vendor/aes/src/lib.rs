//! Pure Rust implementation of the [Advanced Encryption Standard][AES]
//! (AES, a.k.a. Rijndael).
//!
//! # ⚠️ Security Warning: Hazmat!
//!
//! This crate implements only the low-level block cipher function, and is intended
//! for use for implementing higher-level constructions *only*. It is NOT
//! intended for direct use in applications.
//!
//! USE AT YOUR OWN RISK!
//!
//! # Supported backends
//! This crate provides multiple backends including a portable pure Rust
//! backend as well as ones based on CPU intrinsics.
//!
//! With the default `hardware` Cargo feature it detects and uses CPU intrinsics
//! when available. Without that feature only the portable backend is compiled.
//! The portable types are always available under [`soft`], independently of
//! feature unification.
//!
//! ## "soft" portable backend
//! As a baseline implementation, this crate provides a constant-time pure Rust
//! implementation based on [fixslicing], a more advanced form of bitslicing
//! implemented entirely in terms of bitwise arithmetic with no use of any
//! lookup tables or data-dependent branches.
//!
//! Enabling the `aes_backend_soft = "compact"` configuration flag will reduce the code size of this
//! backend at the cost of decreased performance (using a modified form of
//! the fixslicing technique called "semi-fixslicing").
//!
//! ## ARMv8 intrinsics (Rust 1.61+)
//! On `aarch64` targets including `aarch64-apple-darwin` (Apple M1) and Linux
//! targets such as `aarch64-unknown-linux-gnu` and `aarch64-unknown-linux-musl`,
//! support for using AES intrinsics provided by the ARMv8 Cryptography Extensions.
//!
//! On Linux and macOS, support for ARMv8 AES intrinsics is autodetected at
//! runtime. On other platforms the `aes` target feature must be enabled via
//! RUSTFLAGS.
//!
//! ## `x86`/`x86_64` intrinsics (AES-NI and VAES)
//! By default this crate uses runtime detection on `i686`/`x86_64` targets
//! in order to determine if AES-NI and VAES are available, and if they are
//! not, it will fallback to using a constant-time software implementation.
//!
//! Passing `RUSTFLAGS=-Ctarget-feature=+aes,+ssse3` explicitly at
//! compile-time will override runtime detection and ensure that AES-NI is
//! used or passing `RUSTFLAGS=-Ctarget-feature=+aes,+avx512f,+ssse3,+vaes`
//! will ensure that AESNI and VAES are always used.
//!
//! Note: Enabling VAES256 or VAES512 still requires specifying `--cfg
//! aes_backend = "avx256"` or `--cfg aes_backend = "avx512"` explicitly.
//!
//! Programs built in this manner will crash with an illegal instruction on
//! CPUs which do not have AES-NI and VAES enabled.
//!
//! Note: runtime detection is not possible on SGX targets. Please use the
//! aforementioned `RUSTFLAGS` to leverage AES-NI and VAES on these targets.
//!
//! # Examples
//! ```
//! use aes::Aes128;
//! use aes::cipher::{Array, BlockCipherEncrypt, BlockCipherDecrypt, KeyInit};
//!
//! let key = Array::from([0u8; 16]);
//! let mut block = Array::from([42u8; 16]);
//!
//! // Initialize cipher
//! let cipher = Aes128::new(&key);
//!
//! let block_copy = block;
//!
//! // Encrypt block in-place
//! cipher.encrypt_block(&mut block);
//!
//! // And decrypt it back
//! cipher.decrypt_block(&mut block);
//! assert_eq!(block, block_copy);
//!
//! // Implementation supports parallel block processing. Number of blocks
//! // processed in parallel depends in general on hardware capabilities.
//! // This is achieved by instruction-level parallelism (ILP) on a single
//! // CPU core, which is different from multi-threaded parallelism.
//! let mut blocks = [block; 100];
//! cipher.encrypt_blocks(&mut blocks);
//!
//! for block in blocks.iter_mut() {
//!     cipher.decrypt_block(block);
//!     assert_eq!(block, &block_copy);
//! }
//!
//! // `decrypt_blocks` also supports parallel block processing.
//! cipher.decrypt_blocks(&mut blocks);
//!
//! for block in blocks.iter_mut() {
//!     cipher.encrypt_block(block);
//!     assert_eq!(block, &block_copy);
//! }
//! ```
//!
//! For implementation of block cipher modes of operation see
//! [`block-modes`] repository.
//!
//! # Cargo features
//!
//! - `hardware` (default): compile hardware backends with runtime detection.
//! - `zeroize`: erase every key schedule when its cipher is dropped.
//! - `hazmat`: expose low-level round functions.
//!
//! # Configuration Flags
//!
//! You can modify crate using the following configuration flags:
//!
//! - `aes_backend`: explicitly select one of the following backends:
//!   - `soft`: force software backend
//!   - `avx256`: force AVX2 backend
//!   - `avx512`: force AVX-512 backend
//! - `aes_backend_soft`: modify software backend:
//!   - `compact`: use compact implementation (less performant, but results in a smaller binary)
//!
//! It can be enabled using `RUSTFLAGS` environment variable
//! (e.g. `RUSTFLAGS='--cfg aes_backend="soft"'`) or by modifying `.cargo/config`.
//!
//! [AES]: https://en.wikipedia.org/wiki/Advanced_Encryption_Standard
//! [fixslicing]: https://eprint.iacr.org/2020/1123.pdf
//! [AES-NI]: https://en.wikipedia.org/wiki/AES_instruction_set
//! [`block-modes`]: https://github.com/RustCrypto/block-modes/

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/26acc39f/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/26acc39f/logo.svg"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs, rust_2018_idioms)]

pub use cipher;

#[cfg(feature = "hazmat")]
pub mod hazmat;

mod backends;

/// Portable constant-time AES types, independent of hardware feature unification.
pub mod soft;

/// 128-bit AES block.
pub type Block = cipher::Array<u8, cipher::consts::U16>;

#[cfg(all(feature = "hardware", not(aes_backend = "soft")))]
mod autodetect;
#[cfg(all(feature = "hardware", not(aes_backend = "soft")))]
pub use autodetect::*;
#[cfg(any(not(feature = "hardware"), aes_backend = "soft"))]
pub use soft::*;

/// Whether this build can use AES hardware acceleration on this machine.
#[cfg(any(not(feature = "hardware"), aes_backend = "soft"))]
pub fn hardware_accelerated() -> bool {
    false
}
