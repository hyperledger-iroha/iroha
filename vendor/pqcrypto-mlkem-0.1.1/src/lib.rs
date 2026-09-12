//! # mlkem
//!
//! This crate provides bindings to and wrappers around the following
//! implementations from [PQClean][pqc]:
//!
//! * ml-kem-512 - clean
//! * ml-kem-768 - clean
//! * ml-kem-1024 - clean
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
pub mod mlkem1024;
pub mod mlkem512;
pub mod mlkem768;

pub use crate::mlkem1024::{
    ciphertext_bytes as mlkem1024_ciphertext_bytes, decapsulate as mlkem1024_decapsulate,
    encapsulate as mlkem1024_encapsulate, keypair as mlkem1024_keypair,
    public_key_bytes as mlkem1024_public_key_bytes, secret_key_bytes as mlkem1024_secret_key_bytes,
    shared_secret_bytes as mlkem1024_shared_secret_bytes,
};
pub use crate::mlkem512::{
    ciphertext_bytes as mlkem512_ciphertext_bytes, decapsulate as mlkem512_decapsulate,
    encapsulate as mlkem512_encapsulate, keypair as mlkem512_keypair,
    public_key_bytes as mlkem512_public_key_bytes, secret_key_bytes as mlkem512_secret_key_bytes,
    shared_secret_bytes as mlkem512_shared_secret_bytes,
};
pub use crate::mlkem768::{
    ciphertext_bytes as mlkem768_ciphertext_bytes, decapsulate as mlkem768_decapsulate,
    encapsulate as mlkem768_encapsulate, keypair as mlkem768_keypair,
    public_key_bytes as mlkem768_public_key_bytes, secret_key_bytes as mlkem768_secret_key_bytes,
    shared_secret_bytes as mlkem768_shared_secret_bytes,
};
