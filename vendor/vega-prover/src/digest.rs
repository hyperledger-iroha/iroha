// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Cryptographic digest functionality for Spartan.
//!
//! This module provides traits and utilities for computing secure cryptographic
//! digests of data structures used in Spartan. It includes the `Digestible` trait
//! for types that can be converted to byte representations, the `SimpleDigestible`
//! marker trait for serializable types, and the `DigestComputer` utility for
//! computing SHA-256 digests.

use crate::traits::snark::VegaDigest;
use bincode::Options;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io;

/// Trait for components with potentially discrete digests to be included in their container's digest.
pub trait Digestible {
    /// Write the byte representation of Self in a byte buffer
    fn write_bytes<W: Sized + io::Write>(&self, byte_sink: &mut W) -> Result<(), io::Error>;
}

/// Marker trait to be implemented for types that implement `Digestible` and `Serialize`.
/// Their instances will be serialized to bytes then digested.
pub trait SimpleDigestible: Serialize {}

impl<T: SimpleDigestible> Digestible for T {
    fn write_bytes<W: Sized + io::Write>(&self, byte_sink: &mut W) -> Result<(), io::Error> {
        let config = bincode::DefaultOptions::new()
            .with_little_endian()
            .with_fixint_encoding();
        // Note: bincode recursively length-prefixes every field!
        config
            .serialize_into(byte_sink, self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

/// A utility for computing cryptographic digests of `Digestible` instances.
///
/// `DigestComputer` provides a way to compute secure hash digests using SHA-256
/// for any type that implements the `Digestible` trait. It serializes the input
/// data and computes a 32-byte digest that can be used for integrity verification
/// and identification purposes.
pub struct DigestComputer<'a, T> {
    inner: &'a T,
}

impl<'a, T: Digestible> DigestComputer<'a, T> {
    fn hasher() -> Sha256 {
        Sha256::new()
    }

    /// Create a new DigestComputer
    pub fn new(inner: &'a T) -> Self {
        DigestComputer { inner }
    }

    /// Compute the digest of a `Digestible` instance.
    pub fn digest(&self) -> Result<VegaDigest, io::Error> {
        // Stream bytes directly into the hasher via a small buffered adapter.
        // A 64 KiB buffer is large enough to amortize per-call overhead from
        // bincode while avoiding the large upfront allocation that a
        // serialize-then-hash approach would require for big objects.
        let mut writer = io::BufWriter::with_capacity(64 * 1024, HashWriter(Self::hasher()));
        self.inner.write_bytes(&mut writer)?;
        let hasher = writer
            .into_inner()
            .map_err(|e| io::Error::other(e.to_string()))?
            .0;
        Ok(hasher.finalize().into())
    }
}

/// Adapter that forwards `io::Write` calls into a SHA-256 hasher.
struct HashWriter(Sha256);

impl io::Write for HashWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{DigestComputer, SimpleDigestible};
    use once_cell::sync::OnceCell;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct S {
        i: usize,
        #[serde(skip, default = "OnceCell::new")]
        digest: OnceCell<[u8; 32]>,
    }

    impl SimpleDigestible for S {}

    impl S {
        fn new(i: usize) -> Self {
            S {
                i,
                digest: OnceCell::new(),
            }
        }

        fn digest(&self) -> [u8; 32] {
            self.digest
                .get_or_try_init(|| DigestComputer::new(self).digest())
                .cloned()
                .unwrap()
        }
    }

    #[test]
    fn test_digest_field_not_ingested_in_computation() {
        let s1 = S::new(42);

        // let's set up a struct with a weird digest field to make sure the digest computation does not depend of it
        let oc = OnceCell::new();
        oc.set([1u8; 32]).unwrap();

        let s2 = S { i: 42, digest: oc };

        assert_eq!(
            DigestComputer::<_>::new(&s1).digest().unwrap(),
            DigestComputer::<_>::new(&s2).digest().unwrap()
        );

        // note: because of the semantics of `OnceCell::get_or_try_init`, the above
        // equality will not result in `s1.digest() == s2.digest`
        assert_ne!(s2.digest(), DigestComputer::<_>::new(&s2).digest().unwrap());
    }

    #[test]
    fn test_digest_impervious_to_serialization() {
        let good_s = S::new(42);

        // let's set up a struct with a weird digest field to confuse deserializers
        let oc = OnceCell::new();
        oc.set([2u8; 32]).unwrap();

        let bad_s: S = S { i: 42, digest: oc };
        // this justifies the adjective "bad"
        assert_ne!(good_s.digest(), bad_s.digest());

        let naughty_bytes = bincode::serialize(&bad_s).unwrap();

        let retrieved_s: S = bincode::deserialize(&naughty_bytes).unwrap();
        assert_eq!(good_s.digest(), retrieved_s.digest())
    }
}
