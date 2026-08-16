//! BN254 backend wiring for IPA commitments.
//!
//! This backend leverages `halo2curves` BN254 types to expose deterministic generators along with
//! canonical scalar/group encodings compatible with the Norito wire types used across Iroha.
use crate::{
    backend::{IpaBackend, IpaGroup, IpaScalar, traits::generator_derivation_message},
    constants::GENERATOR_HASH_TO_CURVE_DST,
    errors::Error,
    norito_types::ZkCurveId,
};
use core::fmt;
use halo2curves::{
    CurveExt as _,
    bn256::{Fr, G1, G1Affine},
    ff::{Field, FromUniformBytes, PrimeField},
    group::{Group as HaloGroup, GroupEncoding},
    msm::msm_best,
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
    fn derive_group_elem(kind: &[u8], n: u64, i: u64) -> Self::Group {
        let mut message = generator_derivation_message(kind, n, i);
        let base_len = message.len();
        let map = G1::hash_to_curve(GENERATOR_HASH_TO_CURVE_DST);
        let mut retry = 0u64;
        loop {
            if retry > 0 {
                message.truncate(base_len);
                message.push(0xff);
                message.extend_from_slice(&retry.to_le_bytes());
            }
            let candidate = GroupElem::from_projective(map(&message));
            if candidate != GroupElem::identity() {
                return candidate;
            }
            retry = retry
                .checked_add(1)
                .expect("hash-to-curve retry counter cannot be exhausted");
        }
    }
    fn msm(bases: &[Self::Group], scalars: &[Self::Scalar]) -> Result<Self::Group, Error> {
        if bases.len() != scalars.len() {
            return Err(Error::DimensionMismatch {
                expected: bases.len(),
                actual: scalars.len(),
            });
        }
        let affine_bases = bases
            .iter()
            .map(|base| G1Affine::from(base.0))
            .collect::<Vec<_>>();
        let coeffs = scalars.iter().copied().map(Into::into).collect::<Vec<_>>();
        Ok(GroupElem::from_projective(msm_best(&coeffs, &affine_bases)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{constants::DST, hash::sha3_512};
    use hex_literal::hex;

    fn legacy_generator_scalar(kind: &[u8], n: u64, i: u64) -> Scalar {
        let mut message = Vec::with_capacity(DST.len() + kind.len() + 16);
        message.extend_from_slice(DST.as_bytes());
        message.extend_from_slice(kind);
        message.extend_from_slice(&n.to_le_bytes());
        message.extend_from_slice(&i.to_le_bytes());
        Scalar::from_uniform(&sha3_512(&message))
    }

    #[test]
    fn transparent_generators_use_hash_to_curve_and_break_the_legacy_known_relation() {
        let params = Params::new(2).expect("derive BN254 parameters");
        for (actual, kind, index) in [
            (params.g()[0], b"G".as_slice(), 0),
            (params.h()[0], b"H".as_slice(), 0),
            (params.u(), b"U".as_slice(), 0),
        ] {
            let message = generator_derivation_message(kind, 2, index);
            let expected = GroupElem::from_projective(G1::hash_to_curve(
                GENERATOR_HASH_TO_CURVE_DST,
            )(&message));
            assert_eq!(actual, expected);
            assert_ne!(actual, GroupElem::identity());
        }
        assert_ne!(params.g()[0], params.h()[0]);
        assert_ne!(params.g()[0], params.u());
        assert_ne!(params.h()[0], params.u());
        assert_eq!(
            params.g()[0].to_bytes(),
            hex!("3fada648ba55001692a8499434ce1414f5ed5a179f521aaae1f0c235a929fc2c")
        );
        assert_eq!(
            params.g()[1].to_bytes(),
            hex!("2d72f599baeecf9c3cbf11a0d65c63890ca154a4cfe3170565de561f5ed6ea9b")
        );
        assert_eq!(
            params.h()[0].to_bytes(),
            hex!("d4f2d34a5b5a9358fdc1ce80fdb69a95fa5f70e8827a32208720688a2fd41207")
        );
        assert_eq!(
            params.h()[1].to_bytes(),
            hex!("33d3149ea5267a363186e6b69ca43f5528135049180f114d3c36efec69090baf")
        );
        assert_eq!(
            params.u().to_bytes(),
            hex!("086dce930f8fe4ab06efa12ec34ff5876a741456818341d5b108371a8cdee381")
        );

        let legacy_0 = legacy_generator_scalar(b"G", 2, 0);
        let legacy_1 = legacy_generator_scalar(b"G", 2, 1);
        let colliding_coefficients = vec![legacy_1, legacy_0.neg()];
        let commitment = Polynomial::from_coeffs(colliding_coefficients)
            .commit(&params)
            .expect("commit adversarial coefficient vector");
        assert_ne!(
            commitment,
            GroupElem::identity(),
            "the old scalar-derived bases made this nonzero vector commit to identity"
        );
    }
}
