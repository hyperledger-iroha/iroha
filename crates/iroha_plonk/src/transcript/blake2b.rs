//! The `BLAKE2b` `Challenge255` transcript (spec 6.1).
//!
//! One `BLAKE2b` state with a 64-byte output, personalization
//! `"Halo2-Transcript"` and no key, byte-identical to the vendored
//! `Blake2bWrite`/`Blake2bRead` with `Challenge255`:
//!
//! - a point absorbs `0x01 || x || y` (affine coordinates, canonical
//!   little-endian); the identity is rejected;
//! - a scalar absorbs `0x02 || repr`;
//! - a squeeze absorbs `0x00`, finalizes a copy of the state and maps the
//!   64-byte output through `F::from_uniform_bytes`; the state continues.
//!
//! The implementation uses the workspace `blake2` 0.10 core with an explicit
//! lazy block buffer (`BLAKE2` finalizes its last block with a flag, so the
//! buffer must hold back a full trailing block). The KAT test in
//! `transcript::kat_tests` checks byte identity against the `blake2b_simd`
//! 1.0.4 output recorded in `fixtures/native_prover/kats_v1.json`.

use core::{fmt, marker::PhantomData};

use blake2::{
    Blake2bVarCore,
    digest::core_api::{Buffer, UpdateCore, VariableOutputCore},
};
use ff::{FromUniformBytes, PrimeField};
use iroha_pasta::{PastaAffine, PastaCurve};

use super::{TranscriptError, TranscriptHash};

/// Personalization of the transcript state.
pub const PERSONALIZATION: &[u8; 16] = b"Halo2-Transcript";
/// Prefix of a squeeze.
pub const PREFIX_CHALLENGE: u8 = 0;
/// Prefix of an absorbed point.
pub const PREFIX_POINT: u8 = 1;
/// Prefix of an absorbed scalar.
pub const PREFIX_SCALAR: u8 = 2;
/// Bytes of the `BLAKE2b` output a challenge is reduced from.
pub const OUTPUT_BYTES: usize = 64;

/// The `BLAKE2b` `Challenge255` hash state.
#[derive(Clone)]
pub struct Blake2bHash<C: PastaCurve> {
    core: Blake2bVarCore,
    buffer: Buffer<Blake2bVarCore>,
    _curve: PhantomData<C>,
}

impl<C: PastaCurve> fmt::Debug for Blake2bHash<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The internal state is not printed.
        f.write_str("Blake2bHash")
    }
}

impl<C: PastaCurve> Default for Blake2bHash<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: PastaCurve> Blake2bHash<C> {
    /// A fresh state: 64-byte output, personalization `Halo2-Transcript`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: Blake2bVarCore::new_with_params(&[], PERSONALIZATION, 0, OUTPUT_BYTES),
            buffer: Buffer::<Blake2bVarCore>::default(),
            _curve: PhantomData,
        }
    }

    /// Absorbs raw bytes.
    fn update(&mut self, bytes: &[u8]) {
        let core = &mut self.core;
        self.buffer
            .digest_blocks(bytes, |blocks| core.update_blocks(blocks));
    }

    /// Finalizes a copy of the state into 64 bytes.
    fn finalize_copy(&self) -> [u8; OUTPUT_BYTES] {
        let mut core = self.core.clone();
        let mut buffer = self.buffer.clone();
        let mut full = blake2::digest::Output::<Blake2bVarCore>::default();
        core.finalize_variable_core(&mut buffer, &mut full);
        let mut out = [0_u8; OUTPUT_BYTES];
        out.copy_from_slice(&full[..OUTPUT_BYTES]);
        out
    }
}

/// The bytes a point absorbs: `0x01 || x || y`, or `None` for the identity.
pub(crate) fn point_absorption<C: PastaCurve>(point: &C::AffineExt) -> Option<[u8; 65]> {
    let (x, y): (C::Base, C::Base) = Option::from(point.coordinates())?;
    let mut bytes = [0_u8; 65];
    bytes[0] = PREFIX_POINT;
    bytes[1..33].copy_from_slice(&x.to_repr());
    bytes[33..].copy_from_slice(&y.to_repr());
    Some(bytes)
}

impl<C: PastaCurve> TranscriptHash<C> for Blake2bHash<C> {
    fn absorb_point(&mut self, point: &C::AffineExt) -> Result<(), TranscriptError> {
        let bytes = point_absorption::<C>(point).ok_or(TranscriptError::IdentityPoint)?;
        self.update(&bytes);
        Ok(())
    }

    fn absorb_scalar(&mut self, scalar: &C::ScalarExt) {
        self.update(&[PREFIX_SCALAR]);
        self.update(&scalar.to_repr());
    }

    fn squeeze(&mut self) -> C::ScalarExt {
        self.update(&[PREFIX_CHALLENGE]);
        <C::ScalarExt as FromUniformBytes<64>>::from_uniform_bytes(&self.finalize_copy())
    }
}

#[cfg(test)]
mod tests {
    use group::{Curve, Group, prime::PrimeCurveAffine};
    use iroha_pasta::{Ep, EpAffine, Fq};

    use super::*;

    #[test]
    fn point_absorption_layout() {
        assert!(point_absorption::<Ep>(&EpAffine::identity()).is_none());
        let point = (Ep::generator() * Fq::from(5)).to_affine();
        let bytes = point_absorption::<Ep>(&point).expect("finite");
        let (x, y): (iroha_pasta::Fp, iroha_pasta::Fp) =
            Option::from(point.coordinates()).expect("finite");
        assert_eq!(bytes[0], PREFIX_POINT);
        assert_eq!(&bytes[1..33], &x.to_repr());
        assert_eq!(&bytes[33..], &y.to_repr());
    }

    #[test]
    fn squeeze_continues_the_state_and_long_inputs_cross_blocks() {
        let mut hash = Blake2bHash::<Ep>::new();
        let first = hash.squeeze();
        let second = hash.squeeze();
        assert_ne!(first, second);
        // 300 scalars span many 128-byte blocks; a clone squeezes identically.
        let mut long = Blake2bHash::<Ep>::new();
        for i in 0..300_u64 {
            long.absorb_scalar(&Fq::from(i));
        }
        let mut copy = long.clone();
        assert_eq!(long.squeeze(), copy.squeeze());
        assert_eq!(format!("{long:?}"), "Blake2bHash");
        assert_eq!(
            Blake2bHash::<Ep>::default().squeeze(),
            Blake2bHash::<Ep>::new().squeeze()
        );
    }
}
