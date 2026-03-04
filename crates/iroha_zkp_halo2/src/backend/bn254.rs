//! BN254 backend wiring for IPA commitments.
//!
//! This backend leverages `halo2curves` BN254 types to expose deterministic
//! generators along with canonical scalar/group encodings compatible with the
//! Norito wire types used across Iroha.

use core::fmt;

use halo2curves::{
    bn256::{Fr, G1, G1Affine},
    ff::{Field, FromUniformBytes, PrimeField},
    group::{Group as HaloGroup, GroupEncoding},
};

use crate::{
    backend::{IpaBackend, IpaGroup, IpaScalar},
    errors::Error,
    norito_types::ZkCurveId,
};

/// Scalar field element over BN254 (`Fr`).
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct Scalar(Fr);

impl fmt::Debug for Scalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bn254Scalar(0x{} )", hex::encode(self.to_bytes()))
    }
}

impl Scalar {
    /// Returns zero.
    #[inline]
    pub fn zero() -> Self {
        Self(Fr::ZERO)
    }

    /// Returns one.
    #[inline]
    pub fn one() -> Self {
        Self(Fr::ONE)
    }
}

impl From<u64> for Scalar {
    fn from(value: u64) -> Self {
        Self(Fr::from(value))
    }
}

impl From<u32> for Scalar {
    fn from(value: u32) -> Self {
        Self(Fr::from(u64::from(value)))
    }
}

impl From<Fr> for Scalar {
    fn from(value: Fr) -> Self {
        Self(value)
    }
}

impl From<Scalar> for Fr {
    fn from(value: Scalar) -> Self {
        value.0
    }
}

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
        Self(self.0 + rhs.0)
    }

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }

    #[inline]
    fn neg(self) -> Self {
        Self(-self.0)
    }

    fn inv(self) -> Result<Self, Error> {
        Option::<Fr>::from(self.0.invert())
            .map(Self)
            .ok_or(Error::InversionOfZero)
    }

    fn pow_u64(self, mut exp: u64) -> Self {
        let mut base = self;
        let mut acc = Self::one();
        while exp > 0 {
            if exp & 1 == 1 {
                acc = acc.mul(base);
            }
            base = base.mul(base);
            exp >>= 1;
        }
        acc
    }

    fn pow_u128(self, mut exp: u128) -> Self {
        let mut base = self;
        let mut acc = Self::one();
        while exp > 0 {
            if exp & 1 == 1 {
                acc = acc.mul(base);
            }
            base = base.mul(base);
            exp >>= 1;
        }
        acc
    }

    fn to_bytes(self) -> [u8; 32] {
        let repr = self.0.to_repr();
        let mut out = [0u8; 32];
        out.copy_from_slice(repr.as_ref());
        out
    }

    fn from_bytes(bytes: &[u8; 32]) -> Result<Self, Error> {
        let mut repr = <Fr as PrimeField>::Repr::default();
        repr.as_mut().copy_from_slice(bytes);
        Option::<Fr>::from(Fr::from_repr(repr))
            .map(Self)
            .ok_or(Error::InvalidEncoding)
    }

    fn from_uniform(bytes: &[u8; 64]) -> Self {
        let mut val = Fr::from_uniform_bytes(bytes);
        if val.is_zero().into() {
            val = Fr::ONE;
        }
        Self(val)
    }
}

/// G1 group element on BN254 (projective form).
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct GroupElem(G1);

impl fmt::Debug for GroupElem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bn254Group(0x{} )", hex::encode(self.to_bytes()))
    }
}

impl GroupElem {
    fn from_projective(inner: G1) -> Self {
        Self(inner)
    }

    /// Returns the additive identity element (point at infinity).
    pub fn identity() -> Self {
        Self(G1::identity())
    }
}

impl IpaGroup for GroupElem {
    type Scalar = Scalar;

    fn identity() -> Self {
        Self::identity()
    }

    fn mul(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }

    fn inv(self) -> Result<Self, Error> {
        Ok(Self(-self.0))
    }

    fn pow(self, exp: Self::Scalar) -> Self {
        let scalar: Fr = exp.into();
        Self(self.0 * scalar)
    }

    fn to_bytes(self) -> [u8; 32] {
        let repr = G1Affine::from(self.0).to_bytes();
        let mut out = [0u8; 32];
        out.copy_from_slice(repr.as_ref());
        out
    }

    fn from_bytes(bytes: &[u8; 32]) -> Result<Self, Error> {
        let mut repr = <G1Affine as GroupEncoding>::Repr::default();
        repr.as_mut().copy_from_slice(bytes);
        G1Affine::from_bytes(&repr)
            .into_option()
            .map(|aff| Self(G1::from(aff)))
            .ok_or(Error::InvalidEncoding)
    }
}

/// BN254 backend marker.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bn254Backend;

impl IpaBackend for Bn254Backend {
    type Scalar = Scalar;
    type Group = GroupElem;

    const CURVE_ID: ZkCurveId = ZkCurveId::Bn254;

    fn group_from_scalar(scalar: Self::Scalar) -> Self::Group {
        let inner: Fr = scalar.into();
        GroupElem::from_projective(G1::generator() * inner)
    }
}

/// Parameter alias for the BN254 backend.
pub type Params = crate::params::Params<Bn254Backend>;
/// Polynomial alias for the BN254 backend.
pub type Polynomial = crate::poly::Polynomial<Bn254Backend>;
/// IPA proof alias for the BN254 backend.
pub type IpaProof = crate::ipa::IpaProof<Bn254Backend>;
/// IPA prover alias for the BN254 backend.
pub type IpaProver = crate::ipa::IpaProver<Bn254Backend>;
/// IPA verifier alias for the BN254 backend.
pub type IpaVerifier = crate::ipa::IpaVerifier<Bn254Backend>;
