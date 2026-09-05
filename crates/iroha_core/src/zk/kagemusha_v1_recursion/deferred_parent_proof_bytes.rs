//! Canonical bytes copy-bound to the ordinary proof consumed by the scalar verifier.
//!
//! These bytes are reconstructed from the verifier's assigned transcript objects, never from a
//! second host-supplied byte witness. They do not independently establish monetary authority:
//! callers still have to carry the complete history and enforce every reciprocal curve equation.

use super::*;
use crate::zk::pasta_sha256::PastaSha256ByteV1;
use snark_verifier::{
    loader::EcPointLoader as _,
    system::halo2::transcript::halo2::TranscriptObject,
    util::{msm::Msm, transcript::TranscriptRead as _},
    verifier::plonk::PlonkProof,
};

// Kagemusha V1 circuits request no circuit-specific transcript challenges. The authenticated
// Halo2 release profile therefore contains only theta, beta/gamma, and alpha: at most two in one
// phase and four in total. Challenge squeezes do not consume proof bytes, so these bounds must be
// checked independently before the transcript parser allocates their loaded-scalar representation.
const KAGEMUSHA_ORDINARY_CHALLENGES_PER_PHASE_MAX_V1: usize = 2;
const KAGEMUSHA_ORDINARY_CHALLENGES_TOTAL_MAX_V1: usize = 4;

pub(super) fn validate_ordinary_challenge_profile_v1(
    challenge_counts: &[usize],
) -> Result<usize, Error> {
    let mut total = 0_usize;
    for &count in challenge_counts {
        if count > KAGEMUSHA_ORDINARY_CHALLENGES_PER_PHASE_MAX_V1 {
            return Err(transcript_error(format!(
                "Kagemusha ordinary proof challenge phase has {count} challenges, release maximum is {KAGEMUSHA_ORDINARY_CHALLENGES_PER_PHASE_MAX_V1}",
            )));
        }
        total = total.checked_add(count).ok_or_else(|| {
            transcript_error("Kagemusha ordinary proof challenge count overflowed")
        })?;
    }
    if total > KAGEMUSHA_ORDINARY_CHALLENGES_TOTAL_MAX_V1 {
        return Err(transcript_error(format!(
            "Kagemusha ordinary proof has {total} challenges, release maximum is {KAGEMUSHA_ORDINARY_CHALLENGES_TOTAL_MAX_V1}",
        )));
    }
    Ok(total)
}

pub(super) fn validate_hybrid_carrier_lagrange_capacity_v1(
    carrier_instance_count: usize,
    srs_lagrange_capacity: usize,
) -> Result<(), Error> {
    if carrier_instance_count > srs_lagrange_capacity {
        return Err(Error::InvalidInstances);
    }
    Ok(())
}

const KAGEMUSHA_HYBRID_COMMITMENT_BYTES_V1: usize = 32;

pub(super) fn hybrid_proof_supplied_commitment_bytes_v1(
    proof_supplied_commitment_count: usize,
) -> Result<usize, Error> {
    proof_supplied_commitment_count
        .checked_mul(KAGEMUSHA_HYBRID_COMMITMENT_BYTES_V1)
        .ok_or_else(|| transcript_error("Kagemusha hybrid commitment byte length overflowed"))
}

pub(super) fn validate_hybrid_commitment_limb_indices_v1(
    semantic_instance_count: usize,
    carrier_commitment_limb_indices: &[[usize; 2]],
    expected_carrier_count: usize,
) -> Result<(), Error> {
    if !(1..=2).contains(&expected_carrier_count)
        || carrier_commitment_limb_indices.len() != expected_carrier_count
    {
        return Err(Error::InvalidInstances);
    }
    let mut expected_next = None;
    for indices in carrier_commitment_limb_indices {
        if indices[0].checked_add(1) != Some(indices[1])
            || indices[1] >= semantic_instance_count
            || expected_next.is_some_and(|expected| indices[0] != expected)
        {
            return Err(Error::InvalidInstances);
        }
        expected_next = indices[1].checked_add(1);
    }
    Ok(())
}

pub(in crate::zk::kagemusha_v1_recursion) type DeferredProofStreamV1<'chip, C> =
    Vec<TranscriptObject<C, DeferredLoader<'chip, C>>>;

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
    /// Final transcript squeeze after every ordinary-proof object has been absorbed.
    ///
    /// This is a constrained, constant-size commitment to the protocol transcript
    /// initial state, every public-instance commitment, and every proof-read object.
    pub(in crate::zk::kagemusha_v1_recursion) transcript_binding: AssignedValue<C::ScalarExt>,
    /// Constrained scalar and compressed-point encodings in exact proof-read order.
    pub(in crate::zk::kagemusha_v1_recursion) canonical_bytes: Vec<PastaSha256ByteV1<C::ScalarExt>>,
}

/// Result of the authenticated two-column hybrid ordinary-proof reader.
///
/// Column zero remains a compact semantic instance whose commitment is
/// reconstructed through the protocol ICK. Column one is the wide carrier:
/// its commitment is read from the proof and immediately constrained to the
/// canonical two-`u128` point encoding stored in column zero.
pub(in crate::zk::kagemusha_v1_recursion) struct KagemushaHybridOrdinaryProofV1<'chip, C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    /// Current IPA opening claim emitted by the ordinary proof verifier.
    pub(in crate::zk::kagemusha_v1_recursion) accumulator: DeferredAccumulator<'chip, C>,
    /// Final transcript squeeze after both instance commitments and every
    /// hybrid-proof object have been absorbed.
    pub(in crate::zk::kagemusha_v1_recursion) transcript_binding: AssignedValue<C::ScalarExt>,
    /// Proof-read commitment claimed as the canonical wide-carrier commitment.
    /// The reciprocal dense ICK relation must substantiate that claim.
    pub(in crate::zk::kagemusha_v1_recursion) carrier_commitment: DeferredEcPoint<'chip, C>,
    /// Exact proof-read objects, including the hybrid carrier commitment.
    pub(in crate::zk::kagemusha_v1_recursion) loaded_stream: DeferredProofStreamV1<'chip, C>,
}

/// Result of the authenticated three-column hybrid ordinary-proof reader.
///
/// Column zero remains the compact semantic instance. Columns one and two are
/// wide carriers whose proof-supplied commitments are bound, in column order,
/// to four consecutive `u128` limbs in the semantic column.
pub(in crate::zk::kagemusha_v1_recursion) struct KagemushaTwoCarrierHybridOrdinaryProofV1<'chip, C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    /// Current IPA opening claim emitted by the ordinary proof verifier.
    pub(in crate::zk::kagemusha_v1_recursion) accumulator: DeferredAccumulator<'chip, C>,
    /// Final transcript squeeze after all three instance commitments and every
    /// hybrid-proof object have been absorbed.
    pub(in crate::zk::kagemusha_v1_recursion) transcript_binding: AssignedValue<C::ScalarExt>,
    /// Proof-read commitments for instance columns one and two, in that order.
    pub(in crate::zk::kagemusha_v1_recursion) carrier_commitments: [DeferredEcPoint<'chip, C>; 2],
    /// Exact proof-read objects, including both hybrid carrier commitments.
    pub(in crate::zk::kagemusha_v1_recursion) loaded_stream: DeferredProofStreamV1<'chip, C>,
}

struct KagemushaMultiCarrierHybridOrdinaryProofV1<'chip, C, const N: usize>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    accumulator: DeferredAccumulator<'chip, C>,
    transcript_binding: AssignedValue<C::ScalarExt>,
    carrier_commitments: [DeferredEcPoint<'chip, C>; N],
    loaded_stream: DeferredProofStreamV1<'chip, C>,
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
    let (accumulator, stream, transcript_binding) = verify_ordinary_proof_and_stream_at_k_v1(
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
        transcript_binding,
        canonical_bytes,
    })
}

/// Verify an ordinary proof and retain only its accumulator and final transcript binding.
///
/// The parser still consumes the exact authenticated proof shape, but callers that only need a
/// constant-size proof identity avoid reconstructing every transcript object as constrained bytes.
pub(in crate::zk::kagemusha_v1_recursion) fn verify_ordinary_proof_with_transcript_binding_at_k_v1<
    'chip,
    C,
>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    instances: &[Vec<DeferredScalar<'chip, C>>],
    proof_bytes: &[u8],
    expected_k: usize,
) -> Result<(DeferredAccumulator<'chip, C>, AssignedValue<C::ScalarExt>), Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let (accumulator, loaded_stream, transcript_binding) =
        verify_ordinary_proof_and_stream_at_k_v1(
            loader,
            succinct_vk,
            protocol,
            instances,
            proof_bytes,
            expected_k,
        )?;
    drop(loaded_stream);
    Ok((accumulator, transcript_binding))
}

/// Reconstruct exactly the proof-read objects, excluding transcript public inputs and constants.
///
/// The pinned transcript appends only in `read_scalar` and `read_ec_point`; common inputs and
/// challenge squeezes do not append. Both encoding helpers range-check and equality-bind the
/// original assigned cells, including the scalar modulus and compressed-point sign bit.
pub(in crate::zk::kagemusha_v1_recursion) fn canonical_loaded_proof_bytes_v1<'chip, C>(
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
        AssignedValue<C::ScalarExt>,
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

/// Verify one hybrid two-column IPA proof without loading the wide carrier.
///
/// The proof prefix is exactly the one emitted by
/// `ProverIPAHybrid<C, { 1 << 1 }>`: the verifier reconstructs and absorbs the
/// column-zero commitment, then reads the column-one commitment before any
/// advice commitment. The latter is not trusted as a free witness: its exact
/// compressed-point limbs are equality-bound to the selected cells of the
/// semantic column before the parsed proof is verified. The caller must still
/// enforce the reciprocal dense ICK relation between that point, every carrier
/// value, and Halo2's fixed `Blind::default()` constant; this scalar-half
/// parser cannot establish that cross-Pasta relation by itself.
pub(in crate::zk::kagemusha_v1_recursion) fn verify_hybrid_ordinary_proof_and_stream_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    semantic_instances: &[DeferredScalar<'chip, C>],
    carrier_commitment_limb_indices: [usize; 2],
    proof_bytes: &[u8],
) -> Result<KagemushaHybridOrdinaryProofV1<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let KagemushaMultiCarrierHybridOrdinaryProofV1 {
        accumulator,
        transcript_binding,
        carrier_commitments: [carrier_commitment],
        loaded_stream,
    } = verify_multi_carrier_hybrid_ordinary_proof_and_stream_v1(
        loader,
        succinct_vk,
        protocol,
        semantic_instances,
        [carrier_commitment_limb_indices],
        proof_bytes,
    )?;
    Ok(KagemushaHybridOrdinaryProofV1 {
        accumulator,
        transcript_binding,
        carrier_commitment,
        loaded_stream,
    })
}

/// Verify one claim-specific hybrid three-column IPA proof without loading either wide carrier.
///
/// The prefix emitted by `ProverIPAHybrid<C, 0b110>` supplies commitments for
/// instance columns one and two in column order. Both commitments are absorbed
/// before advice, and each canonical two-`u128` compressed-point encoding is
/// equality-bound to its corresponding semantic-column limb pair. The ordinary
/// PLONK proof then opens all three committed instance polynomials.
pub(in crate::zk::kagemusha_v1_recursion) fn verify_two_carrier_hybrid_ordinary_proof_and_stream_v1<
    'chip,
    C,
>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    semantic_instances: &[DeferredScalar<'chip, C>],
    carrier_commitment_limb_indices: [[usize; 2]; 2],
    proof_bytes: &[u8],
) -> Result<KagemushaTwoCarrierHybridOrdinaryProofV1<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let KagemushaMultiCarrierHybridOrdinaryProofV1 {
        accumulator,
        transcript_binding,
        carrier_commitments,
        loaded_stream,
    } = verify_multi_carrier_hybrid_ordinary_proof_and_stream_v1(
        loader,
        succinct_vk,
        protocol,
        semantic_instances,
        carrier_commitment_limb_indices,
        proof_bytes,
    )?;
    Ok(KagemushaTwoCarrierHybridOrdinaryProofV1 {
        accumulator,
        transcript_binding,
        carrier_commitments,
        loaded_stream,
    })
}

fn verify_multi_carrier_hybrid_ordinary_proof_and_stream_v1<'chip, C, const N: usize>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
    semantic_instances: &[DeferredScalar<'chip, C>],
    carrier_commitment_limb_indices: [[usize; 2]; N],
    proof_bytes: &[u8],
) -> Result<KagemushaMultiCarrierHybridOrdinaryProofV1<'chip, C, N>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    validate_zk_ipa_succinct_key_v1(succinct_vk, KagemushaIpaProofKindV1::Ordinary)?;
    if succinct_vk.domain.k != protocol.domain.k
        || succinct_vk.domain.k != KAGEMUSHA_RECURSION_IPA_K_V1 as usize
    {
        return Err(transcript_error(
            "Kagemusha hybrid proof requires its exact authenticated IPA key and domain",
        ));
    }
    let challenge_count = validate_ordinary_challenge_profile_v1(&protocol.num_challenge)?;
    validate_hybrid_commitment_limb_indices_v1(
        semantic_instances.len(),
        &carrier_commitment_limb_indices,
        N,
    )?;
    if protocol.num_instance.len() != N + 1
        || protocol.num_instance[0] != semantic_instances.len()
        || protocol
            .num_instance
            .iter()
            .skip(1)
            .any(|count| *count == 0 || *count <= protocol.num_instance[0])
        || !protocol.accumulator_indices.is_empty()
    {
        return Err(Error::InvalidInstances);
    }
    let instance_committing_key = protocol
        .instance_committing_key
        .as_ref()
        .ok_or(Error::InvalidInstances)?;
    if instance_committing_key.bases.len() != semantic_instances.len()
        || instance_committing_key.constant.is_none()
    {
        return Err(Error::InvalidInstances);
    }
    for &carrier_instance_count in &protocol.num_instance[1..] {
        validate_hybrid_carrier_lagrange_capacity_v1(carrier_instance_count, succinct_vk.domain.n)?;
    }
    let instance_offset = protocol.preprocessed.len();
    let instance_end = instance_offset
        .checked_add(N + 1)
        .ok_or_else(|| transcript_error("Kagemusha hybrid instance index overflowed"))?;
    for polynomial in instance_offset..instance_end {
        if !protocol
            .evaluations
            .iter()
            .any(|query| query.poly == polynomial)
            || !protocol
                .queries
                .iter()
                .any(|query| query.poly == polynomial)
        {
            return Err(transcript_error(
                "Kagemusha hybrid instance columns must remain in the quotient and IPA opening set",
            ));
        }
    }

    let expected_len = ordinary_ipa_proof_profile_v1(protocol)
        .map_err(transcript_error)?
        .byte_len
        .checked_add(hybrid_proof_supplied_commitment_bytes_v1(N)?)
        .ok_or_else(|| transcript_error("Kagemusha hybrid proof byte length overflowed"))?;
    if proof_bytes.len() != expected_len {
        return Err(transcript_error(format!(
            "Kagemusha hybrid parent proof has length {}, expected exactly {expected_len}",
            proof_bytes.len()
        )));
    }

    let (reader, position) = ExactReader::new(proof_bytes);
    let mut transcript =
        DeferredTranscript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(loader, reader);
    if let Some(transcript_initial_state) = protocol.transcript_initial_state.as_ref() {
        transcript.common_scalar(transcript_initial_state)?;
    }

    let semantic_commitment = {
        let bases = instance_committing_key
            .bases
            .iter()
            .map(|base| loader.ec_point_load_const(base))
            .collect::<Vec<_>>();
        let constant = loader.ec_point_load_const(
            instance_committing_key
                .constant
                .as_ref()
                .expect("hybrid protocol requires its default-blind ICK constant"),
        );
        semantic_instances
            .iter()
            .zip(&bases)
            .map(|(scalar, base)| Msm::<C, DeferredLoader<'chip, C>>::base(base) * scalar)
            .chain(std::iter::once(Msm::base(&constant)))
            .sum::<Msm<'_, C, DeferredLoader<'chip, C>>>()
            .evaluate(None)
    };
    transcript.common_ec_point(&semantic_commitment)?;
    let carrier_commitments: [DeferredEcPoint<'chip, C>; N] = transcript
        .read_n_ec_points(N)?
        .try_into()
        .map_err(|_| Error::InvalidInstances)?;

    let mut witnesses = Vec::new();
    let mut challenges = Vec::with_capacity(challenge_count);
    for (&witness_count, &challenge_count) in protocol
        .num_witness
        .iter()
        .zip(protocol.num_challenge.iter())
    {
        witnesses.extend(transcript.read_n_ec_points(witness_count)?);
        challenges.extend(transcript.squeeze_n_challenges(challenge_count));
    }
    let quotients = transcript.read_n_ec_points(protocol.quotient.num_chunk())?;
    let z = transcript.squeeze_challenge();
    let evaluations = transcript.read_n_scalars(protocol.evaluations.len())?;
    let pcs = <IpaAs<C, Bgh19> as snark_verifier::pcs::PolynomialCommitmentScheme<
        C,
        DeferredLoader<'chip, C>,
    >>::read_proof(
        succinct_vk,
        &PlonkProof::<C, DeferredLoader<'chip, C>, IpaAs<C, Bgh19>>::empty_queries(protocol),
        &mut transcript,
    )?;

    {
        let chip = loader.ecc_chip();
        let mut ctx = loader.ctx_mut();
        for (carrier_commitment, limb_indices) in carrier_commitments
            .iter()
            .zip(&carrier_commitment_limb_indices)
        {
            let expected_carrier_limbs =
                (*limb_indices).map(|index| *semantic_instances[index].assigned());
            let actual_carrier_limbs =
                chip.assigned_point_poseidon_elements_v1(&mut ctx, &carrier_commitment.assigned())?;
            for (actual, expected) in actual_carrier_limbs
                .iter()
                .zip(expected_carrier_limbs.iter())
            {
                ctx.main().constrain_equal(actual, expected);
            }
        }
    }

    let mut committed_instances = Vec::with_capacity(N + 1);
    committed_instances.push(semantic_commitment);
    committed_instances.extend(carrier_commitments.iter().cloned());
    let parsed = PlonkProof::<C, DeferredLoader<'chip, C>, IpaAs<C, Bgh19>> {
        committed_instances: Some(committed_instances),
        witnesses,
        challenges,
        quotients,
        z,
        evaluations,
        pcs,
        old_accumulators: Vec::new(),
    };
    // `instance_committing_key = Some` makes the existing verifier consume the
    // proof-read instance evaluations and the commitments above. It never
    // derives an evaluation from, or allocates ICK bases for, these empty
    // carrier placeholders.
    let mut verification_instances = Vec::with_capacity(N + 1);
    verification_instances.push(semantic_instances.to_vec());
    verification_instances.extend((0..N).map(|_| Vec::new()));
    let mut accumulators = PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::verify(
        succinct_vk,
        protocol,
        &verification_instances,
        &parsed,
    )?;
    if position.get() != proof_bytes.len() {
        return Err(transcript_error(
            "Kagemusha hybrid parent proof has trailing bytes",
        ));
    }
    if accumulators.len() != 1 {
        return Err(Error::AssertionFailure(
            "Kagemusha hybrid parent verifier did not emit one IPA accumulator".to_owned(),
        ));
    }
    // This squeeze consumes no proof bytes. It is deliberately taken only
    // after the exact-length parser has absorbed every object and all common
    // instance commitments, so claim-specific batching can bind that complete
    // transcript without re-hashing every verifier equation term.
    let transcript_binding = transcript.squeeze_challenge().into_assigned();
    Ok(KagemushaMultiCarrierHybridOrdinaryProofV1 {
        accumulator: accumulators.remove(0),
        transcript_binding,
        carrier_commitments,
        loaded_stream: transcript.loaded_stream,
    })
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
        AssignedValue<C::ScalarExt>,
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
    validate_ordinary_challenge_profile_v1(&protocol.num_challenge)?;
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
    // The final squeeze commits to the protocol transcript state, every
    // committed public instance, and the complete proof-read stream. It adds
    // no bytes and therefore cannot weaken the exact parser shape checks.
    let transcript_binding = transcript.squeeze_challenge().into_assigned();
    Ok((
        accumulators.remove(0),
        transcript.loaded_stream,
        transcript_binding,
    ))
}
