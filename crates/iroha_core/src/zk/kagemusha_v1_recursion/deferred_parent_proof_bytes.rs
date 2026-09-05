//! Canonical bytes copy-bound to the ordinary proof consumed by the scalar verifier.
//!
//! These bytes are reconstructed from the verifier's assigned transcript objects, never from a
//! second host-supplied byte witness. They do not independently establish monetary authority:
//! callers still have to carry the complete history and enforce every reciprocal curve equation.

use super::*;
use crate::zk::pasta_sha256::PastaSha256ByteV1;
use snark_verifier::system::halo2::transcript::halo2::TranscriptObject;

type DeferredProofStreamV1<'chip, C> = Vec<TranscriptObject<C, DeferredLoader<'chip, C>>>;

/// One succinct scalar-half result and the exact canonical proof bytes it consumed.
///
/// The byte count is fixed by the authenticated ordinary-proof profile, not by a new witness
/// length or an input/history limit. All bytes share the scalar/point cells used by the verifier.
pub(in crate::zk::kagemusha_v1_recursion) struct KagemushaAssignedOrdinaryProofV1<'chip, C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    /// Current opening claim, which must enter the same authenticated history as before.
    pub(in crate::zk::kagemusha_v1_recursion) accumulator: DeferredAccumulator<'chip, C>,
    /// Constrained scalar and compressed-point encodings in exact proof-read order.
    pub(in crate::zk::kagemusha_v1_recursion) canonical_bytes: Vec<PastaSha256ByteV1<C::ScalarExt>>,
}

/// Verify an ordinary proof and expose its assigned canonical bytes for a containing frame.
///
/// Use this entry point when the canonical authorization or credit envelope must hash the exact
/// recursively consumed proof. Passing the original Rust slice to a separate SHA witness would
/// not prove that equality. Callers must finalize the deferred audit only after this function,
/// and must enforce the resulting equations in the reciprocal parity.
pub(in crate::zk::kagemusha_v1_recursion) fn verify_ordinary_proof_with_canonical_bytes_v1<
    'chip,
    C,
>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    instances: &[Vec<DeferredScalar<'chip, C>>],
    proof_bytes: &[u8],
) -> Result<KagemushaAssignedOrdinaryProofV1<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    verify_ordinary_proof_with_canonical_bytes_at_k_v1(
        loader,
        succinct_vk,
        protocol,
        instances,
        proof_bytes,
        KAGEMUSHA_RECURSION_IPA_K_V1 as usize,
    )
}

/// Verify an authenticated internal helper proof at its exact smaller IPA domain.
///
/// The returned opening accumulator retains that smaller round count. A monetary caller must
/// soundly lift/fold it into the fixed `k = 16` history before it can authorize value.
pub(in crate::zk::kagemusha_v1_recursion) fn verify_ordinary_proof_with_canonical_bytes_at_k_v1<
    'chip,
    C,
>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    instances: &[Vec<DeferredScalar<'chip, C>>],
    proof_bytes: &[u8],
    expected_k: usize,
) -> Result<KagemushaAssignedOrdinaryProofV1<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let (accumulator, stream) = verify_ordinary_proof_and_stream_at_k_v1(
        loader,
        succinct_vk,
        protocol,
        instances,
        proof_bytes,
        expected_k,
    )?;
    let expected_len = ordinary_ipa_proof_profile_at_k_v1(protocol, expected_k)
        .map_err(transcript_error)?
        .byte_len;
    let canonical_bytes = canonical_loaded_proof_bytes_v1(loader, &stream, expected_len)?;
    Ok(KagemushaAssignedOrdinaryProofV1 {
        accumulator,
        canonical_bytes,
    })
}

/// Reconstruct exactly the proof-read objects, excluding transcript public inputs and constants.
///
/// The pinned transcript appends only in `read_scalar` and `read_ec_point`; common inputs and
/// challenge squeezes do not append. Both encoding helpers range-check and equality-bind the
/// original assigned cells, including the scalar modulus and compressed-point sign bit.
pub(super) fn canonical_loaded_proof_bytes_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    stream: &[TranscriptObject<C, DeferredLoader<'chip, C>>],
    expected_byte_len: usize,
) -> Result<Vec<PastaSha256ByteV1<C::ScalarExt>>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if stream.len().checked_mul(32) != Some(expected_byte_len) {
        return Err(transcript_error(
            "Kagemusha canonical proof transcript inventory differs from its authenticated length",
        ));
    }
    let chip = loader.ecc_chip();
    let mut ctx = loader.ctx_mut();
    let mut bytes = Vec::with_capacity(expected_byte_len);
    for object in stream {
        match object {
            TranscriptObject::Scalar(scalar) => {
                bytes.extend(chip.assigned_scalar_bytes(&mut ctx, *scalar.assigned()));
            }
            TranscriptObject::EcPoint(point) => {
                bytes.extend(chip.assigned_point_bytes(&mut ctx, &point.assigned())?);
            }
        }
    }
    if bytes.len() != expected_byte_len {
        return Err(transcript_error(
            "Kagemusha canonical proof encodings differ from their authenticated length",
        ));
    }
    Ok(bytes)
}

/// Shared exact ordinary-proof parser and succinct verifier, before optional byte reconstruction.
///
/// Returning the already-loaded stream does not add encoding constraints to ordinary callers.
pub(super) fn verify_ordinary_proof_and_stream_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    instances: &[Vec<DeferredScalar<'chip, C>>],
    proof_bytes: &[u8],
) -> Result<
    (
        DeferredAccumulator<'chip, C>,
        DeferredProofStreamV1<'chip, C>,
    ),
    Error,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    verify_ordinary_proof_and_stream_at_k_v1(
        loader,
        succinct_vk,
        protocol,
        instances,
        proof_bytes,
        KAGEMUSHA_RECURSION_IPA_K_V1 as usize,
    )
}

fn verify_ordinary_proof_and_stream_at_k_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    instances: &[Vec<DeferredScalar<'chip, C>>],
    proof_bytes: &[u8],
    expected_k: usize,
) -> Result<
    (
        DeferredAccumulator<'chip, C>,
        DeferredProofStreamV1<'chip, C>,
    ),
    Error,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    validate_zk_ipa_succinct_key_v1(succinct_vk, KagemushaIpaProofKindV1::Ordinary)?;
    if succinct_vk.domain.k != protocol.domain.k || succinct_vk.domain.k != expected_k {
        return Err(transcript_error(
            "Kagemusha ordinary proof requires its exact authenticated IPA key and domain",
        ));
    }
    let expected_len = ordinary_ipa_proof_profile_at_k_v1(protocol, expected_k)
        .map_err(transcript_error)?
        .byte_len;
    if proof_bytes.len() != expected_len {
        return Err(transcript_error(format!(
            "Kagemusha parent proof has length {}, expected exactly {expected_len}",
            proof_bytes.len()
        )));
    }
    let (reader, position) = ExactReader::new(proof_bytes);
    let mut transcript =
        DeferredTranscript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(loader, reader);
    let parsed = PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::read_proof(
        succinct_vk,
        protocol,
        instances,
        &mut transcript,
    )?;
    let mut accumulators = PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::verify(
        succinct_vk,
        protocol,
        instances,
        &parsed,
    )?;
    if position.get() != proof_bytes.len() {
        return Err(transcript_error(
            "Kagemusha parent proof has trailing bytes",
        ));
    }
    if accumulators.len() != 1 {
        return Err(Error::AssertionFailure(
            "Kagemusha parent verifier did not emit one IPA accumulator".to_owned(),
        ));
    }
    Ok((accumulators.remove(0), transcript.loaded_stream))
}
