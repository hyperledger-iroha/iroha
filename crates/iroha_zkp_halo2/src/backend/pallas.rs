//! Pallas (Pasta) backend wiring for IPA commitments.

use crate::{
    backend::{IpaBackend, IpaGroup, IpaScalar},
    field, group,
    norito_types::ZkCurveId,
};

/// Scalar type for the Pallas backend (existing implementation).
pub type Scalar = field::PrimeField64;
/// Group element type for the Pallas backend.
pub type Group = group::GroupElem;

/// Pallas backend marker.
#[derive(Debug, Clone, Copy, Default)]
pub struct PallasBackend;

impl IpaBackend for PallasBackend {
    type Scalar = Scalar;
    type Group = Group;

    const CURVE_ID: ZkCurveId = ZkCurveId::Pallas;

    fn group_from_scalar(scalar: Self::Scalar) -> Self::Group {
        group::from_scalar(scalar)
    }
}

// Blanket trait impls piggy-back on existing method set.
impl IpaScalar for Scalar {
    #[inline]
    fn zero() -> Self {
        Scalar::zero()
    }

    #[inline]
    fn one() -> Self {
        Scalar::one()
    }

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Scalar::add(self, rhs)
    }

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Scalar::sub(self, rhs)
    }

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Scalar::mul(self, rhs)
    }

    #[inline]
    fn neg(self) -> Self {
        Scalar::neg(self)
    }

    #[inline]
    fn inv(self) -> Result<Self, crate::errors::Error> {
        Scalar::inv(self)
    }

    #[inline]
    fn pow_u64(self, exp: u64) -> Self {
        Scalar::pow_u64(self, exp)
    }

    #[inline]
    fn pow_u128(self, exp: u128) -> Self {
        Scalar::pow_u128(self, exp)
    }

    #[inline]
    fn to_bytes(self) -> [u8; 32] {
        Scalar::to_bytes(self)
    }

    #[inline]
    fn from_bytes(bytes: &[u8; 32]) -> Result<Self, crate::errors::Error> {
        Scalar::from_bytes(bytes)
    }

    #[inline]
    fn from_uniform(bytes: &[u8; 64]) -> Self {
        Scalar::from_uniform(bytes)
    }
}

impl IpaGroup for Group {
    type Scalar = Scalar;

    fn identity() -> Self {
        Group::identity()
    }

    fn mul(self, rhs: Self) -> Self {
        Group::mul(self, rhs)
    }

    fn inv(self) -> Result<Self, crate::errors::Error> {
        Group::inv(self)
    }

    fn pow(self, exp: Self::Scalar) -> Self {
        Group::pow(self, exp)
    }

    fn to_bytes(self) -> [u8; 32] {
        Group::to_bytes(self)
    }

    fn from_bytes(bytes: &[u8; 32]) -> Result<Self, crate::errors::Error> {
        Group::from_bytes(bytes)
    }
}

/// Type alias for the Pallas parameter set.
pub type Params = crate::params::Params<PallasBackend>;
/// Polynomial alias for the Pallas backend.
pub type Polynomial = crate::poly::Polynomial<PallasBackend>;
/// Proof alias for the Pallas backend.
pub type IpaProof = crate::ipa::IpaProof<PallasBackend>;
/// Prover alias for the Pallas backend.
pub type IpaProver = crate::ipa::IpaProver<PallasBackend>;
/// Verifier alias for the Pallas backend.
pub type IpaVerifier = crate::ipa::IpaVerifier<PallasBackend>;
