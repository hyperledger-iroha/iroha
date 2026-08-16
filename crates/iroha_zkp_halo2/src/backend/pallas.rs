//! Pallas (Pasta) backend wiring for IPA commitments.
use crate::{
    backend::{IpaBackend, IpaGroup, IpaScalar, traits::generator_derivation_message},
    constants::GENERATOR_HASH_TO_CURVE_DST,
    errors::Error,
    field, group,
    norito_types::ZkCurveId,
};
use halo2curves::{CurveExt as _, pasta::Vesta};
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
    fn derive_group_elem(kind: &[u8], n: u64, i: u64) -> Self::Group {
        let mut message = generator_derivation_message(kind, n, i);
        let base_len = message.len();
        let map = Vesta::hash_to_curve(GENERATOR_HASH_TO_CURVE_DST);
        let mut retry = 0u64;
        loop {
            if retry > 0 {
                message.truncate(base_len);
                message.push(0xff);
                message.extend_from_slice(&retry.to_le_bytes());
            }
            let candidate = Group::new(map(&message));
            if candidate != Group::identity() {
                return candidate;
            }
            retry = retry
                .checked_add(1)
                .expect("hash-to-curve retry counter cannot be exhausted");
        }
    }
    fn msm(bases: &[Self::Group], scalars: &[Self::Scalar]) -> Result<Self::Group, Error> {
        use halo2curves::{msm::msm_best, pasta::VestaAffine};
        if bases.len() != scalars.len() {
            return Err(Error::DimensionMismatch {
                expected: bases.len(),
                actual: scalars.len(),
            });
        }
        let affine_bases = bases
            .iter()
            .map(|base| VestaAffine::from(base.inner()))
            .collect::<Vec<_>>();
        let coeffs = scalars.iter().copied().map(Into::into).collect::<Vec<_>>();
        Ok(group::GroupElem::new(msm_best(&coeffs, &affine_bases)))
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
        let params = Params::new(2).expect("derive Pallas parameters");
        for (actual, kind, index) in [
            (params.g()[0], b"G".as_slice(), 0),
            (params.h()[0], b"H".as_slice(), 0),
            (params.u(), b"U".as_slice(), 0),
        ] {
            let message = generator_derivation_message(kind, 2, index);
            let expected = Group::new(Vesta::hash_to_curve(GENERATOR_HASH_TO_CURVE_DST)(&message));
            assert_eq!(actual, expected);
            assert_ne!(actual, Group::identity());
        }
        assert_ne!(params.g()[0], params.h()[0]);
        assert_ne!(params.g()[0], params.u());
        assert_ne!(params.h()[0], params.u());
        assert_eq!(
            params.g()[0].to_bytes(),
            hex!("f86598252c47770118304767b31aec670391fbfa3de19e4206dac5727ab9a5ac")
        );
        assert_eq!(
            params.g()[1].to_bytes(),
            hex!("995d3697930bf4f4be8954e158a39a1ec07e14ea4ffeb32f433756589d2eb287")
        );
        assert_eq!(
            params.h()[0].to_bytes(),
            hex!("7f810752062de4c59ca9e05f6625b81c52ae4b3fef0c2a12a52554fd75088995")
        );
        assert_eq!(
            params.h()[1].to_bytes(),
            hex!("1ddf6a631f527e3a97d047e156b0700d6638b8313d03fa442fc488743c8ca500")
        );
        assert_eq!(
            params.u().to_bytes(),
            hex!("d8cb63abba2d44cf2e9aea873c6ddf202bc3953978cca0b8427f0e448af0642b")
        );

        let legacy_0 = legacy_generator_scalar(b"G", 2, 0);
        let legacy_1 = legacy_generator_scalar(b"G", 2, 1);
        let colliding_coefficients = vec![legacy_1, legacy_0.neg()];
        let commitment = Polynomial::from_coeffs(colliding_coefficients)
            .commit(&params)
            .expect("commit adversarial coefficient vector");
        assert_ne!(
            commitment,
            Group::identity(),
            "the old scalar-derived bases made this nonzero vector commit to identity"
        );
    }
}
