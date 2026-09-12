//! # mldsa
//!
//! This crate provides bindings to and wrappers around the following
//! implementations from [PQClean][pqc]:
//!
//! * ml-dsa-44 - clean
//! * ml-dsa-65 - clean
//! * ml-dsa-87 - clean
//!
//! [pqc]: https://github.com/pqclean/pqclean/
//!

#![no_std]
#![allow(clippy::len_without_is_empty)]

// For no-std vectors
extern crate alloc;

// For tests
#[cfg(feature = "std")]
extern crate std;

pub mod ffi;
pub mod mldsa44;
pub mod mldsa65;
pub mod mldsa87;

pub use crate::mldsa44::{
    detached_sign as mldsa44_detached_sign, detached_sign_ctx as mldsa44_detached_sign_ctx,
    keypair as mldsa44_keypair, open as mldsa44_open, open_ctx as mldsa44_open_ctx,
    public_key_bytes as mldsa44_public_key_bytes, secret_key_bytes as mldsa44_secret_key_bytes,
    sign as mldsa44_sign, sign_ctx as mldsa44_sign_ctx, signature_bytes as mldsa44_signature_bytes,
    verify_detached_signature as mldsa44_verify_detached_signature,
    verify_detached_signature_ctx as mldsa44_verify_detached_signature_ctx,
};
pub use crate::mldsa65::{
    detached_sign as mldsa65_detached_sign, detached_sign_ctx as mldsa65_detached_sign_ctx,
    keypair as mldsa65_keypair, open as mldsa65_open, open_ctx as mldsa65_open_ctx,
    public_key_bytes as mldsa65_public_key_bytes, secret_key_bytes as mldsa65_secret_key_bytes,
    sign as mldsa65_sign, sign_ctx as mldsa65_sign_ctx, signature_bytes as mldsa65_signature_bytes,
    verify_detached_signature as mldsa65_verify_detached_signature,
    verify_detached_signature_ctx as mldsa65_verify_detached_signature_ctx,
};
pub use crate::mldsa87::{
    detached_sign as mldsa87_detached_sign, detached_sign_ctx as mldsa87_detached_sign_ctx,
    keypair as mldsa87_keypair, open as mldsa87_open, open_ctx as mldsa87_open_ctx,
    public_key_bytes as mldsa87_public_key_bytes, secret_key_bytes as mldsa87_secret_key_bytes,
    sign as mldsa87_sign, sign_ctx as mldsa87_sign_ctx, signature_bytes as mldsa87_signature_bytes,
    verify_detached_signature as mldsa87_verify_detached_signature,
    verify_detached_signature_ctx as mldsa87_verify_detached_signature_ctx,
};
