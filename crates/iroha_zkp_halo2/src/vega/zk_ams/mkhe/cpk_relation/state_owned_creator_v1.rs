use super::super::collective::borrowed_product::negacyclic_multiply_signed_zeroizing_v1;
use super::super::{
    ZkAmsMkheErrorV1,
    direct_object_transport::{
        ZkAmsMkheDirectObjectCasPublicationV1, ZkAmsMkheDirectObjectPublicationReceiptV1,
        ZkAmsMkheDirectObjectPublicationTransactionV1,
    },
};
use super::*;
use crate::{
    generalized_bulletproof::{GeneralizedBulletproofErrorV1, SecretMultiexpBuilder},
    vega::bulletproof_t256::{
        SecretT256PointEncodingV1, ZeroizingT256ScalarCopyV1, acquire_zk_ams_t256_cpk_workspace_v1,
    },
};

#[allow(clippy::too_many_arguments)]
pub(super) fn prove_state_owned_opening_v1<R: MaskedRelaxedRandomSourceV1>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    expected_transcript: [u8; 32],
    public_a: &[u64],
    statement: ZkAmsMkheCpkShareStatementV1,
    secret_wire: &[u8],
    error_wire: &[u8],
    secret: &[i8],
    error: &[i8],
    secret_blindings: &[Scalar],
    error_blindings: &[Scalar],
    random: &mut R,
) -> Result<(ZkAmsMkheCpkRelationHeaderV1, ZkAmsMkheCpkRelationProofV1), ZkAmsMkheCpkRelationErrorV1>
{
    let profile = statement.validate_against_governed_roster(roster, expected_transcript)?;
    let header = ZkAmsMkheCpkRelationHeaderV1::new(statement, secret_wire, error_wire)?;
    let _workspace = acquire_zk_ams_t256_cpk_workspace_v1().map_err(map_secret_msm_v1)?;
    let response = construct_zk_ams_mkhe_cpk_responses_with_aborts_v1(
        secret,
        error,
        secret_blindings,
        error_blindings,
        random,
        |attempt| {
            reconstruct_zk_ams_mkhe_cpk_challenges_v1(
                statement,
                secret_wire,
                error_wire,
                header,
                mask_commitment_digests_v1(attempt)?,
                mask_rns_digests_v1(attempt, &profile, public_a)?,
            )
        },
    )?;
    let proof = ZkAmsMkheCpkRelationProofV1::from_prover_response(header, response)?;
    Ok((header, proof))
}

fn mask_commitment_digests_v1(
    attempt: &ZkAmsMkheCpkRelationMaskAttemptV1,
) -> Result<[ZkAmsMkheCpkCommitmentFirstMessageDigestV1; 4], ZkAmsMkheCpkRelationErrorV1> {
    let shape = attempt.shape;
    let generators = ZkAmsT256BulletproofSuiteV1::generators();
    let chunk_coefficients = shape.degree / shape.chunks;
    if shape != CpkRelationShapeV1::RELEASE || chunk_coefficients > generators.g_bold.len() {
        return Err(ZkAmsMkheCpkRelationErrorV1::Witness);
    }
    let mut digests = [ZkAmsMkheCpkCommitmentFirstMessageDigestV1([0; 32]); 4];
    for (repetition, digest) in digests.iter_mut().enumerate() {
        let mut hash = Keccak256::new();
        hash.update(CPK_COMMITMENT_FIRST_MESSAGE_DOMAIN_V1);
        hash.update(&[
            MKHE_VERSION_V1,
            repetition as u8,
            shape.witnesses as u8,
            shape.chunks as u8,
        ]);
        for role in 0..shape.witnesses {
            hash.update(&[role as u8]);
            for chunk in 0..shape.chunks {
                hash.update(&[chunk as u8]);
                let start = chunk * chunk_coefficients;
                let mut terms = SecretMultiexpBuilder::<ZkAmsT256BulletproofSuiteV1>::new(
                    chunk_coefficients + 1,
                )
                .map_err(map_secret_msm_v1)?;
                for local in 0..chunk_coefficients {
                    let index = response_index(shape, repetition, role, start + local)?;
                    let scalar = ZeroizingT256ScalarCopyV1::new(t256_scalar_from_signed_i64_v1(
                        attempt.masks()[index],
                    ));
                    terms
                        .push(scalar.as_ref(), &generators.g_bold[local])
                        .map_err(map_secret_msm_v1)?;
                }
                terms
                    .push(
                        &attempt.blind_masks()
                            [blind_response_index(shape, repetition, role, chunk)?],
                        &generators.h,
                    )
                    .map_err(map_secret_msm_v1)?;
                let point = terms.evaluate().map_err(map_secret_msm_v1)?;
                if point.is_identity() {
                    return Err(ZkAmsMkheCpkRelationErrorV1::FirstMessageRejected);
                }
                let encoding = SecretT256PointEncodingV1::new(point.expose_ref())
                    .map_err(map_secret_msm_v1)?;
                hash.update(encoding.as_ref());
                drop((encoding, point));
            }
        }
        *digest = ZkAmsMkheCpkCommitmentFirstMessageDigestV1(hash.finalize());
    }
    Ok(digests)
}

fn map_secret_msm_v1(error: GeneralizedBulletproofErrorV1) -> ZkAmsMkheCpkRelationErrorV1 {
    use GeneralizedBulletproofErrorV1::{PointIdentity, ResourceOverflow};
    use ZkAmsMkheCpkRelationErrorV1::{FirstMessageRejected, ResourceCeiling, Witness};
    match error {
        ResourceOverflow => ResourceCeiling,
        PointIdentity => FirstMessageRejected,
        _ => Witness,
    }
}

fn mask_rns_digests_v1(
    attempt: &ZkAmsMkheCpkRelationMaskAttemptV1,
    profile: &BgvProfile,
    public_a: &[u64],
) -> Result<[ZkAmsMkheCpkRnsFirstMessageDigestV1; 4], ZkAmsMkheCpkRelationErrorV1> {
    let shape = attempt.shape;
    if shape != CpkRelationShapeV1::RELEASE
        || public_a.len() != shape.degree * profile.moduli.len()
        || profile.moduli.len() != ZK_AMS_MKHE_CPK_RNS_LIMBS_V1
    {
        return Err(ZkAmsMkheCpkRelationErrorV1::NativeRelation);
    }
    let mut builders = [
        ZkAmsMkheCpkRnsFirstMessageDigestBuilderV1::new(0)?,
        ZkAmsMkheCpkRnsFirstMessageDigestBuilderV1::new(1)?,
        ZkAmsMkheCpkRnsFirstMessageDigestBuilderV1::new(2)?,
        ZkAmsMkheCpkRnsFirstMessageDigestBuilderV1::new(3)?,
    ];
    for (limb, ((&modulus, &root), a_limb)) in profile
        .moduli
        .iter()
        .zip(profile.negacyclic_roots)
        .zip(public_a.chunks_exact(shape.degree))
        .enumerate()
    {
        for builder in &mut builders {
            builder.begin_limb(limb, modulus)?;
        }
        for (repetition, builder) in builders.iter_mut().enumerate() {
            let secret_start = response_index(shape, repetition, 0, 0)?;
            let error_start = response_index(shape, repetition, 1, 0)?;
            let mut first = negacyclic_multiply_signed_zeroizing_v1(
                a_limb,
                &attempt.masks()[secret_start..secret_start + shape.degree],
                modulus,
                root,
            )
            .map_err(|error| match error {
                ZkAmsMkheErrorV1::ResourceCeilingExceeded => {
                    ZkAmsMkheCpkRelationErrorV1::ResourceCeiling
                }
                _ => ZkAmsMkheCpkRelationErrorV1::NativeRelation,
            })?;
            let plaintext = profile.plaintext_modulus.residue(modulus);
            for (index, value) in first.values_mut().iter_mut().enumerate() {
                *value = mod_add(
                    mod_sub(0, *value, modulus),
                    mod_mul(
                        plaintext,
                        signed_mod(attempt.masks()[error_start + index], modulus),
                        modulus,
                    ),
                    modulus,
                );
            }
            builder.update_residues(first.values())?;
            builder.finish_limb()?;
        }
    }
    let [b0, b1, b2, b3] = builders;
    Ok([b0.finish()?, b1.finish()?, b2.finish()?, b3.finish()?])
}

pub(in super::super) fn publish_canonical_cpk_relation_proof_v1<P>(
    proof: ZkAmsMkheCpkRelationProofV1,
    publisher: &mut P,
) -> Result<ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheCpkRelationErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    proof.body.validate(CpkRelationShapeV1::RELEASE)?;
    let mut transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
        ZkAmsMkheDirectObjectKindV1::CpkRelationProof,
        ZK_AMS_MKHE_CPK_RELATION_PROOF_BYTES_V1 as u64,
        publisher,
    )
    .map_err(map_transport_v1)?;
    transaction
        .write_exact(&proof.header.to_wire_bytes()?)
        .map_err(map_transport_v1)?;
    transaction
        .write_exact(&proof.body.challenge_seed)
        .map_err(map_transport_v1)?;
    let mut buffer = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    for responses in proof.body.responses.chunks(buffer.len() / 8) {
        for (output, response) in buffer.chunks_exact_mut(8).zip(responses) {
            output.copy_from_slice(&response.to_be_bytes());
        }
        transaction
            .write_exact(&buffer[..responses.len() * 8])
            .map_err(map_transport_v1)?;
    }
    for (output, response) in buffer.chunks_exact_mut(32).zip(&proof.body.blind_responses) {
        output.copy_from_slice(&response.to_le_bytes());
    }
    transaction
        .write_exact(&buffer[..ZK_AMS_MKHE_CPK_BLIND_RESPONSE_PAYLOAD_BYTES_V1])
        .map_err(map_transport_v1)?;
    transaction.finish().map_err(map_transport_v1)
}

fn map_transport_v1(error: ZkAmsMkheErrorV1) -> ZkAmsMkheCpkRelationErrorV1 {
    match error {
        ZkAmsMkheErrorV1::ResourceCeilingExceeded => ZkAmsMkheCpkRelationErrorV1::ResourceCeiling,
        _ => ZkAmsMkheCpkRelationErrorV1::DirectObject,
    }
}
