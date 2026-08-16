//! Backend trait hierarchy shared by all implementations.
use crate::{constants::GENERATOR_HASH_TO_CURVE_DST, errors::Error, norito_types::ZkCurveId};
use core::{fmt, mem::size_of};

/// Build the canonical message mapped to one transparent IPA generator.
pub(crate) fn generator_derivation_message(kind: &[u8], n: u64, i: u64) -> Vec<u8> {
    let kind_len = u64::try_from(kind.len()).expect("generator kind length must fit u64");
    let mut message = Vec::with_capacity(
        GENERATOR_HASH_TO_CURVE_DST.len() + 1 + size_of::<u64>() * 3 + kind.len(),
    );
    message.extend_from_slice(GENERATOR_HASH_TO_CURVE_DST.as_bytes());
    message.push(0);
    message.extend_from_slice(&kind_len.to_le_bytes());
    message.extend_from_slice(kind);
    message.extend_from_slice(&n.to_le_bytes());
    message.extend_from_slice(&i.to_le_bytes());
    message
}
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
    /// Deterministically map the generator kind and indices to a group element.
    ///
    /// Cryptographic backends must use a domain-separated hash-to-curve map. Multiplying a fixed
    /// generator by a publicly derived scalar exposes the discrete-log relationships between the
    /// IPA bases and destroys commitment binding.
    fn derive_group_elem(kind: &[u8], n: u64, i: u64) -> Self::Group;
    /// Compute a variable-base multi-scalar multiplication.
    ///
    /// Backends can override this with an optimized deterministic MSM. The default path preserves
    /// correctness for simple backends by accumulating one scalar multiplication per base.
    fn msm(bases: &[Self::Group], scalars: &[Self::Scalar]) -> Result<Self::Group, Error> {
        if bases.len() != scalars.len() {
            return Err(Error::DimensionMismatch {
                expected: bases.len(),
                actual: scalars.len(),
            });
        }
        Ok(product(
            bases
                .iter()
                .zip(scalars.iter())
                .map(|(base, scalar)| base.pow(*scalar)),
        ))
    }
}
