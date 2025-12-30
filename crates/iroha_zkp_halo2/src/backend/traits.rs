//! Backend trait hierarchy shared by all implementations.

use core::fmt;

use crate::{constants::DST, errors::Error, hash::sha3_512, norito_types::ZkCurveId};

/// Scalar behaviour required by the IPA algorithms.
pub trait IpaScalar: Copy + Clone + PartialEq + Eq + fmt::Debug + Default {
    /// Returns the additive identity.
    fn zero() -> Self;
    /// Returns the multiplicative identity.
    fn one() -> Self;
    /// Add two field elements.
    fn add(self, rhs: Self) -> Self;
    /// Subtract two field elements.
    fn sub(self, rhs: Self) -> Self;
    /// Multiply two field elements.
    fn mul(self, rhs: Self) -> Self;
    /// Negate the field element.
    fn neg(self) -> Self;
    /// Multiplicative inverse, returning `Error::InversionOfZero` when undefined.
    fn inv(self) -> Result<Self, Error>;
    /// Exponentiate by a `u64` exponent.
    fn pow_u64(self, exp: u64) -> Self;
    /// Exponentiate by a `u128` exponent.
    fn pow_u128(self, exp: u128) -> Self;
    /// Canonical 32-byte encoding.
    fn to_bytes(self) -> [u8; 32];
    /// Canonical decoding from 32-byte representation.
    fn from_bytes(bytes: &[u8; 32]) -> Result<Self, Error>;
    /// Derive from 64 uniform random bytes (Fiat-Shamir challenge source).
    fn from_uniform(bytes: &[u8; 64]) -> Self;
}

/// Group behaviour required by the IPA algorithms.
pub trait IpaGroup: Copy + Clone + PartialEq + Eq + fmt::Debug {
    /// Scalar type associated with the group.
    type Scalar: IpaScalar;

    /// Additive identity element.
    fn identity() -> Self;
    /// Group addition.
    fn mul(self, rhs: Self) -> Self;
    /// Group inverse.
    fn inv(self) -> Result<Self, Error>;
    /// Scalar multiplication.
    fn pow(self, exp: Self::Scalar) -> Self;
    /// Canonical compressed bytes.
    fn to_bytes(self) -> [u8; 32];
    /// Attempt to decode from canonical bytes.
    fn from_bytes(bytes: &[u8; 32]) -> Result<Self, Error>;
}

/// Multiplicative accumulation of group elements.
pub fn product<G>(iter: impl IntoIterator<Item = G>) -> G
where
    G: IpaGroup,
{
    iter.into_iter().fold(G::identity(), |acc, g| acc.mul(g))
}

/// Trait implemented by each backend to integrate with the generic IPA code.
pub trait IpaBackend {
    /// Scalar field element type.
    type Scalar: IpaScalar;
    /// Commitment group element type.
    type Group: IpaGroup<Scalar = Self::Scalar> + Send + Sync + 'static;

    /// Curve identifier advertised over Norito payloads.
    const CURVE_ID: ZkCurveId;

    /// Deterministically hash the DST, generator kind and indices into a group element.
    fn derive_group_elem(kind: &[u8], n: u64, i: u64) -> Self::Group {
        let mut buf = Vec::with_capacity(DST.len() + kind.len() + 16);
        buf.extend_from_slice(DST.as_bytes());
        buf.extend_from_slice(kind);
        buf.extend_from_slice(&n.to_le_bytes());
        buf.extend_from_slice(&i.to_le_bytes());
        let wide = sha3_512(&buf);
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&wide);
        let scalar = Self::Scalar::from_uniform(&arr);
        Self::group_from_scalar(scalar)
    }

    /// Convert a scalar to a group element (deterministic generator derivation helper).
    fn group_from_scalar(scalar: Self::Scalar) -> Self::Group;
}
