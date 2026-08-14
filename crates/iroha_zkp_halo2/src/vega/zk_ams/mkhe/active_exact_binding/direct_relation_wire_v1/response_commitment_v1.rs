//! Canonical first-message reconstruction for one direct-response chunk.
//!
//! This is verifier-only arithmetic. It reuses the governed T256
//! Bulletproof basis and emits one canonical point encoding, but grants no
//! verification receipt, admission capability, or release authority.

#![allow(dead_code, reason = "the semantic direct verifier is not wired yet")]

use crate::{
    generalized_bulletproof::{
        GeneralizedBulletproofErrorV1, ProofSuite, SecretMultiexpBuilder, SecretPoint,
    },
    vega::{
        VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{
            SecretT256PointEncodingV1, ZeroizingT256ScalarCopyV1, ZkAmsT256BulletproofSuiteV1,
        },
    },
};
use thiserror::Error;

use super::super::{RESPONSE_COEFFICIENT_BOUND_V1, WITNESS_CHUNK_COEFFICIENTS_V1};

const DIRECT_RESPONSE_MSM_TERMS_V1: usize = WITNESS_CHUNK_COEFFICIENTS_V1 + 2;
const DIRECT_RESPONSE_WORD_BYTES_V1: usize = WITNESS_CHUNK_COEFFICIENTS_V1 * 8;
const DIRECT_RESPONSE_RECONSTRUCTIONS_V1: usize = 4 * 6 * 8;
const DIRECT_RESPONSE_COORDINATES_V1: usize =
    DIRECT_RESPONSE_RECONSTRUCTIONS_V1 * WITNESS_CHUNK_COEFFICIENTS_V1;
const DIRECT_RESPONSE_TOTAL_MSM_TERMS_V1: usize =
    DIRECT_RESPONSE_RECONSTRUCTIONS_V1 * DIRECT_RESPONSE_MSM_TERMS_V1;
const DIRECT_RESPONSE_REPETITION_WIRE_BYTES_V1: usize = 6 * 8 * 33;

const _: () = {
    assert!(WITNESS_CHUNK_COEFFICIENTS_V1 == 16_384);
    assert!(DIRECT_RESPONSE_MSM_TERMS_V1 == 16_386);
    assert!(DIRECT_RESPONSE_WORD_BYTES_V1 == 131_072);
    assert!(DIRECT_RESPONSE_RECONSTRUCTIONS_V1 == 192);
    assert!(DIRECT_RESPONSE_COORDINATES_V1 == 3_145_728);
    assert!(DIRECT_RESPONSE_TOTAL_MSM_TERMS_V1 == 3_146_112);
    assert!(DIRECT_RESPONSE_REPETITION_WIRE_BYTES_V1 == 1_584);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(in super::super) enum DirectResponseCommitmentErrorV1 {
    #[error("direct response coefficient {index} exceeds the release bound")]
    ResponseOutOfRange { index: usize },
    #[error("direct response blind is not a canonical big-endian T256 scalar")]
    BlindScalarEncoding,
    #[error("direct response source commitment must be non-identity")]
    SourceCommitmentIdentity,
    #[error(transparent)]
    Backend(#[from] GeneralizedBulletproofErrorV1),
}

/// Move-only canonical encoding of one reconstructed first-message point.
///
/// The identity is encoded by the governed `0x40 || 32*0` sentinel. There is
/// deliberately no decoder or raw-point accessor on this boundary.
#[derive(Debug, PartialEq, Eq)]
pub(in super::super) struct CanonicalDirectResponsePointV1([u8; 33]);

impl CanonicalDirectResponsePointV1 {
    pub(in super::super) const fn as_bytes(&self) -> &[u8; 33] {
        &self.0
    }
}

struct ZeroizingDirectResponsePointV1(SecretPoint<Point>);

impl ZeroizingDirectResponsePointV1 {
    fn into_canonical_bytes(self) -> Result<[u8; 33], GeneralizedBulletproofErrorV1> {
        let encoded = SecretT256PointEncodingV1::new_allow_identity(self.0.expose_ref())?;
        let public = *encoded.as_ref();
        drop(encoded);
        Ok(public)
    }
}

/// Reconstruct `D = sum_i(z_i G_i) + rho H - c C` for one response chunk.
pub(in super::super) fn reconstruct_direct_response_first_message_v1(
    responses: &[i64; WITNESS_CHUNK_COEFFICIENTS_V1],
    blind_response_be: &[u8; 32],
    challenge: u32,
    witness_commitment: &Point,
) -> Result<CanonicalDirectResponsePointV1, DirectResponseCommitmentErrorV1> {
    if let Some(index) = responses
        .iter()
        .position(|response| response.unsigned_abs() > RESPONSE_COEFFICIENT_BOUND_V1 as u64)
    {
        return Err(DirectResponseCommitmentErrorV1::ResponseOutOfRange { index });
    }
    if witness_commitment.is_identity() {
        return Err(DirectResponseCommitmentErrorV1::SourceCommitmentIdentity);
    }

    let generators =
        ZkAmsT256BulletproofSuiteV1::generators().reduce(WITNESS_CHUNK_COEFFICIENTS_V1)?;
    let mut terms =
        SecretMultiexpBuilder::<ZkAmsT256BulletproofSuiteV1>::new(DIRECT_RESPONSE_MSM_TERMS_V1)?;
    for (response, generator) in responses.iter().zip(generators.g_bold) {
        let response = if *response < 0 {
            ZeroizingT256ScalarCopyV1::new(-Scalar::from_u64(response.unsigned_abs()))
        } else {
            ZeroizingT256ScalarCopyV1::new(Scalar::from_u64(response.unsigned_abs()))
        };
        terms.push(response.as_ref(), generator)?;
    }
    let blind = ZeroizingT256ScalarCopyV1::new(
        Scalar::from_be_bytes_exact_ref(blind_response_be)
            .map_err(|_| DirectResponseCommitmentErrorV1::BlindScalarEncoding)?,
    );
    terms.push(blind.as_ref(), &generators.h)?;
    let negative_challenge =
        ZeroizingT256ScalarCopyV1::new(-Scalar::from_u64(u64::from(challenge)));
    terms.push(negative_challenge.as_ref(), witness_commitment)?;

    let reconstructed = ZeroizingDirectResponsePointV1(terms.evaluate()?);
    let encoded = reconstructed.into_canonical_bytes()?;
    Ok(CanonicalDirectResponsePointV1(encoded))
}

#[cfg(test)]
#[path = "response_commitment_v1_tests.rs"]
mod tests;
